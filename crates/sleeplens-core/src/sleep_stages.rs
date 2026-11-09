//! Estimated sleep stages from CPAP breathing data.
//!
//! A CPAP records airflow, not brain activity, so these stages are an
//! ESTIMATE from breathing patterns (rate, regularity, amplitude), not a sleep
//! study. The UI must label them as estimated.
//!
//! We smooth flow over 100 ms and identify breaths lasting 1.5 to 12 seconds.
//! Each 30 second epoch uses 150 seconds of nearby breathing. At each mask-on,
//! Wake lasts at least 5 minutes initially or 2 minutes after a gap, then until
//! 3 consecutive stable epochs. This latency is capped at 45 or 15 minutes.
//! Noise above 18%, artifacts above 2%, leak above 24 L/min, and marked
//! irregularity with large breaths also support Wake.
//!
//! Deep uses the mean of interval, volume and ventilation CV divided by their
//! night medians. The most regular 30% early and 20% later can start Deep;
//! the 40th percentile can sustain it. Deep stops after 60% of mask-on time,
//! avoids events and leak, and needs 5 minute bouts. Absolute CV caps of
//! 0.22, 0.42 and 0.45 reject uniformly irregular breathing.
//! REM needs breathing irregularity within 90 minute cycle windows centered
//! 75 minutes after sleep onset, then every 90 minutes. Windows grow from
//! 16 to 30 minutes. No REM occurs in the first 60 minutes after each onset.
//! Light gaps up to 3 minutes can join REM, and REM needs 5 minute bouts.
//! Everything else is Light. Missing epochs stay NoData.
//!
//! These numerical rules are heuristics calibrated for plausible architecture,
//! not validated against EEG. CPAP cannot distinguish quiet wake from sleep
//! reliably, and breathing events can resemble REM. No stage percentage is
//! forced. Signal quality measures usable breathing, not staging accuracy.

use serde::{Deserialize, Serialize};

/// Length of one scoring epoch, as in sleep studies.
pub const EPOCH_SECONDS: u32 = 30;

