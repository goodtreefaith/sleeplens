//! EDF+ `TAL` annotations: onset, duration, and free text.
//!
//! An annotation channel stores a run of 16-bit integers per record. Each
//! annotation is `+onset[\x15duration]\x14text\x14\x00`; a bare `\x00` ends the
//! record. Onsets are seconds from the recording start and may be negative.
//! Sleep-EDF hypnograms put one annotation per scored epoch with text such as
//! `Sleep stage W` or `Sleep stage R`.

use crate::edf::{Edf, EdfError};

#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    /// Seconds from the recording start.
    pub onset: f64,
    /// Seconds, when the annotation declared one.
    pub duration: Option<f64>,
    pub text: String,
}

/// Decode every annotation in an annotation channel.
///
/// Tolerates truncated trailing bytes: a damaged final record yields the
/// annotations that were complete rather than failing the whole file.
pub fn read(data: &[u8], edf: &Edf, channel: usize) -> Result<Vec<Annotation>, EdfError> {
    let samples = edf.read_digital(data, channel)?;
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(parse_bytes(&bytes))
}

/// Parse concatenated annotation records from raw little-endian bytes.
pub fn parse_bytes(bytes: &[u8]) -> Vec<Annotation> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        // Each record opens with a timekeeping TAL: onset, optional duration,
        // then a text field that is usually empty.
        let Some((tal, next)) = read_tal(bytes, cursor) else {
            break;
        };
        cursor = next;
        let base = tal.onset;
        if !tal.text.is_empty() {
            out.push(tal);
        }
        // Additional TALs in the same record carry absolute onsets in the same
        // time base, so they are read directly rather than accumulated.
        loop {
            let Some((extra, next)) = read_tal(bytes, cursor) else {
                break;
            };
            if extra.onset < 0.0 && extra.duration.is_none() && extra.text.is_empty() {
                // A bare terminator ends the record.
                cursor = next;
                break;
            }
            cursor = next;
            if !extra.text.is_empty() {
                out.push(Annotation {
                    onset: if extra.onset >= 0.0 {
                        extra.onset
                    } else {
                        base + extra.onset
                    },
                    duration: extra.duration,
                    text: extra.text,
                });
            }
        }
    }
    out
}

/// Read one TAL starting at `cursor`, returning it and the position after its
/// terminator. `None` ends parsing.
fn read_tal(bytes: &[u8], mut cursor: usize) -> Option<(Annotation, usize)> {
    while cursor < bytes.len() && bytes[cursor] == 0 {
        cursor += 1;
    }
    if cursor >= bytes.len() {
        return None;
    }
    let start = cursor;
    while cursor < bytes.len() && bytes[cursor] != b'\x14' && bytes[cursor] != b'\x15' {
        cursor += 1;
    }
    if cursor >= bytes.len() {
        return None;
    }
    let onset_text = std::str::from_utf8(&bytes[start..cursor]).ok()?;
    let onset: f64 = onset_text.trim_start_matches('+').parse().ok()?;
    let mut duration = None;
    if bytes[cursor] == b'\x15' {
        cursor += 1;
        let dstart = cursor;
        while cursor < bytes.len() && bytes[cursor] != b'\x14' {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            return None;
        }
        duration = std::str::from_utf8(&bytes[dstart..cursor])
            .ok()
            .and_then(|t| t.trim().parse().ok());
    }
    // Skip the field separator, then read text up to the next separator.
    cursor += 1;
    let tstart = cursor;
    while cursor < bytes.len() && bytes[cursor] != b'\x14' {
        cursor += 1;
    }
    if cursor >= bytes.len() {
        return None;
    }
    let text = String::from_utf8_lossy(&bytes[tstart..cursor]).into_owned();
    cursor += 1;
    Some((
        Annotation {
            onset,
            duration,
            text,
        },
        cursor,
    ))
}

/// Sleep-EDF stage text, e.g. `Sleep stage W` or `Sleep stage 3`.
pub fn sleep_edf_stage(text: &str) -> Option<StageFromText> {
    let lower = text.to_ascii_lowercase();
    let rest = lower.strip_prefix("sleep stage").map(str::trim)?;
    Some(match rest {
        "w" | "wake" => StageFromText::Wake,
        "1" => StageFromText::N1,
        "2" => StageFromText::N2,
        "3" => StageFromText::N3,
        "4" => StageFromText::N4,
        "r" | "rem" => StageFromText::Rem,
        _ => StageFromText::Unscored,
    })
}

/// Stage named by an annotation's text, before mapping onto SleepLens classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageFromText {
    Wake,
    N1,
    N2,
    N3,
    N4,
    Rem,
    Unscored,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tal(onset: &str) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(format!("+{onset}").as_bytes());
        out.push(0x14);
        out.extend_from_slice(b"\x14");
        out.push(0x00);
        out
    }

    #[test]
    fn parses_onsets_and_text() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&tal("0"));
        bytes.extend_from_slice(b"+30630\x14Sleep stage W\x14\x00");
        let out = parse_bytes(&bytes);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].onset, 30_630.0);
        assert_eq!(out[0].text, "Sleep stage W");
    }

    #[test]
    fn parses_declared_duration() {
        let bytes = b"+0\x1510\x14Sleep stage 2\x14\x00";
        let out = parse_bytes(bytes);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].duration, Some(10.0));
        assert_eq!(out[0].text, "Sleep stage 2");
    }

    #[test]
    fn joins_multiple_annotations_in_one_record() {
        let bytes = b"+0\x14\x14\x0030\x14Sleep stage 1\x14\x0060\x14Sleep stage 2\x14\x00";
        let out = parse_bytes(bytes);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].onset, 30.0);
        assert_eq!(out[1].onset, 60.0);
    }

    #[test]
    fn maps_sleep_edf_text_to_stages() {
        assert_eq!(sleep_edf_stage("Sleep stage W"), Some(StageFromText::Wake));
        assert_eq!(sleep_edf_stage("Sleep stage 3"), Some(StageFromText::N3));
        assert_eq!(sleep_edf_stage("Sleep stage R"), Some(StageFromText::Rem));
        assert_eq!(sleep_edf_stage("Movement time"), None);
        assert_eq!(
            sleep_edf_stage("Sleep stage ?"),
            Some(StageFromText::Unscored)
        );
    }

    #[test]
    fn a_truncated_tail_is_ignored() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"+30\x14Sleep stage W\x14\x00");
        bytes.extend_from_slice(b"+60\x14Sleep st");
        let out = parse_bytes(&bytes);
        assert_eq!(out.len(), 1);
    }
}
