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

fn night(minutes: usize) -> (Vec<f32>, Vec<f32>) {
    let flow = breathing(minutes, |minute| match minute {
        10..18 => (3.0, 0.60, 30.0, 0.65, 25.0),
        40..80 => (5.5, 0.01, 20.0, 0.01, 0.0),
        150..190 => (3.5, 0.25, 22.0, 0.28, 0.0),
        _ => (4.0, 0.12, 22.0, 0.12, 0.0),
    });
    let mut leak = vec![0.0; minutes * 60];
    leak[10 * 60..18 * 60].fill(80.0);
    (flow, leak)
}

fn estimate(flow: &[f32], leak: &[f32]) -> SleepStageEstimate {
    estimate_sleep_stages(&SleepStageInput {
        start: "2030-01-15T22:00:00",
        flow,
        hz: HZ,
        excess_leak: leak,
        event_offsets: &[],
    })
}

fn fraction(result: &SleepStageEstimate, from: usize, to: usize, stage: SleepStage) -> f32 {
    let epochs = &result.stages[from * 2..to * 2];
    epochs.iter().filter(|&&value| value == stage).count() as f32 / epochs.len() as f32
}

fn total_minutes(result: &SleepStageEstimate) -> f32 {
    result.minutes.wake + result.minutes.light + result.minutes.deep + result.minutes.rem
}

fn runs(result: &SleepStageEstimate, stage: SleepStage) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut start = 0;
    while start < result.stages.len() {
        let current = result.stages[start];
        let mut end = start + 1;
        while end < result.stages.len() && result.stages[end] == current {
            end += 1;
        }
        if current == stage {
            runs.push((start, end));
        }
        start = end;
    }
    runs
}

fn assert_minimum_bouts(result: &SleepStageEstimate, stage: SleepStage, minimum: usize) {
    for (start, end) in runs(result, stage) {
        assert!(
            end - start >= minimum,
            "short {stage:?} bout {start}..{end}"
        );
    }
}

#[test]
fn stable_breathing_has_sleep_latency_and_restarts_after_mask_gap() {
    let mut flow = breathing(120, |_| (4.0, 0.08, 22.0, 0.10, 0.0));
    flow[60 * 60 * HZ as usize..65 * 60 * HZ as usize].fill(f32::NAN);
    let result = estimate(&flow, &[]);
    assert!(result.stages[..10].iter().all(|&s| s == SleepStage::Wake));
    assert!(result.stages[130..134]
        .iter()
        .all(|&s| s == SleepStage::Wake));
    assert!(fraction(&result, 20, 55, SleepStage::Wake) < 0.1);
    assert!(fraction(&result, 80, 110, SleepStage::Wake) < 0.1);
    assert!(result.stages[120..130]
        .iter()
        .all(|&s| s == SleepStage::NoData));
    assert_eq!(fraction(&result, 65, 120, SleepStage::Rem), 0.0);
}

#[test]
fn unstable_onset_stays_awake_until_breathing_settles() {
    let flow = breathing(75, |minute| {
        if minute < 15 {
            (3.0, 0.60, 30.0, 0.65, 25.0)
        } else {
            (4.0, 0.08, 22.0, 0.10, 0.0)
        }
    });
    let result = estimate(&flow, &[]);
    assert!(result.stages[..30].iter().all(|&s| s == SleepStage::Wake));
    assert!(fraction(&result, 25, 55, SleepStage::Wake) < 0.1);
}

#[test]
fn uniformly_irregular_breathing_does_not_become_deep_by_relative_rank() {
    let flow = breathing(180, |_| (4.0, 0.55, 22.0, 0.65, 0.0));
    let result = estimate(&flow, &[]);
    assert!(result.minutes.deep < 5.0, "{result:?}");
}

#[test]
fn scattered_irregular_breathing_forms_consolidated_rem_periods() {
    let flow = breathing(360, |minute| {
        let in_rem_window = matches!(minute, 65..95 | 155..190 | 245..280 | 335..355);
        if in_rem_window && minute % 5 != 0 {
            (3.5, 0.25, 22.0, 0.28, 0.0)
        } else {
            (4.0, 0.08, 22.0, 0.10, 0.0)
        }
    });
    let result = estimate(&flow, &[]);
    let onset = result
        .stages
        .iter()
        .position(|&s| !matches!(s, SleepStage::Wake | SleepStage::NoData))
        .expect("stable breathing should reach sleep");
    assert!(result.stages[..(onset + 120).min(result.stages.len())]
        .iter()
        .all(|&s| s != SleepStage::Rem));
    let rem_runs = runs(&result, SleepStage::Rem);
    assert!((3..=5).contains(&rem_runs.len()), "REM runs: {rem_runs:?}");
    assert_minimum_bouts(&result, SleepStage::Rem, 10);
}

#[test]
fn noisy_wake_burst_does_not_manufacture_rem_in_quiet_sleep() {
    let flow = breathing(120, |minute| {
        if (75..81).contains(&minute) {
            (3.0, 0.60, 30.0, 0.65, 25.0)
        } else {
            (4.0, 0.01, 22.0, 0.01, 0.0)
        }
    });
    let result = estimate(&flow, &[]);
    assert!(fraction(&result, 76, 80, SleepStage::Wake) > 0.9);
    assert_eq!(result.minutes.rem, 0.0, "{result:?}");
}

