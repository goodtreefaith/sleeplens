use crate::types::CardCandidate;
use sleeplens_core::{detect_as2x, CardSource, FsCardSource};
use std::path::PathBuf;
use tauri::Manager;

pub fn data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}
pub fn sample_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("sample-card"))
}
#[cfg(target_os = "macos")]
fn volumes() -> Vec<PathBuf> {
    std::fs::read_dir("/Volumes")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}
#[cfg(target_os = "windows")]
fn volumes() -> Vec<PathBuf> {
    ('A'..='Z')
        .map(|c| PathBuf::from(format!("{c}:\\")))
        .filter(|p| p.is_dir())
        .collect()
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn volumes() -> Vec<PathBuf> {
    Vec::new()
}

pub fn find_cards() -> Vec<CardCandidate> {
    volumes()
        .into_iter()
        .filter_map(|path| {
            // Detection lists names and never opens night files or backup contents.
            let detection = detect_as2x(&FsCardSource::new(&path).list().ok()?);
            detection.detected.then(|| CardCandidate {
                label: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                path: path.to_string_lossy().into_owned(),
                family: "AS2X",
                nights: detection.nights.len(),
                months: detection.months,
            })
        })
        .collect()
}

/// App timestamps follow the contract's local, zone-free wall-clock format.
pub fn timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}
