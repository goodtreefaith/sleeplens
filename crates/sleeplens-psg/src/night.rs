//! Turn a labelled recording into the exact inputs the estimator takes.
//!
//! Real polysomnography gives whole epochs of labels and a respiration channel
//! sampled at its own rate. The estimator takes 50 Hz patient flow. Two things
//! therefore have to happen, and both are lossy in ways worth naming:
//!
//! * **Rate.** Respiration channels are 10–32 Hz and the estimator's breath
//!   detector only needs the shape, so resampling to 50 Hz by linear
//!   interpolation adds no information and invents none. Downsampling would.
//! * **Units.** Thermistors and pressure transducers differ in both scale and
//!   sign. Everything here is scaled to a plausible flow amplitude and oriented
//!   so inspiration is positive, because the estimator's thresholds are in
//!   litres. Absolute calibration is impossible from a thermistor, so this is a
//!   normalisation, not a measurement — the same caveat the evidence review
//!   makes about transferring a model across sensors.
//!
//! Labels are collapsed to the classes SleepLens emits. `N1` and `N2` become
//! Light; `N3` and R&K stage 4 become Deep; `Movement` and unscored epochs are
//! excluded from scoring rather than counted as errors, since no airflow-based
//! method can be expected to reproduce a movement epoch.

use crate::annotations::StageFromText;

/// One labelled 30-second epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageLabel {
    Wake,
    Light,
    Deep,
    Rem,
    /// Movement, unscored, or artefact: excluded from every metric.
    Excluded,
}

impl StageLabel {
    pub fn from_sleep_edf(stage: StageFromText) -> Self {
        match stage {
            StageFromText::Wake => StageLabel::Wake,
            StageFromText::N1 | StageFromText::N2 => StageLabel::Light,
            StageFromText::N3 | StageFromText::N4 => StageLabel::Deep,
            StageFromText::Rem => StageLabel::Rem,
            StageFromText::Unscored => StageLabel::Excluded,
        }
    }

    /// AASM/Profusion names as written by Compumedics software.
    pub fn from_profusion(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "wake" | "w" => StageLabel::Wake,
            "n1" | "n2" | "stage 1" | "stage 2" | "1" | "2" => StageLabel::Light,
            "n3" | "stage 3" | "stage 4" | "3" | "4" | "deep" => StageLabel::Deep,
            "rem" | "r" => StageLabel::Rem,
            _ => StageLabel::Excluded,
        }
    }
}

/// One night, ready to estimate and score.
#[derive(Clone, Debug)]
pub struct LabelledNight {
    /// Identifier for reporting, normally the file stem.
    pub id: String,
    /// Respiration in arbitrary-but-consistent flow units, 50 Hz, inspiration positive.
    pub flow: Vec<f32>,
    /// One label per 30-second epoch, starting at the first epoch of `flow`.
    pub labels: Vec<StageLabel>,
}

pub const HZ: usize = 50;
pub const EPOCH_SECONDS: usize = 30;

/// Convert an arbitrary respiration channel into estimator input.
///
/// * `samples` — the raw physical channel.
/// * `hz` — its sample rate.
/// * `invert` — set when positive values mean expiration.
///
/// Centring uses the **midrange** of the robust range, not the median. A
/// respiration trace is not symmetric about its mean: inspiration is fast and
/// expiration slower, so the median sits above the zero-flow point whenever the
/// channel has a DC offset. Subtracting the median therefore leaves inspiration
/// crossing cleanly while expiration only dips a little, which drives measured
/// inspiratory duty cycle up to 0.8 and makes every epoch look flow-limited. The
/// midrange is the unbiased estimate of the zero-flow line for a roughly
/// symmetric oscillation.
///
/// Scaling then sets the 95th percentile absolute deviation to a normal tidal
/// excursion, so breath detection works regardless of the sensor's gain.
pub fn normalise_respiration(samples: &[f64], hz: f64, invert: bool) -> Vec<f32> {
    if samples.is_empty() || !hz.is_finite() || hz <= 0.0 {
        return Vec::new();
    }
    let mut sorted: Vec<f64> = samples.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.is_empty() {
        return Vec::new();
    }
    sorted.sort_unstable_by(f64::total_cmp);
    // 2nd and 98th percentiles bound the breathing oscillation without letting a
    // single artefact set the range.
    let low = sorted[((sorted.len() - 1) as f64 * 0.02) as usize];
    let high = sorted[((sorted.len() - 1) as f64 * 0.98) as usize];
    let centre = (low + high) / 2.0;
    let mut deviations: Vec<f64> = sorted.iter().map(|v| (v - centre).abs()).collect();
    deviations.sort_unstable_by(f64::total_cmp);
    let amplitude = deviations[(deviations.len() as f64 * 0.95) as usize].max(1e-9);
    // Target 12 L/min peak excursion: a normal tidal swing through a PAP circuit.
    let scale = 12.0 / amplitude;
    let sign = if invert { -1.0 } else { 1.0 };
    let duration = samples.len() as f64 / hz;
    let out_len = (duration * HZ as f64).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    let step = hz / HZ as f64;
    for i in 0..out_len {
        let position = i as f64 * step;
        let lo = position.floor() as usize;
        let hi = (lo + 1).min(samples.len() - 1);
        let t = position - lo as f64;
        let a = samples[lo];
        let b = samples[hi];
        let value = if a.is_finite() && b.is_finite() {
            a + (b - a) * t
        } else {
            f64::NAN
        };
        out.push((sign * (value - centre) * scale) as f32);
    }
    out
}

