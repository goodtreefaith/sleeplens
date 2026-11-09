pub mod importer;
pub mod platform;
pub mod store;
pub mod types;

use crate::{
    importer::*,
    store::{AppResult, Store},
    types::*,
};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tauri::{Emitter, Manager, State};

struct AppState {
    store: Arc<Mutex<Store>>,
    active: Arc<Mutex<Option<Arc<AtomicBool>>>>,
}
#[tauri::command]
fn app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    lock_store(&state.store)?.app_info()
}
#[tauri::command]
fn find_cards() -> Vec<CardCandidate> {
    platform::find_cards()
}
#[tauri::command]
fn list_nights(
    state: State<'_, AppState>,
    from: Option<String>,
    to: Option<String>,
) -> AppResult<Vec<sleeplens_core::NightListItem>> {
    lock_store(&state.store)?.list_nights(from.as_deref(), to.as_deref())
}
#[tauri::command]
fn night_detail(
    state: State<'_, AppState>,
    date: String,
) -> AppResult<sleeplens_core::NightDetail> {
    lock_store(&state.store)?.night_detail(&date)
}
#[tauri::command]
fn trends(
    state: State<'_, AppState>,
    from: Option<String>,
    to: Option<String>,
) -> AppResult<Trends> {
    lock_store(&state.store)?.trends(from.as_deref(), to.as_deref())
}
#[tauri::command]
fn period_summary(
    state: State<'_, AppState>,
    period: String,
    end: Option<String>,
) -> AppResult<sleeplens_core::PeriodSummary> {
    lock_store(&state.store)?.period_summary(&period, end.as_deref())
}
#[tauri::command]
fn settings_history(state: State<'_, AppState>) -> AppResult<Vec<SettingsChange>> {
    lock_store(&state.store)?.settings_history()
}
#[tauri::command]
fn delete_all_data(state: State<'_, AppState>, confirm: bool) -> AppResult<()> {
    let active = state
        .active
        .lock()
        .map_err(|_| "Import state is unavailable.")?;
    if active.is_some() {
        return Err(
            "Cancel the current import and wait for it to finish before deleting data.".into(),
        );
    }
    lock_store(&state.store)?.delete_all_data(confirm)
}
#[tauri::command]
fn cancel_import(state: State<'_, AppState>) -> AppResult<()> {
    if let Some(cancel) = state
        .active
        .lock()
        .map_err(|_| "Import state is unavailable.")?
        .as_ref()
    {
        cancel.store(true, Ordering::Release);
    }
    Ok(())
}
#[tauri::command]
fn import_card(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> AppResult<ImportStarted> {
    start_import(app, &state, Path::new(&path))
}
#[tauri::command]
fn load_sample_data(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<ImportStarted> {
    let path = platform::sample_dir(&app)?;
    start_import(app, &state, &path)
}
fn start_import(app: tauri::AppHandle, state: &AppState, path: &Path) -> AppResult<ImportStarted> {
    let mut active = state
        .active
        .lock()
        .map_err(|_| "Import state is unavailable.")?;
    if active.is_some() {
        return Err("An import is already running.".into());
    }
    let card = prepare_card(path)?;
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let id = format!(
        "{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    );
    lock_store(&state.store)?.begin_import(&id)?;
    let cancel = Arc::new(AtomicBool::new(false));
    *active = Some(cancel.clone());
    let store = state.store.clone();
    let running = state.active.clone();
    let import_id = id.clone();
    let spawn = std::thread::Builder::new()
        .name("card-import".into())
        .spawn(move || {
            let mut result = new_result(&import_id);
            let mut total = 0;
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                import_prepared(&store, card, &cancel, &mut result, |p| {
                    total = p.total;
                    let _ = app.emit("import-progress", p);
                })
            }));
            let error = match outcome {
                Ok(Ok(())) => None,
                Ok(Err(e)) => Some(e),
                Err(_) => Some("The import stopped unexpectedly.".into()),
            };
            if let Some(message) = error {
                result.status = "error".into();
                result.message = Some(message);
            }
            if let Err(e) = lock_store(&store).and_then(|s| s.finish_import(&result)) {
                result.status = "error".into();
                result.message = Some(e);
            }
            let done = result.nights_added
                + result.nights_updated
                + result.nights_skipped
                + result.nights_failed.len();
            // Release the import slot before notifying listeners that they may start another.
            if let Ok(mut active) = running.lock() {
                *active = None;
            }
            let phase = match result.status.as_str() {
                "cancelled" => "cancelled",
                "error" => "error",
                _ => "done",
            };
            let mut terminal = progress(phase, done, total, None);
            terminal.message = result.message.clone();
            let _ = app.emit("import-progress", terminal);
            let _ = app.emit("import-finished", result);
        });
    if spawn.is_err() {
        *active = None;
        let mut result = new_result(&id);
        result.status = "error".into();
        result.message = Some("Could not start the import worker.".into());
        lock_store(&state.store)?.finish_import(&result)?;
        return Err("Could not start the import worker.".into());
    }
    Ok(ImportStarted { import_id: id })
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = platform::data_dir(app.handle())?;
            let store = Store::open(dir.join("sleeplens.db"))?;
            app.manage(AppState {
                store: Arc::new(Mutex::new(store)),
                active: Arc::new(Mutex::new(None)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            find_cards,
            import_card,
            cancel_import,
            list_nights,
            night_detail,
            trends,
            period_summary,
            settings_history,
            delete_all_data,
            load_sample_data
        ])
        .run(tauri::generate_context!())
        .expect("SleepLens could not start");
}
