//! Estimated therapy score. Missing measurements never count as zero.
use crate::model::{Night, ScoreComponent, SleepScore};

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
        linear(n.usage_minutes, &[(0., 0.), (240., 15.), (420., 30.)]),
        30.,
        format!(
            "{} h {} m of use, with a 7 h goal.",
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
                &[(0., 30.), (1., 30.), (5., 22.), (15., 8.), (30., 0.)],
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
            .map_or(0., |v| linear(v, &[(24., 0.), (40., 5.)]));
        let points = (linear(share, &[(0., 15.), (0.05, 10.), (0.2, 0.)]) - penalty).max(0.);
        components.push(component(
            "maskSeal",
            "Mask seal",
            points,
            15.,
            format!("Large leaks covered {:.1}% of mask-on time.", share * 100.),
        ));
    }
    let wake_share = night
        .stages
        .as_ref()
        .map(|s| s.minutes.wake as f64 / n.usage_minutes);
    let wake_penalty = wake_share.map_or(0., |v| linear(v, &[(0.1, 0.), (1., 4.)]));
    components.push(component(
        "continuity",
        "Unbroken sleep",
        (10. - 2. * n.interruptions as f64 - wake_penalty).max(0.),
        10.,
        format!(
            "There were {} mask-off breaks during the night.",
            n.interruptions
        ),
    ));
    if let Some(stages) = &night.stages {
        let m = &stages.minutes;
        let sleep = (m.light + m.deep + m.rem) as f64;
        {
            let share = if sleep > 0. {
                (m.deep + m.rem) as f64 / sleep
            } else {
                0.
            };
            components.push(component(
                "sleepStages",
                "Sleep stages (estimated)",
                15. * (share / 0.35).clamp(0., 1.),
                15.,
                format!(
                    "Estimated deep and REM sleep made up {:.0}% of sleep with the mask on.",
                    share * 100.
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
        assert_eq!(linear(330., &[(0., 0.), (240., 15.), (420., 30.)]), 22.5);
        assert_eq!(
            linear(
                10.,
                &[(0., 30.), (1., 30.), (5., 22.), (15., 8.), (30., 0.)]
            ),
            15.
        );
        assert!((linear(0.125, &[(0., 15.), (0.05, 10.), (0.2, 0.)]) - 5.).abs() < 1e-10);
        assert_eq!(linear(32., &[(24., 0.), (40., 5.)]), 2.5);
    }
    pub(crate) fn night() -> Night {
        serde_json::from_value(serde_json::json!({"date":"2026-01-01","start":null,"end":null,"usageMinutes":420.,"usageSource":"computed","ahi":null,"pressureMean":null,"pressureP90":null,"pressureSource":"computed","excessLeakMedian":null,"largeLeakMinutes":null,"eventCounts":{"obstructive":0,"central":0,"hypopnea":0},"settingsChanged":false,"sessions":[],"summary":{},"settings":null,"events":[],"series":null})).unwrap()
    }
    #[test]
    fn missing_components_rescale_and_short_use_has_no_score() {
        let mut n = night();
        let score = sleep_score(&n).unwrap();
        assert_eq!(score.total, 100);
        assert_eq!(score.components.len(), 2);
        n.list.usage_minutes = 29.9;
        assert!(sleep_score(&n).is_none());
    }
    #[test]
    fn continuity_stages_and_label() {
        let mut n = night();
        n.list.interruptions = 3;
        n.stages = Some(crate::sleep_stages::SleepStageEstimate {
            start: String::new(),
            epoch_seconds: 30,
            stages: vec![],
            minutes: crate::sleep_stages::StageMinutes {
                wake: 42.,
                light: 245.7,
                deep: 66.15,
                rem: 66.15,
            },
            signal_quality: 1.,
            estimator_version: 1,
        });
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
            15.
        );
        assert_eq!(score.label, "Great");
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
            (30., 1.9),
            (240., 15.),
            (330., 22.5),
            (420., 30.),
            (600., 30.),
        ] {
            n.list.usage_minutes = minutes;
            assert_eq!(points(&n, "duration"), expected);
        }
        n.list.usage_minutes = 420.;
        for (ahi, expected) in [
            (0., 30.),
            (1., 30.),
            (3., 26.),
            (5., 22.),
            (10., 15.),
            (15., 8.),
            (30., 0.),
            (50., 0.),
        ] {
            n.list.ahi = Some(ahi);
            assert_eq!(points(&n, "breathing"), expected);
        }
        n.list.excess_leak_p95 = Some(24.);
        for (share, expected) in [(0., 15.), (0.05, 10.), (0.125, 5.), (0.2, 0.), (0.5, 0.)] {
            n.list.large_leak_minutes = Some(share * 420.);
            assert_eq!(points(&n, "maskSeal"), expected);
        }
        n.list.large_leak_minutes = Some(0.);
        n.list.excess_leak_p95 = Some(32.);
        assert_eq!(points(&n, "maskSeal"), 12.5);
        n.list.excess_leak_p95 = Some(40.);
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
        n.stages = Some(crate::sleep_stages::SleepStageEstimate {
            start: String::new(),
            epoch_seconds: 30,
            stages: vec![],
            minutes: crate::sleep_stages::StageMinutes {
                wake: 42.,
                light: 330.75,
                deep: 33.075,
                rem: 14.175,
            },
            signal_quality: 1.,
            estimator_version: 1,
        });
        assert_eq!(points(&n, "sleepStages"), 5.4);
        assert_eq!(points(&n, "continuity"), 10.);
        n.stages.as_mut().unwrap().minutes.wake = 231.;
        assert_eq!(points(&n, "continuity"), 8.);
        n.stages.as_mut().unwrap().minutes.wake = 420.;
        assert_eq!(points(&n, "continuity"), 6.);
    }

    #[test]
    fn present_but_all_wake_stages_score_zero() {
        let mut n = night();
        n.stages = Some(crate::sleep_stages::SleepStageEstimate {
            start: String::new(),
            epoch_seconds: 30,
            stages: vec![],
            minutes: crate::sleep_stages::StageMinutes {
                wake: 420.,
                light: 0.,
                deep: 0.,
                rem: 0.,
            },
            signal_quality: 1.,
            estimator_version: 1,
        });
        assert_eq!(points(&n, "sleepStages"), 0.);
        assert_eq!(sleep_score(&n).unwrap().total, 65);
    }
}
