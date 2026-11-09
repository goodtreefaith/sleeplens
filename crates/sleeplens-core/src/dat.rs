//! AS2X DAT header, lazy records and one-pass night statistics.
use crate::{
    settings::{parse_settings_block, SettingsSnapshot},
    time::{add_seconds, read_ascii, read_timestamp, LocalTime},
};
use serde::{Deserialize, Serialize};
pub const DAT_HEADER_SIZE: usize = 0x800;
pub const DAT_RECORD_SIZE: usize = 812;
pub const DAT_RECORD_SIZE_V1: usize = 512;
pub const SAMPLES_PER_RECORD: usize = 400;
pub const WAVE_SAMPLES: usize = 50;
pub const PAUSE_GAP_SECONDS: i64 = 60;
pub mod slot {
    pub const SET_PRESSURE: usize = 20;
    pub const PRESSURE_FEEDBACK: usize = 70;
    pub const BLOWER_PRESSURE: usize = PRESSURE_FEEDBACK;
    pub const MASK_PRESSURE: usize = 120;
    pub const SNORE: usize = 170;
    pub const PATIENT_FLOW: usize = 220;
    pub const LEAK_FLOW: usize = 270;
    pub const TOTAL_FLOW: usize = 320;
    pub const ELAPSED: usize = 379;
    pub const STATE: usize = 381;
    pub const LEAK: usize = 384;
}
pub const MODEL_NAMES: [(&str, &str); 8] = [
    ("AS1", "AS100C"),
    ("AS2", "AS100A"),
    ("AS3", "AS100A"),
    ("AS4", "AS100A-R"),
    ("AS5", "AS100A-RT"),
    ("AS6", "AS100C"),
    ("AS7", "AS100C-R"),
    ("AS8", "AS100C-RT"),
];
pub fn model_name_for_id(id: &str) -> Option<&'static str> {
    let prefix = id.chars().take(3).collect::<String>().to_ascii_uppercase();
    MODEL_NAMES
        .iter()
        .find(|(p, _)| *p == prefix)
        .map(|(_, n)| *n)
}
pub fn format_device_version(bytes: &[u8]) -> Option<String> {
    let mut v = [0u8; 4];
    for (to, from) in v.iter_mut().zip(bytes) {
        *to = *from;
    }
    if v.iter().all(|&x| x == 0) || v.iter().all(|&x| x == 255) {
        return None;
    }
    let highest = if v[3] != 0 {
        3
    } else if v[2] != 0 {
        2
    } else {
        1
    };
    let mut out = v[highest].to_string();
    for i in (0..highest).rev() {
        out.push_str(&format!(".{:02}", v[i]));
    }
    Some(out)
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatHeader {
    pub created: Option<LocalTime>,
    pub format_version: u8,
    pub records_supported: bool,
    pub device_model: String,
    pub model_name: Option<String>,
    pub serial: String,
    pub device_type: u8,
    pub software_version: Option<String>,
    pub hardware_version: Option<String>,
    pub settings: Option<SettingsSnapshot>,
    pub record_count: usize,
    pub trailing_bytes: usize,
}
pub fn parse_dat_header(buf: &[u8]) -> DatHeader {
    let body = buf.len().saturating_sub(DAT_HEADER_SIZE);
    let version = buf.get(8).copied().unwrap_or(0);
    let size = if version == 1 {
        DAT_RECORD_SIZE_V1
    } else {
        DAT_RECORD_SIZE
    };
    let id = if buf.len() >= 0x60 {
        read_ascii(buf, 0x50, 24)
    } else {
        String::new()
    };
    let version_at = |o| buf.get(o..o + 4).and_then(format_device_version);
    DatHeader {
        created: read_timestamp(buf, 0),
        format_version: version,
        records_supported: version != 1,
        device_model: id.chars().take(4).collect(),
        model_name: model_name_for_id(&id).map(str::to_owned),
        serial: id.chars().skip(4).collect(),
        device_type: buf.get(0x68).copied().unwrap_or(0),
        software_version: version_at(0x69),
        hardware_version: version_at(0x6d),
        settings: parse_settings_block(buf, 0x140),
        record_count: body / size,
        trailing_bytes: body % size,
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatWaveforms {
    pub set_pressure: Vec<f32>,
    pub pressure_feedback: Vec<f32>,
    pub mask_pressure: Vec<f32>,
    pub snore: Vec<f32>,
    pub patient_flow: Vec<f32>,
    pub leak_flow: Vec<f32>,
    pub total_flow: Vec<f32>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatRecord {
    pub index: usize,
    pub time: Option<LocalTime>,
    pub weekday: u8,
    pub valid: bool,
    pub set_pressure: f64,
    pub mask_pressure: f64,
    pub flow_amplitude: f64,
    pub total_leak: f64,
    pub leak: f64,
    pub excess_leak: f64,
    pub leak_flow: f64,
    pub snore: f64,
    pub elapsed_counter: u16,
    pub state_code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waveforms: Option<DatWaveforms>,
}
pub fn iterate_dat_records(buf: &[u8], waveforms: bool) -> impl Iterator<Item = DatRecord> + '_ {
    let header = parse_dat_header(buf);
    let n = if header.records_supported {
        header.record_count
    } else {
        0
    };
    (0..n).map(move |index| {
        let b = &buf[DAT_HEADER_SIZE + index * DAT_RECORD_SIZE..][..DAT_RECORD_SIZE];
        let u = |s| u16::from_le_bytes([b[12 + 2 * s], b[13 + 2 * s]]);
        let signed = |s| u(s) as i16 as f64;
        let mean_u =
            |s| (0..WAVE_SAMPLES).map(|k| u(s + k) as f64).sum::<f64>() / WAVE_SAMPLES as f64;
        let mean_i =
            |s| (0..WAVE_SAMPLES).map(|k| signed(s + k)).sum::<f64>() / WAVE_SAMPLES as f64;
        let time = if b[..2] == [0x4d, 0x4d] {
            read_timestamp(b, 4)
        } else {
            None
        };
        let (flow_min, flow_max) = (0..WAVE_SAMPLES)
            .map(|k| u(slot::PATIENT_FLOW + k))
            .fold((u16::MAX, 0u16), |(min, max), v| (min.min(v), max.max(v)));
        let read = |s, divisor: f64, offset: f64, is_signed: bool| {
            (0..WAVE_SAMPLES)
                .map(|k| {
                    ((if is_signed {
                        u(s + k) as i16 as f64
                    } else {
                        u(s + k) as f64
                    }) / divisor
                        + offset) as f32
                })
                .collect()
        };
        let wf = waveforms.then(|| DatWaveforms {
            set_pressure: read(slot::SET_PRESSURE, 10., 0., false),
            pressure_feedback: read(slot::PRESSURE_FEEDBACK, 10., 0., false),
            mask_pressure: read(slot::MASK_PRESSURE, 10., 0., false),
            snore: read(slot::SNORE, 1., 0., true),
            patient_flow: read(slot::PATIENT_FLOW, 1., -500., false),
            leak_flow: read(slot::LEAK_FLOW, 1., 0., true),
            total_flow: read(slot::TOTAL_FLOW, 1., -500., false),
        });
        DatRecord {
            index,
            valid: time.is_some(),
            time,
            weekday: b[11],
            set_pressure: mean_u(slot::SET_PRESSURE) / 10.,
            mask_pressure: mean_u(slot::MASK_PRESSURE) / 10.,
            flow_amplitude: (flow_max - flow_min) as f64,
            total_leak: u(slot::LEAK) as f64,
            leak: u(slot::LEAK) as f64,
            excess_leak: mean_i(slot::LEAK_FLOW),
            leak_flow: mean_i(slot::LEAK_FLOW),
            snore: mean_i(slot::SNORE),
            elapsed_counter: u(slot::ELAPSED),
            state_code: u(slot::STATE),
            waveforms: wf,
        }
    })
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveInterval {
    pub start: LocalTime,
    pub end: LocalTime,
    pub minutes: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PressureStats {
    pub min: f64,
    pub mean: f64,
    pub p90: f64,
    pub p95: f64,
    pub max: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeakStats {
    pub median: f64,
    pub p95: f64,
    pub max: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatSeries {
    pub hz: u8,
    pub pressure: Vec<f32>,
    pub total_leak: Vec<f32>,
    pub leak: Vec<f32>,
    pub excess_leak: Vec<f32>,
    pub leak_flow: Vec<f32>,
    pub snore: Vec<f32>,
    pub flow_amplitude: Vec<f32>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatSummary {
    pub serial: String,
    pub start: Option<LocalTime>,
    pub end: Option<LocalTime>,
    pub record_count: usize,
    pub invalid_records: usize,
    pub record_seconds: usize,
    pub usage_seconds: i64,
    pub usage_minutes: f64,
    pub sessions: Vec<ActiveInterval>,
    pub pressure: PressureStats,
    pub total_leak: LeakStats,
    pub leak: LeakStats,
    pub excess_leak: LeakStats,
    pub leak_flow: LeakStats,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<DatSeries>,
}
fn percentile(hist: &[u32], n: usize, p: f64) -> f64 {
    if n == 0 {
        return 0.;
    }
    let rank = ((p * n as f64 - 1e-9).ceil() as u64).max(1);
    let mut sum = 0u64;
    for (i, &v) in hist.iter().enumerate() {
        sum += v as u64;
        if sum >= rank {
            return i as f64;
        }
    }
    (hist.len() - 1) as f64
}
fn round(value: f64, scale: f64) -> f64 {
    (value * scale).round() / scale
}
fn close_session(sessions: &mut Vec<ActiveInterval>, start: &Option<LocalTime>, last: &LocalTime) {
    if let Some(start) = start {
        let end = add_seconds(last, 1);
        let minutes = (end.epoch_sec - start.epoch_sec) as f64 / 60.;
        sessions.push(ActiveInterval {
            start: start.clone(),
            end,
            minutes,
        });
    }
}
pub fn summarize_dat(buf: &[u8], include_series: bool) -> DatSummary {
    let header = parse_dat_header(buf);
    let n = header.record_count;
    let mut series = include_series.then(|| DatSeries {
        hz: 1,
        pressure: vec![0.; n],
        total_leak: vec![0.; n],
        leak: vec![0.; n],
        excess_leak: vec![0.; n],
        leak_flow: vec![0.; n],
        snore: vec![0.; n],
        flow_amplitude: vec![0.; n],
    });
    let (mut ph, mut lh, mut eh) = (vec![0u32; 2048], vec![0u32; 65536], vec![0u32; 20000]);
    let (mut valid, mut invalid) = (0, 0);
    let (mut first, mut last, mut interval): (
        Option<LocalTime>,
        Option<LocalTime>,
        Option<LocalTime>,
    ) = (None, None, None);
    let mut sessions = Vec::new();
    let (mut ps, mut pmin, mut pmax, mut lmax, mut emax) =
        (0., f64::INFINITY, f64::NEG_INFINITY, 0f64, 0f64);
    for rec in iterate_dat_records(buf, false) {
        if let Some(s) = &mut series {
            let i = rec.index;
            s.pressure[i] = rec.set_pressure as f32;
            s.total_leak[i] = rec.total_leak as f32;
            s.leak[i] = rec.total_leak as f32;
            s.excess_leak[i] = rec.excess_leak as f32;
            s.leak_flow[i] = rec.excess_leak as f32;
            s.snore[i] = rec.snore as f32;
            s.flow_amplitude[i] = rec.flow_amplitude as f32;
        }
        let Some(time) = rec.time else {
            invalid += 1;
            continue;
        };
        valid += 1;
        if first.is_none() {
            first = Some(time.clone());
        }
        if let Some(prev) = &last {
            if time.epoch_sec - prev.epoch_sec > PAUSE_GAP_SECONDS {
                close_session(&mut sessions, &interval, prev);
                interval = Some(time.clone());
            }
        }
        if interval.is_none() {
            interval = Some(time.clone());
        }
        last = Some(time);
        let pi = ((rec.set_pressure * 10.).round() as usize).min(ph.len() - 1);
        ph[pi] += 1;
        ps += rec.set_pressure;
        pmin = pmin.min(rec.set_pressure);
        pmax = pmax.max(rec.set_pressure);
        lh[rec.total_leak as usize] += 1;
        lmax = lmax.max(rec.total_leak);
        let ei = ((rec.excess_leak * 10.).round().max(0.) as usize).min(eh.len() - 1);
        eh[ei] += 1;
        emax = emax.max(rec.excess_leak);
    }
    if let Some(t) = &last {
        close_session(&mut sessions, &interval, t);
    }
    let usage_seconds = sessions
        .iter()
        .map(|s| s.end.epoch_sec - s.start.epoch_sec)
        .sum::<i64>();
    let total_leak = LeakStats {
        median: percentile(&lh, valid, 0.5),
        p95: percentile(&lh, valid, 0.95),
        max: lmax,
    };
    let excess_leak = LeakStats {
        median: percentile(&eh, valid, 0.5) / 10.,
        p95: percentile(&eh, valid, 0.95) / 10.,
        max: round(emax, 10.),
    };
    DatSummary {
        serial: header.serial,
        start: first,
        end: last.as_ref().map(|t| add_seconds(t, 1)),
        record_count: n,
        invalid_records: invalid,
        record_seconds: valid,
        usage_seconds,
        usage_minutes: usage_seconds as f64 / 60.,
        sessions,
        pressure: PressureStats {
            min: if valid > 0 { round(pmin, 10.) } else { 0. },
            mean: if valid > 0 {
                round(ps / valid as f64, 1000.)
            } else {
                0.
            },
            p90: percentile(&ph, valid, 0.9) / 10.,
            p95: percentile(&ph, valid, 0.95) / 10.,
            max: if valid > 0 { round(pmax, 10.) } else { 0. },
        },
        leak: total_leak.clone(),
        total_leak,
        leak_flow: excess_leak.clone(),
        excess_leak,
        series,
    }
}
