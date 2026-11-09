//! Estimates 30-second sleep stages from CPAP airflow, leak and machine events.
//!
//! This heuristic is unvalidated against sleep studies. Airflow offers moderate
//! evidence for sleep versus wake, low confidence for REM and very low confidence
//! for Deep. Quiet wake can look like sleep. Signal quality is not stage accuracy.
//!
//! Breath timing, volume, ventilation and inspiratory duty cycle use medians and
//! interquartile ranges over 90 seconds and five minutes. Irregular breathing
//! with large breaths supports Wake; regular breathing supports Deep. Lower
//! relative ventilation adds evidence for REM. Time since mask-on and time of
//! night adjust scores, without fixed sleep cycles or minimum bout lengths.
//! A sequence decoder penalizes abrupt stage changes, but every REM epoch still
//! needs its own nearby breathing evidence. Deep also needs absolute regularity.
//!
//! Large leak, artifacts, clipping, event recovery and repeated waxing and waning
//! breathing produce Uncertain instead of guessing a stage. Missing flow remains
//! NoData. These thresholds need paired airflow and EEG validation.

use serde::{Deserialize, Serialize};

pub const EPOCH_SECONDS: u32 = 30;
pub const ESTIMATOR_VERSION: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SleepStage {
    Wake,
    Light,
    Deep,
    Rem,
    /// Mask on, but the breathing signal cannot support a stage.
    Uncertain,
    NoData,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageMinutes {
    pub wake: f32,
    pub light: f32,
    pub deep: f32,
    pub rem: f32,
    pub uncertain: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageConfidence {
    pub sleep_wake: String,
    pub rem: String,
    pub deep: String,
}

impl Default for StageConfidence {
    fn default() -> Self {
        Self {
            sleep_wake: "moderate".into(),
            rem: "low".into(),
            deep: "veryLow".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SleepStageEstimate {
    pub start: String,
    pub epoch_seconds: u32,
    pub stages: Vec<SleepStage>,
    pub minutes: StageMinutes,
    /// Fraction of observed flow usable for estimation, not staging accuracy.
    pub signal_quality: f32,
    pub confidence: StageConfidence,
    pub estimator_version: u32,
}

pub struct SleepStageInput<'a> {
    pub start: &'a str,
    /// Patient flow in L/min; NAN marks mask-off gaps.
    pub flow: &'a [f32],
    pub hz: u32,
    /// Optional 1 Hz excess leak aligned with flow.
    pub excess_leak: &'a [f32],
    /// Machine-scored apnea/hypopnea end times, in seconds from start.
    pub event_offsets: &'a [u32],
}

#[derive(Clone, Copy)]
struct Breath {
    duration: f64,
    volume: f64,
    ventilation: f64,
    duty: f64,
}

#[derive(Default, Clone)]
struct Epoch {
    valid: usize,
    artifacts: usize,
    clipped: usize,
    energy: f64,
    noise: f64,
    covered: f64,
    leak_samples: usize,
    large_leak: usize,
    event: bool,
    breaths: Vec<Breath>,
}

#[derive(Default, Clone, Copy)]
struct Context {
    count: usize,
    interval_var: f64,
    volume_var: f64,
    ventilation_var: f64,
    duty_var: f64,
    duty: f64,
    volume: f64,
    ventilation: f64,
    rate: f64,
    largest: f64,
}

fn percentile(values: &[f64], fraction: f64, fallback: f64) -> f64 {
    if values.is_empty() {
        return fallback;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable_by(f64::total_cmp);
    sorted[((sorted.len() - 1) as f64 * fraction).round() as usize]
}

fn robust(values: &[f64]) -> (f64, f64) {
    let median = percentile(values, 0.5, 0.0);
    let variability = (percentile(values, 0.75, 0.0) - percentile(values, 0.25, 0.0))
        / (1.349 * median.max(0.001));
    (median, variability)
}

fn context(local: &[Epoch], clean: &[bool], e: usize, before: usize, after: usize) -> Context {
    let mut lo = e;
    let mut hi = e;
    while lo > e.saturating_sub(before) && local[lo - 1].valid > 0 {
        lo -= 1;
    }
    while hi + 1 < local.len().min(e + after + 1) && local[hi + 1].valid > 0 {
        hi += 1;
    }
    let breaths: Vec<_> = (lo..=hi)
        .filter(|&i| clean[i])
        .flat_map(|i| &local[i].breaths)
        .collect();
    let values = |get: fn(&Breath) -> f64| breaths.iter().map(|b| get(b)).collect::<Vec<_>>();
    let (duration, interval_var) = robust(&values(|b| b.duration));
    let (volume, volume_var) = robust(&values(|b| b.volume));
    let (ventilation, ventilation_var) = robust(&values(|b| b.ventilation));
    let (duty, duty_var) = robust(&values(|b| b.duty));
    Context {
        count: breaths.len(),
        interval_var,
        volume_var,
        ventilation_var,
        duty_var,
        duty,
        volume,
        ventilation,
        rate: 60.0 / duration.max(1.0),
        largest: breaths.iter().map(|b| b.volume).fold(0.0, f64::max),
    }
}

// A five-second RMS envelope removes individual breaths. Require three cycles,
// strong autocorrelation and an opposite half-cycle, rather than irregularity.
fn periodic(envelope: &[Option<f64>], epoch: usize) -> bool {
    let center = epoch * 6 + 3;
    let lo = center.saturating_sub(30);
    let hi = envelope.len().min(center + 30);
    let window = &envelope[lo..hi];
    if window.len() < 24 || window.iter().any(Option::is_none) {
        return false;
    }
    let values: Vec<_> = window.iter().flatten().copied().collect();
    let median = percentile(&values, 0.5, 0.0);
    if median < 0.5 || percentile(&values, 0.9, 0.0) - percentile(&values, 0.1, 0.0) < median * 0.55
    {
        return false;
    }
    let correlation = |lag: usize| {
        let a = &values[..values.len() - lag];
        let b = &values[lag..];
        let ma = a.iter().sum::<f64>() / a.len() as f64;
        let mb = b.iter().sum::<f64>() / b.len() as f64;
        let mut cross = 0.0;
        let mut ea = 0.0;
        let mut eb = 0.0;
        for (&x, &y) in a.iter().zip(b) {
            cross += (x - ma) * (y - mb);
            ea += (x - ma).powi(2);
            eb += (y - mb).powi(2);
        }
        cross / (ea * eb).sqrt().max(0.001)
    };
    (8..=18).any(|lag| {
        values.len() >= lag * 3 && correlation(lag) > 0.72 && correlation(lag / 2) < -0.35
    })
}

#[derive(Default)]
struct Diagnostics {
    features: Vec<(&'static str, Vec<f64>)>,
    rules: Vec<(&'static str, usize)>,
}

/// Aggregate diagnostics only, without dates or identifiers.
#[doc(hidden)]
pub fn sleep_stage_debug(input: &SleepStageInput<'_>) -> String {
    use std::fmt::Write;
    let mut diagnostics = Diagnostics::default();
    estimate(input, Some(&mut diagnostics));
    let mut report = String::from("feature populations: observed = epochs with finite flow; robust variability = IQR / (1.349 * median)\nfeature p10 p25 p50 p75 p90\n");
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

/// Estimate stages while counting all finite samples toward mask-on minutes.
/// Empty epochs remain NoData; confounded or unsupported observed epochs are
/// Uncertain. A zero sample rate returns an empty estimate.
pub fn estimate_sleep_stages(input: &SleepStageInput<'_>) -> SleepStageEstimate {
    estimate(input, None)
}

fn estimate(
    input: &SleepStageInput<'_>,
    diagnostics: Option<&mut Diagnostics>,
) -> SleepStageEstimate {
    let mut result = SleepStageEstimate {
        start: input.start.into(),
        epoch_seconds: EPOCH_SECONDS,
        stages: Vec::new(),
        minutes: StageMinutes::default(),
        signal_quality: 0.0,
        confidence: StageConfidence::default(),
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
    let mut local = vec![Epoch::default(); epochs];
    let mut envelope_energy = vec![(0.0, 0_usize); input.flow.len().div_ceil(hz * 5)];
    let alpha = 1.0 / (1.0 + 0.1 * hz as f64);
    let mut filtered = 0.0;
    let mut initialized = false;
    let mut armed = false;
    let mut breath_start: Option<usize> = None;
    let mut inspired = 0.0;
    let mut inspiration = 0_usize;
    let mut previous = 0.0_f32;
    let mut flat_start = 0_usize;
    for (i, &raw) in input.flow.iter().enumerate() {
        if !raw.is_finite() {
            initialized = false;
            armed = false;
            breath_start = None;
            inspired = 0.0;
            inspiration = 0;
            continue;
        }
        // AS100 flow is quantized to whole L/min. Flat peaks over a few
        // samples are normal; only an exact run longer than five seconds is
        // flatline evidence. Count the whole run, including across epochs.
        if !initialized || raw != previous {
            flat_start = i;
        }
        previous = raw;
        let flat_length = i - flat_start + 1;
        if flat_length == hz * 5 + 1 {
            let first = flat_start / per_epoch;
            let last = i / per_epoch;
            for (epoch, f) in local.iter_mut().enumerate().take(last + 1).skip(first) {
                let lo = flat_start.max(epoch * per_epoch);
                let hi = (i + 1).min((epoch + 1) * per_epoch);
                f.clipped += hi - lo;
            }
        } else if flat_length > hz * 5 + 1 {
            local[i / per_epoch].clipped += 1;
        }
        let f = &mut local[i / per_epoch];
        f.valid += 1;
        f.artifacts += usize::from(raw.abs() > 150.0);
        let sample = f64::from(raw.clamp(-200.0, 200.0));
        if !initialized {
            filtered = sample;
            initialized = true;
        }
        filtered += alpha * (sample - filtered);
        f.energy += filtered * filtered;
        f.noise += (sample - filtered).powi(2);
        let bin = &mut envelope_energy[i / (hz * 5)];
        bin.0 += filtered * filtered;
        bin.1 += 1;
        inspired += filtered.max(0.0) / (60.0 * hz as f64);
        inspiration += usize::from(filtered > 0.0);
        if filtered < -0.8 {
            armed = true;
        }
        if armed && filtered > 0.8 {
            armed = false;
            if let Some(start) = breath_start {
                let duration = (i - start) as f64 / hz as f64;
                if (1.5..=12.0).contains(&duration) && inspired >= 0.03 {
                    let center = (start + (i - start) / 2) / per_epoch;
                    local[center].covered += duration;
                    local[center].breaths.push(Breath {
                        duration,
                        volume: inspired,
                        ventilation: inspired * 60.0 / duration,
                        duty: inspiration as f64 / (i - start) as f64,
                    });
                }
            }
            breath_start = Some(i);
            inspired = 0.0;
            inspiration = 0;
        }
    }
    for (second, &leak) in input.excess_leak.iter().enumerate() {
        let Some(f) = local.get_mut(second / EPOCH_SECONDS as usize) else {
            break;
        };
        if leak.is_finite() {
            f.leak_samples += 1;
            f.large_leak += usize::from(leak > 24.0);
        }
    }
    for &offset in input.event_offsets {
        let first = offset.saturating_sub(40) as usize / EPOCH_SECONDS as usize;
        let last = (u64::from(offset) + 30) as usize / EPOCH_SECONDS as usize;
        for f in local.iter_mut().take((last + 1).min(epochs)).skip(first) {
            f.event = true;
        }
    }
    let envelope: Vec<_> = envelope_energy
        .iter()
        .map(|&(energy, count)| (count >= hz * 4).then(|| (energy / count as f64).sqrt()))
        .collect();
    let failed_noisy_detection: Vec<_> = local
        .iter()
        .map(|f| {
            f.covered * hz as f64 / f.valid.max(1) as f64 <= 0.35
                && f.noise / (f.energy + f.noise).max(1.0) > 0.18
        })
        .collect();
    let artifacts: Vec<_> = local
        .iter()
        .enumerate()
        .map(|(e, f)| f.artifacts > 0 || f.clipped > 0 || failed_noisy_detection[e])
        .collect();
    let leaks: Vec<_> = local
        .iter()
        .map(|f| f.large_leak > 0 && f.large_leak * 3 >= f.leak_samples.max(1))
        .collect();
    let periodic: Vec<_> = (0..epochs).map(|e| periodic(&envelope, e)).collect();
    let clean: Vec<_> = (0..epochs)
        .map(|e| !artifacts[e] && !leaks[e] && !local[e].event && !periodic[e])
        .collect();
    let short: Vec<_> = (0..epochs)
        .map(|e| context(&local, &clean, e, 1, 1))
        .collect();
    let long: Vec<_> = (0..epochs)
        .map(|e| context(&local, &clean, e, 4, 5))
        .collect();
    let supported: Vec<_> = (0..epochs)
        .map(|e| {
            local[e].valid as f64 >= per_epoch as f64 * 0.5
                && local[e].covered * hz as f64 / local[e].valid.max(1) as f64 > 0.35
                && short[e].count >= 6
        })
        .collect();
    let uncertain: Vec<_> = (0..epochs)
        .map(|e| !supported[e] || artifacts[e] || leaks[e] || local[e].event || periodic[e])
        .collect();
    let population: Vec<_> = (0..epochs).filter(|&e| !uncertain[e]).collect();
    let baseline_ventilation = percentile(
        &population
            .iter()
            .map(|&e| long[e].ventilation)
            .collect::<Vec<_>>(),
        0.5,
        6.0,
    )
    .max(0.1);
    let regularity: Vec<_> = short
        .iter()
        .map(|c| (c.interval_var + c.volume_var + c.ventilation_var) / 3.0)
        .collect();
    // Rank supported, unconfounded epochs before applying absolute caps.
    // Selecting indices rather than a percentile value keeps tied scores from
    // admitting an entire uniformly regular night to Deep.
    let mut regularity_order = population.clone();
    regularity_order.sort_by(|&a, &b| regularity[a].total_cmp(&regularity[b]).then(a.cmp(&b)));
    let mut regular_quartile = vec![false; epochs];
    for &e in regularity_order.iter().take(population.len().div_ceil(4)) {
        regular_quartile[e] = true;
    }
    let mut scores = vec![[f64::NEG_INFINITY; 5]; epochs];
    let mut rem_evidence = vec![false; epochs];
    let mut wake_evidence = vec![false; epochs];
    let mut deep_evidence = vec![false; epochs];
    let observed = local.iter().filter(|f| f.valid > 0).count().max(1);
    let mut elapsed = 0;
    let mut session_start = 0;
    for e in 0..epochs {
        if local[e].valid == 0 {
            session_start = e + 1;
            continue;
        }
        elapsed += 1;
        if uncertain[e] {
            scores[e][4] = 0.0;
            continue;
        }
        let s = short[e];
        let l = long[e];
        let progress = elapsed as f64 / observed as f64;
        let session_minutes = (e - session_start) as f64 * 0.5;
        let relative_ventilation = s.ventilation / baseline_ventilation;
        let large_breath = local[e].breaths.iter().any(|b| b.volume > s.volume * 2.0);
        wake_evidence[e] = large_breath && (s.interval_var > 0.15 || s.volume_var > 0.23);
        let near_end = e + 1 == epochs || local[e + 1].valid == 0;
        let boundary_irregular = near_end
            && local[e].breaths.iter().any(|b| b.volume > s.volume * 1.5)
            && (s.interval_var > 0.12 || s.volume_var > 0.20);
        let onset_prior = (1.0 - session_minutes / 12.0).max(0.0) * 8.0;
        scores[e][0] = -1.0
            + onset_prior
            + if wake_evidence[e] { 6.0 } else { 0.0 }
            + if boundary_irregular { 4.5 } else { 0.0 };
        scores[e][1] = 2.4;
        // Deep needs both a place in the night's most regular quartile and
        // absolute short-window regularity. Long context guards against an
        // isolated quiet epoch within otherwise irregular breathing.
        deep_evidence[e] = regular_quartile[e]
            && s.interval_var < 0.07
            && s.volume_var < 0.08
            && [s, l].iter().all(|c| {
                c.interval_var < 0.12
                    && c.volume_var < 0.16
                    && c.ventilation_var < 0.20
                    && c.duty_var < 0.18
                    && (0.2..=0.7).contains(&c.duty)
                    && (6.0..=28.0).contains(&c.rate)
            })
            && (0.55..=1.6).contains(&relative_ventilation);
        if deep_evidence[e] {
            scores[e][2] = 2.65 + 0.5 * (1.0 - progress);
        }
        // The short context limits spread to neighboring epochs. Long context
        // and duty cycle contribute scores but cannot fill an entire REM window.
        rem_evidence[e] = s.interval_var > 0.07
            && s.volume_var > 0.12
            && !wake_evidence[e]
            && s.interval_var < 0.55
            && s.volume_var < 0.75;
        if rem_evidence[e] {
            let lower_ventilation = ((1.05 - relative_ventilation) * 2.0).clamp(-0.8, 0.8);
            let long_irregular = ((l.interval_var + l.volume_var) * 0.6).min(0.45);
            let duty_change = ((s.duty - l.duty).abs() * 2.0 + s.duty_var).min(0.35);
            scores[e][3] = 2.0 + 0.65 * progress + lower_ventilation + long_irregular + duty_change;
        }
    }
    // Fixed heuristic transition costs are soft priors, not learned clinical
    // probabilities. Missing epochs reset the decoder between mask sessions.
    let states = [
        SleepStage::Wake,
        SleepStage::Light,
        SleepStage::Deep,
        SleepStage::Rem,
        SleepStage::Uncertain,
    ];
    result.stages = vec![SleepStage::NoData; epochs];
    let mut begin = 0;
    while begin < epochs {
        if local[begin].valid == 0 {
            begin += 1;
            continue;
        }
        let mut end = begin + 1;
        while end < epochs && local[end].valid > 0 {
            end += 1;
        }
        let mut previous = scores[begin];
        let mut back = vec![[0_usize; 5]; end - begin];
        for e in begin + 1..end {
            let mut next = [f64::NEG_INFINITY; 5];
            for target in 0..5 {
                for (source, &prior) in previous.iter().enumerate() {
                    let penalty = if source == target {
                        0.0
                    } else if source == 4 || target == 4 {
                        0.3
                    } else if source == 1 || target == 1 {
                        0.8
                    } else {
                        1.5
                    };
                    let score = prior - penalty + scores[e][target];
                    if score > next[target] {
                        next[target] = score;
                        back[e - begin][target] = source;
                    }
                }
            }
            previous = next;
        }
        let mut state = (0..5)
            .max_by(|&a, &b| previous[a].total_cmp(&previous[b]))
            .unwrap_or(1);
        for e in (begin..end).rev() {
            result.stages[e] = states[state];
            state = back[e - begin][state];
        }
        begin = end;
    }
    let mut minutes = [0.0_f64; 5];
    let mut valid_total = 0_usize;
    let mut usable = 0.0;
    for (e, f) in local.iter().enumerate() {
        valid_total += f.valid;
        if !uncertain[e] {
            usable += f.valid as f64
                * (f.covered * hz as f64 / f.valid.max(1) as f64).min(1.0)
                * (1.0 - f.noise / (f.energy + f.noise).max(1.0));
        }
        if let Some(index) = states.iter().position(|&s| s == result.stages[e]) {
            minutes[index] += f.valid as f64 / (hz as f64 * 60.0);
        }
    }
    result.minutes = StageMinutes {
        wake: minutes[0] as f32,
        light: minutes[1] as f32,
        deep: minutes[2] as f32,
        rem: minutes[3] as f32,
        uncertain: minutes[4] as f32,
    };
    result.signal_quality = (usable / valid_total.max(1) as f64).clamp(0.0, 1.0) as f32;
    if let Some(d) = diagnostics {
        let values = |get: fn(Context) -> f64, contexts: &[Context]| {
            (0..epochs)
                .filter(|&e| local[e].valid > 0)
                .map(|e| get(contexts[e]))
                .collect()
        };
        d.features = vec![
            (
                "observed_short_interval_robust_cv",
                values(|c| c.interval_var, &short),
            ),
            (
                "observed_short_volume_robust_cv",
                values(|c| c.volume_var, &short),
            ),
            (
                "observed_short_ventilation_robust_cv",
                values(|c| c.ventilation_var, &short),
            ),
            (
                "observed_long_interval_robust_cv",
                values(|c| c.interval_var, &long),
            ),
            (
                "observed_long_volume_robust_cv",
                values(|c| c.volume_var, &long),
            ),
            (
                "observed_short_inspiratory_duty",
                values(|c| c.duty, &short),
            ),
            ("observed_long_inspiratory_duty", values(|c| c.duty, &long)),
            (
                "observed_short_relative_ventilation",
                (0..epochs)
                    .filter(|&e| local[e].valid > 0)
                    .map(|e| short[e].ventilation / baseline_ventilation)
                    .collect(),
            ),
            (
                "observed_largest_breath_ratio",
                values(|c| c.largest / c.volume.max(0.001), &short),
            ),
        ];
        let count = |flags: &[bool]| {
            flags
                .iter()
                .enumerate()
                .filter(|&(e, &v)| v && local[e].valid > 0)
                .count()
        };
        d.rules = vec![
            ("supported", count(&supported)),
            ("artifact_or_clipping_uncertain", count(&artifacts)),
            (
                "implausible_flow_artifact",
                local.iter().filter(|f| f.artifacts > 0).count(),
            ),
            (
                "sustained_exact_flatline_artifact",
                local.iter().filter(|f| f.clipped > 0).count(),
            ),
            (
                "failed_noisy_breath_detection_artifact",
                count(&failed_noisy_detection),
            ),
            (
                "supported_regularity_lowest_quartile",
                count(&regular_quartile),
            ),
            ("large_leak_uncertain", count(&leaks)),
            (
                "event_context_uncertain",
                local.iter().filter(|f| f.event && f.valid > 0).count(),
            ),
            ("periodic_breathing_uncertain", count(&periodic)),
            ("uncertain", count(&uncertain)),
            ("positive_irregular_wake", count(&wake_evidence)),
            ("deep_absolute_regularity", count(&deep_evidence)),
            ("rem_local_evidence", count(&rem_evidence)),
        ];
    }
    result
}
