//! Estimates 30-second sleep stages from CPAP airflow, leak and machine events.
//!
//! # What this can and cannot know
//!
//! Patient flow carries real information about sleep state, but much less than
//! EEG. This heuristic has public-PSG measurements but no paired AS100 validation.
//! Quiet wake can look like sleep, and successful therapy can remove the
//! breathing variability that made sleep detectable in the
//! first place. Every claim in the app has to survive that.
//!
//! The directions of the heuristic come from respiratory sleep literature:
//! timing and amplitude variability, inspiratory duty cycle, ventilation and
//! slow ventilation oscillation. Its original weights and caps remain hand-set;
//! the real-participant parameter search did not pass its final test. `docs/SLEEP_STAGE_EVIDENCE.md` lists what the literature
//! supports; `docs/SLEEP_STAGE_METHOD.md` describes this implementation.
//!
//! # How it works
//!
//! 1. **Measurement** (`signal`). Breaths are detected with an amplitude-tracking
//!    hysteresis on baseline-removed flow, then described by cycle length,
//!    inspiratory time, inspired volume, peak flow, duty cycle, mid-inspiratory
//!    squareness and a flat-top flag. Two windows pool them: 90 s for
//!    breath-to-breath detail and 300 s for slower structure.
//! 2. **Evidence** (`score_epochs`). Each stage accumulates independent evidence
//!    contributions rather than a chain of if-statements, so a weak signal adds
//!    little and a contradiction removes support. Wake needs irregular timing or
//!    volume, sighs, or arousal evidence after a machine event. Deep needs
//!    absolute regularity and a normal duty cycle, never rank alone. REM needs
//!    lower ventilation *and* irregularity, with a slow-oscillation term, and is
//!    penalised when the epoch also looks awake.
//! 3. **Sequence** (`decode`). A semi-Markoid Viterbi pass prices leaving each
//!    state early using an expected bout length. This is smoothing, not a sleep
//!    cycle: a long bout is never forced and no clock is imposed.
//! 4. **Uncertainty**. Confounders make an epoch *uncertain* instead of pushing
//!    it to Wake or REM: leak, artefact, flat line, and the recovery period after
//!    a machine event. Periodic breathing is flagged and reported rather than
//!    silently scored as REM. Arousal evidence, by contrast, only *adds* wake
//!    evidence, because arousals are genuinely wake-like physiology.
//!
//! Everything is deterministic, uses no network, and is priced for a night in
//! well under a second.

mod params;
mod signal;

pub use params::Params;
use serde::{Deserialize, Serialize};
use signal::{analyze, periodic_breathing, window, Breath, Epoch, Signal, Window};

/// Length of one scoring epoch, as in sleep studies.
pub const EPOCH_SECONDS: u32 = 30;
/// Bumped whenever the algorithm or its defaults change, so stored nights are
/// recomputed and old estimates are never silently mixed with new ones.
pub const ESTIMATOR_VERSION: u32 = 5;

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

