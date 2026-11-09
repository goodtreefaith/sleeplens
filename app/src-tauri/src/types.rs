use serde::{Deserialize, Serialize};
use sleeplens_core::{Device, Settings};

// Command responses share the core contract, including v2.1 period and tip fields.
pub use sleeplens_core::PeriodSummary;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: &'static str,
    pub data_dir: String,
    pub db_size_bytes: u64,
    pub device: Option<Device>,
    pub night_count: usize,
    pub needs_reread: usize,
    pub first_date: Option<String>,
    pub last_date: Option<String>,
    pub last_import_at: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardCandidate {
    pub path: String,
    pub label: String,
    pub family: &'static str,
    pub nights: usize,
    pub months: Vec<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportStarted {
    pub import_id: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NightFailure {
    pub date: String,
    pub reason: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub import_id: String,
    pub status: String,
    pub nights_added: usize,
    pub nights_updated: usize,
    pub nights_skipped: usize,
    pub nights_failed: Vec<NightFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub phase: String,
    pub done: usize,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsChange {
    pub date: String,
    pub time: Option<String>,
    pub settings: Settings,
    pub changed: Vec<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendPoint {
    pub date: String,
    pub usage_minutes: f64,
    pub ahi: Option<f64>,
    pub pressure_p90: Option<f64>,
    pub excess_leak_median: Option<f64>,
    pub settings_changed: bool,
    pub sleep_score: Option<u32>,
    pub excess_leak_p95: Option<f64>,
    pub prescription_changed: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trends {
    pub from: String,
    pub to: String,
    pub nights: usize,
    pub days_in_range: i64,
    pub valid_nights: usize,
    pub compliance_pct: f64,
    pub avg_usage_minutes: f64,
    pub avg_ahi: Option<f64>,
    pub avg_pressure_p90: Option<f64>,
    pub avg_excess_leak: Option<f64>,
    pub avg_sleep_score: Option<f64>,
    pub avg_excess_leak_p95: Option<f64>,
    pub points: Vec<TrendPoint>,
}
