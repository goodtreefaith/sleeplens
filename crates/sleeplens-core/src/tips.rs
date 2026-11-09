//! Calm guides based on recorded therapy measurements.
use crate::model::{Night, PeriodSummary, Tip};

fn tip(id: &str, severity: &str, title: &str, body: &str, slug: &str) -> Tip {
    Tip {
        id: id.into(),
        severity: severity.into(),
        title: title.into(),
        body: body.into(),
        learn_slug: (!slug.is_empty()).then(|| slug.into()),
        evidence: None,
        evidence_dates: Vec::new(),
    }
}
fn evidence(mut tip: Tip, text: String, dates: impl IntoIterator<Item = String>) -> Tip {
    tip.evidence = Some(text);
    tip.evidence_dates = dates.into_iter().collect();
    tip.evidence_dates.sort_unstable_by(|a, b| b.cmp(a));
    tip.evidence_dates.dedup();
    tip.evidence_dates.truncate(5);
    tip
}
fn rank(t: &Tip) -> u8 {
    match t.severity.as_str() {
        "talk" => 0,
        "watch" => 1,
        "info" => 2,
        _ => 3,
    }
}
fn finish(mut tips: Vec<Tip>, limit: usize, fallback: Tip) -> Vec<Tip> {
    if tips.is_empty() {
        tips.push(fallback);
    }
    tips.sort_by_key(rank);
    tips.truncate(limit);
    tips
}
fn all_good() -> Tip {
    tip(
        "all-good",
        "good",
        "Keep going",
        "Your recorded therapy measures look good. Keep using your mask for the whole night.",
        "",
    )
}
fn mask_issue(n: &Night) -> bool {
    n.list
        .large_leak_minutes
        .is_some_and(|v| n.list.usage_minutes > 0. && v / n.list.usage_minutes > 0.05)
        || n.list.excess_leak_p95.is_some_and(|v| v > 24.)
}
fn central_issue(n: &Night) -> bool {
    let c = &n.list.event_counts;
    let total = c.obstructive + c.central + c.hypopnea;
    n.summary.cai.is_some_and(|v| v > 5.)
        || (n.list.ahi.is_some_and(|v| v > 5.) && c.central > total / 2)
}
fn prescription_tip() -> Tip {
    tip("prescription-changed","info","Compare before and after","The recorded mode or pressure prescription changed. Compare the nights before and after the change with your clinician.","pressure")
}
/// `recent_nights` may include this night; the last seven distinct recorded nights count.
pub fn night_tips(night: &Night, recent_nights: &[Night]) -> Vec<Tip> {
    let n = &night.list;
    let mut tips = Vec::new();
    let night_evidence = |tip, text| evidence(tip, text, [n.date.clone()]);
    if n.usage_minutes < 240. {
        tips.push(night_evidence(tip("short-night","watch","Aim for the whole night","Keep your mask on for the whole night when you can. If Auto ON is already enabled, it can help when putting the mask back on.","validNight"), format!("{:.0} minutes of recorded use, below 4 hours", n.usage_minutes)));
    }
    if mask_issue(night) {
        let mut values = Vec::new();
        if let Some(minutes) = n.large_leak_minutes {
            values.push(format!("{minutes:.1} minutes of large leak"));
        }
        if let Some(p95) = n.excess_leak_p95 {
            values.push(format!("excess-leak p95 {p95:.1} L/min"));
        }
        tips.push(night_evidence(tip("mask-fit","watch","Check your mask fit","Check how your mask fits and look for cushion wear. A comfortable seal can reduce leaks.","excessLeak"), values.join(", ")));
    }
    let mut recorded = std::collections::BTreeMap::new();
    for recent in recent_nights.iter().chain(std::iter::once(night)) {
        if recent.list.date <= n.date {
            recorded.insert(recent.list.date.clone(), recent.list.ahi);
        }
    }
    let high_dates: Vec<_> = recorded
        .iter()
        .rev()
        .take(7)
        .filter(|(_, ahi)| ahi.is_some_and(|v| v >= 5.))
        .map(|(date, _)| date.clone())
        .collect();
    if n.ahi.is_some_and(|v| v > 15.) || high_dates.len() >= 3 {
        let t = tip(
            "ahi-talk",
            "talk",
            "Discuss breathing events",
            "Contact your clinician about the breathing events recorded during therapy.",
            "ahi",
        );
        if n.ahi.is_some_and(|v| v > 15.) {
            tips.push(night_evidence(
                t,
                format!("AHI {:.1} events per hour, above 15", n.ahi.unwrap()),
            ));
        } else {
            tips.push(evidence(
                t,
                format!(
                    "AHI at least 5 on {} of the last {} recorded nights; this night's AHI {}",
                    high_dates.len(),
                    recorded.len().min(7),
                    n.ahi
                        .map_or_else(|| "not recorded".into(), |v| format!("{v:.1}"))
                ),
                high_dates,
            ));
        }
    } else if n.ahi.is_some_and(|v| (5. ..=15.).contains(&v)) {
        tips.push(night_evidence(tip("ahi-watch","watch","Watch breathing events","Breathing events were above the usual guide of fewer than 5 per hour. Keep an eye on the next few nights.","ahi"), format!("AHI {:.1} events per hour", n.ahi.unwrap())));
    }
    if central_issue(night) {
        let c = &n.event_counts;
        let total = c.obstructive + c.central + c.hypopnea;
        let text = match night.summary.cai {
            Some(cai) => format!(
                "Central apnea index {cai:.1}; {} of {total} events were central; AHI {}",
                c.central,
                n.ahi
                    .map_or_else(|| "not recorded".into(), |v| format!("{v:.1}"))
            ),
            None => format!(
                "{} of {total} events were central; AHI {:.1}",
                c.central,
                n.ahi.unwrap_or_default()
            ),
        };
        tips.push(night_evidence(tip("central-apnea", "talk", "Discuss central events", "Contact your clinician about the central breathing events recorded during therapy.", "centralApnea"), text));
    }
    if n.interruptions >= 3 {
        tips.push(night_evidence(tip("interruptions","info","Several mask-off breaks","There were several breaks in mask use during the night. Consider what made you take the mask off.","interruptions"), format!("{} mask-off breaks longer than 1 minute", n.interruptions)));
    }
    if n.prescription_changed {
        tips.push(night_evidence(
            prescription_tip(),
            format!(
                "A recorded mode or pressure prescription change on {}",
                n.date
            ),
        ));
    }
    let mut text = format!("{:.0} minutes of recorded use", n.usage_minutes);
    if let Some(ahi) = n.ahi {
        text.push_str(&format!(", AHI {ahi:.1}"));
    }
    finish(tips, 4, night_evidence(all_good(), text))
}
/// Generate period guides from the summary and distinct nights within its date range.
pub fn period_tips(summary: &PeriodSummary, nights: &[Night]) -> Vec<Tip> {
    let selected: std::collections::BTreeMap<_, _> = nights
        .iter()
        .filter(|n| n.list.date >= summary.from && n.list.date <= summary.to)
        .map(|n| (n.list.date.clone(), n))
        .collect();
    let selected: Vec<_> = selected.into_values().collect();
    let dates = |predicate: fn(&Night) -> bool| {
        selected
            .iter()
            .filter(|n| predicate(n))
            .map(|n| n.list.date.clone())
            .collect::<Vec<_>>()
    };
    let all_dates = || {
        selected
            .iter()
            .map(|n| n.list.date.clone())
            .collect::<Vec<_>>()
    };
    let count_evidence = |tip, text, dates: Vec<String>| evidence(tip, text, dates);
    let mut tips = Vec::new();
    let compliance_text = format!(
        "{} of {} days had more than 4 hours of use ({:.0}%)",
        summary.valid_nights, summary.days_in_range, summary.compliance_pct
    );
    if summary.compliance_pct < 70. {
        tips.push(count_evidence(tip("compliance-watch","watch","Build regular mask use","Aim to use your mask for the whole night, every night. Fewer than 70% of days in this period had more than 4 hours of recorded use.","compliance"), compliance_text, all_dates()));
    } else if summary.compliance_pct >= 90. {
        tips.push(count_evidence(tip("compliance-good","good","Consistent mask use","At least 90% of days in this period had more than 4 hours of recorded use. Keep using your mask for the whole night.","compliance"), compliance_text, dates(|n| n.list.usage_minutes > 240.)));
    }
    if let Some((now, old)) = summary
        .avg_sleep_score
        .zip(summary.previous.avg_sleep_score)
    {
        let delta = now - old;
        let text = format!("Average score {now:.1}, compared with {old:.1} in the previous period; {} scored nights in this period", selected.iter().filter(|n| n.list.sleep_score.is_some()).count());
        let score_dates = dates(|n| n.list.sleep_score.is_some());
        if delta >= 5. {
            tips.push(count_evidence(tip("score-improved","good","Your average score rose","Your average estimated sleep score rose by at least 5 points from the previous period.","sleepScore"), text, score_dates));
        } else if delta <= -5. {
            tips.push(count_evidence(tip("score-declined","info","Your average score fell","Your average estimated sleep score fell by at least 5 points. Compare hours used, leaks and breathing events across the two periods.","sleepScore"), text, score_dates));
        }
    }
    let short = dates(|n| n.list.usage_minutes < 240.);
    if !short.is_empty() {
        tips.push(count_evidence(tip("short-night","watch","Aim for the whole night","Some nights in this period had less than 4 hours of use. Keep your mask on for the whole night when you can; Auto ON can help if it is already enabled.","validNight"), format!("{} nights with less than 4 hours of use", short.len()), short));
    }
    let mask = dates(mask_issue);
    if !mask.is_empty() {
        tips.push(count_evidence(tip("mask-fit","watch","Check your mask fit","Leaks were above the usual guide on some nights in this period. Check your mask fit and look for cushion wear.","excessLeak"), format!("{} nights with large leak over 5% or excess-leak p95 over 24 L/min", mask.len()), mask));
    }
    let high = dates(|n| n.list.ahi.is_some_and(|v| v > 5.));
    let severe = dates(|n| n.list.ahi.is_some_and(|v| v > 15.));
    let recorded: std::collections::BTreeMap<_, _> = nights
        .iter()
        .map(|n| (n.list.date.clone(), n.list.ahi))
        .collect();
    let recurring = selected.iter().rev().find_map(|night| {
        let recent: Vec<_> = recorded
            .range(..=night.list.date.clone())
            .rev()
            .take(7)
            .collect();
        let high: Vec<_> = recent
            .iter()
            .filter(|(_, ahi)| ahi.is_some_and(|v| v >= 5.))
            .map(|(date, _)| (*date).clone())
            .collect();
        (high.len() >= 3).then(|| (high, recent.len(), night.list.date.clone()))
    });
    if !severe.is_empty() || recurring.is_some() {
        let (text, relevant) = if !severe.is_empty() {
            (
                format!("AHI above 15 on {} nights in this period", severe.len()),
                severe,
            )
        } else {
            let (recurrent, recent_count, date) = recurring.unwrap();
            (format!("AHI at least 5 on {} of the last {recent_count} recorded nights through {date}, a night in this period", recurrent.len()), recurrent)
        };
        tips.push(count_evidence(
            tip(
                "ahi-talk",
                "talk",
                "Discuss breathing events",
                "Contact your clinician about the breathing events recorded in this period.",
                "ahi",
            ),
            text,
            relevant,
        ));
    } else if high.len() >= 3 || summary.avg_ahi.is_some_and(|v| v > 5.) {
        let text = format!(
            "AHI above 5 on {} nights in this period; average AHI {}",
            high.len(),
            summary
                .avg_ahi
                .map_or_else(|| "not recorded".into(), |v| format!("{v:.1}"))
        );
        tips.push(count_evidence(tip("ahi-watch","watch","Watch breathing events","Breathing events were above the usual guide during this period. Compare the affected nights for patterns.","ahi"), text, high));
    }
    let central = dates(central_issue);
    if !central.is_empty() {
        tips.push(count_evidence(tip("central-apnea","talk","Discuss central events","Contact your clinician about the central breathing events recorded in this period.","centralApnea"), format!("{} nights with central apnea index above 5 or most events central and AHI above 5", central.len()), central));
    }
    let interrupted = dates(|n| n.list.interruptions >= 3);
    if !interrupted.is_empty() {
        tips.push(count_evidence(tip("interruptions","info","Several mask-off breaks","Some nights in this period had several mask-off breaks. Compare those nights and consider what made you take the mask off.","interruptions"), format!("{} nights with at least 3 mask-off breaks", interrupted.len()), interrupted));
    }
    let changed = dates(|n| n.list.prescription_changed);
    if !changed.is_empty() {
        tips.push(count_evidence(
            prescription_tip(),
            format!(
                "{} nights with a recorded mode or pressure prescription change",
                changed.len()
            ),
            changed,
        ));
    }
    finish(
        tips,
        5,
        count_evidence(
            all_good(),
            format!(
                "{} nights used out of {} days in this period",
                summary.nights, summary.days_in_range
            ),
            all_dates(),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::tests::night;

    fn summary() -> PeriodSummary {
        serde_json::from_value(serde_json::json!({
            "period":"week", "from":"2026-01-01", "to":"2026-01-07", "daysInRange":7,
            "nights":7, "validNights":7, "compliancePct":100., "avgUsageMinutes":420.,
            "avgAhi":1., "avgSleepScore":80., "avgExcessLeakP95":0., "avgPressureP90":null,
            "avgStageMinutes":null, "previous":{}, "bestNight":null, "worstNight":null,
            "scoreSeries":[], "tips":[]
        }))
        .unwrap()
    }
    fn nights(ahis: &[f64]) -> Vec<Night> {
        ahis.iter()
            .enumerate()
            .map(|(i, ahi)| {
                let mut n = night();
                n.list.date = format!("2026-01-{:02}", i + 1);
                n.list.ahi = Some(*ahi);
                n
            })
            .collect()
    }
    #[test]
    fn night_evidence_and_severity_order() {
        let mut n = night();
        n.list.usage_minutes = 100.;
        n.list.ahi = Some(16.);
        n.list.excess_leak_p95 = Some(25.);
        n.list.interruptions = 3;
        let tips = night_tips(&n, &[]);
        assert_eq!(tips.len(), 4);
        assert_eq!(tips[0].id, "ahi-talk");
        assert!(tips[0].body.contains("clinician"));
        assert!(tips
            .iter()
            .all(|t| t.evidence.is_some() && t.evidence_dates == [n.list.date.clone()]));
        assert!(tips[0].evidence.as_ref().unwrap().contains("16.0"));
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
    fn good_and_central_guides_have_numeric_evidence() {
        let mut n = night();
        let good = night_tips(&n, &[]);
        assert_eq!(good[0].id, "all-good");
        assert!(good[0].evidence.as_ref().unwrap().contains("minutes"));
        n.summary.cai = Some(5.1);
        let central = night_tips(&n, &[]);
        assert_eq!(central[0].id, "central-apnea");
        assert!(central[0].evidence.as_ref().unwrap().contains("5.1"));
    }
    #[test]
    fn period_watch_requires_pattern_or_elevated_average() {
        let mut summary = summary();
        let isolated = nights(&[6., 6., 0., 0., 0., 0., 0.]);
        assert!(!period_tips(&summary, &isolated)
            .iter()
            .any(|t| t.id.starts_with("ahi-")));
        summary.avg_ahi = Some(6.);
        let tips = period_tips(&summary, &isolated);
        let watch = tips.iter().find(|t| t.id == "ahi-watch").unwrap();
        assert!(watch.evidence.as_ref().unwrap().contains("2 nights"));
        assert_eq!(watch.evidence_dates, ["2026-01-02", "2026-01-01"]);
        assert!(!watch.body.contains("next few nights"));
        let high = nights(&[6., 6., 6., 0., 0., 0., 0.]);
        assert!(period_tips(&summary, &high)
            .iter()
            .any(|t| t.id == "ahi-talk"));
    }
    #[test]
    fn period_watch_can_count_three_nights_without_rolling_recurrence() {
        let mut summary = summary();
        summary.to = "2026-01-20".into();
        let mut values = vec![0.; 20];
        values[0] = 6.;
        values[8] = 6.;
        values[16] = 6.;
        let tips = period_tips(&summary, &nights(&values));
        assert!(tips.iter().any(|t| t.id == "ahi-watch"));
        assert!(!tips.iter().any(|t| t.id == "ahi-talk"));
    }
    #[test]
    fn period_talk_keeps_rolling_context_and_excludes_future() {
        let mut summary = summary();
        summary.from = "2026-01-03".into();
        summary.to = "2026-01-03".into();
        let tips = period_tips(&summary, &nights(&[6., 6., 6., 16.]));
        let talk = tips.iter().find(|t| t.id == "ahi-talk").unwrap();
        assert_eq!(
            talk.evidence_dates,
            ["2026-01-03", "2026-01-02", "2026-01-01"]
        );
        let tips = period_tips(&summary, &nights(&[0., 0., 6., 16.]));
        assert!(!tips.iter().any(|t| t.id == "ahi-talk"));
    }
    #[test]
    fn period_evidence_dates_are_newest_first_and_capped() {
        let mut records = nights(&[1.; 7]);
        for n in &mut records {
            n.list.excess_leak_p95 = Some(25.);
        }
        let tips = period_tips(&summary(), &records);
        assert!(tips.iter().all(|t| t.evidence.is_some()));
        let mask = tips.iter().find(|t| t.id == "mask-fit").unwrap();
        assert!(mask.evidence.as_ref().unwrap().contains("7 nights"));
        assert_eq!(
            mask.evidence_dates,
            [
                "2026-01-07",
                "2026-01-06",
                "2026-01-05",
                "2026-01-04",
                "2026-01-03"
            ]
        );
        let json = serde_json::to_value(mask).unwrap();
        assert!(json.get("evidenceDates").is_some());
        assert!(json.get("evidence_dates").is_none());
    }
    #[test]
    fn period_compliance_comparison_and_prescription_guides() {
        let mut summary = summary();
        summary.compliance_pct = 0.;
        summary.avg_sleep_score = Some(60.);
        summary.previous.avg_sleep_score = Some(65.);
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
        let mut recent = nights(&[6., 6., 6., 0., 0., 0., 0., 0., 0.]);
        let mut future = n.clone();
        future.list.date = "2026-02-01".into();
        recent.push(future);
        assert_eq!(night_tips(&n, &recent)[0].id, "ahi-watch");
    }
}
