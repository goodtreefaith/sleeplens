//! Assemble app records without retaining raw night files across calls.
use crate::{
    bkp::parse_bkp,
    dat::{iterate_dat_records, parse_dat_header, summarize_dat, DatHeader},
    detect::{detect_as2x, As2xDetection},
    model::*,
    source::CardSource,
    time::LocalTime,
};
use chrono::Duration;
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardMetadata {
    pub device: Option<Device>,
    pub night_count: usize,
    pub months: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardImport {
    pub metadata: CardMetadata,
    pub nights: Vec<Night>,
}

/// Evening date for a device-local timestamp. Before noon belongs to yesterday.
pub fn evening_date(time: &LocalTime) -> String {
    // LocalTime has already validated the components. Treat UTC only as arithmetic.
    let dt = chrono::DateTime::from_timestamp(time.epoch_sec, 0)
        .expect("validated card timestamp")
        .naive_utc();
    (dt - Duration::hours(12)).format("%Y-%m-%d").to_string()
}
fn file_date(date: &str) -> String {
    format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..8])
}
fn device(header: &DatHeader) -> Device {
    let suffix: String = header
        .serial
        .chars()
        .rev()
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    Device {
        brand: "Aeonmed".into(),
        model: header
            .model_name
            .clone()
            .unwrap_or_else(|| "unknown".into()),
        family: "AS2X".into(),
        firmware: header.software_version.clone(),
        hardware: header.hardware_version.clone(),
        serial_masked: if header.serial.chars().count() > 2 {
            format!("•••{suffix}")
        } else {
            "•••".into()
        },
        pressure_unit: header
            .settings
            .as_ref()
            .map(|s| s.pressure_unit_label.clone())
            .unwrap_or_else(|| "unknown".into()),
    }
}
/// Parse and deliver each night before reading the next `.dat` file.
/// The callback may persist a night and discard it to keep import memory bounded.
pub fn stream_card(
    source: &impl CardSource,
    mut visit: impl FnMut(Night),
) -> io::Result<CardMetadata> {
    stream_detected_card(source, &detect_as2x(&source.list()?), &mut visit)
}
/// As `stream_card`, using a previously obtained file listing/detection.
pub fn stream_detected_card(
    source: &impl CardSource,
    detection: &As2xDetection,
    mut visit: impl FnMut(Night),
) -> io::Result<CardMetadata> {
    if !detection.detected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "No Aeonmed AS100 series card layout found (expected <serial><YYYYMM>/<YYYYMMDD>.dat).",
        ));
    }
    let bkp = detection
        .bkp
        .as_ref()
        .map(|path| source.read(path).map(|buf| parse_bkp(&buf)))
        .transpose()?;
    stream_card_with_backup(source, detection, bkp.as_ref(), &mut visit)
}
/// Import selected nights using a backup already parsed by the caller.
/// This keeps retryable, per-night imports from decoding the same log repeatedly.
pub fn stream_card_with_backup(
    source: &impl CardSource,
    detection: &As2xDetection,
    bkp: Option<&crate::bkp::BkpParseResult>,
    mut visit: impl FnMut(Night),
) -> io::Result<CardMetadata> {
    let mut metadata = CardMetadata {
        device: None,
        night_count: 0,
        months: detection.months.clone(),
    };
    let mut previous_settings: Option<Settings> = None;
    let mut recent_nights = Vec::new();
    for file in &detection.nights {
        let bytes = source.read(&file.path)?;
        let header = parse_dat_header(&bytes);
        if metadata.device.is_none() {
            metadata.device = Some(device(&header));
        }
        let sum = summarize_dat(&bytes, false);
        let date = file_date(&file.date);
        let machine = bkp.as_ref().and_then(|b| {
            b.sessions
                .iter()
                .find(|s| s.start.as_ref().is_some_and(|t| evening_date(t) == date))
        });
        let events: Vec<_> = bkp
            .as_ref()
            .into_iter()
            .flat_map(|b| &b.events)
            .filter(|e| evening_date(&e.time) == date)
            .map(|e| TherapyEvent {
                time: e.time.iso.clone(),
                kind: match e.event_type.as_str() {
                    "snoreLevel" | "snoreCount" => "snore",
                    "level" => "leakLevel",
                    "treatmentStart" | "treatmentEnd" | "unknown" => "other",
                    k => k,
                }
                .into(),
                code: e.code,
                value: e.value,
                pressure: e.pressure,
            })
            .collect();
        let mut counts = EventCounts::default();
        for event in &events {
            match event.kind.as_str() {
                "obstructiveApnea" => counts.obstructive += 1,
                "centralApnea" => counts.central += 1,
                "hypopnea" => counts.hypopnea += 1,
                _ => {}
            }
        }
        let settings = header.settings.as_ref().map(Settings::from).or_else(|| {
            bkp.as_ref()
                .and_then(|b| {
                    b.settings
                        .iter()
                        .filter(|s| {
                            s.time.as_ref().is_some_and(|t| {
                                sum.start
                                    .as_ref()
                                    .is_some_and(|start| t.epoch_sec <= start.epoch_sec)
                            })
                        })
                        .max_by_key(|s| s.time.as_ref().map(|t| t.epoch_sec))
                })
                .map(Settings::from)
        });
        let intranight_prescription_changed = bkp.is_some_and(|b| {
            prescription_changed_within(
                b,
                sum.start.as_ref().map(|s| s.epoch_sec),
                sum.end.as_ref().map(|s| s.epoch_sec),
            )
        });
        let prescription_changed = previous_settings
            .as_ref()
            .zip(settings.as_ref())
            .is_some_and(|(old, new)| prescription_differs(old, new))
            || intranight_prescription_changed;
        let changed = previous_settings
            .as_ref()
            .is_some_and(|old| settings.as_ref().is_some_and(|new| old != new));
        if settings.is_some() {
            previous_settings = settings.clone();
        }
        if sum
            .start
            .as_ref()
            .zip(sum.end.as_ref())
            .is_some_and(|(start, end)| !(0..=172_800).contains(&(end.epoch_sec - start.epoch_sec)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "night timestamp span exceeds 48 hours",
            ));
        }
        let mut flow = Vec::new();
        let series = sum
            .start
            .as_ref()
            .zip(sum.end.as_ref())
            .and_then(|(start, end)| {
                let length = usize::try_from(end.epoch_sec - start.epoch_sec).ok()?;
                let mut series = Series {
                    series_start: start.iso.clone(),
                    pressure: vec![None; length],
                    excess_leak: vec![None; length],
                    total_leak: vec![None; length],
                    flow_amplitude: vec![None; length],
                    snore: vec![None; length],
                };
                flow.resize(length * 50, f32::NAN);
                for record in iterate_dat_records(&bytes, true).filter(|r| r.valid) {
                    let Some(time) = record.time else { continue };
                    let Ok(index) = usize::try_from(time.epoch_sec - start.epoch_sec) else {
                        continue;
                    };
                    if index >= length {
                        continue;
                    }
                    if let Some(waveforms) = record.waveforms {
                        flow[index * 50..(index + 1) * 50].copy_from_slice(&waveforms.patient_flow);
                    }
                    series.pressure[index] = Some(record.set_pressure as f32);
                    series.excess_leak[index] = Some(record.excess_leak as f32);
                    series.total_leak[index] = Some(record.total_leak as f32);
                    series.flow_amplitude[index] = Some(record.flow_amplitude as f32);
                    series.snore[index] = Some(record.snore as f32);
                }
                Some(series)
            });
        let source = if machine.is_some() {
            Source::Machine
        } else {
            Source::Computed
        };
        let stages = series.as_ref().map(|s| {
            let excess_leak: Vec<_> = s
                .excess_leak
                .iter()
                .map(|v| v.unwrap_or(f32::NAN))
                .collect();
            let start = sum.start.as_ref().expect("series has start").epoch_sec;
            let event_offsets: Vec<_> = events
                .iter()
                .filter(|e| {
                    matches!(
                        e.kind.as_str(),
                        "obstructiveApnea" | "centralApnea" | "hypopnea"
                    )
                })
                .filter_map(|e| {
                    chrono::NaiveDateTime::parse_from_str(&e.time, "%Y-%m-%dT%H:%M:%S").ok()
                })
                .filter_map(|t| u32::try_from(t.and_utc().timestamp() - start).ok())
                .collect();
            crate::sleep_stages::estimate_sleep_stages(&crate::sleep_stages::SleepStageInput {
                start: &s.series_start,
                flow: &flow,
                hz: 50,
                excess_leak: &excess_leak,
                event_offsets: &event_offsets,
            })
        });
        let mut night = Night {
            list: NightListItem {
                date,
                start: sum.start.as_ref().map(|t| t.iso.clone()),
                end: sum.end.as_ref().map(|t| t.iso.clone()),
                usage_minutes: machine.map_or(sum.usage_minutes, |s| s.usage_minutes),
                usage_source: source,
                ahi: machine.map(|s| s.ahi),
                pressure_mean: machine
                    .map(|s| s.pressure.mean)
                    .or_else(|| (sum.record_seconds > 0).then_some(sum.pressure.mean)),
                pressure_p90: machine
                    .map(|s| s.pressure.p90)
                    .or_else(|| (sum.record_seconds > 0).then_some(sum.pressure.p90)),
                pressure_source: source,
                excess_leak_median: (sum.record_seconds > 0).then_some(sum.excess_leak.median),
                large_leak_minutes: machine.map(|s| s.large_leak_minutes),
                event_counts: counts,
                settings_changed: changed,
                prescription_changed,
                excess_leak_p95: (sum.record_seconds > 0).then_some(sum.excess_leak.p95),
                sleep_score: None,
                interruptions: sum.sessions.len().saturating_sub(1),
            },
            sessions: sum
                .sessions
                .iter()
                .map(|s| Session {
                    start: s.start.iso.clone(),
                    end: s.end.iso.clone(),
                    minutes: s.minutes,
                })
                .collect(),
            summary: machine
                .map(|s| NightSummary {
                    start_pressure: Some(s.start_pressure),
                    max_pressure: Some(s.observed_max_pressure),
                    ai: Some(s.apnea_index),
                    cai: Some(s.central_apnea_index),
                    oai: Some(s.obstructive_apnea_index),
                    hi: Some(s.hypopnea_index),
                    sni: Some(s.snore_index),
                })
                .unwrap_or_default(),
            settings,
            events,
            series,
            stages,
            score: None,
            tips: Vec::new(),
            intranight_prescription_changed,
        };
        crate::score::refresh_score(&mut night);
        night.tips = crate::tips::night_tips(&night, &recent_nights);
        recent_nights.push(Night {
            list: night.list.clone(),
            sessions: Vec::new(),
            summary: night.summary.clone(),
            settings: None,
            events: Vec::new(),
            series: None,
            score: None,
            stages: None,
            tips: Vec::new(),
            intranight_prescription_changed: night.intranight_prescription_changed,
        });
        if recent_nights.len() > 7 {
            recent_nights.remove(0);
        }
        visit(night);
        metadata.night_count += 1;
    }
    Ok(metadata)
}
/// Convenience API for callers that want to retain every assembled night.
/// Use `stream_card` when importing a large card into a database.
pub fn assemble_card(source: &impl CardSource) -> io::Result<CardImport> {
    let mut nights = Vec::new();
    let metadata = stream_card(source, |night| nights.push(night))?;
    Ok(CardImport { metadata, nights })
}

