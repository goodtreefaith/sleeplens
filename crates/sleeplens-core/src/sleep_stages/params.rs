//! Every adjustable number in the stage estimate, in one place.
//!
//! The defaults are the shipped values. They are grouped so that a change can be
//! described, reviewed and ablated one group at a time, which the evidence
//! review asks for. `sleep-eval` can perturb individual fields to search for
//! better values on labelled nights; nothing at runtime does that.
//!
//! # How the numbers are used
//!
//! For each awake/sleep-substage candidate the estimator sums evidence into a
//! single non-negative number. A **threshold** then says how much evidence is
//! needed before that candidate is preferred to plain Light sleep:
//!
//! ```text
//! strength = evidence / (evidence + threshold)      // 0 .. 1, 0.5 at the threshold
//! score    = gain * (2 * strength - 1)              // positive only above it
//! ```
//!
//! Light sleep is the reference and scores zero. This is what stops a night from
//! being labelled entirely wake or entirely REM: weak evidence is *negative*
//! once converted, so it cannot accumulate into a claim simply by being present.
//!
//! Magnitudes were set by hand against synthetic nights with known stages and are
//! **not** PSG calibration. Directions come from the respiratory sleep literature;
//! see `docs/SLEEP_STAGE_EVIDENCE.md` and `docs/SLEEP_STAGE_METHOD.md`.

/// One tunable parameter set. `Default` is what ships.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Multiplies every emission score. Each converted score lies in -1..1, so
    /// this is what puts breathing evidence on the same scale as the transition
    /// costs (a few units). Too low and no stage can ever outvote the smoothing
    /// prior; too high and the decode becomes noisy epoch-by-epoch argmax.
    pub emission_scale: f64,

    // --- Wake -----------------------------------------------------------
    /// Short-window interval CV where irregular-timing evidence starts and
    /// saturates.
    pub wake_interval_cv: (f64, f64),
    /// Short-window volume CV where irregular-volume evidence starts and saturates.
    pub wake_volume_cv: (f64, f64),
    /// Short-window duty-cycle CV where that evidence starts and saturates.
    pub wake_duty_cv: (f64, f64),
    /// Breathing rate (breaths per minute) where rate evidence starts and saturates.
    pub wake_rate: (f64, f64),
    /// Weight of the combined irregularity evidence.
    pub wake_irregular: f64,
    /// Weight of fast breathing.
    pub wake_fast: f64,
    /// Weight of sighing, which only counts alongside some irregularity.
    pub wake_sigh: f64,
    /// Weight of machine-event arousal evidence near the epoch.
    pub wake_arousal: f64,
    /// Minutes after mask-on during which wake is favoured, and its weight.
    pub onset_minutes: f64,
    pub onset_weight: f64,
    /// Weight of a large breath at a mask edge.
    pub wake_boundary: f64,
    /// How far below baseline ventilation must fall before wake evidence is
    /// discounted, as a `(start, full)` pair over the ratio baseline/measured.
    /// `(1.0, 1.25)` means the gate begins at baseline and is fully closed when
    /// ventilation is 20% below it.
    pub wake_ventilation_discount: (f64, f64),
    /// Evidence needed before wake outweighs Light.
    pub wake_threshold: f64,

    // --- Deep -----------------------------------------------------------
    /// Weight of sustained absolute regularity.
    pub deep_regularity: f64,
    /// Weight of a rounded (non-flow-limited) inspiratory contour.
    pub deep_round_breath: f64,
    /// Weight for being among the night's most regular epochs. Required as well
    /// as, not instead of, the absolute caps: a night with no N3 has no regular
    /// quartile distinct from the rest, so nothing is promoted to Deep.
    pub deep_rank_prior: f64,
    /// Short-window interval CV that must not be exceeded for Deep at all.
    pub deep_interval_cap: f64,
    /// Short-window volume CV that must not be exceeded for Deep at all.
    pub deep_volume_cap: f64,
    /// Long-window caps, which reject a quiet epoch inside noisy surroundings.
    pub deep_long_interval_cap: f64,
    pub deep_long_volume_cap: f64,
    pub deep_long_ventilation_cap: f64,
    pub deep_long_duty_cap: f64,
    /// Accepted inspiratory duty-cycle range for Deep.
    pub deep_duty: (f64, f64),
    /// Accepted breathing-rate range for Deep.
    pub deep_rate: (f64, f64),
    /// Minimum mid-inspiratory squareness for Deep: a flat-topped contour is
    /// flow limitation, not quiet sleep.
    pub deep_squareness: f64,
    /// How much contrast in regularity a night must show before any of it can be
    /// called Deep: the ratio between the night's most irregular decile and its
    /// most regular decile. A perfectly steady signal has a ratio near 1 and
    /// yields no Deep, which is intended - "unvaryingly regular" is not the same
    /// observation as "slow-wave sleep".
    pub deep_contrast: f64,
    /// Evidence needed before Deep outweighs Light.
    pub deep_threshold: f64,

    // --- REM ------------------------------------------------------------
    /// Weight of lower relative ventilation.
    pub rem_hypoventilation: f64,
    /// Relative ventilation where that evidence starts and saturates.
    pub rem_ventilation: (f64, f64),
    /// Weight of breath-to-breath irregularity.
    pub rem_irregular: f64,
    /// Weight of uneven respiratory effort: widely spread peak inspiratory flow
    /// at the same average ventilation. REM drive varies breath to breath, while
    /// obstructive hypopnoea narrows rather than widens effort.
    pub rem_effort_spread: f64,
    /// Peak-flow spread where that evidence starts and saturates.
    pub rem_peak_cv: (f64, f64),
    /// Irregularity start and saturation, for timing and volume separately.
    pub rem_interval_cv: (f64, f64),
    pub rem_volume_cv: (f64, f64),
    /// Weight of slow ventilation oscillation across epochs.
    pub rem_slow: f64,
    pub rem_slow_full: f64,
    /// Weight of evidence that the long window is irregular too, which keeps a
    /// short disturbance from being scored as a REM bout, plus the long-window
    /// interval CV that qualifies.
    pub rem_sustained: f64,
    pub rem_sustained_cv: f64,
    /// Irregularity beyond these bounds is not REM-shaped breathing.
    pub rem_interval_cap: f64,
    pub rem_volume_cap: f64,
    /// Subtracted per unit of co-occurring wake evidence.
    pub rem_wake_conflict: f64,
    /// Subtracted for a flat-topped, flow-limited contour: obstructive
    /// variability resembles REM but is not REM.
    pub rem_flow_limitation: f64,
    /// Extra weight given to REM later in the night, without imposing a schedule.
    pub rem_progress: f64,
    /// Evidence needed before REM outweighs Light.
    pub rem_threshold: f64,

    // --- Uncertainty ----------------------------------------------------
    /// Minimum fraction of an epoch that must carry usable flow.
    pub usable_flow: f64,
    /// Minimum covered fraction of an epoch.
    pub usable_coverage: f64,
    /// Minimum breaths in the short window.
    pub usable_breaths: usize,
    /// Fraction of samples that may be noisy before the epoch is unusable.
    pub noise_limit: f64,
    /// Excess leak above which an epoch cannot be interpreted, L/min.
    pub leak_limit: f64,

    // --- Sequence model --------------------------------------------------
    /// Expected bout length in epochs for each state. Used only to price leaving
    /// a state early: a smoothing prior, not a sleep cycle.
    pub expected_bout: [f64; 5],
    /// Cost of leaving each state at its expected length.
    pub leave_cost: [f64; 5],
    /// Cost of entering each state from a different one.
    pub enter_cost: [f64; 5],
}

