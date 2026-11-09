use serde_json::{json, Value};
use sleeplens_core::{
    bkp::{parse_bkp, SessionSummary, TherapyEvent},
    dat::{parse_dat_header, summarize_dat, DatSummary},
    detect::detect_as2x,
    source::{CardSource, FsCardSource},
    stream_card_with_backup,
    tips::night_tips,
    Night,
};
use std::{collections::BTreeMap, process::ExitCode};

struct Row {
    date: String,
    sum: DatSummary,
    session: Option<SessionSummary>,
    events: Vec<TherapyEvent>,
    json: Value,
    night: Night,
}
fn round1(n: f64) -> f64 {
    (n * 10.0).round() / 10.0
}
fn clock(t: Option<&str>) -> &str {
    t.and_then(|s| s.get(11..16)).unwrap_or("--:--")
}
fn unit(label: Option<&str>) -> &str {
    if label == Some("hPa") {
        "hPa"
    } else {
        "card units"
    }
}
fn mark(text: &str, machine: bool, width: usize) -> String {
    format!("{:>width$}{}", text, if machine { ' ' } else { '*' })
}
fn number(n: f64) -> String {
    n.to_string()
}
fn run() -> Result<(), (u8, String)> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "Usage: sleeplens inspect <card> [--json] [--night YYYYMMDD]";
    if args.first().map(String::as_str) != Some("inspect") {
        return Err((1, usage.into()));
    }
    let mut path = None;
    let mut json_mode = false;
    let mut only = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_mode = true,
            "--night" => {
                i += 1;
                only = args.get(i).cloned();
                if !only
                    .as_ref()
                    .is_some_and(|n| n.len() == 8 && n.bytes().all(|b| b.is_ascii_digit()))
                {
                    return Err((1, usage.into()));
                }
            }
            arg if !arg.starts_with('-') && path.is_none() => path = Some(arg.to_owned()),
            _ => return Err((1, usage.into())),
        }
        i += 1;
    }
    let path = path.ok_or_else(|| (1, usage.to_owned()))?;
    let source = FsCardSource::new(path);
    // Do not print filesystem error paths: they may include a device serial.
    let io_error = |_| (1, "Unable to read card files.".to_owned());
    let detection = detect_as2x(&source.list().map_err(io_error)?);
    if !detection.detected {
        return Err((
            2,
            "No Aeonmed AS100 series card layout found (expected <serial><YYYYMM>/<YYYYMMDD>.dat)."
                .into(),
        ));
    }
    let serial = detection.serial.as_deref().unwrap_or("");
    let redact = |text: &str| {
        if serial.is_empty() {
            text.to_owned()
        } else {
            text.replace(serial, "<SERIAL>")
        }
    };
    let bkp = detection
        .bkp
        .as_ref()
        .map(|p| source.read(p).map(|b| parse_bkp(&b)))
        .transpose()
        .map_err(io_error)?;
    let mut rows = Vec::new();
    let mut header = None;
    for file in detection
        .nights
        .iter()
        .filter(|f| only.as_ref().is_none_or(|date| &f.date == date))
    {
        let bytes = source.read(&file.path).map_err(io_error)?;
        if header.is_none() {
            header = Some(parse_dat_header(&bytes));
        }
        let sum = summarize_dat(&bytes, false);
        let range = sum
            .start
            .as_ref()
            .zip(sum.end.as_ref())
            .map(|(s, e)| (s.epoch_sec - 120, e.epoch_sec + 120));
        let session = bkp
            .as_ref()
            .and_then(|b| {
                b.sessions.iter().find(|s| {
                    s.start
                        .as_ref()
                        .zip(range)
                        .is_some_and(|(s, (lo, hi))| s.epoch_sec >= lo && s.epoch_sec <= hi)
                })
            })
            .cloned();
        let events: Vec<_> = if session.is_some() {
            bkp.as_ref()
                .into_iter()
                .flat_map(|b| b.events.iter())
                .filter(|e| {
                    range.is_some_and(|(lo, hi)| e.time.epoch_sec >= lo && e.time.epoch_sec <= hi)
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        let mut selected = detection.clone();
        selected.nights = vec![file.clone()];
        let mut assembled = None;
        stream_card_with_backup(&source, &selected, bkp.as_ref(), |night| {
            assembled = Some(night);
        })
        .map_err(io_error)?;
        let mut night = assembled.expect("selected night was assembled");
        if let Some(previous) = rows
            .last()
            .and_then(|row: &Row| row.night.settings.as_ref())
        {
            night.list.prescription_changed |= night
                .settings
                .as_ref()
                .is_some_and(|settings| sleeplens_core::prescription_differs(previous, settings));
        }
        let recent: Vec<_> = rows
            .iter()
            .rev()
            .take(7)
            .map(|row: &Row| row.night.clone())
            .collect();
        night.tips = night_tips(&night, &recent);
        let mut counts = BTreeMap::new();
        for e in &events {
            *counts.entry(e.event_type.clone()).or_insert(0usize) += 1;
        }
        let machine = session.is_some();
        let mut value = json!({"date": file.date, "file": redact(&file.path), "start": sum.start.as_ref().map(|t| &t.iso), "end": sum.end.as_ref().map(|t| &t.iso), "usageMinutes":session.as_ref().map_or(round1(sum.usage_minutes), |s| s.usage_minutes), "usageSource":if machine {"summary"} else {"dat"}, "pressureMean":session.as_ref().map_or(round1(sum.pressure.mean),|s|s.pressure.mean),"pressureP90":session.as_ref().map_or(sum.pressure.p90,|s|s.pressure.p90),"pressureSource":if machine {"summary"} else {"dat"},"datUsageMinutes":round1(sum.usage_minutes),"datPressureMean":round1(sum.pressure.mean),"datPressureP90":sum.pressure.p90,"activeIntervals":sum.sessions.len(),"leakMedian":sum.total_leak.median,"leakFlowMedian":sum.excess_leak.median,"excessLeakMedian":sum.excess_leak.median,"totalLeakMedian":sum.total_leak.median,"largeLeakMinutes":session.as_ref().map(|s|s.large_leak_minutes),"session":session,"eventCounts":if machine {Some(counts)} else {None},"sessions":sum.sessions.iter().map(|s|json!({"start":s.start.iso,"end":s.end.iso,"minutes":round1(s.minutes)})).collect::<Vec<_>>(),"recordCount":sum.record_count,"invalidRecords":sum.invalid_records});
        value["sleepScore"] = json!(night.list.sleep_score);
        value["excessLeakP95"] = json!(night.list.excess_leak_p95);
        value["interruptions"] = json!(night.list.interruptions);
        value["score"] = json!(night.score);
        value["stages"] = json!(night.stages);
        value["tips"] = json!(night.tips);
        // Keep recent summary context without retaining each night's 1 Hz arrays.
        night.series = None;
        night.events.clear();
        rows.push(Row {
            date: file.date.clone(),
            sum,
            session,
            events,
            json: value,
            night,
        });
    }
    if rows.is_empty() {
        return Err((3, format!("No night file {}.", only.unwrap_or_default())));
    }
    let h = header.as_ref().expect("non-empty rows");
    let settings = h.settings.as_ref();
    let label = settings.map(|s| s.pressure_unit_label.as_str());
    let unit = unit(label);
    if json_mode {
        let device = json!({"model":h.model_name.as_deref().unwrap_or("unknown"),"idPrefix":h.device_model,"formatVersion":h.format_version,"softwareVersion":h.software_version,"hardwareVersion":h.hardware_version});
        let settings_out = settings.map(|s| {
            let mut v = serde_json::to_value(s).expect("settings serialization");
            v.as_object_mut().expect("object").remove("raw");
            v["time"] = json!(s.time.as_ref().map(|t| &t.iso));
            v
        });
        let mut out = json!({"serial":"<SERIAL>","device":device,"settings":settings_out,"pressureUnitLabel":label,"nights":rows.iter().map(|r|&r.json).collect::<Vec<_>>()});
        if only.is_some() {
            out["events"] = json!(rows[0]
                .events
                .iter()
                .map(|e| {
                    let mut v = serde_json::to_value(e).expect("event serialization");
                    v["time"] = json!(e.time.iso);
                    v
                })
                .collect::<Vec<_>>());
        }
        println!(
            "{}",
            redact(&serde_json::to_string_pretty(&out).expect("JSON serialization"))
        );
        return Ok(());
    }
    println!(
        "Aeonmed AS100 series card <SERIAL>: {} nights, {} months, bkp {}",
        detection.nights.len(),
        detection.months.len(),
        if bkp.is_some() { "found" } else { "missing" }
    );
    println!(
        "device: {} ({}), firmware {}, hardware {}, night file format v{}",
        h.model_name.as_deref().unwrap_or("unknown"),
        redact(&h.device_model),
        h.software_version.as_deref().unwrap_or("?"),
        h.hardware_version.as_deref().unwrap_or("?"),
        h.format_version
    );
    if let Some(s) = settings {
        println!("settings (first night): mode {}, start {}, min {}, max {}, fixed {} {} (unit byte {}), ramp {} min, humidifier {}, AS-Elex {}, auto on {}, auto off {}",s.mode_label,s.start_pressure,s.min_pressure,s.max_pressure,s.fixed_pressure,unit,s.pressure_unit,s.ramp_minutes,s.humidifier_level,s.comfort_level,if s.auto_on {"yes"} else {"no"},if s.auto_off {"yes"} else {"no"});
    }
    if !h.records_supported {
        println!("Night files use the version 1 (512-byte) record layout, which is not decoded.");
    }
    println!("{:<8}  {:<11}  {}  {}  {}  {:>11}  {:>14}  {:>5}  events (OA obstructive, CA central, H hypopnea)","date","start-end",mark("usage",true,5),mark("mean",true,4),mark("p90",true,4),"excess leak","large leak min","AHI");
    for r in &rows {
        let machine = r.session.is_some();
        let usage = r
            .session
            .as_ref()
            .map_or(round1(r.sum.usage_minutes), |s| s.usage_minutes)
            .round() as u64;
        let mean = r
            .session
            .as_ref()
            .map_or(round1(r.sum.pressure.mean), |s| s.pressure.mean);
        let p90 = r
            .session
            .as_ref()
            .map_or(r.sum.pressure.p90, |s| s.pressure.p90);
        let mut order: Vec<(&str, usize)> = Vec::new();
        for e in &r.events {
            let short = match e.event_type.as_str() {
                "obstructiveApnea" => "OA",
                "centralApnea" => "CA",
                "hypopnea" => "H",
                _ => continue,
            };
            if let Some((_, n)) = order.iter_mut().find(|(k, _)| *k == short) {
                *n += 1
            } else {
                order.push((short, 1))
            }
        }
        let events = if !machine {
            "-".into()
        } else if order.is_empty() {
            "0".into()
        } else {
            order
                .iter()
                .map(|(k, n)| format!("{n} {k}"))
                .collect::<Vec<_>>()
                .join(" + ")
        };
        println!(
            "{}  {}-{}  {}  {}  {}  {:>11.1}  {:>14}  {:>5}  {}",
            r.date,
            clock(r.sum.start.as_ref().map(|t| t.iso.as_str())),
            clock(r.sum.end.as_ref().map(|t| t.iso.as_str())),
            mark(&format!("{}:{:02}", usage / 60, usage % 60), machine, 5),
            mark(&format!("{mean:.1}"), machine, 4),
            mark(&format!("{p90:.1}"), machine, 4),
            r.sum.excess_leak.median,
            r.session
                .as_ref()
                .map(|s| format!("{:.1}", s.large_leak_minutes))
                .unwrap_or_else(|| "-".into()),
            r.session
                .as_ref()
                .map(|s| format!("{:.1}", s.ahi))
                .unwrap_or_else(|| "-".into()),
            events
        );
        if let Some(score) = &r.night.score {
            println!(
                "  estimated sleep score: {}/100 ({})",
                score.total, score.label
            );
        } else {
            println!("  estimated sleep score: unavailable for nights under 30 minutes");
        }
        if let Some(stages) = &r.night.stages {
            println!(
                "  estimated stages: wake {:.1}, light {:.1}, deep {:.1}, REM {:.1} min; signal quality {:.0}%",
                stages.minutes.wake,
                stages.minutes.light,
                stages.minutes.deep,
                stages.minutes.rem,
                stages.signal_quality * 100.0
            );
        }
        for tip in &r.night.tips {
            println!("  {}: {}", tip.title, tip.body);
        }
    }
    println!("pressure in {}{}; usage, mean and p90 from the bkp session summary when there is one; excess leak = median L/min of slots 270-319; large leak min = minutes above 90 L/min",unit,if label == Some("hPa") {" (about the same number in cmH2O)"} else {" (pressure unit byte not recognised)"});
    if rows.iter().any(|r| r.session.is_none()) {
        println!("* no bkp session summary for this night: value computed from the night file");
    }
    if only.is_some() {
        let r = &rows[0];
        println!(
            "\nNight {}: {} records ({} invalid), {} min in {} active interval(s)",
            r.date,
            r.sum.record_count,
            r.sum.invalid_records,
            round1(r.sum.usage_minutes),
            r.sum.sessions.len()
        );
        for s in &r.sum.sessions {
            println!(
                "  {} - {}  {} min",
                &s.start.iso[11..],
                &s.end.iso[11..],
                round1(s.minutes)
            );
        }
        if let Some(s) = &r.session {
            println!("bkp summary: session {}, usage {} min, start pressure {}, mean/P90 {}/{}, max {} {}, large leak (screen \"LEAK90\") {} min, SNI {}",s.session_number,s.usage_minutes,s.start_pressure,s.pressure.mean,s.pressure.p90,s.observed_max_pressure,unit,s.large_leak_minutes,s.snore_index);
            println!(
                "  AHI {}, apnea index {} (central {}, obstructive {}), hypopnea index {}",
                s.ahi,
                s.apnea_index,
                s.central_apnea_index,
                s.obstructive_apnea_index,
                s.hypopnea_index
            );
            println!("  night file: mean/P90 {}/{} {}; excess leak (slots 270-319) median {}, total leak (slot 384) median {} L/min",round1(r.sum.pressure.mean),r.sum.pressure.p90,unit,r.sum.excess_leak.median,r.sum.total_leak.median);
            for e in &r.events {
                println!(
                    "  {}  {} (0x{:X}) {}",
                    &e.time.iso[11..],
                    e.event_type,
                    e.code,
                    number(e.pressure.unwrap_or(e.value as f64))
                );
            }
        } else {
            println!("No matching bkp session summary (older than the ring buffer, or no bkp).");
        }
    } else if let Some(b) = bkp {
        println!(
            "\nbkp: {} session summaries, {} events, {} counter records, {} unknown blocks",
            b.sessions.len(),
            b.events.len(),
            b.counters.len(),
            b.stats.unknown_blocks
        );
    }
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err((code, message)) => {
            eprintln!("{message}");
            ExitCode::from(code)
        }
    }
}