/// Bumped whenever the algorithm changes, so stored nights get recomputed.
pub const ESTIMATOR_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SleepStage {
    /// Mask on but likely awake (irregular breathing, movement, leak bursts).
    Wake,
    Light,
    Deep,
    Rem,
    /// Mask off / no data for this epoch.
    NoData,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageMinutes {
    pub wake: f32,
    pub light: f32,
    pub deep: f32,
    pub rem: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SleepStageEstimate {
    /// Wall-clock time of the first epoch, `YYYY-MM-DDTHH:MM:SS`.
    pub start: String,
    pub epoch_seconds: u32,
    /// One entry per epoch from `start`, covering the whole night including gaps.
    pub stages: Vec<SleepStage>,
    pub minutes: StageMinutes,
    /// 0..1, how much the breathing signal supported the estimate (signal quality, not accuracy).
    pub signal_quality: f32,
    pub estimator_version: u32,
}

/// Input for one night.
pub struct SleepStageInput<'a> {
    /// Wall-clock time of `flow[0]`, `YYYY-MM-DDTHH:MM:SS`.
    pub start: &'a str,
    /// Patient flow in L/min at `hz` samples per second (50 on the AS100),
    /// continuous from `start`; `f32::NAN` where the mask was off.
    pub flow: &'a [f32],
    pub hz: u32,
    /// Optional 1 Hz excess leak (L/min) aligned with `start`, NAN for gaps.
    pub excess_leak: &'a [f32],
    /// Seconds from `start` of machine-scored apnea/hypopnea events (end time).
    pub event_offsets: &'a [u32],
}

#[derive(Clone, Copy, Default)]
struct EpochFeatures {
    valid: usize,
    artifacts: usize,
    energy: f64,
    noise: f64,
    breaths: usize,
    duration: f64,
    duration_squared: f64,
    volume: f64,
    volume_squared: f64,
    largest_volume: f64,
    ventilation: f64,
    ventilation_squared: f64,
    leak: f32,
    events: usize,
}

fn cv(sum: f64, squared: f64, count: usize) -> f64 {
    if count < 2 || sum <= 0.0 {
        return 0.0;
    }
    let mean = sum / count as f64;
    (squared / count as f64 - mean * mean).max(0.0).sqrt() / mean
}

fn percentile(values: &[f64], fraction: f64, fallback: f64) -> f64 {
    if values.is_empty() {
        return fallback;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable_by(f64::total_cmp);
    sorted[((sorted.len() - 1) as f64 * fraction).round() as usize]
}

#[derive(Default)]
struct Diagnostics {
    features: Vec<(&'static str, Vec<f64>)>,
    rules: Vec<(&'static str, usize)>,
}

/// Aggregate calibration diagnostics only. Contains no dates or identifiers.
#[doc(hidden)]
pub fn sleep_stage_debug(input: &SleepStageInput<'_>) -> String {
    use std::fmt::Write;
    let mut diagnostics = Diagnostics::default();
    estimate(input, Some(&mut diagnostics));
    let mut report = String::from("feature populations: sleep = supported epochs excluding Wake, artifacts, nearby events and NoData; observed = all epochs with finite flow, including unsupported epochs\nfeature p10 p25 p50 p75 p90\n");
    for (name, values) in diagnostics.features {
        let _ = write!(report, "{name}");
        for fraction in [0.1, 0.25, 0.5, 0.75, 0.9] {
            let _ = write!(report, " {:.4}", percentile(&values, fraction, 0.0));
        }
        let _ = writeln!(report, " n={}", values.len());
    }
    for (name, count) in diagnostics.rules {
        let _ = writeln!(report, "rule {name}: {count} epochs");
    }
    report
}

/// Estimate sleep stages for one night.
///
/// All finite flow samples count toward masked-on minutes, including a partial
/// last epoch. A wholly absent epoch is NoData. Sparse or flat observed epochs
/// default to Light with low signal quality after the capped onset period.
/// A zero sample rate returns no estimate because sample times are unknown.
pub fn estimate_sleep_stages(input: &SleepStageInput<'_>) -> SleepStageEstimate {
    estimate(input, None)
}

fn estimate(
    input: &SleepStageInput<'_>,
    diagnostics: Option<&mut Diagnostics>,
) -> SleepStageEstimate {
    let mut result = SleepStageEstimate {
        start: input.start.to_string(),
        epoch_seconds: EPOCH_SECONDS,
        stages: Vec::new(),
        minutes: StageMinutes::default(),
        signal_quality: 0.0,
        estimator_version: ESTIMATOR_VERSION,
    };
    if input.hz == 0 || input.flow.is_empty() {
        return result;
    }
    let hz = input.hz as usize;
    let Some(per_epoch) = hz.checked_mul(EPOCH_SECONDS as usize) else {
        return result;
    };
    let epochs = input.flow.len().div_ceil(per_epoch);
    let mut local = vec![EpochFeatures::default(); epochs];
    let alpha = 1.0 / (1.0 + 0.1 * hz as f64);
    let mut filtered = 0.0;
    let mut initialized = false;
    let mut armed = false;
    let mut breath_start: Option<usize> = None;
    let mut inspired = 0.0;
    for (i, &sample) in input.flow.iter().enumerate() {
        let f = &mut local[i / per_epoch];
        if !sample.is_finite() {
            // Never join two breaths across missing samples.
            initialized = false;
            armed = false;
            breath_start = None;
            inspired = 0.0;
            continue;
        }
        f.valid += 1;
        if sample.abs() > 120.0 {
            f.artifacts += 1;
        }
        let sample = f64::from(sample.clamp(-200.0, 200.0));
        if !initialized {
            filtered = sample;
            initialized = true;
        }
        filtered += alpha * (sample - filtered);
        f.energy += filtered * filtered;
        f.noise += (sample - filtered).powi(2);
        inspired += filtered.max(0.0) / (60.0 * hz as f64);
        if filtered < -0.8 {
            armed = true;
        }
        if armed && filtered > 0.8 {
            armed = false;
            if let Some(start) = breath_start {
                let duration = (i - start) as f64 / hz as f64;
                // Tiny oscillations are noise. Large breaths are retained so
                // their variability can support Wake instead of disappearing.
                if (1.5..=12.0).contains(&duration) && inspired >= 0.03 {
                    let center = (start + (i - start) / 2) / per_epoch;
                    let b = &mut local[center];
                    b.breaths += 1;
                    b.duration += duration;
                    b.duration_squared += duration * duration;
                    b.volume += inspired;
                    b.volume_squared += inspired * inspired;
                    b.largest_volume = b.largest_volume.max(inspired);
                    b.ventilation += inspired * 60.0 / duration;
                    b.ventilation_squared += (inspired * 60.0 / duration).powi(2);
                }
            }
            breath_start = Some(i);
            inspired = 0.0;
        }
    }
    for (second, &leak) in input.excess_leak.iter().enumerate() {
        if second / (EPOCH_SECONDS as usize) >= epochs {
            break;
        }
        if leak.is_finite() {
            let f = &mut local[second / EPOCH_SECONDS as usize];
            f.leak = f.leak.max(leak);
        }
    }
    for &offset in input.event_offsets {
        if let Some(f) = local.get_mut(offset as usize / EPOCH_SECONDS as usize) {
            f.events += 1;
        }
    }
    // Five epochs give a 2.5 minute context. Stop at missing epochs so mask
    // breaks cannot manufacture variability between disconnected sessions.
    let mut contexts = Vec::with_capacity(epochs);
    for e in 0..epochs {
        let mut lo = e;
        let mut hi = e;
        while lo > e.saturating_sub(2) && local[lo - 1].valid > 0 {
            lo -= 1;
        }
        while hi + 1 < epochs.min(e + 3) && local[hi + 1].valid > 0 {
            hi += 1;
        }
        let mut c = EpochFeatures::default();
        for f in &local[lo..=hi] {
            c.breaths += f.breaths;
            c.events += f.events;
            c.leak = c.leak.max(f.leak);
            c.duration += f.duration;
            c.duration_squared += f.duration_squared;
            c.volume += f.volume;
            c.volume_squared += f.volume_squared;
            c.ventilation += f.ventilation;
            c.ventilation_squared += f.ventilation_squared;
        }
        contexts.push(c);
    }
    let intervals: Vec<_> = contexts
        .iter()
        .map(|c| cv(c.duration, c.duration_squared, c.breaths))
        .collect();
    let volumes: Vec<_> = contexts
        .iter()
        .map(|c| cv(c.volume, c.volume_squared, c.breaths))
        .collect();
    let ventilations: Vec<_> = contexts
        .iter()
        .map(|c| cv(c.ventilation, c.ventilation_squared, c.breaths))
        .collect();
    let rates: Vec<_> = contexts
        .iter()
        .map(|c| 60.0 * c.breaths as f64 / c.duration.max(1.0))
        .collect();
    let ventilation: Vec<_> = contexts
        .iter()
        .map(|c| 60.0 * c.volume / c.duration.max(1.0))
        .collect();
    let noises: Vec<_> = local
        .iter()
        .map(|f| f.noise / (f.energy + f.noise).max(1.0))
        .collect();
    let artifacts: Vec<_> = local
        .iter()
        .map(|f| f.artifacts as f64 / f.valid.max(1) as f64)
        .collect();
    let supported: Vec<_> = local
        .iter()
        .zip(&contexts)
        .map(|(f, c)| {
            c.breaths >= 6
                && f.duration * hz as f64 / f.valid.max(1) as f64 > 0.35
                && f.valid as f64 / per_epoch as f64 >= 0.5
        })
        .collect();
    let artifact: Vec<_> = (0..epochs)
        .map(|e| noises[e] > 0.18 || artifacts[e] > 0.02 || local[e].leak > 24.0)
        .collect();
    let population: Vec<_> = (0..epochs)
        .filter(|&e| supported[e] && !artifact[e] && contexts[e].events == 0)
        .collect();
    let values = |feature: &[f64]| population.iter().map(|&e| feature[e]).collect::<Vec<_>>();
    let night_ventilation = percentile(&values(&ventilation), 0.5, 6.0).max(0.1);
    let relative_ventilation: Vec<_> = ventilation.iter().map(|v| v / night_ventilation).collect();
    let interval_stable = percentile(&values(&intervals), 0.75, 0.15).clamp(0.15, 0.30);
    let volume_stable = percentile(&values(&volumes), 0.75, 0.25).clamp(0.25, 0.50);
    let mut onset_wake = vec![false; epochs];
    let mut irregular_wake = vec![false; epochs];
    let mut boundary_wake = vec![false; epochs];
    let mut sleep_elapsed = vec![0.0; epochs];
    let mut quality_seconds = 0.0;
    let mut valid_total = 0;
    let mut session_start = 0;
    let mut first_session = true;
    let mut onset = None;
    let mut stable_run = 0;
    for e in 0..epochs {
        let f = local[e];
        valid_total += f.valid;
        if f.valid == 0 {
            result.stages.push(SleepStage::NoData);
            session_start = e + 1;
            onset = None;
            stable_run = 0;
            continue;
        }
        if e == session_start && e > 0 {
            first_session = false;
        }
        let coverage = (f.duration * hz as f64 / f.valid as f64).min(1.0);
        quality_seconds +=
            f.valid as f64 / hz as f64 * coverage * (1.0 - noises[e]) * (1.0 - artifacts[e]);
        let mean_volume = contexts[e].volume / contexts[e].breaths.max(1) as f64;
        let large_breath = f.largest_volume > mean_volume * 1.9 || relative_ventilation[e] > 1.65;
        irregular_wake[e] = supported[e]
            && (intervals[e] > 0.48
                || volumes[e] > 0.65
                || (large_breath && (intervals[e] > 0.24 || volumes[e] > 0.38)));
        let near_end = (e + 1..(e + 4).min(epochs)).any(|i| local[i].valid == 0) || e + 3 >= epochs;
        boundary_wake[e] = near_end
            && supported[e]
            && (intervals[e] > interval_stable || volumes[e] > volume_stable || large_breath);
        if onset.is_none() {
            let minimum = if first_session { 10 } else { 4 };
            let cap = if first_session { 90 } else { 30 };
            let stable = supported[e]
                && !artifact[e]
                && !irregular_wake[e]
                && intervals[e] <= interval_stable
                && volumes[e] <= volume_stable;
            stable_run = if stable && e - session_start >= minimum {
                stable_run + 1
            } else {
                0
            };
            if stable_run >= 3 || e - session_start >= cap {
                onset = Some(e);
            } else {
                onset_wake[e] = true;
            }
        }
        sleep_elapsed[e] = onset.map_or(0.0, |start| (e - start) as f64 * 0.5);
        result.stages.push(
            if artifact[e] || onset_wake[e] || irregular_wake[e] || boundary_wake[e] {
                SleepStage::Wake
            } else {
                SleepStage::Light
            },
        );
    }
    // Recompute the relative distribution after excluding likely wake.
    let sleeping: Vec<_> = population
        .iter()
        .copied()
        .filter(|&e| result.stages[e] != SleepStage::Wake)
        .collect();
    let sleep_values = |feature: &[f64]| sleeping.iter().map(|&e| feature[e]).collect::<Vec<_>>();
    let interval_median = percentile(&sleep_values(&intervals), 0.5, 0.1).max(0.02);
    let volume_median = percentile(&sleep_values(&volumes), 0.5, 0.15).max(0.02);
    let ventilation_median = percentile(&sleep_values(&ventilations), 0.5, 0.15).max(0.02);
    let regularity: Vec<_> = (0..epochs)
        .map(|e| {
            (intervals[e] / interval_median
                + volumes[e] / volume_median
                + ventilations[e] / ventilation_median)
                / 3.0
        })
        .collect();
    let scores: Vec<_> = sleeping.iter().map(|&e| regularity[e]).collect();
    let early_threshold = percentile(&scores, 0.30, 0.0);
    let later_threshold = percentile(&scores, 0.20, 0.0);
    let sustain_threshold = percentile(&scores, 0.40, 0.0);
    let observed_epochs = local.iter().filter(|f| f.valid > 0).count().max(1);
    let mut observed = 0;
    let mut deep_active = false;
    let mut deep_candidates = 0;
    let mut deep_sanity = 0;
    for e in 0..epochs {
        if local[e].valid > 0 {
            observed += 1;
        }
        let progress = observed as f64 / observed_epochs as f64;
        let sanity = intervals[e] < 0.22
            && volumes[e] < 0.42
            && ventilations[e] < 0.45
            && (6.0..=28.0).contains(&rates[e])
            && (0.55..=1.6).contains(&relative_ventilation[e]);
        if supported[e] && sanity {
            deep_sanity += 1;
        }
        let clean = contexts[e].events == 0 && contexts[e].leak < 12.0;
        let threshold = if deep_active {
            sustain_threshold
        } else if progress < 0.33 {
            early_threshold
        } else {
            later_threshold
        };
        deep_active = result.stages[e] == SleepStage::Light
            && supported[e]
            && sanity
            && clean
            && progress <= 0.60
            && regularity[e] <= threshold;
        if deep_active {
            result.stages[e] = SleepStage::Deep;
            deep_candidates += 1;
        }
    }
    let mut rem_evidence = vec![false; epochs];
    let mut rem_windows = vec![false; epochs];
    for e in 0..epochs {
        rem_evidence[e] = result.stages[e] != SleepStage::Wake
            && supported[e]
            && intervals[e] > (interval_median * 0.85).max(0.07)
            && volumes[e] > (volume_median * 0.85).max(0.12)
            && contexts[e].events == 0
            && contexts[e].leak < 12.0;
        let elapsed = sleep_elapsed[e];
        let cycle = ((elapsed - 75.0) / 90.0).round().max(0.0) as usize;
        let half_width = [8.0, 11.0, 13.0, 15.0][cycle.min(3)];
        rem_windows[e] =
            elapsed >= 60.0 && (elapsed - (75.0 + cycle as f64 * 90.0)).abs() <= half_width;
    }
    let mut begin = 0;
    let mut rem_candidates = 0;
    while begin < epochs {
        if !rem_windows[begin] || local[begin].valid == 0 {
            begin += 1;
            continue;
        }
        let mut end = begin + 1;
        while end < epochs && rem_windows[end] && local[end].valid > 0 {
            end += 1;
        }
        let evidence = rem_evidence[begin..end].iter().filter(|&&v| v).count();
        // A window needs actual irregular breathing in at least a fifth of its
        // epochs. Once supported, the cycle prior groups the intervening sleep.
        if evidence * 5 >= end - begin {
            for (e, stage) in result.stages.iter_mut().enumerate().take(end).skip(begin) {
                if *stage != SleepStage::Wake
                    && supported[e]
                    && contexts[e].events == 0
                    && contexts[e].leak < 12.0
                {
                    *stage = SleepStage::Rem;
                    rem_candidates += 1;
                }
            }
        }
        begin = end;
    }
    // Bridge only Light. Wake, events and missing data always break REM.
    let mut e = 0;
    let mut rem_bridged = 0;
    while e < epochs {
        if result.stages[e] != SleepStage::Light {
            e += 1;
            continue;
        }
        let start = e;
        while e < epochs && result.stages[e] == SleepStage::Light {
            e += 1;
        }
        if start > 0
            && e < epochs
            && e - start <= 6
            && result.stages[start - 1] == SleepStage::Rem
            && result.stages[e] == SleepStage::Rem
            && (start..e).all(|i| {
                supported[i]
                    && contexts[i].events == 0
                    && contexts[i].leak < 12.0
                    && sleep_elapsed[i] >= 60.0
            })
        {
            result.stages[start..e].fill(SleepStage::Rem);
            rem_bridged += e - start;
        }
    }
    let mut dropped_deep = 0;
    let mut dropped_rem = 0;
    let mut begin = 0;
    while begin < epochs {
        let stage = result.stages[begin];
        let mut end = begin + 1;
        while end < epochs && result.stages[end] == stage {
            end += 1;
        }
        if matches!(stage, SleepStage::Deep | SleepStage::Rem) && end - begin < 10 {
            result.stages[begin..end].fill(SleepStage::Light);
            if stage == SleepStage::Deep {
                dropped_deep += end - begin;
            } else {
                dropped_rem += end - begin;
            }
        }
        begin = end;
    }
    if let Some(d) = diagnostics {
        let feature_values = |v: &[f64]| sleeping.iter().map(|&e| v[e]).collect();
        d.features = vec![
            ("sleep_interval_cv", feature_values(&intervals)),
            ("sleep_volume_cv", feature_values(&volumes)),
            ("sleep_ventilation_cv", feature_values(&ventilations)),
            ("sleep_combined_regularity", feature_values(&regularity)),
            ("sleep_rate_per_min", feature_values(&rates)),
            (
                "sleep_relative_ventilation",
                feature_values(&relative_ventilation),
            ),
            ("sleep_noise_fraction", feature_values(&noises)),
            ("sleep_artifact_fraction", feature_values(&artifacts)),
            (
                "sleep_largest_breath_ratio",
                sleeping
                    .iter()
                    .map(|&e| {
                        local[e].largest_volume
                            / (contexts[e].volume / contexts[e].breaths.max(1) as f64).max(0.001)
                    })
                    .collect(),
            ),
            (
                "sleep_leak_l_per_min",
                sleeping.iter().map(|&e| f64::from(local[e].leak)).collect(),
            ),
        ];
        let observed_values = |v: &[f64]| {
            (0..epochs)
                .filter(|&e| local[e].valid > 0)
                .map(|e| v[e])
                .collect()
        };
        d.features.extend([
            ("observed_interval_cv", observed_values(&intervals)),
            ("observed_volume_cv", observed_values(&volumes)),
            ("observed_ventilation_cv", observed_values(&ventilations)),
            ("observed_combined_regularity", observed_values(&regularity)),
            ("observed_rate_per_min", observed_values(&rates)),
            (
                "observed_relative_ventilation",
                observed_values(&relative_ventilation),
            ),
            ("observed_noise_fraction", observed_values(&noises)),
            ("observed_artifact_fraction", observed_values(&artifacts)),
            (
                "observed_largest_breath_ratio",
                (0..epochs)
                    .filter(|&e| local[e].valid > 0)
                    .map(|e| {
                        local[e].largest_volume
                            / (contexts[e].volume / contexts[e].breaths.max(1) as f64).max(0.001)
                    })
                    .collect(),
            ),
            (
                "observed_leak_l_per_min",
                (0..epochs)
                    .filter(|&e| local[e].valid > 0)
                    .map(|e| f64::from(local[e].leak))
                    .collect(),
            ),
        ]);
        d.features.extend([
            (
                "observed_event_context_count",
                (0..epochs)
                    .filter(|&e| local[e].valid > 0)
                    .map(|e| contexts[e].events as f64)
                    .collect(),
            ),
            (
                "observed_context_breaths",
                (0..epochs)
                    .filter(|&e| local[e].valid > 0)
                    .map(|e| contexts[e].breaths as f64)
                    .collect(),
            ),
            (
                "observed_breath_coverage",
                (0..epochs)
                    .filter(|&e| local[e].valid > 0)
                    .map(|e| (local[e].duration * hz as f64 / local[e].valid as f64).min(1.0))
                    .collect(),
            ),
            (
                "observed_finite_fraction",
                (0..epochs)
                    .filter(|&e| local[e].valid > 0)
                    .map(|e| local[e].valid as f64 / per_epoch as f64)
                    .collect(),
            ),
        ]);
        let count = |v: &[bool]| v.iter().filter(|&&b| b).count();
        d.rules = vec![
            ("supported", count(&supported)),
            (
                "unsupported_observed",
                (0..epochs)
                    .filter(|&e| local[e].valid > 0 && !supported[e])
                    .count(),
            ),
            (
                "event_context_exclusion",
                (0..epochs)
                    .filter(|&e| local[e].valid > 0 && contexts[e].events > 0)
                    .count(),
            ),
            ("artifact_or_leak_wake", count(&artifact)),
            ("onset_wake", count(&onset_wake)),
            ("irregular_wake", count(&irregular_wake)),
            ("mask_off_irregular_wake", count(&boundary_wake)),
            ("deep_absolute_sanity", deep_sanity),
            ("deep_candidates", deep_candidates),
            ("deep_short_bout_dropped", dropped_deep),
            ("rem_irregular_evidence", count(&rem_evidence)),
            ("rem_cycle_window", count(&rem_windows)),
            ("rem_candidates", rem_candidates),
            ("rem_light_gap_bridged", rem_bridged),
            ("rem_short_bout_dropped", dropped_rem),
        ];
    }
    // Accumulate in f64 before conversion to avoid drift on long recordings.
    let mut minutes = [0.0_f64; 4];
    for (stage, f) in result.stages.iter().zip(&local) {
        let index = match stage {
            SleepStage::Wake => 0,
            SleepStage::Light => 1,
            SleepStage::Deep => 2,
            SleepStage::Rem => 3,
            SleepStage::NoData => continue,
        };
        minutes[index] += f.valid as f64 / (hz as f64 * 60.0);
    }
    result.minutes = StageMinutes {
        wake: minutes[0] as f32,
        light: minutes[1] as f32,
        deep: minutes[2] as f32,
        rem: minutes[3] as f32,
    };
    if valid_total > 0 {
        result.signal_quality =
            (quality_seconds / (valid_total as f64 / hz as f64)).clamp(0.0, 1.0) as f32;
    }
    result
}
