//! Offline diagnosis using the existing PSG reader; no datasets are embedded.
use sleeplens_core::sleep_stages::{
    sleep_stage_debug, sleep_stage_epoch_features, SleepStageInput,
};
use sleeplens_psg::{
    runner::{self, Layout},
    StageLabel,
};
use std::{collections::BTreeMap, path::PathBuf};

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    let dir = args.get(1).ok_or("expected directory and layout")?;
    let layout = Layout::parse(args.get(2).ok_or("expected layout")?).ok_or("bad layout")?;
    let only = args.get(3);
    let (files, _) = runner::discover(&PathBuf::from(dir), layout)?;
    let mut groups: BTreeMap<&str, Vec<[f64; 14]>> = BTreeMap::new();
    let mut gates: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    let p = sleeplens_core::sleep_stages::default_params();
    for file in files
        .iter()
        .filter(|f| only.is_none_or(|id| f.id.contains(id)))
    {
        let night = match runner::load(file) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("skip {e}");
                continue;
            }
        };
        let input = SleepStageInput {
            start: "diagnostic",
            flow: &night.flow,
            hz: 50,
            excess_leak: &[],
            event_offsets: &[],
        };
        if only.is_some() {
            println!("{}\n{}", night.id, sleep_stage_debug(&input));
        }
        let features = sleep_stage_epoch_features(&input);
        for (f, label) in features.iter().zip(&night.labels) {
            let name = match label {
                StageLabel::Deep => "Deep",
                StageLabel::Light => "Light",
                _ => continue,
            };
            let mut row = [0.0; 14];
            row[..7].copy_from_slice(&f.short);
            row[7..].copy_from_slice(&f.long);
            groups.entry(name).or_default().push(row);
            let counts = gates.entry(name).or_default();
            *counts.entry("total").or_default() += 1;
            *counts.entry("uncertain").or_default() += usize::from(f.uncertain);
            *counts.entry("regular_quartile").or_default() += usize::from(f.regular_quartile);
            if !f.uncertain && f.regular_quartile {
                let s = f.short;
                let l = f.long;
                for (gate, failed) in [
                    ("short_interval", s[0] >= p.deep_interval_cap),
                    ("short_volume", s[1] >= p.deep_volume_cap),
                    ("short_duty", s[3] >= p.deep_long_duty_cap),
                    ("short_ventilation", s[2] >= p.deep_long_ventilation_cap),
                    ("squareness", s[6] > p.deep_squareness),
                    ("original_min_squareness", s[6] < 0.90),
                    ("long_interval", l[0] >= p.deep_long_interval_cap),
                    ("long_volume", l[1] >= p.deep_long_volume_cap),
                    ("long_ventilation", l[2] >= p.deep_long_ventilation_cap),
                    ("long_duty", l[3] >= p.deep_long_duty_cap),
                    (
                        "duty_range",
                        ![s[4], l[4]]
                            .iter()
                            .all(|x| (p.deep_duty.0..=p.deep_duty.1).contains(x)),
                    ),
                    (
                        "rate_range",
                        ![s[5], l[5]]
                            .iter()
                            .all(|x| (p.deep_rate.0..=p.deep_rate.1).contains(x)),
                    ),
                ] {
                    *counts.entry(gate).or_default() += usize::from(failed);
                }
                *counts.entry("above_deep_threshold").or_default() +=
                    usize::from(f.deep_evidence > p.deep_threshold);
            }
        }
    }
    let names = [
        "short_interval_cv",
        "short_volume_cv",
        "short_ventilation_cv",
        "short_duty_cv",
        "short_duty",
        "short_rate",
        "short_squareness",
        "long_interval_cv",
        "long_volume_cv",
        "long_ventilation_cv",
        "long_duty_cv",
        "long_duty",
        "long_rate",
        "long_squareness",
    ];
    for (name, rows) in groups {
        println!(
            "{name} n={} gate failures among supported regular-quartile epochs: {:?}",
            rows.len(),
            gates[name]
        );
        for (i, feature) in names.iter().enumerate() {
            let mut v: Vec<_> = rows.iter().map(|r| r[i]).collect();
            v.sort_by(f64::total_cmp);
            println!(
                "{name} {feature} p10={:.5} p25={:.5} p50={:.5} p75={:.5} p90={:.5}",
                v[((v.len() - 1) as f64 * 0.1) as usize],
                v[((v.len() - 1) as f64 * 0.25) as usize],
                v[(v.len() - 1) / 2],
                v[((v.len() - 1) as f64 * 0.75) as usize],
                v[((v.len() - 1) as f64 * 0.9) as usize]
            );
        }
    }
    Ok(())
}
