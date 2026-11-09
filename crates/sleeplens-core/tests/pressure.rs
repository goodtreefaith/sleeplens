use sleeplens_core::{model::*, pressure::*, tips};

fn night() -> Night {
    serde_json::from_value(serde_json::json!({
        "date":"2026-01-01", "start":null,"end":null,"usageMinutes":420.,
        "usageSource":"machine","ahi":1.,"pressureMean":8.,"pressureP90":9.,
        "pressureSource":"machine","excessLeakMedian":0.,"largeLeakMinutes":0.,
        "excessLeakP95":0.,"eventCounts":{"obstructive":1,"central":0,"hypopnea":0},
        "settingsChanged":false,"sessions":[],"summary":{"oai":0.2,"hi":0.},
        "settings":{"mode":1,"modeLabel":"Auto CPAP","startPressure":4.,"minPressure":6.,
        "maxPressure":12.,"fixedPressure":8.,"rampMinutes":20,"humidifierLevel":2,
        "comfortLevel":1,"autoOn":true,"autoOff":false,"leakAlert":true,"pressureUnit":"hPa"},
        "events":[],"series":null
    }))
    .unwrap()
}
fn week(n: &Night) -> Vec<Night> {
    (1..=7)
        .map(|day| {
            let mut n = n.clone();
            n.list.date = format!("2026-01-{day:02}");
            n
        })
        .collect()
}
fn has(tips: &[Tip], id: &str) -> bool {
    tips.iter().any(|t| t.id == id)
}
fn event(n: &mut Night, kind: &str, value: u8) {
    n.events.push(TherapyEvent {
        time: "2026-01-01T23:00:00".into(),
        kind: kind.into(),
        code: 0,
        value,
        pressure: Some(99.),
    });
}
fn leaking_series(n: &mut Night) {
    n.series = Some(Series {
        series_start: "2026-01-01T23:00:00".into(),
        pressure: [vec![Some(6.); 60], vec![Some(12.); 60], vec![None; 10]].concat(),
        excess_leak: [vec![Some(4.); 60], vec![Some(35.); 60], vec![None; 10]].concat(),
        total_leak: vec![],
        flow_amplitude: vec![],
        snore: vec![],
    });
}
#[test]
fn every_contract_rule_has_night_and_period_evidence() {
    let mut ceiling = night();
    ceiling.list.pressure_p90 = Some(11.75);
    ceiling.list.ahi = Some(5.);
    let mut top = night();
    top.list.pressure_p90 = Some(12.);
    pressure_share(&mut top, 12., 20);
    let mut floor = night();
    floor.summary.oai = Some(3.);
    for _ in 0..5 {
        event(&mut floor, "obstructiveApnea", 12);
    }
    let mut low = night();
    low.list.pressure_p90 = Some(6.25);
    let mut central = night();
    central.summary.cai = Some(5.1);
    let mut leak = night();
    leaking_series(&mut leak);
    let cases = [
        (ceiling, "pressure-ceiling-events"),
        (top, "pressure-ceiling"),
        (floor, "pressure-floor-events"),
        (low, "pressure-low-use"),
        (central, "central-with-pressure"),
        (leak, "leak-at-high-pressure"),
        (night(), "pressure-ok"),
    ];
    for (n, id) in &cases {
        let single = night_pressure_tips(n);
        if *id == "pressure-floor-events" {
            assert!(!has(&single, id));
        } else {
            assert!(has(&single, id), "{id}: {single:?}");
        }
        let period = period_pressure_tips(&week(n));
        assert!(has(&period, id), "{id}: {period:?}");
        for t in single.iter().chain(&period) {
            assert_eq!(t.category, "pressure");
            assert!(t.evidence.as_ref().is_some_and(|e| !e.is_empty()));
            assert!(!t.evidence_dates.is_empty());
            assert!(t.evidence_dates.len() <= 5);
        }
    }
}
#[test]
fn all_generated_pressure_text_avoids_setting_instructions_and_targets() {
    let mut candidates = Vec::new();
    for p90 in [6., 9., 12.] {
        for ahi in [1., 5., 20.] {
            for cai in [0., 6.] {
                let mut n = night();
                n.list.pressure_p90 = Some(p90);
                n.list.ahi = Some(ahi);
                n.summary.cai = Some(cai);
                candidates.push(n.clone());
                for _ in 0..5 {
                    event(&mut n, "hypopnea", 12);
                }
                n.summary.hi = Some(1.);
                candidates.push(n.clone());
                leaking_series(&mut n);
                candidates.push(n);
            }
        }
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut floor_severities = std::collections::BTreeSet::new();
    for n in &candidates {
        for t in night_pressure_tips(n)
            .into_iter()
            .chain(period_pressure_tips(&week(n)))
        {
            ids.insert(t.id.clone());
            if t.id == "pressure-floor-events" {
                floor_severities.insert(t.severity.clone());
            }
            let text =
                format!("{} {} {}", t.title, t.body, t.evidence.unwrap()).to_ascii_lowercase();
            for forbidden in [
                "increase",
                "decrease",
                "raise",
                "lower",
                "turn up",
                "turn down",
            ] {
                assert!(!text.contains(forbidden), "{}: {text}", t.id);
            }
            for target in [
                "set to",
                "setting to",
                "target pressure",
                "cmh2o",
                "cm h2o",
                "hpa",
            ] {
                assert!(!text.contains(target), "{}: {text}", t.id);
            }
            let words: Vec<_> = text.split_whitespace().collect();
            for triple in words.windows(3) {
                assert!(
                    !(triple[0] == "pressure"
                        && matches!(triple[1], "of" | "to" | "at")
                        && triple[2].chars().next().is_some_and(|c| c.is_ascii_digit())),
                    "{}: {text}",
                    t.id
                );
            }
            for pair in words.windows(2) {
                assert!(
                    !(matches!(pair[0], "minimum" | "maximum" | "pressure")
                        && pair[1].chars().next().is_some_and(|c| c.is_ascii_digit())),
                    "{}: {text}",
                    t.id
                );
            }
        }
    }
    assert_eq!(ids.len(), 7, "all rule templates must be scanned");
    assert_eq!(
        floor_severities,
        ["info".to_owned(), "watch".to_owned()].into()
    );
}
#[test]
fn facts_use_raw_event_encoding_and_mask_on_samples() {
    let mut n = night();
    leaking_series(&mut n);
    event(&mut n, "obstructiveApnea", 12);
    event(&mut n, "hypopnea", 24);
    event(&mut n, "centralApnea", 18);
    event(&mut n, "hypopnea", 0);
    let f = night_pressure_facts(&n);
    assert_eq!(f.share_at_max_pct, Some(50.));
    assert_eq!(f.share_at_min_pct, Some(50.));
    assert_eq!(f.observed_max, Some(12.));
    assert_eq!(f.obstructive_event_pressures, [6.]);
    assert_eq!(f.hypopnea_event_pressures, [12.]);
    assert_eq!(f.central_event_pressures, [9.]);
    assert_eq!(f.events_at_min, 1);
    assert_eq!(f.high_pressure_leak_mean, Some(35.));
    assert_eq!(f.low_pressure_leak_mean, Some(4.));
    assert_eq!(pressure_summary(&[n]).unwrap().events_at_min_pct, Some(50.));
}
#[test]
fn period_requires_seven_distinct_used_dates_and_three_floor_event_nights() {
    let mut n = night();
    n.summary.hi = Some(1.);
    for _ in 0..5 {
        event(&mut n, "hypopnea", 12);
    }
    assert!(period_pressure_tips(&week(&n)[..6]).is_empty());
    assert!(period_pressure_tips(&vec![n.clone(); 7]).is_empty());
    let mut nights = week(&n);
    for n in &mut nights[2..] {
        n.events.clear();
    }
    assert!(!has(
        &period_pressure_tips(&nights),
        "pressure-floor-events"
    ));
    for _ in 0..5 {
        event(&mut nights[2], "hypopnea", 12);
    }
    assert!(has(&period_pressure_tips(&nights), "pressure-floor-events"));
    nights[6].list.usage_minutes = 0.;
    assert!(period_pressure_tips(&nights).is_empty());
}
#[test]
fn ceiling_fraction_and_residual_events_are_separate_tests() {
    let mut nights = week(&night());
    for n in &mut nights[..2] {
        n.list.pressure_p90 = Some(12.);
        n.list.ahi = Some(10.);
    }
    assert!(!has(
        &period_pressure_tips(&nights),
        "pressure-ceiling-events"
    ));
    nights[2].list.pressure_p90 = Some(12.);
    nights[2].list.ahi = Some(0.);
    assert!(has(
        &period_pressure_tips(&nights),
        "pressure-ceiling-events"
    ));
    for n in &mut nights[..3] {
        n.list.ahi = Some(1.);
        n.summary.oai = Some(2.);
    }
    assert!(has(
        &period_pressure_tips(&nights),
        "pressure-ceiling-events"
    ));
}
#[test]
fn low_use_never_reassures_when_other_nights_have_residual_events() {
    let mut n = night();
    n.list.pressure_p90 = Some(6.);
    let mut nights = week(&n);
    nights[6].list.ahi = Some(8.);
    nights[6].list.pressure_p90 = Some(9.);
    assert!(!has(&period_pressure_tips(&nights), "pressure-low-use"));
}
#[test]
fn missing_settings_do_not_dilute_event_share_and_missing_leak_prevents_ok() {
    let mut known = night();
    event(&mut known, "hypopnea", 12);
    let mut unknown = night();
    unknown.list.date = "2026-01-02".into();
    unknown.settings = None;
    event(&mut unknown, "hypopnea", 24);
    assert_eq!(
        pressure_summary(&[known, unknown.clone()])
            .unwrap()
            .events_at_min_pct,
        Some(100.)
    );
    assert_eq!(
        pressure_summary(&[unknown]).unwrap().events_at_min_pct,
        None
    );
    let mut n = night();
    n.list.excess_leak_p95 = None;
    assert!(!has(&night_pressure_tips(&n), "pressure-ok"));
    n.settings = None;
    n.list.pressure_p90 = None;
    n.summary.max_pressure = None;
    assert!(pressure_summary(&[n]).is_none());
}
#[test]
fn fixed_mode_and_intranight_changes_do_not_claim_apap_boundary_patterns() {
    let mut n = night();
    n.list.pressure_p90 = Some(12.);
    n.settings.as_mut().unwrap().mode_label = "CPAP".into();
    assert!(night_pressure_tips(&n).is_empty());
    n.settings.as_mut().unwrap().mode_label = "Auto CPAP".into();
    n.intranight_prescription_changed = true;
    assert!(night_pressure_tips(&n).is_empty());
    n.summary.cai = Some(6.);
    assert!(has(&night_pressure_tips(&n), "central-with-pressure"));
}
#[test]
fn period_leak_compares_nights_and_compact_facts_match_full_series() {
    let mut nights = week(&night());
    for n in &mut nights[..3] {
        n.list.pressure_p90 = Some(11.);
        n.list.excess_leak_p95 = Some(40.);
    }
    for n in &mut nights[3..] {
        n.list.pressure_p90 = Some(7.);
        n.list.excess_leak_p95 = Some(5.);
    }
    let tips = period_pressure_tips(&nights);
    assert!(has(&tips, "leak-at-high-pressure"));
    for n in &mut nights {
        leaking_series(n);
    }
    let facts: Vec<_> = nights.iter().map(night_pressure_facts).collect();
    let full = pressure_summary(&nights).unwrap();
    for n in &mut nights {
        n.series = None;
    }
    assert_eq!(
        serde_json::to_value(full).unwrap(),
        serde_json::to_value(pressure_summary_with_facts(&nights, &facts)).unwrap()
    );
    assert!(has(
        &period_pressure_tips_with_facts(&nights, &facts),
        "leak-at-high-pressure"
    ));
}
#[test]
fn therapy_integration_reserves_pressure_slots_and_suppresses_generic_reassurance() {
    let mut n = night();
    n.list.pressure_p90 = Some(12.);
    n.list.usage_minutes = 100.;
    n.list.ahi = Some(20.);
    n.summary.cai = Some(6.);
    n.list.excess_leak_p95 = Some(40.);
    n.list.interruptions = 4;
    let tips = tips::night_tips(&n, &[]);
    assert_eq!(tips.iter().filter(|t| t.category == "pressure").count(), 2);
    assert_eq!(tips.iter().filter(|t| t.category == "therapy").count(), 4);
    assert_eq!(tips.len(), 6);
    let mut n = night();
    n.list.pressure_p90 = Some(12.);
    pressure_share(&mut n, 12., 20);
    let tips = tips::night_tips(&n, &[]);
    assert!(has(&tips, "pressure-ceiling"));
    assert!(!has(&tips, "all-good"));
}

#[test]
fn partial_event_indices_and_narrow_ranges_are_handled_conservatively() {
    let mut n = night();
    n.list.pressure_p90 = Some(6.);
    n.list.ahi = Some(4.);
    n.summary.oai = Some(3.);
    n.summary.hi = None;
    assert!(!has(&night_pressure_tips(&n), "pressure-low-use"));
    n.summary.oai = None;
    n.summary.hi = Some(0.);
    assert!(!has(&night_pressure_tips(&n), "pressure-low-use"));
    n.summary.hi = None;
    assert!(!has(&night_pressure_tips(&n), "pressure-low-use"));
    n.settings.as_mut().unwrap().max_pressure = 6.8;
    n.list.pressure_p90 = Some(6.8);
    n.list.ahi = Some(5.);
    assert!(has(&night_pressure_tips(&n), "pressure-ceiling-events"));
}

#[test]
fn exact_thresholds_and_short_leak_stretches_do_not_overstate_evidence() {
    let mut n = night();
    n.summary.cai = Some(5.);
    assert!(!has(&night_pressure_tips(&n), "central-with-pressure"));
    n.list.ahi = Some(6.);
    n.list.event_counts = EventCounts {
        obstructive: 1,
        central: 1,
        hypopnea: 0,
    };
    assert!(!has(&night_pressure_tips(&n), "central-with-pressure"));
    n.list.event_counts.central = 2;
    assert!(has(&night_pressure_tips(&n), "central-with-pressure"));
    let mut n = night();
    leaking_series(&mut n);
    n.series.as_mut().unwrap().pressure[0] = None;
    assert!(!has(&night_pressure_tips(&n), "leak-at-high-pressure"));
    let mut n = night();
    leaking_series(&mut n);
    for leak in &mut n.series.as_mut().unwrap().excess_leak[60..120] {
        *leak = Some(24.);
    }
    assert!(!has(&night_pressure_tips(&n), "leak-at-high-pressure"));
    let mut n = night();
    event(&mut n, "hypopnea", 11);
    assert_eq!(night_pressure_facts(&n).events_at_min, 1);
    n.events[0].value = 10;
    assert_eq!(
        night_pressure_facts(&n).events_at_min,
        0,
        "below the proximity window is not automatically a ramp event"
    );
}

#[test]
fn latest_prescription_and_each_nights_own_limits_are_preserved() {
    let mut first = night();
    first.list.pressure_p90 = Some(12.);
    event(&mut first, "hypopnea", 12);
    let mut latest = first.clone();
    latest.list.date = "2026-01-02".into();
    latest.settings.as_mut().unwrap().min_pressure = 8.;
    latest.settings.as_mut().unwrap().max_pressure = 14.;
    latest.list.pressure_p90 = Some(8.);
    event(&mut latest, "hypopnea", 16);
    let summary = pressure_summary(&[latest, first]).unwrap();
    assert_eq!(summary.setting_min, Some(8.));
    assert_eq!(summary.setting_max, Some(14.));
    assert_eq!(summary.nights_near_max, 1);
    assert_eq!(summary.nights_near_min, 1);
    assert!((summary.events_at_min_pct.unwrap() - 200. / 3.).abs() < 1e-9);
}

fn pressure_share(n: &mut Night, boundary: f32, percent: usize) {
    n.series = Some(Series {
        series_start: "2026-01-01T23:00:00".into(),
        pressure: [vec![Some(boundary); percent], vec![Some(9.); 100 - percent]].concat(),
        excess_leak: vec![],
        total_leak: vec![],
        flow_amplitude: vec![],
        snore: vec![],
    });
}

#[test]
fn near_boundaries_use_quarter_step_p90_or_recorded_time() {
    let mut n = night();
    for (p90, near_max, near_min) in [(11.74, 0, 0), (11.75, 1, 0), (6.25, 0, 1), (6.26, 0, 0)] {
        n.list.pressure_p90 = Some(p90);
        let summary = pressure_summary(&[n.clone()]).unwrap();
        assert_eq!(summary.nights_near_max, near_max);
        assert_eq!(summary.nights_near_min, near_min);
    }
    n.list.pressure_p90 = Some(9.);
    for (percent, expected) in [(9, 0), (10, 1)] {
        pressure_share(&mut n, 11.5, percent);
        assert_eq!(
            pressure_summary(&[n.clone()]).unwrap().nights_near_max,
            expected
        );
        assert_eq!(has(&night_pressure_tips(&n), "pressure-ok"), expected == 0);
        n.list.ahi = Some(5.);
        assert_eq!(
            has(&night_pressure_tips(&n), "pressure-ceiling-events"),
            expected == 1
        );
        n.list.ahi = Some(1.);
    }
    for (percent, expected) in [(49, 0), (50, 1)] {
        pressure_share(&mut n, 6.5, percent);
        assert_eq!(
            pressure_summary(&[n.clone()]).unwrap().nights_near_min,
            expected
        );
        assert_eq!(
            has(&night_pressure_tips(&n), "pressure-low-use"),
            expected == 1
        );
    }
}

#[test]
fn coarse_p90_one_step_below_max_with_six_percent_at_max_is_not_near_max() {
    let mut n = night();
    n.settings.as_mut().unwrap().min_pressure = 5.;
    n.settings.as_mut().unwrap().max_pressure = 8.;
    n.list.pressure_p90 = Some(7.5);
    pressure_share(&mut n, 8., 6);
    n.series.as_mut().unwrap().pressure[6..].fill(Some(7.));
    assert_eq!(pressure_summary(&[n.clone()]).unwrap().nights_near_max, 0);
    assert!(!has(&night_pressure_tips(&n), "pressure-ceiling"));
    assert!(has(&night_pressure_tips(&n), "pressure-ok"));
    assert!(!has(&period_pressure_tips(&week(&n)), "pressure-ceiling"));
    n.list.ahi = Some(5.);
    assert!(!has(
        &period_pressure_tips(&week(&n)),
        "pressure-ceiling-events"
    ));
}

#[test]
fn nightly_low_ahi_ceiling_info_requires_twenty_percent_recorded_time() {
    let mut n = night();
    n.list.pressure_p90 = Some(12.);
    assert!(!has(&night_pressure_tips(&n), "pressure-ceiling"));
    assert!(has(&period_pressure_tips(&week(&n)), "pressure-ceiling"));
    for percent in [0, 9, 10, 19, 20] {
        pressure_share(&mut n, 12., percent);
        assert_eq!(
            has(&night_pressure_tips(&n), "pressure-ceiling"),
            percent >= 20
        );
    }
}

#[test]
fn floor_events_require_five_events_and_mean_clinical_burden() {
    let mut n = night();
    n.list.ahi = Some(0.9);
    n.summary.oai = Some(0.5);
    n.summary.hi = Some(0.4);
    for _ in 0..5 {
        event(&mut n, "hypopnea", 12);
    }
    assert!(!has(
        &period_pressure_tips(&week(&n)),
        "pressure-floor-events"
    ));
    for (burden, ahi, severity) in [
        (1., 1., "info"),
        (2.99, 1., "info"),
        (3., 1., "watch"),
        (0.9, 5., "watch"),
    ] {
        n.summary.oai = Some(burden);
        n.summary.hi = Some(0.);
        n.list.ahi = Some(ahi);
        let result = period_pressure_tips(&week(&n));
        let tip = result
            .iter()
            .find(|t| t.id == "pressure-floor-events")
            .unwrap();
        assert_eq!(tip.severity, severity);
        if severity == "info" {
            assert_eq!(tip.body, "Some breathing events happened at the lowest pressure, often early in the night. Mention this at your next review.");
        }
        assert!(!has(&night_pressure_tips(&n), "pressure-floor-events"));
    }
    n.events.pop();
    assert!(!has(
        &period_pressure_tips(&week(&n)),
        "pressure-floor-events"
    ));
}

#[test]
fn floor_event_share_threshold_and_missing_index_fallback_are_preserved() {
    let mut n = night();
    n.summary.oai = None;
    n.summary.hi = None;
    n.list.event_counts.obstructive = 14;
    n.list.event_counts.hypopnea = 7;
    for _ in 0..3 {
        event(&mut n, "hypopnea", 12);
    }
    for _ in 0..7 {
        event(&mut n, "hypopnea", 18);
    }
    let result = period_pressure_tips(&week(&n));
    assert_eq!(
        result
            .iter()
            .find(|t| t.id == "pressure-floor-events")
            .unwrap()
            .severity,
        "watch"
    );
    event(&mut n, "hypopnea", 18);
    assert!(!has(
        &period_pressure_tips(&week(&n)),
        "pressure-floor-events"
    ));
}

#[test]
fn floor_event_burden_means_only_include_qualifying_nights() {
    let mut n = night();
    n.summary.oai = Some(1.);
    n.summary.hi = Some(0.);
    for _ in 0..5 {
        event(&mut n, "hypopnea", 12);
    }
    let mut nights = week(&n);
    for n in &mut nights[3..] {
        n.events.clear();
        n.summary.oai = Some(20.);
        n.list.ahi = Some(20.);
    }
    let result = period_pressure_tips(&nights);
    assert_eq!(
        result
            .iter()
            .find(|t| t.id == "pressure-floor-events")
            .unwrap()
            .severity,
        "info"
    );
    for n in &mut nights[..3] {
        n.summary.oai = Some(0.99);
    }
    assert!(!has(
        &period_pressure_tips(&nights),
        "pressure-floor-events"
    ));
}

#[test]
fn evidence_uses_singular_and_plural_used_nights() {
    let mut n = night();
    n.summary.cai = Some(6.);
    let single = night_pressure_tips(&n);
    assert!(single[0]
        .evidence
        .as_ref()
        .unwrap()
        .contains("1 of 1 used night had"));
    let mut nights = week(&night());
    for count in [1, 2] {
        for n in &mut nights[..count] {
            n.summary.cai = Some(6.);
        }
        let result = period_pressure_tips(&nights);
        let evidence = result
            .iter()
            .find(|t| t.id == "central-with-pressure")
            .unwrap()
            .evidence
            .as_ref()
            .unwrap();
        assert!(evidence.contains(&format!("{count} of 7 used nights had")));
    }
    n.summary.cai = None;
    n.list.pressure_p90 = Some(6.);
    assert!(night_pressure_tips(&n)[0]
        .evidence
        .as_ref()
        .unwrap()
        .contains("1 of 1 used night was near"));
}

#[test]
fn low_use_suppresses_info_floor_events_and_watch_burden_wins() {
    let mut n = night();
    n.list.pressure_p90 = Some(6.);
    n.summary.oai = Some(1.);
    n.list.event_counts.obstructive = 7;
    for _ in 0..7 {
        event(&mut n, "obstructiveApnea", 12);
    }
    let mut nights = week(&n);
    let tips = period_pressure_tips(&nights);
    assert!(has(&tips, "pressure-low-use"));
    assert!(!has(&tips, "pressure-floor-events"));
    // A smaller set of qualifying nights can have substantial burden even when
    // most nights stay near the minimum, so retain that observation alone.
    for n in &mut nights[..3] {
        n.summary.oai = Some(3.);
        n.list.event_counts.obstructive = 21;
    }
    let tips = period_pressure_tips(&nights);
    // Restrict floor evidence to the higher-burden nights.
    for n in &mut nights[3..] {
        n.events.clear();
    }
    let watch = period_pressure_tips(&nights);
    assert!(!has(&tips, "pressure-low-use"));
    assert!(watch
        .iter()
        .any(|t| t.id == "pressure-floor-events" && t.severity == "watch"));
    assert!(!has(&watch, "pressure-low-use"));
}