impl Default for Params {
    fn default() -> Self {
        Self {
            emission_scale: 4.0,

            wake_interval_cv: (0.10, 0.30),
            wake_volume_cv: (0.14, 0.40),
            wake_duty_cv: (0.12, 0.32),
            wake_rate: (17.0, 24.0),
            wake_irregular: 1.0,
            wake_fast: 0.12,
            wake_sigh: 0.16,
            wake_arousal: 0.30,
            onset_minutes: 8.0,
            onset_weight: 1.1,
            wake_boundary: 0.10,
            wake_ventilation_discount: (1.10, 1.45),
            wake_threshold: 0.50,

            deep_regularity: 0.70,
            deep_round_breath: 0.18,
            deep_rank_prior: 0.30,
            deep_interval_cap: 0.09,
            deep_volume_cap: 0.11,
            deep_long_interval_cap: 0.13,
            deep_long_volume_cap: 0.18,
            deep_long_ventilation_cap: 0.22,
            deep_long_duty_cap: 0.20,
            deep_duty: (0.20, 0.70),
            deep_rate: (5.0, 26.0),
            deep_squareness: 0.90,
            deep_contrast: 3.0,
            deep_threshold: 0.55,

            rem_hypoventilation: 0.42,
            rem_ventilation: (0.98, 0.72),
            rem_irregular: 0.34,
            rem_effort_spread: 0.16,
            rem_peak_cv: (0.10, 0.40),
            rem_interval_cv: (0.05, 0.17),
            rem_volume_cv: (0.07, 0.24),
            rem_slow: 0.26,
            rem_slow_full: 0.26,
            rem_sustained: 0.22,
            rem_sustained_cv: 0.10,
            rem_interval_cap: 0.62,
            rem_volume_cap: 0.95,
            rem_wake_conflict: 0.85,
            rem_flow_limitation: 0.30,
            rem_progress: 0.10,
            rem_threshold: 0.30,

            usable_flow: 0.5,
            usable_coverage: 0.35,
            usable_breaths: 6,
            noise_limit: 0.18,
            leak_limit: 24.0,

            expected_bout: [5.0, 11.0, 8.0, 18.0, 4.0],
            leave_cost: [1.4, 1.6, 2.0, 2.2, 0.9],
            enter_cost: [1.3, 0.7, 1.8, 1.9, 1.1],
        }
    }
}