/// Build the epoch label array from annotation onsets.
///
/// Sleep-EDF hypnograms annotate the *start* of each epoch with an explicit
/// onset, sometimes with a duration. A gap in the onsets means an unscored
/// epoch and becomes `Excluded`, so the metric never rewards or punishes a guess
/// in a stretch no human scored.
pub fn labels_from_onsets(
    annotations: &[(f64, Option<f64>, StageLabel)],
    epoch_seconds: f64,
    epochs: usize,
) -> Vec<StageLabel> {
    let mut labels = vec![StageLabel::Excluded; epochs];
    for &(onset, duration, stage) in annotations {
        if onset < 0.0 || !onset.is_finite() {
            continue;
        }
        let first = (onset / epoch_seconds).round() as usize;
        let span = duration
            .filter(|d| *d > 0.0)
            .map(|d| (d / epoch_seconds).round().max(1.0) as usize)
            .unwrap_or(1);
        for epoch in first..(first + span).min(epochs) {
            labels[epoch] = stage;
        }
    }
    labels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_five_class_labels_onto_four() {
        assert_eq!(
            StageLabel::from_sleep_edf(StageFromText::N1),
            StageLabel::Light
        );
        assert_eq!(
            StageLabel::from_sleep_edf(StageFromText::N2),
            StageLabel::Light
        );
        assert_eq!(
            StageLabel::from_sleep_edf(StageFromText::N3),
            StageLabel::Deep
        );
        assert_eq!(
            StageLabel::from_sleep_edf(StageFromText::N4),
            StageLabel::Deep
        );
        assert_eq!(
            StageLabel::from_sleep_edf(StageFromText::Unscored),
            StageLabel::Excluded
        );
        assert_eq!(StageLabel::from_profusion("REM"), StageLabel::Rem);
        assert_eq!(StageLabel::from_profusion("N3"), StageLabel::Deep);
        assert_eq!(StageLabel::from_profusion("Movement"), StageLabel::Excluded);
    }

    #[test]
    fn onsets_place_labels_and_leave_gaps_excluded() {
        let annotations = [
            (0.0, None, StageLabel::Wake),
            (60.0, None, StageLabel::Deep),
            (90.0, Some(60.0), StageLabel::Rem),
        ];
        let labels = labels_from_onsets(&annotations, 30.0, 6);
        assert_eq!(
            labels,
            vec![
                StageLabel::Wake,
                StageLabel::Excluded,
                StageLabel::Deep,
                StageLabel::Rem,
                StageLabel::Rem,
                StageLabel::Excluded
            ]
        );
    }

    #[test]
    fn resampling_preserves_shape_and_orientation() {
        // 10 Hz sine at 0.25 Hz; 60 s. Positive peak should stay positive.
        let samples: Vec<f64> = (0..600)
            .map(|i| 100.0 + 40.0 * (i as f64 / 10.0 * std::f64::consts::TAU * 0.25).sin())
            .collect();
        let normal = normalise_respiration(&samples, 10.0, false);
        assert_eq!(normal.len(), 3000);
        let max = normal.iter().cloned().fold(f32::MIN, f32::max);
        let min = normal.iter().cloned().fold(f32::MAX, f32::min);
        assert!(max > 5.0 && min < -5.0);
        let inverted = normalise_respiration(&samples, 10.0, true);
        assert!((inverted[0] + normal[0]).abs() < 1e-3);
    }

    #[test]
    fn degenerate_input_is_empty_not_a_panic() {
        assert!(normalise_respiration(&[], 10.0, false).is_empty());
        assert!(normalise_respiration(&[1.0, 2.0], 0.0, false).is_empty());
        let flat = normalise_respiration(&[5.0; 100], 10.0, false);
        assert_eq!(flat.len(), 500);
        assert!(flat.iter().all(|v| v.is_finite()));
    }
}
