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
    #[serde(default)]
    pub excess_leak_p95: Option<f64>,
    #[serde(default)]
    pub sleep_score: Option<u32>,
    #[serde(default)]
    pub interruptions: usize,
    #[serde(default)]
    pub prescription_changed: bool,
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
    #[serde(default)]
    pub score: Option<SleepScore>,
    #[serde(default)]
    pub stages: Option<crate::sleep_stages::SleepStageEstimate>,
    #[serde(default)]
    pub tips: Vec<Tip>,
    #[serde(default, skip_serializing)]
    pub intranight_prescription_changed: bool,
}
pub type NightDetail = Night;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SleepScore {
    pub total: u32,
    pub label: String,
    pub estimated: bool,
    pub components: Vec<ScoreComponent>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreComponent {
    pub key: String,
    pub label: String,
    pub points: f64,
    pub max_points: f64,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tip {
    pub id: String,
    pub severity: String,
    pub title: String,
    pub body: String,
    pub learn_slug: Option<String>,
    #[serde(default)]
    pub evidence: Option<String>,
    #[serde(default)]
    pub evidence_dates: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviousPeriod {
    pub avg_usage_minutes: Option<f64>,
    pub avg_ahi: Option<f64>,
    pub avg_sleep_score: Option<f64>,
    pub compliance_pct: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoredNight {
    pub date: String,
    pub sleep_score: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScorePoint {
    pub date: String,
    pub sleep_score: Option<u32>,
    pub usage_minutes: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodSummary {
    pub period: String,
    pub from: String,
    pub to: String,
    pub days_in_range: usize,
    #[serde(default)]
    pub clamped_to_first_night: bool,
    #[serde(default)]
    pub headline: String,
    pub nights: usize,
    pub valid_nights: usize,
    pub compliance_pct: f64,
    pub avg_usage_minutes: Option<f64>,
    pub avg_ahi: Option<f64>,
    pub avg_sleep_score: Option<f64>,
    pub avg_excess_leak_p95: Option<f64>,
    pub avg_pressure_p90: Option<f64>,
    pub avg_stage_minutes: Option<crate::sleep_stages::StageMinutes>,
    pub previous: PreviousPeriod,
    pub best_night: Option<ScoredNight>,
    pub worst_night: Option<ScoredNight>,
    pub score_series: Vec<ScorePoint>,
    pub tips: Vec<Tip>,
}
