//! Minimal EDF/EDF+ reader: header plus selected digital channels.
//!
//! The header is fixed-layout 8-bit ASCII, and every signal is stored as
//! little-endian 16-bit integers scaled from a digital to a physical range.
//! Records are interleaved, so reading one channel means skipping the others.
//! Only the fields needed for staging are surfaced.

use std::fmt;

const HEADER_FIXED: usize = 256;

#[derive(Debug)]
pub enum EdfError {
    TooShort(&'static str),
    NotEdf,
    Header(&'static str),
    Unsupported(&'static str),
}

impl fmt::Display for EdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EdfError::TooShort(what) => write!(f, "EDF file is too short to contain {what}"),
            EdfError::NotEdf => write!(f, "file does not start with an EDF version field"),
            EdfError::Header(what) => write!(f, "EDF header field is invalid: {what}"),
            EdfError::Unsupported(what) => write!(f, "unsupported EDF feature: {what}"),
        }
    }
}

impl std::error::Error for EdfError {}

/// One signal definition from the header.
#[derive(Clone, Debug)]
pub struct EdfChannel {
    pub label: String,
    pub physical_dimension: String,
    /// Samples per data record.
    pub samples_per_record: usize,
    pub physical_min: f64,
    pub physical_max: f64,
    pub digital_min: f64,
    pub digital_max: f64,
    pub prefilter: String,
    /// Byte offset of this channel's samples within one record.
    pub record_offset: usize,
}

impl EdfChannel {
    /// Physical value for a stored integer sample.
    fn scale(&self, digital: i16) -> f64 {
        let digital = f64::from(digital);
        let range = self.digital_max - self.digital_min;
        if range == 0.0 {
            return self.physical_min;
        }
        self.physical_min
            + (digital - self.digital_min) * (self.physical_max - self.physical_min) / range
    }
}

/// A parsed EDF header, plus the offsets needed to stream channel data.
#[derive(Clone, Debug)]
pub struct Edf {
    pub channels: Vec<EdfChannel>,
    pub records: usize,
    /// Seconds per data record.
    pub record_seconds: f64,
    pub header_bytes: usize,
    /// Bytes per data record, all channels interleaved.
    pub record_bytes: usize,
    /// EDF+ annotation channel index, if the file is EDF+.
    pub annotation_channel: Option<usize>,
}

impl Edf {
    pub fn sample_rate(&self, channel: usize) -> f64 {
        if self.record_seconds > 0.0 {
            return self.channels[channel].samples_per_record as f64 / self.record_seconds;
        }
        self.channels[channel].samples_per_record as f64
    }

    /// Duration covered by the data records, in seconds. Annotation-only files
    /// declare a zero record duration, so they report zero.
    pub fn duration_seconds(&self) -> f64 {
        self.records as f64 * self.record_seconds
    }

    /// Whether this is an annotation-only EDF+ file (a hypnogram), where the
    /// time base comes from the annotations themselves and there is no signal.
    pub fn is_annotation_only(&self) -> bool {
        self.record_seconds <= 0.0
    }

    /// Effective records. A zero-duration annotation file holds one record whose
    /// length is implied by its sample count.
    fn effective_records(&self) -> usize {
        if self.is_annotation_only() {
            1
        } else {
            self.records
        }
    }

