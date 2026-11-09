//! Compumedics Profusion XML, the sidecar NSRR ships for MESA and SHHS.
//!
//! A `SleepStages` element holds one block per scored 30-second epoch:
//!
//! ```xml
//! <SleepStages>
//!   <SleepStage>0</SleepStage>
//!   <SleepStage>2</SleepStage>
//!   ...
//! </SleepStages>
//! ```
//!
//! Codes follow the Compumedics convention: 0 Wake, 1 N1, 2 N2, 3 N3,
//! 4 N4 (R&K only), 5 REM, 9 unscored. Files also carry event tables, which this
//! reader ignores — the evaluation needs labels, not events.
//!
//! Written as a small purpose-built scanner rather than with an XML crate: the
//! files are large, the needed structure is two levels deep, and adding an
//! external dependency to the build path of a health-adjacent tool is worth
//! avoiding when a targeted parser is this small.

use std::fmt;

use crate::night::StageLabel;

#[derive(Debug)]
pub enum ProfusionError {
    NoSleepStages,
    NoEpochs,
}

impl fmt::Display for ProfusionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfusionError::NoSleepStages => {
                write!(f, "no <SleepStages> block was found in the annotation file")
            }
            ProfusionError::NoEpochs => {
                write!(f, "<SleepStages> contains no <SleepStage> entries")
            }
        }
    }
}

impl std::error::Error for ProfusionError {}

/// Compumedics stage code to a SleepLens class.
///
/// R&K stage 4 (deepest slow-wave sleep) is merged into Deep, matching AASM.
/// Movement time (8) and unscored (9) are excluded from scoring.
pub fn stage_from_code(code: u8) -> StageLabel {
    match code {
        0 => StageLabel::Wake,
        1 | 2 => StageLabel::Light,
        3 | 4 => StageLabel::Deep,
        5 => StageLabel::Rem,
        _ => StageLabel::Excluded,
    }
}

/// Read one label per 30-second epoch, in order.
pub fn parse(xml: &str) -> Result<Vec<StageLabel>, ProfusionError> {
    let start = xml
        .find("<SleepStages")
        .ok_or(ProfusionError::NoSleepStages)?;
    let block = &xml[start..];
    let end = block.find("</SleepStages>").unwrap_or(block.len());
    let block = &block[..end];
    let mut labels = Vec::new();
    let mut rest = block;
    while let Some(open) = rest.find("<SleepStage>") {
        let after = &rest[open + "<SleepStage>".len()..];
        let Some(close) = after.find("</SleepStage>") else {
            break;
        };
        if let Ok(code) = after[..close].trim().parse::<u8>() {
            labels.push(stage_from_code(code));
        }
        rest = &after[close + "</SleepStage>".len()..];
    }
    if labels.is_empty() {
        return Err(ProfusionError::NoEpochs);
    }
    Ok(labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_one_label_per_epoch() {
        let xml = r#"<PSGAnnotation><Events></Events><SleepStages>
            <SleepStage>0</SleepStage>
            <SleepStage>2</SleepStage>
            <SleepStage>3</SleepStage>
            <SleepStage>5</SleepStage>
            <SleepStage>9</SleepStage>
        </SleepStages></PSGAnnotation>"#;
        assert_eq!(
            parse(xml).unwrap(),
            vec![
                StageLabel::Wake,
                StageLabel::Light,
                StageLabel::Deep,
                StageLabel::Rem,
                StageLabel::Excluded
            ]
        );
    }

    #[test]
    fn tolerates_a_missing_close_tag() {
        let xml = "<SleepStages><SleepStage>2</SleepStage><SleepStage>5</SleepStage>";
        assert_eq!(
            parse(xml).unwrap(),
            vec![StageLabel::Light, StageLabel::Rem]
        );
    }

    #[test]
    fn missing_or_empty_blocks_are_errors() {
        assert!(parse("<PSGAnnotation></PSGAnnotation>").is_err());
        assert!(parse("<SleepStages></SleepStages>").is_err());
    }

    #[test]
    fn r_and_k_stage_four_is_deep() {
        assert_eq!(stage_from_code(4), StageLabel::Deep);
        assert_eq!(stage_from_code(8), StageLabel::Excluded);
        assert_eq!(stage_from_code(9), StageLabel::Excluded);
    }
}
