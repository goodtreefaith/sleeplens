use sleeplens_core::sleep_stages::{
    estimate_sleep_stages, sleep_stage_debug, SleepStage, SleepStageEstimate, SleepStageInput,
    ESTIMATOR_VERSION,
};
use std::{f32::consts::TAU, time::Instant};

const HZ: u32 = 50;

fn random(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*seed >> 8) as f32 / 16_777_215.0 * 2.0 - 1.0
}

// Each breath has its own period and amplitude. Changes happen at zero flow,
// so irregular breathing does not accidentally introduce discontinuities.
fn breathing(minutes: usize, profile: impl Fn(usize) -> (f32, f32, f32, f32, f32)) -> Vec<f32> {
    let samples = minutes * 60 * HZ as usize;
    let mut flow = Vec::with_capacity(samples);
    let mut seed = 7;
    while flow.len() < samples {
        let minute = flow.len() / (60 * HZ as usize);
        let (period, interval_jitter, amplitude, volume_jitter, noise) = profile(minute);
        let length = ((period * (1.0 + interval_jitter * random(&mut seed))) * HZ as f32) as usize;
        let amplitude = amplitude * (1.0 + volume_jitter * random(&mut seed));
        for i in 0..length {
            if flow.len() == samples {
                break;
            }
            let value =
                amplitude * (TAU * i as f32 / length as f32).sin() + noise * random(&mut seed);
            flow.push(value);
        }
    }
    flow
}

fn estimate(flow: &[f32], leak: &[f32]) -> SleepStageEstimate {
    estimate_with_events(flow, leak, &[])
}

fn estimate_with_events(flow: &[f32], leak: &[f32], events: &[u32]) -> SleepStageEstimate {
    estimate_sleep_stages(&SleepStageInput {
        start: "2030-01-15T22:00:00",
        flow,
        hz: HZ,
        excess_leak: leak,
        event_offsets: events,
    })
}

fn fraction(result: &SleepStageEstimate, from: usize, to: usize, stage: SleepStage) -> f32 {
    let epochs = &result.stages[from * 2..to * 2];
    epochs.iter().filter(|&&value| value == stage).count() as f32 / epochs.len() as f32
}

fn total_minutes(result: &SleepStageEstimate) -> f32 {
    result.minutes.wake
        + result.minutes.light
        + result.minutes.deep
        + result.minutes.rem
        + result.minutes.uncertain
}

#[test]
fn leak_and_flow_artifacts_are_uncertain_instead_of_wake() {
    let mut flow = breathing(90, |_| (4.0, 0.06, 22.0, 0.08, 0.0));
    let mut leak = vec![0.0; 90 * 60];
    leak[20 * 60..30 * 60].fill(80.0);
    // A large, rapid alternating signal cannot be interpreted as breaths.
    for (i, sample) in flow[50 * 60 * HZ as usize..60 * 60 * HZ as usize]
        .iter_mut()
        .enumerate()
    {
        *sample = if i % 2 == 0 { 160.0 } else { -160.0 };
    }
    let result = estimate(&flow, &leak);
    for (from, to) in [(21, 29), (51, 59)] {
        assert_eq!(fraction(&result, from, to, SleepStage::Uncertain), 1.0);
        assert_eq!(fraction(&result, from, to, SleepStage::Wake), 0.0);
    }
    assert!(result.minutes.uncertain >= 16.0);
    assert!((total_minutes(&result) - 90.0).abs() < 0.01);
}

#[test]
fn sustained_zero_and_nonzero_flat_lines_are_uncertain() {
    for level in [0.0, 22.0] {
        let mut flow = breathing(50, |_| (4.0, 0.05, 22.0, 0.05, 0.0));
        flow[20 * 60 * HZ as usize..30 * 60 * HZ as usize].fill(level);
        let result = estimate(&flow, &[]);
        assert_eq!(fraction(&result, 21, 29, SleepStage::Uncertain), 1.0);
        assert!(debug_rule(&debug_report(&flow), "artifact_or_clipping_uncertain") >= 18);
    }
}

