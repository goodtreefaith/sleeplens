//! Estimated therapy score. Missing measurements never count as zero.
use crate::model::{Night, ScoreComponent, SleepScore};

/// Bumped when score rules change, so stored totals can be refreshed.
pub const SCORE_VERSION: u32 = 3;

fn linear(value: f64, knots: &[(f64, f64)]) -> f64 {
    if value <= knots[0].0 {
        return knots[0].1;
    }
    for pair in knots.windows(2) {
        let [(x0, y0), (x1, y1)] = [pair[0], pair[1]];
        if value <= x1 {
            return y0 + (y1 - y0) * (value - x0) / (x1 - x0);
        }
    }
    knots.last().unwrap().1
}
fn component(
    key: &str,
    label: &str,
    points: f64,
    max_points: f64,
    detail: String,
) -> ScoreComponent {
    ScoreComponent {
        key: key.into(),
        label: label.into(),
        points,
        max_points,
        detail,
    }
}
/// Compute a score using available components, rescaled to a total of 100.
pub fn sleep_score(night: &Night) -> Option<SleepScore> {
    let n = &night.list;
    if !n.usage_minutes.is_finite() || n.usage_minutes < 30. {
        return None;
    }
    let minutes = n.usage_minutes.round() as u64;
    let mut components = vec![component(
        "duration",
        "Hours used",
        linear(
            n.usage_minutes,
            &[(0., 0.), (240., 12.), (360., 22.), (420., 27.), (480., 30.)],
        ),
        30.,
        format!(
            "{} h {} m of use, with full points at 8 h.",
            minutes / 60,
            minutes % 60
        ),
    )];
    if let Some(ahi) = n.ahi.filter(|v| v.is_finite()) {
        components.push(component(
            "breathing",
            "Breathing events",
            linear(
                ahi,
                &[
                    (0., 30.),
                    (1., 28.),
                    (2., 26.),
                    (5., 20.),
                    (15., 8.),
                    (30., 0.),
                ],
            ),
            30.,
            format!("{ahi:.1} breathing events per hour were recorded."),
        ));
    }
    // A missing large-leak measurement cannot be treated as no large leak.
    if let Some(leak) = n.large_leak_minutes.filter(|v| v.is_finite()) {
        let share = (leak / n.usage_minutes).max(0.);
        let penalty = n
            .excess_leak_p95
            .filter(|v| v.is_finite())
            .map_or(0., |v| linear(v, &[(20., 0.), (36., 5.)]));
        let points = (linear(share, &[(0., 15.), (0.05, 10.), (0.2, 0.)]) - penalty).max(0.);
        components.push(component(
            "maskSeal",
            "Mask seal",
            points,
            15.,
            match n.excess_leak_p95.filter(|v| v.is_finite()) {
                Some(p95) => format!(
                    "Large leaks covered {:.1}% of mask-on time, with excess leak at or below {p95:.1} L/min for 95% of the night.",
                    share * 100.
                ),
                None => format!("Large leaks covered {:.1}% of mask-on time.", share * 100.),
            },
        ));
    }
    let wake_share = night
        .stages
        .as_ref()
        .map(|s| s.minutes.wake as f64 / n.usage_minutes);
    let wake_penalty = wake_share.map_or(0., |v| linear(v, &[(0.05, 0.), (0.2, 6.)]));
    components.push(component(
        "continuity",
        "Unbroken sleep",
        (15. - 3. * n.interruptions as f64 - wake_penalty).max(0.),
        15.,
        match wake_share {
            Some(share) => format!(
                "There were {} mask-off breaks and {:.0}% estimated wake time with the mask on.",
                n.interruptions,
                share * 100.
            ),
            None => format!(
                "There were {} mask-off breaks during the night.",
                n.interruptions
            ),
        },
    ));
    if let Some(stages) = &night.stages {
        let m = &stages.minutes;
        // Confounded mask-on time must not dilute the REM share of estimated sleep.
        if m.uncertain as f64 / n.usage_minutes <= 0.40 {
            let sleep = m.light as f64 + m.deep as f64 + m.rem as f64;
            let rem_share = if sleep > 0. { m.rem as f64 / sleep } else { 0. };
            components.push(component(
                "sleepStages",
                "REM sleep (estimated, low confidence)",
                10. * (rem_share / 0.20).clamp(0., 1.),
                10.,
                format!(
                    "Estimated REM sleep was {:.0} min, with low confidence.",
                    (m.rem / 5.).round() * 5.
                ),
            ));
        }
    }
    let total = (components.iter().map(|c| c.points).sum::<f64>()
        / components.iter().map(|c| c.max_points).sum::<f64>()
        * 100.)
        .round()
        .clamp(0., 100.) as u32;
    for c in &mut components {
        c.points = (c.points * 10.).round() / 10.;
    }
    Some(SleepScore {
        total,
        label: match total {
            85.. => "Great",
            70..=84 => "Good",
            50..=69 => "Fair",
            _ => "Poor",
        }
        .into(),
        estimated: true,
        components,
    })
}
/// Keep the compact list total and full detail score in agreement.
pub fn refresh_score(night: &mut Night) {
    night.score = sleep_score(night);
    night.list.sleep_score = night.score.as_ref().map(|s| s.total);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    #[test]
    fn component_knots_and_interpolation() {
        assert_eq!(linear(390., &[(360., 22.), (420., 27.)]), 24.5);
        assert_eq!(
            linear(
                10.,
                &[
                    (0., 30.),
                    (1., 28.),
                    (2., 26.),
                    (5., 20.),
                    (15., 8.),
                    (30., 0.)
                ]
            ),
            14.
        );
        assert!((linear(0.125, &[(0., 15.), (0.05, 10.), (0.2, 0.)]) - 5.).abs() < 1e-10);
        assert_eq!(linear(28., &[(20., 0.), (36., 5.)]), 2.5);
    }
    pub(crate) fn night() -> Night {
        serde_json::from_value(serde_json::json!({"date":"2026-01-01","start":null,"end":null,"usageMinutes":420.,"usageSource":"computed","ahi":null,"pressureMean":null,"pressureP90":null,"pressureSource":"computed","excessLeakMedian":null,"largeLeakMinutes":null,"eventCounts":{"obstructive":0,"central":0,"hypopnea":0},"settingsChanged":false,"sessions":[],"summary":{},"settings":null,"events":[],"series":null})).unwrap()
    }
    #[test]
    fn missing_components_rescale_and_short_use_has_no_score() {
        let mut n = night();
        let score = sleep_score(&n).unwrap();
        assert_eq!(score.total, 93);
        assert_eq!(score.components.len(), 2);
        n.list.usage_minutes = 29.9;
        assert!(sleep_score(&n).is_none());
    }
    fn stages(
        wake: f32,
        light: f32,
        deep: f32,
        rem: f32,
        uncertain: f32,
    ) -> crate::sleep_stages::SleepStageEstimate {
        serde_json::from_value(serde_json::json!({
            "start": "2026-01-01T22:00:00", "epochSeconds": 30, "stages": [],
            "minutes": { "wake": wake, "light": light, "deep": deep, "rem": rem, "uncertain": uncertain },
            "signalQuality": 1., "estimatorVersion": 3,
            "confidence": { "sleepWake": "moderate", "rem": "low", "deep": "veryLow" }
        })).unwrap()
    }
    #[test]
    fn continuity_stages_and_label() {
        let mut n = night();
        n.list.interruptions = 3;
        n.stages = Some(stages(42., 245.7, 66.15, 66.15, 0.));
        let score = sleep_score(&n).unwrap();
        assert_eq!(
            score
                .components
                .iter()
                .find(|c| c.key == "continuity")
                .unwrap()
                .points,
            4.
        );
        assert_eq!(
            score
                .components
                .iter()
                .find(|c| c.key == "sleepStages")
                .unwrap()
                .points,
            8.8
        );
        assert_eq!(score.label, "Good");
    }
    fn points(n: &Night, key: &str) -> f64 {
        sleep_score(n)
            .unwrap()
            .components
            .iter()
            .find(|c| c.key == key)
            .unwrap()
            .points
    }
    #[test]
    fn score_component_boundaries_and_labels() {
        let mut n = night();
        for (minutes, expected) in [
            (30., 1.5),
            (240., 12.),
            (330., 19.5),
            (360., 22.),
            (420., 27.),
            (450., 28.5),
            (480., 30.),
            (600., 30.),
        ] {
            n.list.usage_minutes = minutes;
            assert_eq!(points(&n, "duration"), expected);
        }
        n.list.usage_minutes = 420.;
        for (ahi, expected) in [
            (0., 30.),
            (1., 28.),
            (2., 26.),
            (3., 24.),
            (5., 20.),
            (10., 14.),
            (15., 8.),
            (30., 0.),
            (50., 0.),
        ] {
            n.list.ahi = Some(ahi);
            assert_eq!(points(&n, "breathing"), expected);
        }
        n.list.excess_leak_p95 = Some(20.);
        for (share, expected) in [(0., 15.), (0.05, 10.), (0.125, 5.), (0.2, 0.), (0.5, 0.)] {
            n.list.large_leak_minutes = Some(share * 420.);
            assert_eq!(points(&n, "maskSeal"), expected);
        }
        n.list.large_leak_minutes = Some(0.);
        n.list.excess_leak_p95 = Some(28.);
        assert_eq!(points(&n, "maskSeal"), 12.5);
        n.list.excess_leak_p95 = Some(36.);
        assert_eq!(points(&n, "maskSeal"), 10.);
        n.list.interruptions = 8;
        assert_eq!(points(&n, "continuity"), 0.);
        for (minutes, ahi, expected) in [
            (420., 0., "Great"),
            (420., 10., "Good"),
            (240., 10., "Fair"),
            (30., 30., "Poor"),
        ] {
            n.list.usage_minutes = minutes;
            n.list.ahi = Some(ahi);
            n.list.large_leak_minutes = None;
            n.list.interruptions = 0;
            assert_eq!(sleep_score(&n).unwrap().label, expected);
        }
    }
    #[test]
    fn stage_and_wake_linear_penalties() {
        let mut n = night();
        n.stages = Some(stages(42., 330.75, 33.075, 14.175, 0.));
        assert_eq!(points(&n, "sleepStages"), 1.9);
        assert_eq!(points(&n, "continuity"), 13.);
        n.stages.as_mut().unwrap().minutes.wake = 52.5;
        assert_eq!(points(&n, "continuity"), 12.);
        n.stages.as_mut().unwrap().minutes.wake = 84.;
        assert_eq!(points(&n, "continuity"), 9.);
        n.stages.as_mut().unwrap().minutes.wake = 420.;
        assert_eq!(points(&n, "continuity"), 9.);
    }

    #[test]
    fn only_rem_earns_stage_points_and_plain_details() {
        let mut n = night();
        n.list.usage_minutes = 418.;
        n.stages = Some(stages(18., 260., 140., 0., 0.));
        // Extra deep sleep cannot make up for absent REM sleep.
        assert_eq!(points(&n, "sleepStages"), 0.);
        let m = &mut n.stages.as_mut().unwrap().minutes;
        m.deep = 62.;
        m.rem = 78.;
        let score = sleep_score(&n).unwrap();
        assert_eq!(
            score.components[0].detail,
            "6 h 58 m of use, with full points at 8 h."
        );
        assert_eq!(
            score.components.last().unwrap().detail,
            "Estimated REM sleep was 80 min, with low confidence."
        );
    }

    #[test]
    fn rem_share_excludes_uncertain_and_drops_only_above_forty_percent() {
        let mut n = night();
        n.list.usage_minutes = 500.;
        n.stages = Some(stages(0., 240., 0., 60., 200.));
        // Exactly 40% uncertain retains the component; REM is 20% of known sleep.
        let score = sleep_score(&n).unwrap();
        let rem = score.components.last().unwrap();
        assert_eq!(rem.key, "sleepStages");
        assert_eq!(rem.label, "REM sleep (estimated, low confidence)");
        assert_eq!(rem.max_points, 10.);
        assert_eq!(rem.points, 10.);
        assert_eq!(score.total, 100);
        // Reclassifying non-REM from light to deep must not affect the score.
        n.stages.as_mut().unwrap().minutes.light = 0.;
        n.stages.as_mut().unwrap().minutes.deep = 240.;
        assert_eq!(
            serde_json::to_value(sleep_score(&n).unwrap()).unwrap(),
            serde_json::to_value(score).unwrap()
        );
        // The remaining available components are rescaled after dropping REM.
        n.stages.as_mut().unwrap().minutes.uncertain = 200.1;
        let without_rem = sleep_score(&n).unwrap();
        assert!(!without_rem
            .components
            .iter()
            .any(|c| c.key == "sleepStages"));
        assert_eq!(without_rem.total, 100);
    }

    #[test]
    fn long_nights_still_show_a_range_of_scores() {
        let mut n = night();
        n.list.usage_minutes = 480.;
        for (ahi, leak, p95, interruptions, wake, deep, rem, total) in [
            (1., 0., 16., 0, 24., 68.4, 91.2, 98),
            (5., 24., 28., 1, 48., 43.2, 64.8, 75),
            (20., 72., 36., 4, 96., 19.2, 19.2, 38),
        ] {
            n.list.ahi = Some(ahi);
            n.list.large_leak_minutes = Some(leak);
            n.list.excess_leak_p95 = Some(p95);
            n.list.interruptions = interruptions;
            n.stages = Some(stages(wake, 480. - wake - deep - rem, deep, rem, 0.));
            assert_eq!(sleep_score(&n).unwrap().total, total);
        }
    }

    #[test]
    fn present_but_all_wake_stages_score_zero() {
        let mut n = night();
        n.stages = Some(stages(420., 0., 0., 0., 0.));
        assert_eq!(points(&n, "sleepStages"), 0.);
        assert_eq!(sleep_score(&n).unwrap().total, 65);
    }
}
