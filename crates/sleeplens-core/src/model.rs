//! App-facing JSON types. Full device serials are deliberately absent.
use crate::settings::SettingsSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Machine,
    Computed,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub brand: String,
    pub model: String,
    pub family: String,
    pub firmware: Option<String>,
    pub hardware: Option<String>,
    pub serial_masked: String,
    pub pressure_unit: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventCounts {
    pub obstructive: usize,
    pub central: usize,
    pub hypopnea: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub mode: u8,
    pub mode_label: String,
    pub start_pressure: f64,
    pub min_pressure: f64,
    pub max_pressure: f64,
    pub fixed_pressure: f64,
    pub ramp_minutes: u8,
    pub humidifier_level: u8,
    pub comfort_level: u8,
    pub auto_on: bool,
    pub auto_off: bool,
    pub leak_alert: bool,
    pub pressure_unit: String,
}
impl From<&SettingsSnapshot> for Settings {
    fn from(s: &SettingsSnapshot) -> Self {
        Self {
            mode: s.mode,
            mode_label: s.mode_label.clone(),
            start_pressure: s.start_pressure,
            min_pressure: s.min_pressure,
            max_pressure: s.max_pressure,
            fixed_pressure: s.fixed_pressure,
            ramp_minutes: s.ramp_minutes,
            humidifier_level: s.humidifier_level,
            comfort_level: s.comfort_level,
            auto_on: s.auto_on,
            auto_off: s.auto_off,
            leak_alert: s.leak_alert != 0,
            pressure_unit: s.pressure_unit_label.clone(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TherapyEvent {
    pub time: String,
    pub kind: String,
    pub code: u8,
    pub value: u8,
    pub pressure: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub start: String,
    pub end: String,
    pub minutes: f64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NightSummary {
    pub start_pressure: Option<f64>,
    pub max_pressure: Option<f64>,
    pub ai: Option<f64>,
    pub cai: Option<f64>,
    pub oai: Option<f64>,
    pub hi: Option<f64>,
    pub sni: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Series {
    pub series_start: String,
    pub pressure: Vec<Option<f32>>,
    pub excess_leak: Vec<Option<f32>>,
    pub total_leak: Vec<Option<f32>>,
    pub flow_amplitude: Vec<Option<f32>>,
    pub snore: Vec<Option<f32>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NightListItem {
    pub date: String,
    pub start: Option<String>,
    pub end: Option<String>,
    pub usage_minutes: f64,
    pub usage_source: Source,
    pub ahi: Option<f64>,
    pub pressure_mean: Option<f64>,
    pub pressure_p90: Option<f64>,
    pub pressure_source: Source,
    pub excess_leak_median: Option<f64>,
    pub large_leak_minutes: Option<f64>,
    pub event_counts: EventCounts,
    pub settings_changed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Night {
    #[serde(flatten)]
    pub list: NightListItem,
    pub sessions: Vec<Session>,
    pub summary: NightSummary,
    pub settings: Option<Settings>,
    pub events: Vec<TherapyEvent>,
    pub series: Option<Series>,
}
pub type NightDetail = Night;