fn debug_report(flow: &[f32]) -> String {
    sleep_stage_debug(&SleepStageInput {
        start: "2030-01-15T22:00:00",
        flow,
        hz: HZ,
        excess_leak: &[],
        event_offsets: &[],
    })
}

fn debug_rule(report: &str, rule: &str) -> usize {
    let prefix = format!("rule {rule}: ");
    report
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("missing {rule}: {report}"))
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn integer_quantized_flow_keeps_breath_detection_and_signal_quality() {
    // AS100 flow is stored in whole L/min. Slow, shallow breaths have long
    // repeated peak samples, while still carrying a complete breathing signal.
    for amplitude in [6.0, 22.0] {
        let mut flow = breathing(60, |_| (5.0, 0.07, amplitude, 0.09, 0.0));
        for sample in &mut flow {
            *sample = sample.round();
        }
        assert!(flow.windows(12).any(|samples| {
            samples[0].abs() >= amplitude - 1.0 && samples.iter().all(|&value| value == samples[0])
        }));
        let result = estimate(&flow, &[]);
        let report = debug_report(&flow);
        assert_eq!(debug_rule(&report, "artifact_or_clipping_uncertain"), 0);
        assert!(debug_rule(&report, "supported") >= 118);
        assert!(result.minutes.uncertain <= 1.0, "{:?}", result.minutes);
        assert!(result.signal_quality > 0.9, "{}", result.signal_quality);
    }
}

#[test]
fn rapid_noise_without_detectable_breaths_is_an_artifact() {
    let mut flow = breathing(40, |_| (4.0, 0.05, 22.0, 0.05, 0.0));
    for (index, sample) in flow[15 * 60 * HZ as usize..25 * 60 * HZ as usize]
        .iter_mut()
        .enumerate()
    {
        *sample = if index % 2 == 0 { 20.0 } else { -20.0 };
    }
    let result = estimate(&flow, &[]);
    assert_eq!(fraction(&result, 16, 24, SleepStage::Uncertain), 1.0);
    assert!(debug_rule(&debug_report(&flow), "artifact_or_clipping_uncertain") >= 18);
}

#[test]
fn ordinary_variability_defaults_to_light_and_limits_deep_to_regular_epochs() {
    let flow = breathing(120, |minute| {
        if (20..40).contains(&minute) {
            (4.0, 0.015, 22.0, 0.02, 0.0)
        } else {
            (4.0, 0.15, 22.0, 0.12, 0.0)
        }
    });
    let result = estimate(&flow, &[]);
    assert!(fraction(&result, 22, 38, SleepStage::Deep) > 0.5);
    assert!(
        fraction(&result, 50, 110, SleepStage::Light) > 0.8,
        "Light fraction {}, {:?}",
        fraction(&result, 50, 110, SleepStage::Light),
        result.minutes
    );
    assert!(result.minutes.deep <= 30.0, "{:?}", result.minutes);
    assert!(result.minutes.light > 60.0, "{:?}", result.minutes);
}

#[test]
fn equally_regular_epochs_do_not_expand_deep_beyond_the_lowest_quartile() {
    let flow = breathing(120, |_| (4.0, 0.0, 22.0, 0.0, 0.0));
    let result = estimate(&flow, &[]);
    assert!(result.minutes.deep <= 30.0, "{:?}", result.minutes);
    assert!(result.minutes.light > 60.0, "{:?}", result.minutes);
}

#[test]
fn event_end_context_and_recovery_are_uncertain_not_rem() {
    let flow = breathing(180, |minute| {
        if (130..160).contains(&minute) {
            (4.5, 0.27, 14.0, 0.30, 0.0)
        } else {
            (4.0, 0.07, 22.0, 0.08, 0.0)
        }
    });
    let event_end = 145 * 60 + 15;
    let result = estimate_with_events(&flow, &[], &[u32::MAX, event_end + 300, event_end]);
    // The confounded span is the event epoch, the one before it (events are
    // reported at their end) and the three after it, where recovery breaths sit.
    // Those are exactly the epochs most likely to be mistaken for REM.
    let first = event_end / 30 - 1;
    let last = event_end / 30 + 3;
    for epoch in first..=last {
        assert_eq!(
            result.stages[epoch as usize],
            SleepStage::Uncertain,
            "epoch {epoch} should be uncertain"
        );
    }
    // Recovery breaths must never be claimed as REM, which is the confusion the
    // evidence review calls out for exactly this situation.
    assert!(result.stages[first as usize..=last as usize]
        .iter()
        .all(|s| *s != SleepStage::Rem));
    assert!(fraction(&result, 100, 120, SleepStage::Uncertain) < 0.1);
}

