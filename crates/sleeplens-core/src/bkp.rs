//! Aeonmed's 128-byte backup-log blocks.
use crate::settings::{parse_settings_block, SettingsSnapshot};
use crate::time::{make_local_time, read_timestamp, LocalTime};
use serde::{Deserialize, Serialize};

const BLOCK_SIZE: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TherapyEvent {
    pub time: LocalTime,
    pub event_type: String,
    pub code: u8,
    pub value: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pressure: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryPressure {
    /// Deprecated alias of start_pressure, rather than an observed minimum.
    pub min: f64,
    pub mean: f64,
    pub p90: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub session_number: u16,
    pub start: Option<LocalTime>,
    pub usage_minutes: f64,
    pub start_pressure: f64,
    pub pressure: SummaryPressure,
    pub observed_max_pressure: f64,
    pub large_leak_minutes: f64,
    pub snore_index: f64,
    pub ahi: f64,
    pub apnea_index: f64,
    pub central_apnea_index: f64,
    /// Deprecated alias of central_apnea_index.
    pub apnea2_index: f64,
    pub obstructive_apnea_index: f64,
    pub hypopnea_index: f64,
    pub has_b5_marker: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextEvent {
    pub time: LocalTime,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CounterRecord {
    pub date: String,
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub counter_a: u32,
    pub counter_b: u32,
    pub counter_c: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BkpStats {
    pub blocks: usize,
    pub empty_blocks: usize,
    pub unknown_blocks: usize,
    pub counter_blocks: usize,
    pub skipped_entries: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BkpParseResult {
    pub settings: Vec<SettingsSnapshot>,
    pub sessions: Vec<SessionSummary>,
    pub events: Vec<TherapyEvent>,
    pub text_events: Vec<TextEvent>,
    pub counters: Vec<CounterRecord>,
    pub stats: BkpStats,
}

/// Both firmware event-code families share their low-nibble meanings.
pub fn event_type_for_code(code: u8) -> Option<&'static str> {
    if !matches!(code & 0xf0, 0xa0 | 0xe0) {
        return None;
    }
    Some(match code {
        0xe8 => "level",
        _ => match code & 0x0f {
            0 => "pressureChange",
            1 => "obstructiveApnea",
            2 => "centralApnea",
            3 => "hypopnea",
            4 => "snoreLevel",
            5 => "largeLeak",
            6 => "flowLimitation",
            7 => "snoreCount",
            14 => "treatmentStart",
            15 => "treatmentEnd",
            _ => "unknown",
        },
    })
}

/// Decode an eight-byte entry, including the older u16-year layout.
pub fn parse_therapy_entry(buf: &[u8], offset: usize) -> Option<TherapyEvent> {
    let b = buf.get(offset..offset.checked_add(8)?)?;
    if b[0] == 0xff {
        return None;
    }
    let event_type = event_type_for_code(b[4])?;
    let old_layout = b[1] == 7 && b[0] >= 0xd0;
    let year = if old_layout {
        i32::from(u16::from_le_bytes([b[0], b[1]]))
    } else {
        2000 + i32::from(b[0])
    };
    let time = make_local_time(
        year,
        u32::from(b[2]),
        u32::from(b[3]),
        u32::from(b[5]),
        u32::from(b[6]),
        if old_layout { 0 } else { u32::from(b[1]) },
    )?;
    Some(TherapyEvent {
        time,
        event_type: event_type.into(),
        code: b[4],
        value: b[7],
        pressure: matches!(b[4] & 0xf, 0..=3).then_some(f64::from(b[7]) / 2.0),
    })
}

fn u16_at(buf: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buf[offset], buf[offset + 1]])
}

fn parse_session_summary(b: &[u8]) -> Option<SessionSummary> {
    let start = read_timestamp(b, 6)?;
    let idx = |o| match u16_at(b, o) {
        0xffff => 0.0,
        n => f64::from(n) / 10.0,
    };
    let start_pressure = f64::from(i16::from_le_bytes([b[0x4c], b[0x4d]])) / 10.0;
    let apnea_index = idx(0x5a);
    let central_apnea_index = idx(0x5c);
    Some(SessionSummary {
        session_number: u16_at(b, 4),
        start: Some(start),
        usage_minutes: f64::from(u16_at(b, 0x54)),
        start_pressure,
        pressure: SummaryPressure {
            min: start_pressure,
            mean: f64::from(u16_at(b, 0x4e)) / 10.0,
            p90: f64::from(u16_at(b, 0x50)) / 10.0,
        },
        observed_max_pressure: idx(0x5e),
        large_leak_minutes: f64::from(u16_at(b, 0x52)) / 10.0,
        snore_index: f64::from(u16_at(b, 0x56)) / 10.0,
        ahi: f64::from(u16_at(b, 0x58)) / 10.0,
        apnea_index,
        central_apnea_index,
        apnea2_index: central_apnea_index,
        obstructive_apnea_index: (((apnea_index - central_apnea_index) * 10.0).round() / 10.0)
            .max(0.0),
        hypopnea_index: idx(0x60),
        has_b5_marker: b[0x40..0x42] == [0xb5, 0xb5],
    })
}

// The outer Option distinguishes malformed records from empty ones.
fn parse_counter_record(b: &[u8]) -> Option<Option<CounterRecord>> {
    if b.iter().all(|&v| v == 0xff) || b.iter().all(|&v| v == 0) {
        return Some(None);
    }
    if b[0x3f] != 0x88 {
        return None;
    }
    let year = u16_at(b, 0);
    let month = b[2];
    let day = b[3];
    // NeoRead's counter records validate ranges, but not calendar overflow.
    if !(2000..=2100).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let u32_at = |o| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let counter_a = u32_at(4);
    let counter_b = u32_at(8);
    let counter_c = u32_at(12);
    if counter_b.checked_sub(1) != Some(counter_c) {
        return None;
    }
    Some(Some(CounterRecord {
        date: format!("{year:04}-{month:02}-{day:02}"),
        year,
        month,
        day,
        counter_a,
        counter_b,
        counter_c,
    }))
}

fn parse_counter_block(b: &[u8]) -> Option<Vec<CounterRecord>> {
    let mut out = Vec::new();
    for record in b.as_chunks::<64>().0 {
        if let Some(record) = parse_counter_record(record)? {
            out.push(record);
        }
    }
    (!out.is_empty()).then_some(out)
}

fn parse_event_block(b: &[u8]) -> Option<(Vec<TherapyEvent>, usize)> {
    let mut events = Vec::new();
    let mut skipped = 0;
    for offset in (0..BLOCK_SIZE).step_by(8) {
        if let Some(event) = parse_therapy_entry(b, offset) {
            events.push(event);
        } else if b[offset] == 0xff {
            skipped += 1;
        } else {
            return None;
        }
    }
    (!events.is_empty()).then_some((events, skipped))
}

/// Parse whole blocks of a backup log. A trailing partial block is ignored.
/// Unknown blocks are counted, and no partially valid event/counter block is accepted.
pub fn parse_bkp(buf: &[u8]) -> BkpParseResult {
    let mut out = BkpParseResult::default();
    for b in buf.as_chunks::<BLOCK_SIZE>().0 {
        out.stats.blocks += 1;
        match &b[..2] {
            [0xff, 0xff] => out.stats.empty_blocks += 1,
            [0xc5, 0xc5] => {
                if let Some(settings) = parse_settings_block(b, 0) {
                    out.settings.push(settings);
                } else {
                    out.stats.unknown_blocks += 1;
                }
            }
            [0xa5, 0xa5] => {
                if let Some(session) = parse_session_summary(b) {
                    out.sessions.push(session);
                } else {
                    out.stats.unknown_blocks += 1;
                }
            }
            _ => {
                if let Some(records) = parse_counter_block(b) {
                    out.counters.extend(records);
                    out.stats.counter_blocks += 1;
                    continue;
                }
                if let Some(time) = read_timestamp(b, 0).filter(|_| b[7] == 0xff) {
                    let text = b[8..]
                        .iter()
                        .take_while(|&&v| v != 0 && v != 0xff)
                        .map(|&v| char::from(v))
                        .collect();
                    out.text_events.push(TextEvent { time, text });
                    continue;
                }
                if let Some((events, skipped)) = parse_event_block(b) {
                    out.events.extend(events);
                    out.stats.skipped_entries += skipped;
                } else {
                    out.stats.unknown_blocks += 1;
                }
            }
        }
    }
    out.events.sort_by_key(|v| v.time.epoch_sec);
    out.settings
        .sort_by_key(|v| v.time.as_ref().map_or(0, |t| t.epoch_sec));
    out.sessions
        .sort_by_key(|v| v.start.as_ref().map_or(0, |t| t.epoch_sec));
    out.text_events.sort_by_key(|v| v.time.epoch_sec);
    out.counters
        .sort_by(|a, b| a.date.cmp(&b.date).then(a.counter_b.cmp(&b.counter_b)));
    out
}
