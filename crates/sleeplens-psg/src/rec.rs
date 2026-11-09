//! European Data Format `.rec` variant used by the ucddb sleep-apnoea database.
//!
//! Same fixed-layout header as EDF with two differences that matter:
//!
//! * the reserved 44-byte field is empty rather than `+EDF+C`, so the file is
//!   neither EDF nor EDF+ strictly speaking. It is EDF-compatible in layout, so
//!   the header parser handles it once the version and reserved fields are
//!   allowed to be absent;
//! * the record duration is signed and written as `+1`, and one record is one
//!   second. That is why a `.rec` has many more records than an EDF of the same
//!   length: 22 470 records for 22 470 seconds.
//!
//! The signal list includes a true `Flow` channel in L/min at 8 Hz, which is the
//! closest openly available analogue to PAP patient flow.

use crate::edf::{self, Edf, EdfError};

/// Whether a file looks like a ucddb `.rec` rather than an EDF.
pub fn is_rec(data: &[u8]) -> bool {
    data.get(..8).is_some_and(|v| {
        std::str::from_utf8(v)
            .map(|s| s.trim() == "0")
            .unwrap_or(false)
    }) && data.get(192..236).is_some_and(|v| {
        // EDF+ puts `+EDF+C` here; `.rec` leaves it blank.
        std::str::from_utf8(v)
            .map(|s| s.trim().is_empty())
            .unwrap_or(false)
    })
}

/// Parse a `.rec` header. Accepts the signed `+1` record duration.
pub fn parse_header(data: &[u8]) -> Result<Edf, EdfError> {
    // Normalise the signed duration in-place on a scratch copy of the header so
    // the shared EDF parser can stay strict about the standard.
    let header_len = data
        .get(184..192)
        .and_then(|v| std::str::from_utf8(v).ok())
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if header_len == 0 || header_len > data.len() {
        return Err(EdfError::Header("record duration"));
    }
    let mut patched = data[..header_len].to_vec();
    let duration: f64 = std::str::from_utf8(&data[244..252])
        .map_err(|_| EdfError::Header("record duration"))?
        .trim()
        .trim_start_matches('+')
        .parse()
        .map_err(|_| EdfError::Header("record duration"))?;
    patched[244..252].copy_from_slice(format!("{duration:<8}").as_bytes());
    edf::parse_header(&patched)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec_header() -> Vec<u8> {
        let n = 2;
        let mut buf = vec![b' '; 256 + n * 256];
        let mut put = |text: &str, offset: usize| {
            buf[offset..offset + text.len()].copy_from_slice(text.as_bytes());
        };
        put("0       ", 0);
        put(&(256 + n * 256).to_string(), 184);
        put("2", 236);
        put("+1", 244);
        put(&n.to_string(), 252);
        let at = |previous: usize| 256 + n * previous;
        for (i, label) in ["C3A2", "Flow"].iter().enumerate() {
            put(label, at(0) + 16 * i);
            put("L/min", at(16 + 80) + 8 * i);
            put("-5", at(16 + 80 + 8) + 8 * i);
            put("5", at(16 + 80 + 16) + 8 * i);
            put("-2048", at(16 + 80 + 24) + 8 * i);
            put("2047", at(16 + 80 + 32) + 8 * i);
            put("8", at(16 + 160 + 40) + 8 * i);
        }
        buf
    }

    #[test]
    fn rec_header_parses_despite_signed_duration() {
        let header = rec_header();
        let edf = parse_header(&header).unwrap();
        assert_eq!(edf.channels.len(), 2);
        assert_eq!(edf.records, 2);
        assert_eq!(edf.record_seconds, 1.0);
        assert_eq!(edf.sample_rate(1), 8.0);
        assert!(is_rec(&header));
        // A standard EDF+ header is not mistaken for a .rec.
        let mut edf_plus = header.clone();
        edf_plus[192..198].copy_from_slice(b"+EDF+C");
        assert!(!is_rec(&edf_plus));
    }

    #[test]
    fn bad_or_short_input_is_an_error() {
        assert!(parse_header(&[]).is_err());
        let mut header = rec_header();
        header[184..192].copy_from_slice(b"99999999");
        assert!(parse_header(&header).is_err());
    }
}