fn envelope_breathing(period: f32) -> Vec<f32> {
    let mut flow = breathing(40, |_| (4.0, 0.0, 22.0, 0.0, 0.0));
    for (i, sample) in flow.iter_mut().enumerate() {
        let second = i as f32 / HZ as f32;
        if (10.0 * 60.0..30.0 * 60.0).contains(&second) {
            *sample *= 0.62 + 0.55 * (TAU * (second - 600.0) / period).sin();
        }
    }
    flow
}

#[test]
fn repeated_crescendo_decrescendo_breathing_is_uncertain() {
    for period in [40.0, 60.0, 90.0] {
        let flow = envelope_breathing(period);
        let result = estimate(&flow, &[]);
        assert!(
            fraction(&result, 14, 26, SleepStage::Uncertain) > 0.8,
            "period {period}: {:?}",
            result.minutes
        );
        assert!(fraction(&result, 3, 7, SleepStage::Uncertain) < 0.1);
    }
}

#[test]
fn ordinary_breathing_and_slow_amplitude_drift_are_not_periodic_confounds() {
    let flow = envelope_breathing(240.0);
    let result = estimate(&flow, &[]);
    assert!(fraction(&result, 14, 26, SleepStage::Uncertain) < 0.2);
}

#[test]
fn one_sigh_and_nonperiodic_irregular_breathing_are_not_periodic_confounds() {
    let mut flow = breathing(40, |minute| {
        if (10..30).contains(&minute) {
            (4.0, 0.28, 22.0, 0.32, 0.0)
        } else {
            (4.0, 0.04, 22.0, 0.04, 0.0)
        }
    });
    // One large smooth breath does not establish several repeating cycles.
    for sample in &mut flow[5 * 60 * HZ as usize..(5 * 60 + 4) * HZ as usize] {
        *sample *= 2.5;
    }
    let result = estimate(&flow, &[]);
    assert!(fraction(&result, 3, 8, SleepStage::Uncertain) < 0.1);
    assert!(fraction(&result, 14, 26, SleepStage::Uncertain) < 0.2);
}

#[test]
fn regular_early_breathing_supports_some_deep() {
    let flow = breathing(120, |minute| {
        if (20..60).contains(&minute) {
            (5.0, 0.01, 22.0, 0.01, 0.0)
        } else {
            (4.0, 0.12, 22.0, 0.14, 0.0)
        }
    });
    let result = estimate(&flow, &[]);
    assert!(fraction(&result, 25, 55, SleepStage::Deep) > 0.25);
    assert_eq!(fraction(&result, 25, 55, SleepStage::Uncertain), 0.0);
}

#[test]
fn uniformly_irregular_breathing_does_not_become_deep_by_relative_rank() {
    let flow = breathing(100, |_| (4.0, 0.55, 22.0, 0.65, 0.0));
    let result = estimate(&flow, &[]);
    assert!(result.minutes.deep < 2.0, "{:?}", result.minutes);
}

#[test]
fn irregular_large_breaths_support_wake_without_leak_or_artifact() {
    let flow = breathing(75, |minute| {
        if (30..40).contains(&minute) {
            (4.0, 0.58, 30.0, 0.80, 0.0)
        } else {
            (4.0, 0.03, 22.0, 0.03, 0.0)
        }
    });
    let result = estimate(&flow, &[]);
    assert!(fraction(&result, 32, 38, SleepStage::Wake) > 0.5);
    assert!(fraction(&result, 50, 65, SleepStage::Wake) < 0.1);
}