    /// Channel indices whose label matches any of `patterns` (case-insensitive
    /// substring), in file order.
    pub fn find(&self, patterns: &[&str]) -> Vec<usize> {
        self.channels
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                let label = c.label.to_ascii_lowercase();
                patterns
                    .iter()
                    .any(|p| label.contains(&p.to_ascii_lowercase()))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Decode one physical channel across every **complete** record.
    ///
    /// A partially downloaded or interrupted recording is common in practice, so a
    /// truncated tail is not an error: the complete records are returned and
    /// `complete_records` reports how many were read. Refusing the whole file would
    /// make a recoverable night unusable, and silently padding it would fabricate
    /// data.
    pub fn read_channel(&self, data: &[u8], channel: usize) -> Result<Vec<f64>, EdfError> {
        let _ch = self
            .channels
            .get(channel)
            .ok_or(EdfError::Header("channel index out of range"))?;
        let (out, _) = self.read_channel_with_records(data, channel)?;
        Ok(out)
    }

    /// Physical channel samples plus the number of complete records actually read,
    /// which is fewer than `records` for a truncated file.
    pub fn read_channel_with_records(
        &self,
        data: &[u8],
        channel: usize,
    ) -> Result<(Vec<f64>, usize), EdfError> {
        let ch = self
            .channels
            .get(channel)
            .ok_or(EdfError::Header("channel index out of range"))?;
        let available = data.len().saturating_sub(self.header_bytes);
        let complete = if self.record_bytes == 0 {
            0
        } else {
            (available / self.record_bytes).min(self.effective_records())
        };
        if complete == 0 {
            return Err(EdfError::TooShort("a single complete data record"));
        }
        let mut out = Vec::with_capacity(ch.samples_per_record * complete);
        for record in 0..complete {
            let base = self.header_bytes + record * self.record_bytes + ch.record_offset;
            for i in 0..ch.samples_per_record {
                let lo = data[base + i * 2];
                let hi = data[base + i * 2 + 1];
                out.push(ch.scale(i16::from_le_bytes([lo, hi])));
            }
        }
        Ok((out, complete))
    }

    /// Raw digital samples, which is what annotation channels contain.
    pub fn read_digital(&self, data: &[u8], channel: usize) -> Result<Vec<i16>, EdfError> {
        let ch = self
            .channels
            .get(channel)
            .ok_or(EdfError::Header("channel index out of range"))?;
        let available = data.len().saturating_sub(self.header_bytes);
        let complete = if self.record_bytes == 0 {
            0
        } else {
            (available / self.record_bytes).min(self.effective_records())
        };
        if complete == 0 {
            return Err(EdfError::TooShort("a single complete data record"));
        }
        let mut out = Vec::with_capacity(ch.samples_per_record * complete);
        for record in 0..complete {
            let base = self.header_bytes + record * self.record_bytes + ch.record_offset;
            for i in 0..ch.samples_per_record {
                let lo = data[base + i * 2];
                let hi = data[base + i * 2 + 1];
                out.push(i16::from_le_bytes([lo, hi]));
            }
        }
        Ok(out)
    }
}

fn field(data: &[u8], offset: usize, width: usize) -> Result<&str, EdfError> {
    let slice = data
        .get(offset..offset + width)
        .ok_or(EdfError::TooShort("header fields"))?;
    std::str::from_utf8(slice).map_err(|_| EdfError::Header("non-ASCII characters"))
}

fn trimmed(data: &[u8], offset: usize, width: usize) -> Result<String, EdfError> {
    Ok(field(data, offset, width)?.trim().to_string())
}

fn number<T: std::str::FromStr>(
    data: &[u8],
    offset: usize,
    width: usize,
    name: &'static str,
) -> Result<T, EdfError> {
    field(data, offset, width)?
        .trim()
        .parse()
        .map_err(|_| EdfError::Header(name))
}

/// Parse the header. `data` only needs to contain the header bytes.
pub fn parse_header(data: &[u8]) -> Result<Edf, EdfError> {
    if data.len() < HEADER_FIXED {
        return Err(EdfError::TooShort("the 256-byte header"));
    }
    let version = field(data, 0, 8)?.trim();
    // "0" is plain EDF; EDF+C, EDF+D and EDF+X appear in EDF+ files.
    if !version.starts_with('0') {
        return Err(EdfError::NotEdf);
    }
    let header_bytes: usize = number(data, 184, 8, "header bytes")?;
    let records: usize = number(data, 236, 8, "record count")?;
    let record_seconds: f64 = number(data, 244, 8, "record duration")?;
    let signals: usize = number(data, 252, 4, "signal count")?;
    if signals == 0 {
        return Err(EdfError::Header("no signals"));
    }
    if record_seconds < 0.0 {
        return Err(EdfError::Header("record duration"));
    }
    let expected = HEADER_FIXED + signals * 256;
    if header_bytes < expected {
        return Err(EdfError::Header(
            "header byte count is too small for the signal count",
        ));
    }
    if data.len() < header_bytes {
        return Err(EdfError::TooShort("the declared header"));
    }

    let labels = HEADER_FIXED;
    let transducer = labels + 16 * signals;
    let dimensions = transducer + 80 * signals;
    let physical_min = dimensions + 8 * signals;
    let physical_max = physical_min + 8 * signals;
    let digital_min = physical_max + 8 * signals;
    let digital_max = digital_min + 8 * signals;
    let prefilter = digital_max + 8 * signals;
    let samples = prefilter + 80 * signals;

    let mut channels = Vec::with_capacity(signals);
    let mut offset = 0usize;
    let mut annotation_channel = None;
    for i in 0..signals {
        let label = trimmed(data, labels + 16 * i, 16)?;
        let samples_per_record: usize = number(data, samples + 8 * i, 8, "samples per record")?;
        channels.push(EdfChannel {
            physical_dimension: trimmed(data, dimensions + 8 * i, 8)?,
            physical_min: number(data, physical_min + 8 * i, 8, "physical minimum")?,
            physical_max: number(data, physical_max + 8 * i, 8, "physical maximum")?,
            digital_min: number(data, digital_min + 8 * i, 8, "digital minimum")?,
            digital_max: number(data, digital_max + 8 * i, 8, "digital maximum")?,
            prefilter: trimmed(data, prefilter + 80 * i, 80)?,
            record_offset: offset,
            samples_per_record,
            label: label.clone(),
        });
        offset += samples_per_record * 2;
        if label.eq_ignore_ascii_case("EDF Annotations") {
            annotation_channel = Some(i);
        }
    }
    Ok(Edf {
        channels,
        records,
        record_seconds,
        header_bytes,
        record_bytes: offset,
        annotation_channel,
    })
}

/// Whether the file declares itself EDF+ (continuous), which is what carries
/// annotations. The reserved field at byte 192 holds `+EDF+C`.
pub fn is_continuous_edf_plus(data: &[u8]) -> bool {
    data.get(192..197).is_some_and(|v| v == b"+EDF+")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a two-signal continuous EDF+ header with correct field offsets.
    fn two_channel_header(annotation: bool) -> Vec<u8> {
        let ns = if annotation { 3 } else { 2 };
        let mut buf = vec![b' '; 256 + ns * 256];
        let mut put = |text: &str, offset: usize| {
            buf[offset..offset + text.len()].copy_from_slice(text.as_bytes());
        };
        put("0       ", 0);
        put("+EDF+C", 192);
        put(&(256 + ns * 256).to_string(), 184);
        put("2", 236);
        put("1", 244);
        put(&ns.to_string(), 252);
        // Field blocks follow the fixed 256 bytes, each ns * width long.
        let block = |_width: usize, previous: usize| 256 + ns * previous;
        let labels_at = block(16, 0);
        let transducer_at = block(80, 16);
        let dimensions_at = block(8, 16 + 80);
        let phys_min_at = block(8, 16 + 80 + 8);
        let phys_max_at = block(8, 16 + 80 + 16);
        let dig_min_at = block(8, 16 + 80 + 24);
        let dig_max_at = block(8, 16 + 80 + 32);
        let prefilter_at = block(80, 16 + 80 + 40);
        let samples_at = block(8, 16 + 160 + 40);
        let mut labels: Vec<&str> = vec!["EEG Fpz-Cz", "Resp oro-nasal"];
        if annotation {
            labels.push("EDF Annotations");
        }
        for (i, label) in labels.iter().enumerate() {
            put(label, labels_at + 16 * i);
            put("uV", dimensions_at + 8 * i);
            put("-100", phys_min_at + 8 * i);
            put("100", phys_max_at + 8 * i);
            put("-32768", dig_min_at + 8 * i);
            put("32767", dig_max_at + 8 * i);
            let samples = [2, 3, 1][i];
            put(&samples.to_string(), samples_at + 8 * i);
        }
        let _ = (transducer_at, prefilter_at);
        buf
    }

    #[test]
    fn parses_two_channel_header_and_offsets() {
        let edf = parse_header(&two_channel_header(false)).unwrap();
        assert_eq!(edf.channels.len(), 2);
        assert_eq!(edf.record_bytes, 10);
        assert_eq!(edf.channels[1].label, "Resp oro-nasal");
        assert_eq!(edf.channels[1].record_offset, 4);
        assert_eq!(edf.channels[1].samples_per_record, 3);
        assert_eq!(edf.find(&["resp"]), vec![1]);
        assert_eq!(edf.sample_rate(1), 3.0);
        assert!(is_continuous_edf_plus(&two_channel_header(false)));
    }

    #[test]
    fn decodes_interleaved_records_and_scales_physical_units() {
        let mut data = two_channel_header(false);
        data[236..244].copy_from_slice(b"1       ");
        // One record: channel A samples 0 and -32768, then channel B 0, 16384, 32767.
        for sample in [0i16, i16::MIN, 0, 16384, 32767] {
            data.extend_from_slice(&sample.to_le_bytes());
        }
        let edf = parse_header(&data).unwrap();
        assert_eq!(edf.records, 1);
        let b = edf.read_channel(&data, 1).unwrap();
        assert_eq!(b.len(), 3);
        assert!((b[1] - 50.0).abs() < 1e-2, "{b:?}");
        assert!((b[2] - 100.0).abs() < 1e-2, "{b:?}");
        let a = edf.read_channel(&data, 0).unwrap();
        assert!((a[1] + 100.0).abs() < 1e-6, "{a:?}");
        // A truncated tail yields the complete records that are present, and
        // reports how many, rather than failing or padding with invented data.
        // The header now claims three records but only one is present.
        let mut short = data.clone();
        short[236..244].copy_from_slice(b"3       ");
        let (partial, complete) = parse_header(&short)
            .unwrap()
            .read_channel_with_records(&short, 0)
            .unwrap();
        assert_eq!(complete, 1);
        assert_eq!(partial.len(), 2);
        // Nothing at all is still an error.
        let nothing = parse_header(&data).unwrap();
        assert!(nothing
            .read_channel(&data[..nothing.header_bytes], 0)
            .is_err());
    }

    #[test]
    fn rejects_bad_headers_and_truncated_data() {
        assert!(matches!(parse_header(&[]), Err(EdfError::TooShort(_))));
        let mut data = two_channel_header(false);
        data[0] = b'9';
        assert!(matches!(parse_header(&data), Err(EdfError::NotEdf)));
        let mut data = two_channel_header(false);
        data[184..192].copy_from_slice(b"    10  ");
        assert!(parse_header(&data).is_err());
        let edf = parse_header(&two_channel_header(false)).unwrap();
        assert!(edf.read_channel(&two_channel_header(false), 0).is_err());
    }

    #[test]
    fn finds_the_annotation_channel_in_edf_plus() {
        let edf = parse_header(&two_channel_header(true)).unwrap();
        assert_eq!(edf.annotation_channel, Some(2));
        assert!(!is_continuous_edf_plus(&{
            let mut plain = two_channel_header(false);
            plain[192..197].copy_from_slice(b"     ");
            plain
        }));
    }
}
