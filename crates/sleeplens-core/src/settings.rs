//! C5 C5 therapy settings blocks.
use crate::time::{read_timestamp, LocalTime};
use serde::{Deserialize, Serialize};
pub const BLOCK_SIZE: usize = 128;
pub const THERAPY_MODES: [&str; 5] = ["CPAP", "Auto CPAP", "AUTOS", "S", "ST"];
pub fn therapy_mode_label(mode: u8) -> &'static str {
    THERAPY_MODES
        .get(mode as usize)
        .copied()
        .unwrap_or("unknown")
}
pub fn pressure_unit_label(unit: u8) -> &'static str {
    if unit == 1 {
        "hPa"
    } else {
        "unknown"
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub time: Option<LocalTime>,
    pub pressure_fields: Vec<u16>,
    pub mode: u8,
    pub mode_label: String,
    pub start_pressure: f64,
    pub fixed_pressure: f64,
    pub min_pressure: f64,
    pub max_pressure: f64,
    pub comfort_time: u8,
    pub comfort_level: u8,
    pub leak_alert: u8,
    pub humidifier_level: u8,
    pub ramp_minutes: u8,
    pub auto_on: bool,
    pub auto_off: bool,
    pub pressure_unit: u8,
    pub pressure_unit_label: String,
    pub raw: Vec<u8>,
}
pub fn parse_settings_block(buf: &[u8], offset: usize) -> Option<SettingsSnapshot> {
    let b = buf.get(offset..offset.checked_add(BLOCK_SIZE)?)?;
    if b[..2] != [0xc5, 0xc5] {
        return None;
    }
    let p = |o| i16::from_le_bytes([b[o], b[o + 1]]) as f64 / 10.;
    Some(SettingsSnapshot {
        time: read_timestamp(b, 0x21),
        pressure_fields: [3, 5, 7, 9]
            .iter()
            .map(|&o| u16::from_le_bytes([b[o], b[o + 1]]))
            .collect(),
        mode: b[2],
        mode_label: therapy_mode_label(b[2]).into(),
        start_pressure: p(3),
        fixed_pressure: p(5),
        min_pressure: p(7),
        max_pressure: p(9),
        comfort_time: b[0x14],
        comfort_level: b[0x15] & 15,
        leak_alert: b[0x16],
        humidifier_level: b[0x17],
        ramp_minutes: b[0x18],
        auto_on: b[0x19] != 0,
        auto_off: b[0x1a] != 0,
        pressure_unit: b[0x1d],
        pressure_unit_label: pressure_unit_label(b[0x1d]).into(),
        raw: b.to_vec(),
    })
}