/// Refresh machine fields for a cached night without reading its `.dat` file.
/// Dates outside the backup's current coverage retain their imported values.
pub fn refresh_backup_fields(night: &mut Night, bkp: &crate::bkp::BkpParseResult) -> bool {
    let machine = bkp.sessions.iter().find(|s| {
        s.start
            .as_ref()
            .is_some_and(|t| evening_date(t) == night.list.date)
    });
    let covered = machine.is_some()
        || bkp
            .events
            .iter()
            .any(|e| evening_date(&e.time) == night.list.date);
    if !covered {
        return false;
    }
    let events: Vec<_> = bkp
        .events
        .iter()
        .filter(|e| evening_date(&e.time) == night.list.date)
        .map(|e| TherapyEvent {
            time: e.time.iso.clone(),
            kind: match e.event_type.as_str() {
                "snoreLevel" | "snoreCount" => "snore",
                "level" => "leakLevel",
                "treatmentStart" | "treatmentEnd" | "unknown" => "other",
                k => k,
            }
            .into(),
            code: e.code,
            value: e.value,
            pressure: e.pressure,
        })
        .collect();
    let mut counts = EventCounts::default();
    for event in &events {
        match event.kind.as_str() {
            "obstructiveApnea" => counts.obstructive += 1,
            "centralApnea" => counts.central += 1,
            "hypopnea" => counts.hypopnea += 1,
            _ => {}
        }
    }

    let epoch = |value: &Option<String>| {
        value
            .as_ref()
            .and_then(|v| chrono::NaiveDateTime::parse_from_str(v, "%Y-%m-%dT%H:%M:%S").ok())
            .map(|v| v.and_utc().timestamp())
    };
    night.intranight_prescription_changed =
        prescription_changed_within(bkp, epoch(&night.list.start), epoch(&night.list.end));
    night.list.prescription_changed |= night.intranight_prescription_changed;
    night.events = events;
    night.list.event_counts = counts;
    if let Some(s) = machine {
        night.list.usage_minutes = s.usage_minutes;
        night.list.usage_source = Source::Machine;
        night.list.pressure_mean = Some(s.pressure.mean);
        night.list.pressure_p90 = Some(s.pressure.p90);
        night.list.pressure_source = Source::Machine;
        night.list.ahi = Some(s.ahi);
        night.list.large_leak_minutes = Some(s.large_leak_minutes);
        night.summary = machine
            .map(|s| NightSummary {
                start_pressure: Some(s.start_pressure),
                max_pressure: Some(s.observed_max_pressure),
                ai: Some(s.apnea_index),
                cai: Some(s.central_apnea_index),
                oai: Some(s.obstructive_apnea_index),
                hi: Some(s.hypopnea_index),
                sni: Some(s.snore_index),
            })
            .unwrap_or_default();
    }
    crate::score::refresh_score(night);
    night.tips = crate::tips::night_tips(night, &[]);
    true
}

/// Prescription markers exclude comfort, humidity, ramp and convenience settings.
pub fn prescription_differs(old: &Settings, new: &Settings) -> bool {
    old.mode != new.mode
        || old.start_pressure != new.start_pressure
        || old.min_pressure != new.min_pressure
        || old.max_pressure != new.max_pressure
        || old.fixed_pressure != new.fixed_pressure
}

/// Detect logged prescription changes during mask use, independently of the header snapshot.
pub fn prescription_changed_within(
    bkp: &crate::bkp::BkpParseResult,
    start: Option<i64>,
    end: Option<i64>,
) -> bool {
    let Some((start, end)) = start.zip(end) else {
        return false;
    };
    let mut entries: Vec<_> = bkp
        .settings
        .iter()
        .filter(|s| s.time.as_ref().is_some_and(|t| t.epoch_sec <= end))
        .collect();
    entries.sort_by_key(|s| s.time.as_ref().map(|t| t.epoch_sec));
    entries.windows(2).any(|pair| {
        pair[1].time.as_ref().is_some_and(|t| t.epoch_sec >= start)
            && prescription_differs(&Settings::from(pair[0]), &Settings::from(pair[1]))
    })
}