impl Params {
    /// Reject parameter sets that would make the model incoherent, so a tuning
    /// run cannot silently produce nonsense.
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(0.05..=10.0).contains(&self.emission_scale) {
            return Err("emission_scale must be between 0.05 and 10");
        }
        if self
            .expected_bout
            .iter()
            .chain(self.leave_cost.iter())
            .chain(self.enter_cost.iter())
            .any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err("transition parameters must be finite and non-negative");
        }
        if self.expected_bout.iter().any(|v| *v < 1.0) {
            return Err("expected_bout must be at least one epoch");
        }
        for (name, threshold) in [
            ("wake_threshold", self.wake_threshold),
            ("deep_threshold", self.deep_threshold),
            ("rem_threshold", self.rem_threshold),
        ] {
            if !(threshold.is_finite() && threshold > 0.0) {
                return Err(match name {
                    "wake_threshold" => "wake_threshold must be positive",
                    "deep_threshold" => "deep_threshold must be positive",
                    _ => "rem_threshold must be positive",
                });
            }
        }
        Ok(())
    }

    /// Linear ramp from zero at `start` to one at `full`.
    pub(crate) fn ramp(value: f64, start: f64, full: f64) -> f64 {
        if full <= start {
            return if value >= full { 1.0 } else { 0.0 };
        }
        ((value - start) / (full - start)).clamp(0.0, 1.0)
    }

    /// Ramp over a `(start, full)` pair, allowing a descending pair
    /// (`full < start`, e.g. ventilation falling from 0.98 to 0.72).
    pub(crate) fn band(value: f64, band: (f64, f64)) -> f64 {
        if band.1 >= band.0 {
            Self::ramp(value, band.0, band.1)
        } else {
            Self::ramp(-value, -band.0, -band.1)
        }
    }

    /// Convert accumulated evidence into a signed score, centred on zero at the
    /// threshold, so evidence below the threshold is actively negative.
    pub(crate) fn score(evidence: f64, threshold: f64) -> f64 {
        let evidence = evidence.max(0.0);
        if evidence <= 0.0 {
            return -1.0;
        }
        let strength = evidence / (evidence + threshold);
        2.0 * strength - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_coherent() {
        Params::default().validate().unwrap();
    }

    #[test]
    fn score_is_zero_at_the_threshold_and_negative_below_it() {
        let half = Params::score(0.3, 0.3);
        assert!(half.abs() < 1e-12, "{half}");
        assert!(Params::score(0.1, 0.3) < 0.0);
        assert!(Params::score(0.9, 0.3) > 0.0);
        // Absent evidence is the floor, not neutral.
        assert_eq!(Params::score(0.0, 0.3), -1.0);
    }

    #[test]
    fn band_supports_both_directions() {
        assert_eq!(Params::band(0.5, (0.0, 1.0)), 0.5);
        // Descending band: 0.85 is halfway from 0.98 down to 0.72.
        assert!((Params::band(0.85, (0.98, 0.72)) - 0.5).abs() < 1e-9);
        assert_eq!(Params::band(0.72, (0.98, 0.72)), 1.0);
        assert_eq!(Params::band(0.99, (0.98, 0.72)), 0.0);
    }

    #[test]
    fn ramp_handles_degenerate_bounds() {
        assert_eq!(Params::ramp(0.5, 1.0, 1.0), 0.0);
        assert_eq!(Params::ramp(1.0, 1.0, 1.0), 1.0);
        assert_eq!(Params::ramp(0.0, 0.0, 1.0), 0.0);
        assert_eq!(Params::ramp(2.0, 0.0, 1.0), 1.0);
    }

    #[test]
    fn validation_rejects_incoherent_thresholds() {
        let mut p = Params::default();
        p.wake_threshold = 0.0;
        assert!(p.validate().is_err());
        let mut p = Params::default();
        p.emission_scale = 100.0;
        assert!(p.validate().is_err());
    }
}
