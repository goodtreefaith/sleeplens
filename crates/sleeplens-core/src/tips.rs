//! Calm guides based on recorded therapy measurements.
use crate::model::{Night, PeriodSummary, Tip};
fn tip(id: &str, severity: &str, title: &str, body: &str, slug: &str) -> Tip {
    Tip {
        id: id.into(),
        severity: severity.into(),
        title: title.into(),
        body: body.into(),
        learn_slug: (!slug.is_empty()).then(|| slug.into()),
    }
}
fn rank(t: &Tip) -> u8 {
    match t.severity.as_str() {
        "talk" => 0,
        "watch" => 1,
        "info" => 2,
        _ => 3,
    }
}
fn finish(mut tips: Vec<Tip>, limit: usize) -> Vec<Tip> {
    if tips.is_empty() {
        tips.push(tip(
            "all-good",
            "good",
            "Keep going",
            "Your recorded therapy measures look good. Keep using your mask for the whole night.",
            "",
        ));
    }
    tips.sort_by_key(rank);
    tips.truncate(limit);
    tips
}
/// `recent_nights` may include this night; the last seven distinct recorded nights count.
pub fn night_tips(night: &Night, recent_nights: &[Night]) -> Vec<Tip> {
    let n = &night.list;
    let mut tips = Vec::new();
    if n.usage_minutes < 240. {
        tips.push(tip("short-night","watch","Aim for the whole night","Keep your mask on for the whole night when you can. If Auto ON is already enabled, it can help when putting the mask back on.","validNight"));
    }
    if n.large_leak_minutes
        .is_some_and(|v| n.usage_minutes > 0. && v / n.usage_minutes > 0.05)
        || n.excess_leak_p95.is_some_and(|v| v > 24.)
    {
        tips.push(tip("mask-fit","watch","Check your mask fit","Check how your mask fits and look for cushion wear. A comfortable seal can reduce leaks.","excessLeak"));
    }
    let mut recorded = std::collections::BTreeMap::new();
    for recent in recent_nights.iter().chain(std::iter::once(night)) {
        if recent.list.date <= n.date {
            recorded.insert(&recent.list.date, recent.list.ahi);
        }
    }
    let high_count = recorded
        .values()
        .rev()
        .take(7)
        .filter(|ahi| ahi.is_some_and(|v| v >= 5.))
        .count();
    if n.ahi.is_some_and(|v| v > 15.) || high_count >= 3 {
        tips.push(tip(
            "ahi-talk",
            "talk",
            "Discuss breathing events",
            "Contact your clinician about the breathing events recorded during therapy.",
            "ahi",
        ));
    } else if n.ahi.is_some_and(|v| (5. ..=15.).contains(&v)) {
        tips.push(tip("ahi-watch","watch","Watch breathing events","Breathing events were above the usual guide of fewer than 5 per hour. Keep an eye on the next few nights.","ahi"));
    }
    let c = &n.event_counts;
    let total = c.obstructive + c.central + c.hypopnea;
    if night.summary.cai.is_some_and(|v| v > 5.)
        || (n.ahi.is_some_and(|v| v > 5.) && c.central > total / 2)
    {
        tips.push(tip(
            "central-apnea",
            "talk",
            "Discuss central events",
            "Contact your clinician about the central breathing events recorded during therapy.",
            "centralApnea",
        ));
    }
    if n.interruptions >= 3 {
        tips.push(tip("interruptions","info","Several mask-off breaks","There were several breaks in mask use during the night. Consider what made you take the mask off.","interruptions"));
    }
    if n.prescription_changed {
        tips.push(prescription_tip());
    }
    finish(tips, 4)
}
fn prescription_tip() -> Tip {
    tip("prescription-changed","info","Compare before and after","The recorded mode or pressure prescription changed. Compare the nights before and after the change with your clinician.","pressure")
}
/// Generate period guides from the summary and nights within its date range.
pub fn period_tips(summary: &PeriodSummary, nights: &[Night]) -> Vec<Tip> {
    let mut tips = Vec::new();
    if summary.compliance_pct < 70. {
        tips.push(tip("compliance-watch","watch","Build regular mask use","Aim to use your mask for the whole night, every night. Fewer than 70% of days had more than 4 hours of recorded use.","compliance"));
    } else if summary.compliance_pct >= 90. {
        tips.push(tip("compliance-good","good","Consistent mask use","At least 90% of days had more than 4 hours of recorded use. Keep using your mask for the whole night.","compliance"));
    }
    if let Some(delta) = summary
        .avg_sleep_score
        .zip(summary.previous.avg_sleep_score)
        .map(|(now, old)| now - old)
    {
        if delta >= 5. {
            tips.push(tip("score-improved","good","Your average score rose","Your average estimated sleep score rose by at least 5 points from the previous period.","sleepScore"));
        } else if delta <= -5. {
            tips.push(tip("score-declined","info","Your average score fell","Your average estimated sleep score fell by at least 5 points. Compare hours used, leaks and breathing events across the two periods.","sleepScore"));
        }
    }
    let selected: Vec<_> = nights
        .iter()
        .filter(|n| n.list.date >= summary.from && n.list.date <= summary.to)
        .collect();
    for n in &selected {
        for t in night_tips(n, nights) {
            if t.id != "all-good"
                && t.id != "prescription-changed"
                && !tips.iter().any(|existing: &Tip| existing.id == t.id)
            {
                tips.push(t);
            }
        }
    }
    if selected.iter().any(|n| n.list.prescription_changed) {
        tips.push(prescription_tip());
    }
    finish(tips, 5)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::tests::night;
    #[test]
    fn calm_guides_and_severity_order() {
        let mut n = night();
        n.list.usage_minutes = 100.;
        n.list.ahi = Some(16.);
        n.list.excess_leak_p95 = Some(25.);
        n.list.interruptions = 3;
        let t = night_tips(&n, &[]);
        assert_eq!(t.len(), 4);
        assert_eq!(t[0].id, "ahi-talk");
        assert!(t[0].body.contains("clinician"));
        assert!(t.iter().any(|t| t.id == "mask-fit"));
    }
    #[test]
    fn recurrent_ahi_counts_distinct_recent_dates() {
        let mut n = night();
        n.list.ahi = Some(6.);
        n.list.date = "2026-01-07".into();
        let mut recent = vec![n.clone(), n.clone()];
        assert_eq!(night_tips(&n, &recent)[0].id, "ahi-watch");
        recent[0].list.date = "2026-01-06".into();
        recent[1].list.date = "2025-12-01".into();
        assert_eq!(night_tips(&n, &recent)[0].id, "ahi-talk");
    }
    #[test]
    fn good_and_central_guides() {
        let mut n = night();
        assert_eq!(night_tips(&n, &[])[0].id, "all-good");
        n.summary.cai = Some(5.1);
        assert_eq!(night_tips(&n, &[])[0].id, "central-apnea");
    }
    #[test]
    fn period_compliance_comparison_and_prescription_guides() {
        let mut summary: PeriodSummary=serde_json::from_value(serde_json::json!({"period":"week","from":"2026-01-01","to":"2026-01-07","daysInRange":7,"nights":1,"validNights":0,"compliancePct":0.,"avgUsageMinutes":100.,"avgAhi":null,"avgSleepScore":60.,"avgExcessLeakP95":null,"avgPressureP90":null,"avgStageMinutes":null,"previous":{"avgSleepScore":65.},"bestNight":null,"worstNight":null,"scoreSeries":[],"tips":[]})).unwrap();
        let mut n = night();
        n.list.prescription_changed = true;
        let tips = period_tips(&summary, &[n]);
        assert_eq!(tips[0].id, "compliance-watch");
        assert!(tips.iter().any(|t| t.id == "score-declined"));
        assert!(tips.iter().any(|t| t.id == "prescription-changed"));
        summary.compliance_pct = 90.;
        summary.avg_sleep_score = Some(70.);
        let tips = period_tips(&summary, &[]);
        assert!(tips.iter().any(|t| t.id == "compliance-good"));
        assert!(tips.iter().any(|t| t.id == "score-improved"));
    }
    #[test]
    fn recurrence_excludes_future_and_only_last_seven_recorded_nights() {
        let mut n = night();
        n.list.date = "2026-01-10".into();
        n.list.ahi = Some(6.);
        let mut recent: Vec<_> = (1..=9)
            .map(|day| {
                let mut r = n.clone();
                r.list.date = format!("2026-01-{day:02}");
                r.list.ahi = Some(if day < 4 { 6. } else { 0. });
                r
            })
            .collect();
        let mut future = n.clone();
        future.list.date = "2026-02-01".into();
        recent.push(future);
        assert_eq!(night_tips(&n, &recent)[0].id, "ahi-watch");
    }
}
