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
                for record in iterate_dat_records(&bytes, false).filter(|r| r.valid) {
                    let Some(time) = record.time else { continue };
                    let Ok(index) = usize::try_from(time.epoch_sec - start.epoch_sec) else {
                        continue;
                    };
                    if index >= length {
                        continue;
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
        let night = Night {
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
        };
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
    true
}