#[test]
fn debug_report_has_feature_percentiles_and_rule_counts_without_identifiers() {
    let flow = breathing(30, |_| (4.0, 0.08, 22.0, 0.10, 0.0));
    let report = sleep_stage_debug(&SleepStageInput {
        start: "PRIVATE_START_MARKER",
        flow: &flow,
        hz: HZ,
        excess_leak: &[],
        event_offsets: &[],
    });
    assert!(report.contains("p10 p25 p50 p75 p90"));
    let expected_sleep_epochs = estimate(&flow, &[])
        .stages
        .iter()
        .filter(|&&stage| !matches!(stage, SleepStage::Wake | SleepStage::NoData))
        .count();
    assert!((1..60).contains(&expected_sleep_epochs));
    for (population, expected_count) in [("observed", 60), ("sleep", expected_sleep_epochs)] {
        for feature in [
            "interval_cv",
            "volume_cv",
            "ventilation_cv",
            "combined_regularity",
        ] {
            let label = format!("{population}_{feature} ");
            let line = report
                .lines()
                .find(|line| line.starts_with(&label))
                .unwrap_or_else(|| panic!("missing {label} in {report}"));
            assert!(line.ends_with(&format!("n={expected_count}")), "{line}");
        }
    }
    for rule in ["onset_wake", "deep_candidates", "rem_candidates"] {
        assert!(report.contains(&format!("rule {rule}:")), "{report}");
    }
    assert!(!report.contains("PRIVATE_START_MARKER"));
}

#[test]
fn controlled_breathing_distinguishes_deep_rem_and_noisy_wake() {
    let (flow, leak) = night(240);
    let result = estimate(&flow, &leak);
    assert!(
        fraction(&result, 43, 77, SleepStage::Deep) > 0.70,
        "{result:?}"
    );
    assert!(
        fraction(&result, 150, 190, SleepStage::Rem) > 0.15,
        "{result:?}"
    );
    assert!(
        fraction(&result, 11, 17, SleepStage::Wake) > 0.70,
        "{result:?}"
    );
    assert_minimum_bouts(&result, SleepStage::Deep, 10);
    assert_minimum_bouts(&result, SleepStage::Rem, 10);
    assert!((total_minutes(&result) - 240.0).abs() < 0.01);
    assert!((0.0..=1.0).contains(&result.signal_quality));
    assert_eq!(result.estimator_version, ESTIMATOR_VERSION);
    assert_eq!(ESTIMATOR_VERSION, 2);
    assert_eq!(result, estimate(&flow, &leak));
    let without_leak = estimate(&flow, &[]);
    assert!(fraction(&without_leak, 11, 17, SleepStage::Wake) > 0.70);

    let clean = &flow[43 * 60 * HZ as usize..77 * 60 * HZ as usize];
    let noisy = &flow[11 * 60 * HZ as usize..17 * 60 * HZ as usize];
    assert!(estimate(noisy, &[]).signal_quality < estimate(clean, &[]).signal_quality);

    let mut leak_only = vec![0.0; leak.len()];
    leak_only[100 * 60..110 * 60].fill(80.0);
    let leaked = estimate(&flow, &leak_only);
    assert!(fraction(&leaked, 101, 109, SleepStage::Wake) > 0.90);

    let events: Vec<_> = (45 * 60..75 * 60).step_by(15).collect();
    let eventful = estimate_sleep_stages(&SleepStageInput {
        start: "2030-01-15T22:00:00",
        flow: &flow,
        hz: HZ,
        excess_leak: &[],
        event_offsets: &events,
    });
    assert_eq!(fraction(&eventful, 46, 74, SleepStage::Deep), 0.0);

    let mut early_irregular = flow.clone();
    early_irregular.copy_within(
        150 * 60 * HZ as usize..190 * 60 * HZ as usize,
        20 * 60 * HZ as usize,
    );
    let early = estimate(&early_irregular, &[]);
    assert_eq!(fraction(&early, 23, 57, SleepStage::Rem), 0.0);
}

#[test]
fn gaps_stay_no_data_and_partial_epochs_count_only_masked_on_time() {
    let (mut flow, leak) = night(100);
    flow[30 * 60 * HZ as usize..35 * 60 * HZ as usize].fill(f32::NAN);
    flow[90 * 60 * HZ as usize..90 * 60 * HZ as usize + 100].fill(f32::NAN);
    flow.truncate(99 * 60 * HZ as usize + 13 * HZ as usize);
    let result = estimate(&flow, &leak);
    assert!(result.stages[60..70]
        .iter()
        .all(|&s| s == SleepStage::NoData));
    assert_eq!(result.stages.len(), flow.len().div_ceil(30 * HZ as usize));
    let expected = flow.iter().filter(|x| x.is_finite()).count() as f32 / HZ as f32 / 60.0;
    assert!((total_minutes(&result) - expected).abs() < 0.01);
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
    let (flow, leak) = night(540);
    let started = Instant::now();
    let result = estimate(&flow, &leak);
    let elapsed = started.elapsed();
    let limit = if cfg!(debug_assertions) { 10.0 } else { 1.0 };
    assert!(
        elapsed.as_secs_f32() < limit,
        "nine-hour estimate took {elapsed:?}"
    );
    assert_eq!(result.stages.len(), 1_080);
    assert!((total_minutes(&result) - 540.0).abs() < 0.01);
    eprintln!("nine-hour estimate: {elapsed:?}");
}