#[test]
fn moderate_irregularity_with_sighs_supports_wake() {
    let mut flow = breathing(75, |minute| {
        if (30..40).contains(&minute) {
            (4.0, 0.22, 22.0, 0.30, 0.0)
        } else {
            (4.0, 0.05, 22.0, 0.06, 0.0)
        }
    });
    let mut breath = 0;
    let mut previous = 0.0;
    for sample in &mut flow[30 * 60 * HZ as usize..40 * 60 * HZ as usize] {
        let original = *sample;
        if previous <= 0.0 && original > 0.0 {
            breath += 1;
        }
        if breath % 7 == 0 {
            *sample *= 2.7;
        }
        previous = original;
    }
    let result = estimate(&flow, &[]);
    assert!(fraction(&result, 32, 38, SleepStage::Wake) > 0.25);
    assert!(fraction(&result, 50, 65, SleepStage::Wake) < 0.1);
    assert!(fraction(&result, 32, 38, SleepStage::Uncertain) < 0.1);
}

#[test]
fn sustained_irregular_lower_ventilation_supports_rem() {
    let flow = breathing(200, |minute| {
        if (130..165).contains(&minute) {
            (4.5, 0.27, 14.0, 0.30, 0.0)
        } else {
            (4.0, 0.05, 22.0, 0.05, 0.0)
        }
    });
    let result = estimate(&flow, &[]);
    assert!(
        fraction(&result, 135, 160, SleepStage::Rem) > 0.2,
        "{:?}",
        result.minutes
    );
    assert!(fraction(&result, 100, 120, SleepStage::Rem) < 0.1);
}

#[test]
fn strong_evidence_can_support_early_rem_and_late_deep() {
    let flow = breathing(180, |minute| {
        if (15..40).contains(&minute) {
            (4.5, 0.30, 13.0, 0.34, 0.0)
        } else if (135..165).contains(&minute) {
            (5.0, 0.005, 22.0, 0.005, 0.0)
        } else {
            (4.0, 0.10, 22.0, 0.12, 0.0)
        }
    });
    let result = estimate(&flow, &[]);
    assert!(
        fraction(&result, 20, 35, SleepStage::Rem) > 0.0,
        "{:?}",
        result.minutes
    );
    assert!(
        fraction(&result, 140, 160, SleepStage::Deep) > 0.0,
        "{:?}",
        result.minutes
    );
}

#[test]
fn short_irregular_burst_does_not_fill_a_rem_window() {
    let flow = breathing(200, |minute| {
        if (150..152).contains(&minute) {
            (4.5, 0.27, 14.0, 0.30, 0.0)
        } else {
            (4.0, 0.03, 22.0, 0.03, 0.0)
        }
    });
    let result = estimate(&flow, &[]);
    assert!(fraction(&result, 140, 147, SleepStage::Rem) < 0.1);
    assert!(fraction(&result, 157, 170, SleepStage::Rem) < 0.1);
    assert!(result.minutes.rem <= 7.0, "{:?}", result.minutes);
}

#[test]
fn onset_prior_is_brief_and_restarts_after_mask_gap() {
    let mut flow = breathing(60, |_| (4.0, 0.03, 22.0, 0.03, 0.0));
    flow[30 * 60 * HZ as usize..35 * 60 * HZ as usize].fill(f32::NAN);
    let result = estimate(&flow, &[]);
    assert_eq!(result.stages[0], SleepStage::Wake);
    assert_eq!(result.stages[70], SleepStage::Wake);
    assert!(fraction(&result, 10, 25, SleepStage::Wake) < 0.1);
    assert!(fraction(&result, 45, 55, SleepStage::Wake) < 0.1);
    assert!(result.stages[60..70]
        .iter()
        .all(|&s| s == SleepStage::NoData));
}

#[test]
fn gaps_stay_no_data_and_partial_epochs_count_only_observed_time() {
    let mut flow = breathing(60, |_| (4.0, 0.05, 22.0, 0.05, 0.0));
    let mut leak = vec![0.0; 60 * 60];
    leak[10 * 60..15 * 60].fill(80.0);
    flow[30 * 60 * HZ as usize..35 * 60 * HZ as usize].fill(f32::NAN);
    flow[50 * 60 * HZ as usize..50 * 60 * HZ as usize + 100].fill(f32::NAN);
    flow.truncate(59 * 60 * HZ as usize + 13 * HZ as usize);
    let result = estimate(&flow, &leak);
    assert!(result.stages[60..70]
        .iter()
        .all(|&s| s == SleepStage::NoData));
    assert_eq!(result.stages.len(), flow.len().div_ceil(30 * HZ as usize));
    let expected = flow.iter().filter(|x| x.is_finite()).count() as f32 / HZ as f32 / 60.0;
    assert!((total_minutes(&result) - expected).abs() < 0.01);
    assert!(result.minutes.uncertain >= 4.0);
}

