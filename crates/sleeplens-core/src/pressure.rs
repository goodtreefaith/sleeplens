//! Descriptive pressure observations. These never prescribe settings.
use crate::model::{Night, PressureSummary, Tip};
use serde::{Deserialize, Serialize};

const NEAR: f64 = 0.5;
const P90_NEAR: f64 = 0.25;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NightPressureFacts {
    pub setting_min: Option<f64>,
    pub setting_max: Option<f64>,
    pub p90: Option<f64>,
    pub observed_max: Option<f64>,
    pub share_at_max_pct: Option<f64>,
    pub share_at_min_pct: Option<f64>,
    pub obstructive_event_pressures: Vec<f64>,
    pub hypopnea_event_pressures: Vec<f64>,
    pub central_event_pressures: Vec<f64>,
    pub events_at_min: usize,
    pub high_pressure_leak_mean: Option<f64>,
    pub low_pressure_leak_mean: Option<f64>,
}
fn finite(v: Option<f64>) -> Option<f64> {
    v.filter(|v| v.is_finite())
}
fn mean(values: impl IntoIterator<Item = f64>) -> Option<f64> {
    let (sum, count) = values
        .into_iter()
        .filter(|v| v.is_finite())
        .fold((0., 0), |(s, c), v| (s + v, c + 1));
    (count > 0).then(|| sum / count as f64)
}
fn near(value: f64, boundary: f64) -> bool {
    (value - boundary).abs() <= NEAR
}
fn near_max(f: &NightPressureFacts) -> bool {
    f.p90
        .zip(f.setting_max)
        .is_some_and(|(p, max)| p >= max - P90_NEAR)
        || f.share_at_max_pct.is_some_and(|share| share >= 10.)
}
fn near_min(f: &NightPressureFacts) -> bool {
    f.p90
        .zip(f.setting_min)
        .is_some_and(|(p, min)| p <= min + P90_NEAR)
        || f.share_at_min_pct.is_some_and(|share| share >= 50.)
}
fn apap(n: &Night) -> bool {
    !n.intranight_prescription_changed
        && n.settings.as_ref().is_some_and(|s| {
            matches!(s.mode_label.as_str(), "Auto CPAP" | "AUTOS")
                && s.min_pressure.is_finite()
                && s.max_pressure.is_finite()
                && s.min_pressure > 0.
                && s.max_pressure > s.min_pressure
        })
}
/// Pressure samples exist only for recorded mask-on seconds; null gaps and nonfinite
/// values are excluded. Summary P90 is kept as a separate, coarse statistic.
pub fn night_pressure_facts(n: &Night) -> NightPressureFacts {
    let mut f = NightPressureFacts {
        setting_min: n
            .settings
            .as_ref()
            .and_then(|s| finite(Some(s.min_pressure))),
        setting_max: n
            .settings
            .as_ref()
            .and_then(|s| finite(Some(s.max_pressure))),
        p90: finite(n.list.pressure_p90),
        observed_max: finite(n.summary.max_pressure),
        ..Default::default()
    };
    for e in &n.events {
        // Backup event values encode pressure in half-unit increments.
        if e.value == 0 {
            continue;
        }
        let p = f64::from(e.value) * 0.5;
        match e.kind.as_str() {
            "obstructiveApnea" => f.obstructive_event_pressures.push(p),
            "hypopnea" => f.hypopnea_event_pressures.push(p),
            "centralApnea" => f.central_event_pressures.push(p),
            _ => {}
        }
    }
    if let Some(min) = f.setting_min {
        f.events_at_min = f
            .obstructive_event_pressures
            .iter()
            .chain(&f.hypopnea_event_pressures)
            .filter(|&&p| near(p, min))
            .count();
    }
    if let Some(s) = &n.series {
        let pressures: Vec<_> = s
            .pressure
            .iter()
            .filter_map(|v| finite(v.map(f64::from)))
            .filter(|v| *v > 0.)
            .collect();
        if let Some(max) = pressures.iter().copied().reduce(f64::max) {
            f.observed_max = Some(max);
        }
        if !pressures.is_empty() {
            let pct = |limit| {
                100. * pressures.iter().filter(|&&p| near(p, limit)).count() as f64
                    / pressures.len() as f64
            };
            f.share_at_min_pct = f.setting_min.map(pct);
            f.share_at_max_pct = f.setting_max.map(pct);
        }
        if let Some((min, max)) = f
            .setting_min
            .zip(f.setting_max)
            .filter(|(min, max)| max > min)
        {
            let mut high = Vec::new();
            let mut low = Vec::new();
            for (pressure, leak) in s.pressure.iter().zip(&s.excess_leak) {
                let Some((p, l)) = finite(pressure.map(f64::from)).zip(finite(leak.map(f64::from)))
                else {
                    continue;
                };
                if p <= 0. || l < 0. {
                    continue;
                }
                if p >= min + (max - min) * 2. / 3. {
                    high.push(l);
                } else if p <= min + (max - min) / 3. {
                    low.push(l);
                }
            }
            if high.len() >= 60 && low.len() >= 60 {
                f.high_pressure_leak_mean = mean(high);
                f.low_pressure_leak_mean = mean(low);
            }
        }
    }
    f
}
/// Mean P90 and time shares are per-night means, event share is pooled by event.
pub fn pressure_summary(nights: &[Night]) -> Option<PressureSummary> {
    let facts: Vec<_> = nights.iter().map(night_pressure_facts).collect();
    pressure_summary_with_facts(nights, &facts)
}
/// Facts must correspond to nights in the same order. Series can be discarded.
pub fn pressure_summary_with_facts(
    nights: &[Night],
    facts: &[NightPressureFacts],
) -> Option<PressureSummary> {
    let pairs = paired(nights, facts);
    let nights: Vec<_> = pairs.iter().map(|(n, _)| *n).collect();
    let facts: Vec<_> = pairs.iter().map(|(_, f)| *f).collect();
    if !facts
        .iter()
        .any(|f| f.setting_min.is_some() || f.p90.is_some() || f.observed_max.is_some())
    {
        return None;
    }
    let latest = nights.iter().rev().find_map(|n| n.settings.as_ref());
    let event_count: usize = facts
        .iter()
        .filter(|f| f.setting_min.is_some())
        .map(|f| f.obstructive_event_pressures.len() + f.hypopnea_event_pressures.len())
        .sum();
    let at_min: usize = facts.iter().map(|f| f.events_at_min).sum();
    Some(PressureSummary {
        setting_min: latest.and_then(|s| finite(Some(s.min_pressure))),
        setting_max: latest.and_then(|s| finite(Some(s.max_pressure))),
        avg_p90: mean(facts.iter().filter_map(|f| f.p90)),
        nights_near_max: facts.iter().filter(|f| near_max(f)).count(),
        nights_near_min: facts.iter().filter(|f| near_min(f)).count(),
        avg_share_at_max_pct: mean(facts.iter().filter_map(|f| f.share_at_max_pct)),
        events_at_min_pct: (event_count > 0).then(|| at_min as f64 / event_count as f64 * 100.),
    })
}
fn obstruction_index(n: &Night) -> Option<f64> {
    finite(n.summary.oai).or_else(|| {
        (n.list.usage_minutes > 0.)
            .then(|| n.list.event_counts.obstructive as f64 * 60. / n.list.usage_minutes)
    })
}
fn obstructive_hypopnea_index(n: &Night) -> Option<f64> {
    let index = |value, count| {
        finite(value).or_else(|| {
            (n.list.usage_minutes > 0.).then(|| count as f64 * 60. / n.list.usage_minutes)
        })
    };
    index(n.summary.oai, n.list.event_counts.obstructive)
        .zip(index(n.summary.hi, n.list.event_counts.hypopnea))
        .map(|(o, h)| o + h)
}
fn few_events(n: &Night) -> bool {
    let raw_rate = (n.list.event_counts.obstructive + n.list.event_counts.hypopnea) as f64 * 60.
        / n.list.usage_minutes;
    match (finite(n.summary.oai), finite(n.summary.hi)) {
        (Some(o), Some(h)) => o + h < 2. && raw_rate < 2.,
        (None, None) => raw_rate < 2. && n.list.ahi.is_some_and(|a| a < 2.),
        _ => false,
    }
}
fn central(n: &Night) -> bool {
    let e = &n.list.event_counts;
    n.summary.cai.is_some_and(|c| c > 5.)
        || (n.list.ahi.is_some_and(|a| a > 5.)
            && e.central > (e.central + e.obstructive + e.hypopnea) / 2)
}
fn leak_fine(n: &Night) -> bool {
    n.list.excess_leak_p95.is_some_and(|l| l <= 24.)
        && n.list
            .large_leak_minutes
            .is_some_and(|l| l / n.list.usage_minutes <= 0.05)
}
fn leak_contrast(high: f64, low: f64) -> bool {
    high > 24. && high - low >= 10.
}
fn make(
    id: &str,
    severity: &str,
    title: &str,
    body: &str,
    evidence: String,
    nights: &[&Night],
) -> Tip {
    let mut dates: Vec<_> = nights.iter().map(|n| n.list.date.clone()).collect();
    dates.sort_unstable_by(|a, b| b.cmp(a));
    dates.dedup();
    dates.truncate(5);
    Tip {
        category: "pressure".into(),
        id: id.into(),
        severity: severity.into(),
        title: title.into(),
        body: body.into(),
        learn_slug: Some("pressure".into()),
        evidence: Some(evidence),
        evidence_dates: dates,
    }
}
fn used_nights(count: usize) -> String {
    format!(
        "{count} used {}",
        if count == 1 { "night" } else { "nights" }
    )
}
fn rank(t: &Tip) -> u8 {
    match t.severity.as_str() {
        "talk" => 0,
        "watch" => 1,
        "info" => 2,
        _ => 3,
    }
}
fn paired<'a>(
    nights: &'a [Night],
    facts: &'a [NightPressureFacts],
) -> Vec<(&'a Night, &'a NightPressureFacts)> {
    nights
        .iter()
        .zip(facts)
        .filter(|(n, _)| n.list.usage_minutes > 0.)
        .map(|(n, f)| (n.list.date.clone(), (n, f)))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_values()
        .collect()
}
fn generate(nights: Vec<&Night>, facts: Vec<&NightPressureFacts>, period: bool) -> Vec<Tip> {
    if nights.is_empty() || (period && nights.len() < 7) {
        return Vec::new();
    }
    let eligible: Vec<_> = nights.iter().zip(&facts).filter(|(n, _)| apap(n)).collect();
    let ceiling: Vec<_> = eligible
        .iter()
        .filter(|(_, f)| near_max(f))
        .map(|(n, _)| **n)
        .collect();
    let floor: Vec<_> = eligible
        .iter()
        .filter(|(_, f)| near_min(f))
        .map(|(n, _)| **n)
        .collect();
    let mut tips = Vec::new();
    let phrase = if period {
        "in this period"
    } else {
        "during this night"
    };
    let central_nights: Vec<_> = nights.iter().copied().filter(|n| central(n)).collect();
    if !central_nights.is_empty() {
        tips.push(make("central-with-pressure","talk","Discuss central events and pressure","Central breathing events can be related to pressure or other causes. Ask your clinician to review these events and your therapy.",format!("{} of {} had central apnea index above 5, or most events central with AHI above 5",central_nights.len(),used_nights(nights.len())),&central_nights));
    }
    if ceiling.len() as f64 / nights.len() as f64 >= 0.30 {
        if mean(ceiling.iter().filter_map(|n| finite(n.list.ahi))).is_some_and(|a| a >= 5.)
            || mean(ceiling.iter().filter_map(|n| obstruction_index(n))).is_some_and(|a| a >= 2.)
        {
            tips.push(make("pressure-ceiling-events","talk","Events while pressure was near its maximum",&format!("Pressure often reached the top of its recorded range while breathing events were recorded {phrase}. Ask your clinician whether the prescribed maximum still suits you."),format!("{} of {} {} near the maximum; average AHI {} and average obstructive index {} for this evidence",ceiling.len(),used_nights(nights.len()), if ceiling.len() == 1 { "was" } else { "were" }, mean(ceiling.iter().filter_map(|n| finite(n.list.ahi))).map_or_else(|| "not recorded".into(), |v| format!("{v:.1}")), mean(ceiling.iter().filter_map(|n| obstruction_index(n))).map_or_else(|| "not recorded".into(), |v| format!("{v:.1}"))),&ceiling));
        } else if ceiling.iter().all(|n| n.list.ahi.is_some_and(|a| a < 5.))
            && (period
                || facts
                    .iter()
                    .all(|f| near_max(f) && f.share_at_max_pct.is_some_and(|share| share >= 20.)))
        {
            tips.push(make("pressure-ceiling","info","Pressure used the top of its range",&format!("The machine often used the top of its recorded range {phrase}, with AHI below 5. Mention this at your next clinician review."),format!("{} of {} {} near the maximum, with AHI below 5",ceiling.len(),used_nights(nights.len()), if ceiling.len() == 1 { "was" } else { "were" }),&ceiling));
        }
    }
    let floor_events: Vec<_> = eligible
        .iter()
        .filter(|(_, f)| {
            let count = f.obstructive_event_pressures.len() + f.hypopnea_event_pressures.len();
            count >= 5 && f.events_at_min as f64 / count as f64 >= 0.30
        })
        .map(|(n, _)| **n)
        .collect();
    let low_use = floor.len() as f64 / nights.len() as f64 >= 0.70
        && nights
            .iter()
            .all(|n| n.list.ahi.is_some_and(|a| a < 5.) && few_events(n));
    let mut floor_watch = false;
    if period && floor_events.len() >= 3 {
        let burden = mean(
            floor_events
                .iter()
                .filter_map(|n| obstructive_hypopnea_index(n)),
        );
        let ahi = mean(floor_events.iter().filter_map(|n| finite(n.list.ahi)));
        let severity =
            if burden.is_some_and(|index| index >= 3.) || ahi.is_some_and(|index| index >= 5.) {
                Some("watch")
            } else if burden.is_some_and(|index| index >= 1.) {
                Some("info")
            } else {
                None
            };
        floor_watch = severity == Some("watch");
        if let Some(severity) = severity.filter(|severity| *severity == "watch" || !low_use) {
            let body = if severity == "watch" {
                "Obstructive apneas or hypopneas often happened near the recorded minimum, including any ramp time at that pressure. Ask your clinician whether the minimum and start settings still suit you."
            } else {
                "Some breathing events happened at the lowest pressure, often early in the night. Mention this at your next review."
            };
            tips.push(make("pressure-floor-events",severity,"Events near the minimum pressure",body,format!("{} of {} had at least 5 recorded obstructive apnea and hypopnea event pressures, with at least 30% near the minimum; average obstructive apnea plus hypopnea index {} and average AHI {} for this evidence",floor_events.len(),used_nights(nights.len()),burden.map_or_else(|| "not recorded".into(), |v| format!("{v:.1}")),ahi.map_or_else(|| "not recorded".into(), |v| format!("{v:.1}"))),&floor_events));
        }
    }
    if low_use && !floor_watch {
        tips.push(make("pressure-low-use","info","Pressure stayed near the minimum","Pressure rarely moved beyond the recorded minimum and few breathing events were recorded. Ask at your next review whether this range still suits you.",format!("{} of {} {} near the minimum; each used night had AHI below 5 and obstructive apnea plus hypopnea index below 2",floor.len(),used_nights(nights.len()), if floor.len() == 1 { "was" } else { "were" }),&nights));
    }
    let stretch_leaks: Vec<_> = nights
        .iter()
        .zip(&facts)
        .filter(|(_, f)| {
            f.high_pressure_leak_mean
                .zip(f.low_pressure_leak_mean)
                .is_some_and(|(h, l)| leak_contrast(h, l))
        })
        .map(|(n, _)| *n)
        .collect();
    let (high_nights, low_nights): (Vec<_>, Vec<_>) = eligible
        .iter()
        .filter(|(_, f)| {
            f.p90
                .zip(f.setting_min.zip(f.setting_max))
                .is_some_and(|(p, (min, max))| {
                    p >= min + (max - min) * 2. / 3. || p <= min + (max - min) / 3.
                })
        })
        .partition(|(_, f)| {
            f.p90
                .zip(f.setting_min.zip(f.setting_max))
                .is_some_and(|(p, (min, max))| p >= min + (max - min) * 2. / 3.)
        });
    let p95 = |ns: &[&(&&Night, &&NightPressureFacts)]| {
        mean(
            ns.iter()
                .filter_map(|(n, _)| finite(n.list.excess_leak_p95)),
        )
    };
    let share = |ns: &[&(&&Night, &&NightPressureFacts)]| {
        mean(ns.iter().filter_map(|(n, _)| {
            n.list
                .large_leak_minutes
                .map(|l| l / n.list.usage_minutes * 100.)
        }))
    };
    let nightly_leak = period
        && p95(&high_nights)
            .zip(p95(&low_nights))
            .is_some_and(|(h, l)| leak_contrast(h, l));
    let nightly_large = period
        && share(&high_nights)
            .zip(share(&low_nights))
            .is_some_and(|(h, l)| h > 5. && h - l >= 5.);
    if !stretch_leaks.is_empty() || nightly_leak || nightly_large {
        let dates = if !nightly_leak && !nightly_large {
            stretch_leaks
        } else {
            high_nights
                .iter()
                .chain(&low_nights)
                .map(|(n, _)| **n)
                .collect()
        };
        let detail = if nightly_large {
            "Average large-leak share was above 5% on high-pressure nights and at least 5 percentage points above comparison nights"
        } else if nightly_leak {
            "Average excess-leak p95 was above 24 L/min on high-pressure nights and at least 10 L/min above comparison nights"
        } else {
            "Mean excess leak was above 24 L/min in high-pressure stretches and at least 10 L/min above low-pressure stretches, with at least 60 recorded seconds in each group"
        };
        tips.push(make("leak-at-high-pressure","watch","More leak with high pressure","Leak was greater when pressure was high. Check mask fit with your equipment provider or clinician and discuss this pattern.",detail.into(),&dates));
    }
    if tips.is_empty()
        && !eligible.is_empty()
        && eligible.len() == nights.len()
        && eligible.iter().all(|(n, f)| {
            n.list.ahi.is_some_and(|a| a < 5.)
                && leak_fine(n)
                && f.p90
                    .zip(f.setting_min.zip(f.setting_max))
                    .is_some_and(|(p, (min, max))| p >= min && p <= max)
                && !near_max(f)
        })
    {
        tips.push(make("pressure-ok","good","The pressure range looks suitable","Recorded breathing events and leak were within the usual guides. The pressure range looks like it is doing its job; keep your regular clinician reviews.",format!("{} had AHI below 5, P90 inside the range and neither P90 nor recorded time near the maximum, excess-leak p95 at most 24 L/min and large-leak share at most 5%",used_nights(nights.len())),&nights));
    }
    tips.sort_by_key(rank);
    tips.truncate(2);
    tips
}
/// At most two pressure observations from this night's evidence.
pub fn night_pressure_tips(night: &Night) -> Vec<Tip> {
    let facts = night_pressure_facts(night);
    if night.list.usage_minutes <= 0. {
        return Vec::new();
    }
    generate(vec![night], vec![&facts], false)
}
/// At least seven distinct nights with use are needed for period observations.
pub fn period_pressure_tips(nights: &[Night]) -> Vec<Tip> {
    let facts: Vec<_> = nights.iter().map(night_pressure_facts).collect();
    period_pressure_tips_with_facts(nights, &facts)
}
/// Facts must correspond to nights in the same order. Series can be discarded.
pub fn period_pressure_tips_with_facts(nights: &[Night], facts: &[NightPressureFacts]) -> Vec<Tip> {
    let pairs = paired(nights, facts);
    generate(
        pairs.iter().map(|(n, _)| *n).collect(),
        pairs.iter().map(|(_, f)| *f).collect(),
        true,
    )
}

/// Reference selection avoids copying full series when the caller has a wider history.
pub(crate) fn period_pressure_tips_in_range(nights: &[Night], from: &str, to: &str) -> Vec<Tip> {
    let selected: Vec<_> = nights
        .iter()
        .filter(|n| {
            n.list.usage_minutes > 0. && n.list.date.as_str() >= from && n.list.date.as_str() <= to
        })
        .map(|n| (n.list.date.as_str(), n))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_values()
        .collect();
    let facts: Vec<_> = selected.iter().map(|n| night_pressure_facts(n)).collect();
    generate(selected, facts.iter().collect(), true)
}