/// A run of epochs the signal could not support, kept in the output so the UI
/// can explain *why* rather than showing a gap or a guess.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confounder {
    LargeLeak,
    Artifact,
    BreathingEvent,
    PeriodicBreathing,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UncertaintyRun {
    /// Index of the first epoch in the run.
    pub first_epoch: usize,
    /// Number of epochs covered.
    pub epochs: usize,
    pub reason: Confounder,
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
    /// Confounder runs behind the uncertain minutes, longest first.
    #[serde(default)]
    pub uncertain_runs: Vec<UncertaintyRun>,
    /// Periodic (crescendo-decrescendo) breathing was seen, so REM and Deep
    /// claims are withheld and leak or cardiac causes are worth checking.
    #[serde(default)]
    pub periodic_breathing: bool,
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

/// Aggregate diagnostics only, without dates or identifiers.
#[doc(hidden)]
pub fn sleep_stage_debug(input: &SleepStageInput<'_>) -> String {
    use std::fmt::Write;
    let mut diagnostics = Diagnostics::default();
    estimate(input, &Params::default(), Some(&mut diagnostics));
    let mut report = String::from(
        "feature populations: observed = epochs with finite flow; robust variability = IQR / (1.349 * median)\nfeature p10 p25 p50 p75 p90\n",
    );
    for (name, values) in diagnostics.features {
        let _ = write!(report, "{name}");
        for fraction in [0.1, 0.25, 0.5, 0.75, 0.9] {
            let _ = write!(report, " {:.4}", signal::percentile(&values, fraction, 0.0));
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
    estimate(input, &Params::default(), None)
}

/// As [`estimate_sleep_stages`] with explicit parameters. Used by the offline
/// evaluation tool; the app always uses the shipped defaults.
pub fn estimate_sleep_stages_with(
    input: &SleepStageInput<'_>,
    params: &Params,
) -> SleepStageEstimate {
    estimate(input, params, None)
}

/// Detected breaths for a trace, as `(epoch, duration, inspiratory, volume, duty)`
/// tuples. Development tooling only: it exposes the measurement layer so a
/// boundary bug can be seen directly instead of inferred from a summary.
#[doc(hidden)]
pub fn signal_probe(flow: &[f32], hz: u32) -> Vec<(usize, f64, f64, f64, f64)> {
    let per_epoch = hz as usize * EPOCH_SECONDS as usize;
    let analysed = signal::analyze(flow, hz as usize, &[], &[], per_epoch);
    analysed
        .epoch
        .iter()
        .enumerate()
        .flat_map(|(epoch, e)| {
            e.breaths
                .iter()
                .map(move |b| (epoch, b.duration, b.inspiratory, b.volume, b.duty))
        })
        .collect()
}

/// Per-epoch measurements for offline PSG diagnostics, not runtime confidence.
/// Arrays contain interval CV, volume CV, ventilation CV, duty CV, duty, rate,
/// and squareness in that order, using the same windows and exclusions as scoring.
#[doc(hidden)]
pub struct StageFeatureProbe {
    pub short: [f64; 7],
    pub long: [f64; 7],
    pub regular_quartile: bool,
    pub uncertain: bool,
    pub deep_evidence: f64,
}

#[doc(hidden)]
pub fn sleep_stage_epoch_features(input: &SleepStageInput<'_>) -> Vec<StageFeatureProbe> {
    let hz = input.hz as usize;
    if hz == 0 {
        return Vec::new();
    }
    let Some(per_epoch) = hz.checked_mul(EPOCH_SECONDS as usize) else {
        return Vec::new();
    };
    let signal = analyze(
        input.flow,
        hz,
        input.excess_leak,
        input.event_offsets,
        per_epoch,
    );
    let n = signal.epoch.len();
    let periodic = periodic_breathing(&signal.envelope, n);
    let evidence = score_epochs(&signal.epoch, &periodic, &Params::default(), per_epoch, hz);
    let usable: Vec<bool> = (0..n)
        .map(|e| !evidence.artifact[e] && !evidence.leak[e])
        .collect();
    let values = |w: Window| {
        [
            w.interval_cv,
            w.volume_cv,
            w.ventilation_cv,
            w.duty_cv,
            w.duty,
            w.rate,
            w.squareness,
        ]
    };
    (0..n)
        .map(|e| StageFeatureProbe {
            short: values(window(&signal.epoch, &usable, e, 1, 1)),
            long: values(window(&signal.epoch, &usable, e, 4, 5)),
            regular_quartile: evidence.regular_quartile[e],
            uncertain: evidence.uncertain[e],
            deep_evidence: evidence.deep[e],
        })
        .collect()
}

/// The shipped parameter set, for tooling that needs to perturb it.
pub fn default_params() -> Params {
    Params::default()
}

#[derive(Default)]
struct Diagnostics {
    features: Vec<(&'static str, Vec<f64>)>,
    rules: Vec<(&'static str, usize)>,
}

/// Per-epoch evidence, before the sequence model.
struct Evidence {
    scores: Vec<[f64; 5]>,
    uncertain: Vec<bool>,
    causes: Vec<Option<Confounder>>,
    wake: Vec<f64>,
    rem: Vec<f64>,
    deep: Vec<f64>,
    regular_quartile: Vec<bool>,
    periodic: Vec<bool>,
    artifact: Vec<bool>,
    leak: Vec<bool>,
}

fn estimate(
    input: &SleepStageInput<'_>,
    params: &Params,
    mut diagnostics: Option<&mut Diagnostics>,
) -> SleepStageEstimate {
    let mut result = SleepStageEstimate {
        start: input.start.into(),
        epoch_seconds: EPOCH_SECONDS,
        stages: Vec::new(),
        minutes: StageMinutes::default(),
        signal_quality: 0.0,
        confidence: StageConfidence::default(),
        estimator_version: ESTIMATOR_VERSION,
        uncertain_runs: Vec::new(),
        periodic_breathing: false,
    };
    if input.hz == 0 || input.flow.is_empty() {
        return result;
    }
    let hz = input.hz as usize;
    let Some(per_epoch) = hz.checked_mul(EPOCH_SECONDS as usize) else {
        return result;
    };
    let epochs = input.flow.len().div_ceil(per_epoch);
    let Signal { epoch, envelope } = analyze(
        input.flow,
        hz,
        input.excess_leak,
        input.event_offsets,
        per_epoch,
    );
    let periodic = periodic_breathing(&envelope, epochs);
    let evidence = score_epochs(&epoch, &periodic, params, per_epoch, hz);

    result.stages = decode(&evidence, &epoch, params);
    result.periodic_breathing = periodic
        .iter()
        .enumerate()
        .any(|(e, &p)| p && epoch[e].valid > 0);
    result.uncertain_runs = uncertain_runs(&evidence, &epoch);
    let (minutes, quality) = summarize(&epoch, &result.stages, hz);
    result.minutes = minutes;
    result.signal_quality = quality;

    if let Some(d) = diagnostics.as_mut() {
        record_diagnostics(
            d,
            input,
            &epoch,
            &evidence,
            &result.stages,
            hz,
            per_epoch,
            params,
        );
    }
    result
}

fn score_epochs(
    epochs: &[Epoch],
    periodic: &[bool],
    params: &Params,
    per_epoch: usize,
    hz: usize,
) -> Evidence {
    let n = epochs.len();
    let noisy: Vec<bool> = epochs
        .iter()
        .map(|f| {
            f.covered * hz as f64 / f.valid.max(1) as f64 <= params.usable_coverage
                && f.noise / (f.energy + f.noise).max(1.0) > params.noise_limit
        })
        .collect();
    let artifact: Vec<bool> = epochs
        .iter()
        .enumerate()
        .map(|(e, f)| f.artifacts > 0 || f.clipped > 0 || noisy[e])
        .collect();
    let leak: Vec<bool> = epochs
        .iter()
        .map(|f| f.large_leak > 0 && f.large_leak * 3 >= f.leak_samples.max(1))
        .collect();
    // Periodic breathing is itself informative, so it is not removed from the
    // feature windows. It is only kept from producing REM or Deep claims.
    let usable: Vec<bool> = (0..n).map(|e| !artifact[e] && !leak[e]).collect();

    let short: Vec<Window> = (0..n).map(|e| window(epochs, &usable, e, 1, 1)).collect();
    let long: Vec<Window> = (0..n).map(|e| window(epochs, &usable, e, 4, 5)).collect();

    let supported: Vec<bool> = (0..n)
        .map(|e| {
            epochs[e].valid as f64 >= per_epoch as f64 * params.usable_flow
                && epochs[e].covered * hz as f64 / epochs[e].valid.max(1) as f64
                    > params.usable_coverage
                && short[e].count >= params.usable_breaths
        })
        .collect();
    let uncertain: Vec<bool> = (0..n)
        .map(|e| !supported[e] || artifact[e] || leak[e] || epochs[e].event || periodic[e])
        .collect();
    let causes: Vec<Option<Confounder>> = (0..n)
        .map(|e| {
            if leak[e] {
                Some(Confounder::LargeLeak)
            } else if artifact[e] {
                Some(Confounder::Artifact)
            } else if periodic[e] {
                Some(Confounder::PeriodicBreathing)
            } else if epochs[e].event {
                Some(Confounder::BreathingEvent)
            } else {
                None
            }
        })
        .collect();

    let population: Vec<usize> = (0..n).filter(|&e| !uncertain[e]).collect();
    let baseline_ventilation = signal::median(
        &population
            .iter()
            .map(|&e| long[e].ventilation)
            .collect::<Vec<_>>(),
        6.0,
    )
    .max(0.1);

    // Deep's rank gate. Absolute regularity is necessary but not sufficient: a
    // night where every epoch is equally regular would then be labelled entirely
    // Deep, which is what an unvarying signal looks like rather than physiology.
    // What separates Deep from Light in real sleep is that the night contains a
    // stretch distinctly *quieter* than the rest, so the gate also requires real
    // spread in regularity. Within a night that does show contrast, the quietest
    // quarter is where Deep can occur; the absolute caps below then decide which
    // of those epochs actually qualify.
    let regularity: Vec<f64> = short
        .iter()
        .map(|c| (c.interval_cv + c.volume_cv + c.ventilation_cv + c.duty_cv) / 4.0)
        .collect();
    let mut order = population.clone();
    order.sort_by(|&a, &b| regularity[a].total_cmp(&regularity[b]).then(a.cmp(&b)));
    let mut regular_quartile = vec![false; n];
    if order.len() >= 8 {
        let sorted: Vec<f64> = order.iter().map(|&e| regularity[e]).collect();
        let low = signal::percentile(&sorted, 0.10, 0.0);
        let high = signal::percentile(&sorted, 0.90, 0.0);
        let contrast = if low > 1e-6 { high / low } else { 0.0 };
        if contrast >= params.deep_contrast {
            let quartile = order.len().div_ceil(4);
            for &e in order.iter().take(quartile) {
                regular_quartile[e] = true;
            }
        }
    }

    let mut scores = vec![[f64::NEG_INFINITY; 5]; n];
    let mut wake = vec![0.0; n];
    let mut rem = vec![0.0; n];
    let mut deep = vec![0.0; n];

    let observed = epochs.iter().filter(|f| f.valid > 0).count().max(1);
    let mut seen = 0usize;
    let mut session_start = 0usize;
    for e in 0..n {
        if epochs[e].valid == 0 {
            session_start = e + 1;
            continue;
        }
        seen += 1;
        if uncertain[e] {
            continue;
        }
        let s = short[e];
        let l = long[e];
        // Wake evidence: irregular timing, volume or duty cycle, fast breathing,
        // sighs, and machine-event arousals.
        //
        // The irregularity term takes the *strongest* of the three cues rather
        // than their mean. Inspiration timing and duty cycle are remarkably
        // stable even in light sleep, so averaging them in dilutes a genuinely
        // irregular epoch down to a third of its evidence and no epoch ever
        // crosses the threshold. Wakefulness is signalled by irregularity in
        // timing *or* amplitude (Ayappa and others found visual irregularity a
        // strong wake marker), so the maximum is the physiologically right
        // combination.
        let irregular = Params::band(s.interval_cv, params.wake_interval_cv)
            .max(Params::band(s.volume_cv, params.wake_volume_cv))
            .max(Params::band(s.duty_cv, params.wake_duty_cv));
        let fast = Params::band(s.rate, params.wake_rate);
        let sighing = (s.sighs as f64 / 3.0).clamp(0.0, 1.0);
        let boundary = if is_edge(epochs, e) && s.largest > s.volume * 1.5 {
            params.wake_boundary
        } else {
            0.0
        };
        let session_minutes = (e - session_start) as f64 * 0.5;
        // Mask-on almost always starts awake: a person has just donned the mask
        // and is settling. This is added as evidence, not imposed as a rule, so a
        // genuinely asleep-looking onset can still be overridden by the decoder.
        let onset = if session_minutes < params.onset_minutes {
            params.onset_weight * (1.0 - session_minutes / params.onset_minutes)
        } else {
            0.0
        };
        // Wake breathing is typically larger and faster than sleep, so evidence for
        // wakefulness is discounted when ventilation is clearly *below* the night's
        // baseline. Without this, the low-ventilation irregularity of REM is
        // misread as wakefulness, which is the most consequential confusion the
        // literature warns about: REM and wake share irregularity, and only
        // ventilation separates them. At or above baseline the gate is 1; it falls
        // to 0 as measured ventilation drops to 20% below baseline.
        let deficit = baseline_ventilation / s.ventilation.max(0.1);
        let wake_ventilation_gate = 1.0
            - Params::band(
                deficit,
                (
                    params.wake_ventilation_discount.0,
                    params.wake_ventilation_discount.1,
                ),
            );
        wake[e] = (params.wake_irregular * irregular
            + params.wake_fast * fast
            // A sigh only supports wake alongside some irregularity, so a single
            // deep breath in steady sleep cannot produce a wake claim.
            + params.wake_sigh * sighing * irregular
            + params.wake_arousal * epochs[e].arousal
            + boundary
            + onset)
            * wake_ventilation_gate;

        // Deep needs absolute regularity, a normal duty cycle and rate, quiet
        // surroundings in the long window, a non-flow-limited contour, *and* a
        // place among the night's most regular epochs. The rank term is required,
        // not just additive: without it, a night whose every epoch is equally
        // regular would be entirely Deep, which is not physiology and would make
        // Deep minutes meaningless.
        let deep_ok = regular_quartile[e]
            && s.interval_cv < params.deep_interval_cap
            && s.volume_cv < params.deep_volume_cap
            && s.duty_cv < params.deep_long_duty_cap
            && s.squareness <= params.deep_squareness
            && [s, l].iter().all(|c| {
                c.interval_cv < params.deep_long_interval_cap
                    && c.volume_cv < params.deep_long_volume_cap
                    && c.ventilation_cv < params.deep_long_ventilation_cap
                    && c.duty_cv < params.deep_long_duty_cap
                    && (params.deep_duty.0..=params.deep_duty.1).contains(&c.duty)
                    && (params.deep_rate.0..=params.deep_rate.1).contains(&c.rate)
            });
        if deep_ok {
            // How far inside every cap this epoch sits, in 0..1.
            let calm = 1.0
                - (s.interval_cv / params.deep_interval_cap)
                    .max(s.volume_cv / params.deep_volume_cap)
                    .max(l.interval_cv / params.deep_long_interval_cap)
                    .max(l.volume_cv / params.deep_long_volume_cap)
                    .clamp(0.0, 1.0);
            let roundness = ((params.deep_squareness - s.squareness) / 0.15).clamp(0.0, 1.0);
            deep[e] = params.deep_regularity * calm
                + params.deep_round_breath * roundness
                + params.deep_rank_prior;
        }

        // REM requires lower relative ventilation *and* irregular breathing. The
        // ventilation requirement is what distinguishes REM from wake: both are
        // irregular, but REM breathing is smaller and slower while wakefulness is
        // larger and faster. Keeping hypoventilation as a *gate* rather than one
        // weighted term among several is what stops the two being confused, and it
        // is why an irregular stretch without reduced ventilation stays Light
        // rather than being claimed as REM.
        //
        // Wake evidence subtracts, and flow-limited variability subtracts: recovery
        // breaths and obstructive cycles resemble REM but are not REM.
        let relative = s.ventilation / baseline_ventilation;
        let hypoventilation = Params::band(relative, params.rem_ventilation);
        let irregularity = (Params::band(s.interval_cv, params.rem_interval_cv)
            + Params::band(s.volume_cv, params.rem_volume_cv))
            / 2.0;
        let effort_spread = Params::ramp(s.peak_cv, params.rem_peak_cv.0, params.rem_peak_cv.1);
        let slow = Params::ramp(l.slow_ventilation_cv, 0.0, params.rem_slow_full);
        let rem_shaped = hypoventilation > 0.0
            && s.interval_cv < params.rem_interval_cap
            && s.volume_cv < params.rem_volume_cap
            && (s.interval_cv > params.rem_interval_cv.0 * 0.8
                || s.volume_cv > params.rem_volume_cv.0 * 0.8);
        if rem_shaped {
            let sustained = if l.interval_cv > params.rem_sustained_cv {
                params.rem_sustained
            } else {
                0.0
            };
            let limited = epochs[e]
                .breaths
                .iter()
                .map(Breath::limitation)
                .fold(0.0_f64, f64::max);
            rem[e] = (params.rem_hypoventilation * hypoventilation
                + params.rem_irregular * irregularity
                + params.rem_effort_spread * effort_spread
                + params.rem_slow * slow
                + sustained
                + params.rem_progress * (seen as f64 / observed as f64)
                - params.rem_wake_conflict * wake[e]
                - params.rem_flow_limitation * limited)
                .max(0.0);
        }

        // Light is the reference and scores zero, so weak evidence for any other
        // stage is actively negative once converted. That is what prevents a
        // whole night being labelled Wake, Deep or REM merely because those
        // quantities are non-negative.
        scores[e][0] = Params::score(wake[e], params.wake_threshold) * params.emission_scale;
        scores[e][1] = 0.0;
        scores[e][2] = Params::score(deep[e], params.deep_threshold) * params.emission_scale;
        scores[e][3] = Params::score(rem[e], params.rem_threshold) * params.emission_scale;
        if !scores[e][0].is_finite() {
            scores[e][0] = 0.0;
        }
        if !scores[e][2].is_finite() {
            scores[e][2] = 0.0;
        }
        if !scores[e][3].is_finite() {
            scores[e][3] = 0.0;
        }
    }

    Evidence {
        scores,
        uncertain,
        causes,
        wake,
        rem,
        deep,
        regular_quartile,
        periodic: periodic.to_vec(),
        artifact,
        leak,
    }
}

fn is_edge(epochs: &[Epoch], e: usize) -> bool {
    e + 1 >= epochs.len() || epochs[e + 1].valid == 0
}

/// Viterbi decode with run-length transition costs, per mask-on session.
///
/// `Uncertain` is not voted on: an epoch is uncertain exactly when the
/// confidence model says its signal is confounded, and those epochs also break
/// the chain so a guess is never smoothed across a stretch that could not be
/// interpreted. That keeps the uncertainty honest — the decoder cannot invent
/// it, and it cannot quietly overwrite it either.
fn decode(evidence: &Evidence, epochs: &[Epoch], params: &Params) -> Vec<SleepStage> {
    const STATES: [SleepStage; 5] = [
        SleepStage::Wake,
        SleepStage::Light,
        SleepStage::Deep,
        SleepStage::Rem,
        SleepStage::Uncertain,
    ];
    let n = epochs.len();
    let mut stages = vec![SleepStage::NoData; n];
    // Every contiguous stretch that is observed and not confounded is decoded
    // as one chain.
    let mut epoch = 0;
    while epoch < n {
        if epochs[epoch].valid == 0 || evidence.uncertain[epoch] {
            if epochs[epoch].valid > 0 {
                stages[epoch] = STATES[4];
            }
            epoch += 1;
            continue;
        }
        let begin = epoch;
        let mut end = begin + 1;
        while end < n && epochs[end].valid > 0 && !evidence.uncertain[end] {
            end += 1;
        }
        decode_chain(
            &evidence.scores[begin..end],
            &STATES[..4],
            params,
            &mut stages[begin..end],
        );
        epoch = end;
    }
    stages
}

fn decode_chain(
    scores: &[[f64; 5]],
    states: &[SleepStage],
    params: &Params,
    out: &mut [SleepStage],
) {
    let n = scores.len();
    let k = states.len();
    if n == 0 {
        return;
    }
    // `cost[s]` is the best score of a path ending in state `s`; `run[s]` is how
    // many consecutive epochs that path has spent there. The departure cost is
    // charged once, at the transition, and shrinks as the run grows past the
    // state's expected length. Staying put is free.
    let mut cost = [f64::NEG_INFINITY; 5];
    let mut run = [1usize; 5];
    let mut back = vec![[0u8; 5]; n];
    for s in 0..k {
        cost[s] = scores[0][s] - params.enter_cost[s];
    }
    for e in 1..n {
        let mut next = [f64::NEG_INFINITY; 5];
        let mut next_run = [1usize; 5];
        for target in 0..k {
            let mut best = f64::NEG_INFINITY;
            let mut best_from = target as u8;
            let mut best_run = 1;
            for source in 0..k {
                let (value, length) = if source == target {
                    (cost[source], run[source] + 1)
                } else {
                    // Leaving early is expensive; past the expected bout length
                    // it costs only the plain switch penalty.
                    let early = 1.0 - (run[source] as f64 / params.expected_bout[source]).min(1.0);
                    (
                        cost[source]
                            - params.leave_cost[source] * early
                            - params.enter_cost[target],
                        1,
                    )
                };
                let value = value + scores[e][target];
                if value > best {
                    best = value;
                    best_from = source as u8;
                    best_run = length;
                }
            }
            next[target] = best;
            next_run[target] = best_run;
            back[e][target] = best_from;
        }
        cost = next;
        run = next_run;
    }
    let mut state = (0..k)
        .max_by(|&a, &b| cost[a].total_cmp(&cost[b]))
        .unwrap_or(1);
    for e in (0..n).rev() {
        out[e] = states[state];
        if e > 0 {
            state = back[e][state] as usize;
        }
    }
}

fn summarize(epochs: &[Epoch], stages: &[SleepStage], hz: usize) -> (StageMinutes, f32) {
    let mut minutes = [0.0_f64; 5];
    let mut valid_total = 0usize;
    let mut usable = 0.0;
    for (e, f) in epochs.iter().enumerate() {
        valid_total += f.valid;
        let coverage = f.covered * hz as f64 / f.valid.max(1) as f64;
        let confounded = matches!(stages[e], SleepStage::Uncertain);
        if !confounded {
            usable += f.valid as f64
                * coverage.min(1.0)
                * (1.0 - f.noise / (f.energy + f.noise).max(1.0));
        }
        let index = match stages[e] {
            SleepStage::Wake => 0,
            SleepStage::Light => 1,
            SleepStage::Deep => 2,
            SleepStage::Rem => 3,
            SleepStage::Uncertain => 4,
            SleepStage::NoData => continue,
        };
        minutes[index] += f.valid as f64 / (hz as f64 * 60.0);
    }
    (
        StageMinutes {
            wake: minutes[0] as f32,
            light: minutes[1] as f32,
            deep: minutes[2] as f32,
            rem: minutes[3] as f32,
            uncertain: minutes[4] as f32,
        },
        (usable / valid_total.max(1) as f64).clamp(0.0, 1.0) as f32,
    )
}

fn uncertain_runs(evidence: &Evidence, epochs: &[Epoch]) -> Vec<UncertaintyRun> {
    let mut runs: Vec<UncertaintyRun> = Vec::new();
    let mut open: Option<(usize, Confounder)> = None;
    for e in 0..epochs.len() {
        let cause = evidence
            .uncertain
            .get(e)
            .copied()
            .unwrap_or(false)
            .then(|| evidence.causes[e])
            .flatten();
        match (open, cause) {
            (Some((first, reason)), Some(c)) if c == reason => {
                open = Some((first, reason));
            }
            (Some((first, reason)), _) => {
                runs.push(UncertaintyRun {
                    first_epoch: first,
                    epochs: e - first,
                    reason,
                });
                open = cause.map(|c| (e, c));
            }
            (None, Some(c)) => open = Some((e, c)),
            (None, None) => {}
        }
    }
    if let Some((first, reason)) = open {
        runs.push(UncertaintyRun {
            first_epoch: first,
            epochs: epochs.len() - first,
            reason,
        });
    }
    runs.retain(|run| run.epochs > 0);
    runs.sort_by(|a, b| {
        b.epochs
            .cmp(&a.epochs)
            .then(a.first_epoch.cmp(&b.first_epoch))
    });
    runs
}

#[allow(clippy::too_many_arguments)]
fn record_diagnostics(
    d: &mut Diagnostics,
    input: &SleepStageInput<'_>,
    epochs: &[Epoch],
    evidence: &Evidence,
    stages: &[SleepStage],
    hz: usize,
    per_epoch: usize,
    params: &Params,
) {
    let n = epochs.len();
    let usable: Vec<bool> = (0..n)
        .map(|e| !evidence.artifact[e] && !evidence.leak[e])
        .collect();
    let short: Vec<Window> = (0..n).map(|e| window(epochs, &usable, e, 1, 1)).collect();
    let long: Vec<Window> = (0..n).map(|e| window(epochs, &usable, e, 4, 5)).collect();
    let observed: Vec<usize> = (0..n).filter(|&e| epochs[e].valid > 0).collect();
    let values = |get: fn(Window) -> f64| observed.iter().map(|&e| get(short[e])).collect();
    let long_values =
        |get: fn(Window) -> f64| -> Vec<f64> { observed.iter().map(|&e| get(long[e])).collect() };
    let long_volume = long_values(|c| c.volume_cv);
    let long_interval = long_values(|c| c.interval_cv);
    let long_ventilation = long_values(|c| c.ventilation_cv);
    let baseline = signal::median(
        &observed
            .iter()
            .map(|&e| long[e].ventilation)
            .collect::<Vec<_>>(),
        6.0,
    )
    .max(0.1);
    d.features = vec![
        (
            "observed_short_interval_robust_cv",
            values(|c| c.interval_cv),
        ),
        ("observed_short_volume_robust_cv", values(|c| c.volume_cv)),
        (
            "observed_short_ventilation_robust_cv",
            values(|c| c.ventilation_cv),
        ),
        ("observed_short_duty_robust_cv", values(|c| c.duty_cv)),
        ("observed_short_inspiratory_duty", values(|c| c.duty)),
        ("observed_short_squareness", values(|c| c.squareness)),
        (
            "observed_short_relative_ventilation",
            observed
                .iter()
                .map(|&e| short[e].ventilation / baseline)
                .collect(),
        ),
        (
            "observed_long_slow_ventilation_cv",
            long_values(|c| c.slow_ventilation_cv),
        ),
        ("observed_long_volume_robust_cv", long_volume),
        ("observed_long_interval_robust_cv", long_interval),
        ("observed_long_ventilation_robust_cv", long_ventilation),
        (
            "observed_largest_breath_ratio",
            values(|c| c.largest / c.volume.max(0.001)),
        ),
    ];
    let count = |flags: &[bool]| {
        flags
            .iter()
            .enumerate()
            .filter(|&(e, &v)| v && epochs[e].valid > 0)
            .count()
    };
    let count_where = |flag: &dyn Fn(usize) -> bool| observed.iter().filter(|&&e| flag(e)).count();
    let decoded = |stage: SleepStage| observed.iter().filter(|&&e| stages[e] == stage).count();
    let quartile_epochs: Vec<usize> = observed
        .iter()
        .copied()
        .filter(|&e| evidence.regular_quartile[e])
        .collect();
    // Which individual Deep condition fails, counted over the regular quartile.
    // This is diagnosis only; it does not affect the estimate.
    let fails =
        |test: &dyn Fn(usize) -> bool| quartile_epochs.iter().filter(|&&e| !test(e)).count();
    d.rules = vec![
        (
            "supported",
            observed.iter().filter(|&&e| !evidence.uncertain[e]).count(),
        ),
        ("artifact_or_clipping_uncertain", count(&evidence.artifact)),
        ("large_leak_uncertain", count(&evidence.leak)),
        (
            "event_context_uncertain",
            count_where(&|e: usize| epochs[e].event),
        ),
        ("periodic_breathing_uncertain", count(&evidence.periodic)),
        ("uncertain", count(&evidence.uncertain)),
        (
            "regularity_lowest_quartile",
            count(&evidence.regular_quartile),
        ),
        (
            "wake_evidence",
            count_where(&|e: usize| evidence.wake[e] > params.wake_threshold * 0.6),
        ),
        (
            "wake_evidence_max_hundredths",
            (evidence.wake.iter().cloned().fold(0.0_f64, f64::max) * 100.0) as usize,
        ),
        (
            "rem_evidence",
            count_where(&|e: usize| evidence.rem[e] > params.rem_threshold * 0.6),
        ),
        (
            "rem_evidence_max_hundredths",
            (evidence.rem.iter().cloned().fold(0.0_f64, f64::max) * 100.0) as usize,
        ),
        (
            "deep_evidence",
            count_where(&|e: usize| evidence.deep[e] > params.deep_threshold * 0.6),
        ),
        (
            "deep_evidence_max_hundredths",
            (evidence.deep.iter().cloned().fold(0.0_f64, f64::max) * 100.0) as usize,
        ),
        ("decoded_wake", decoded(SleepStage::Wake)),
        ("decoded_rem", decoded(SleepStage::Rem)),
        ("decoded_deep", decoded(SleepStage::Deep)),
        (
            "deep_gate_fail_short_interval",
            fails(&|e| short[e].interval_cv < params.deep_interval_cap),
        ),
        (
            "deep_gate_fail_short_volume",
            fails(&|e| short[e].volume_cv < params.deep_volume_cap),
        ),
        (
            "deep_gate_fail_short_duty",
            fails(&|e| short[e].duty_cv < params.deep_long_duty_cap),
        ),
        (
            "deep_gate_fail_squareness",
            fails(&|e| short[e].squareness <= params.deep_squareness),
        ),
        (
            "deep_gate_fail_long_interval",
            fails(&|e| long[e].interval_cv < params.deep_long_interval_cap),
        ),
        (
            "deep_gate_fail_long_volume",
            fails(&|e| long[e].volume_cv < params.deep_long_volume_cap),
        ),
        (
            "deep_gate_fail_long_ventilation",
            fails(&|e| long[e].ventilation_cv < params.deep_long_ventilation_cap),
        ),
        (
            "deep_gate_fail_long_duty_cv",
            fails(&|e| long[e].duty_cv < params.deep_long_duty_cap),
        ),
        (
            "deep_gate_fail_duty_range",
            fails(&|e| {
                (params.deep_duty.0..=params.deep_duty.1).contains(&short[e].duty)
                    && (params.deep_duty.0..=params.deep_duty.1).contains(&long[e].duty)
            }),
        ),
        (
            "deep_gate_fail_rate_range",
            fails(&|e| {
                (params.deep_rate.0..=params.deep_rate.1).contains(&short[e].rate)
                    && (params.deep_rate.0..=params.deep_rate.1).contains(&long[e].rate)
            }),
        ),
    ];
    let _ = (hz, per_epoch, input);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_parameters_are_coherent() {
        Params::default().validate().unwrap();
    }

    #[test]
    fn deep_contour_gate_rejects_flattening_instead_of_rounded_breaths() {
        let mut epochs: Vec<Epoch> = (0..60)
            .map(|e| {
                let spread = if e < 15 { 0.01 } else { 0.25 };
                Epoch {
                    valid: 1500,
                    covered: 30.0,
                    breaths: (0..10)
                        .map(|i| {
                            let offset = (i as f64 - 4.5) * spread;
                            Breath {
                                duration: 3.0 + offset,
                                inspiratory: 1.2,
                                volume: 0.5 + offset * 0.15,
                                peak: 12.0,
                                duty: 0.4,
                                squareness: 0.8,
                                flat_top: false,
                            }
                        })
                        .collect(),
                    ..Epoch::default()
                }
            })
            .collect();
        let periodic = vec![false; epochs.len()];
        let rounded = score_epochs(&epochs, &periodic, &Params::default(), 1500, 50);
        assert!(rounded.deep.iter().any(|&evidence| evidence > 0.0));
        for epoch in &mut epochs {
            for breath in &mut epoch.breaths {
                breath.squareness = 1.0;
                breath.flat_top = true;
            }
        }
        let flattened = score_epochs(&epochs, &periodic, &Params::default(), 1500, 50);
        assert!(flattened.deep.iter().all(|&evidence| evidence == 0.0));
    }

    #[test]
    fn current_algorithm_version_is_five() {
        assert_eq!(ESTIMATOR_VERSION, 5);
    }

    #[test]
    fn ramp_handles_degenerate_bounds() {
        assert_eq!(Params::ramp(0.5, 1.0, 1.0), 0.0);
        assert_eq!(Params::ramp(1.0, 1.0, 1.0), 1.0);
        assert_eq!(Params::ramp(0.0, 0.0, 1.0), 0.0);
        assert_eq!(Params::ramp(0.5, 0.0, 1.0), 0.5);
        assert_eq!(Params::ramp(2.0, 0.0, 1.0), 1.0);
    }
}