#[test]
fn version_four_serializes_uncertainty_evidence_and_confounders() {
    let flow = breathing(10, |_| (4.0, 0.05, 22.0, 0.05, 0.0));
    let result = estimate(&flow, &[]);
    assert_eq!(ESTIMATOR_VERSION, 4);
    assert_eq!(result.estimator_version, 4);
    assert_eq!(result, estimate(&flow, &[]));
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["estimatorVersion"], 4);
    // Version 4 adds the machine-readable uncertainty reasons and the periodic
    // breathing flag, so a UI can explain a gap instead of only showing one.
    assert!(value["uncertainRuns"].is_array());
    assert!(value["periodicBreathing"].is_boolean());
    assert_eq!(
        value["confidence"],
        serde_json::json!({
            "sleepWake": "moderate", "rem": "low", "deep": "veryLow"
        })
    );
    assert!(value["minutes"]["uncertain"].is_number());
    assert_eq!(
        serde_json::to_value(SleepStage::Uncertain).unwrap(),
        "uncertain"
    );
    assert_eq!(
        serde_json::from_value::<SleepStageEstimate>(value).unwrap(),
        result
    );
}

#[test]
fn debug_report_contains_aggregate_diagnostics_without_identifiers() {
    let flow = breathing(15, |_| (4.0, 0.05, 22.0, 0.05, 0.0));
    let report = sleep_stage_debug(&SleepStageInput {
        start: "PRIVATE_START_MARKER",
        flow: &flow,
        hz: HZ,
        excess_leak: &[],
        event_offsets: &[],
    });
    assert!(report.contains("p10 p25 p50 p75 p90"));
    assert!(report.contains("rule "));
    assert!(!report.contains("PRIVATE_START_MARKER"));
}

#[test]
fn missing_and_unusable_signal_has_bounded_quality_without_panics() {
    assert!(estimate(&[], &[]).stages.is_empty());
    let missing = estimate(&vec![f32::NAN; 3_000], &[]);
    assert!(missing.stages.iter().all(|&s| s == SleepStage::NoData));
    assert_eq!(total_minutes(&missing), 0.0);
    assert_eq!(missing.signal_quality, 0.0);
    let flat = estimate(&vec![0.0; 3_000], &[]);
    assert!((0.0..=1.0).contains(&flat.signal_quality));
    assert!(flat.signal_quality < 0.5);
    assert!(flat.stages.iter().all(|&s| s == SleepStage::Uncertain));
    let invalid_rate = estimate_sleep_stages(&SleepStageInput {
        start: "2030-01-15T22:00:00",
        flow: &[f32::INFINITY, f32::NAN],
        hz: 0,
        excess_leak: &[],
        event_offsets: &[u32::MAX],
    });
    assert!((0.0..=1.0).contains(&invalid_rate.signal_quality));
}

#[test]
fn nine_hour_night_meets_runtime_budget() {
    let flow = breathing(540, |minute| {
        if (150..190).contains(&minute) {
            (4.5, 0.25, 16.0, 0.28, 0.0)
        } else {
            (4.0, 0.08, 22.0, 0.10, 0.0)
        }
    });
    let started = Instant::now();
    let result = estimate(&flow, &[]);
    let elapsed = started.elapsed();
    let limit = if cfg!(debug_assertions) { 10.0 } else { 1.0 };
    assert!(
        elapsed.as_secs_f32() < limit,
        "nine-hour estimate took {elapsed:?}"
    );
    assert_eq!(result.stages.len(), 1_080);
    assert!((total_minutes(&result) - 540.0).abs() < 0.01);
    assert!((0.0..=1.0).contains(&result.signal_quality));
    eprintln!("nine-hour estimate: {elapsed:?}");
}
