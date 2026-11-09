use sleeplens_app::{
    importer::{import_prepared, new_result, prepare_card},
    store::Store,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tempfile::TempDir;
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/synthetic-as2x")
}
fn db(temp: &TempDir) -> Arc<Mutex<Store>> {
    Arc::new(Mutex::new(
        Store::open(temp.path().join("sleeplens.db")).unwrap(),
    ))
}
fn run(store: &Arc<Mutex<Store>>, path: &Path, id: &str) -> sleeplens_app::types::ImportResult {
    store.lock().unwrap().begin_import(id).unwrap();
    let mut result = new_result(id);
    import_prepared(
        store,
        prepare_card(path).unwrap(),
        &AtomicBool::new(false),
        &mut result,
        |_| {},
    )
    .unwrap();
    store.lock().unwrap().finish_import(&result).unwrap();
    result
}
fn copy_card(temp: &TempDir) -> PathBuf {
    let root = temp.path().join("card");
    std::fs::create_dir_all(root.join("TEST0000203001")).unwrap();
    for path in [
        "TEST0000.bkp",
        "TEST0000203001/20300115.dat",
        "TEST0000203001/20300116.dat",
        "TEST0000203001/20300117.dat",
    ] {
        std::fs::copy(fixture().join(path), root.join(path)).unwrap();
    }
    root
}
#[test]
fn fixture_round_trip_incremental_and_delete() {
    let temp = TempDir::new().unwrap();
    let store = db(&temp);
    let first = run(&store, &fixture(), "first");
    assert_eq!(first.nights_added, 3);
    assert!(first.nights_failed.is_empty());
    {
        let s = store.lock().unwrap();
        let nights = s.list_nights(None, None).unwrap();
        assert_eq!(nights.len(), 3);
        assert_eq!(nights[0].date, "2030-01-15");
        assert_eq!(nights[0].usage_minutes, 11.0);
        assert_eq!(nights[0].pressure_mean, Some(6.4));
        assert_eq!(nights[0].pressure_p90, Some(8.0));
        assert_eq!(nights[0].excess_leak_median, Some(15.0));
        assert_eq!(nights[0].event_counts.obstructive, 1);
        assert_eq!(nights[0].event_counts.central, 1);
        assert_eq!(nights[0].event_counts.hypopnea, 1);
        let first = s.night_detail("2030-01-15").unwrap();
        assert_eq!(first.summary.ai, Some(10.9));
        assert_eq!(first.summary.cai, Some(5.5));
        assert_eq!(first.summary.oai, Some(5.4));
        let paused = s.night_detail("2030-01-17").unwrap();
        assert_eq!(paused.sessions.len(), 2);
        let series = paused.series.unwrap();
        assert_eq!(series.pressure.len(), 2401);
        assert_eq!(series.pressure.iter().filter(|v| v.is_some()).count(), 600);
        assert!(series.pressure[301..2101].iter().all(Option::is_none));
        let trends = s.trends(Some("2030-01-14"), Some("2030-01-18")).unwrap();
        assert_eq!(trends.days_in_range, 5);
        assert_eq!(trends.nights, 3);
        assert_eq!(trends.valid_nights, 0);
        assert_eq!(trends.compliance_pct, 0.0);
        assert_eq!(
            trends.avg_usage_minutes,
            nights.iter().map(|n| n.usage_minutes).sum::<f64>() / 3.0
        );
        assert_eq!(
            trends.avg_ahi,
            Some(nights.iter().map(|n| n.ahi.unwrap()).sum::<f64>() / 3.0)
        );
        assert_eq!(s.settings_history().unwrap().len(), 1);
        let info = s.app_info().unwrap();
        assert_eq!(info.night_count, 3);
        assert_eq!(info.device.unwrap().serial_masked, "•••00");
        let imported_at = info.last_import_at.unwrap();
        assert_eq!(imported_at.len(), 19);
        assert!(chrono::NaiveDateTime::parse_from_str(&imported_at, "%Y-%m-%dT%H:%M:%S").is_ok());
        assert_eq!(
            s.list_nights(Some("2030-01-16"), Some("2030-01-16"))
                .unwrap()
                .len(),
            1
        );
        assert!(s.trends(Some("bad"), None).is_err());
        assert!(s.trends(Some("2030-01-18"), Some("2030-01-15")).is_err());
    }
    let second = run(&store, &fixture(), "second");
    assert_eq!(second.nights_added, 0);
    assert_eq!(second.nights_updated, 0);
    assert_eq!(second.nights_skipped, 3);
    // Full serials must not occur anywhere in SQLite, including the file index.
    let bytes = std::fs::read(temp.path().join("sleeplens.db")).unwrap();
    assert!(!bytes.windows(8).any(|b| b == b"TEST0000"));
    let mut s = store.lock().unwrap();
    assert!(s.delete_all_data(false).is_err());
    assert_eq!(s.app_info().unwrap().night_count, 3);
    s.delete_all_data(true).unwrap();
    assert!(s.list_nights(None, None).unwrap().is_empty());
    assert!(s.app_info().unwrap().device.is_none());
    assert_eq!(s.trends(None, None).unwrap().days_in_range, 0);
    assert!(s.settings_history().unwrap().is_empty());
}
#[test]
fn backup_refresh_and_changed_night_are_incremental() {
    let temp = TempDir::new().unwrap();
    let path = copy_card(&temp);
    let store = db(&temp);
    run(&store, &path, "first");
    let backup = path.join("TEST0000.bkp");
    let mut bytes = std::fs::read(&backup).unwrap();
    // Clear the event blocks while preserving machine summaries and settings.
    for block in bytes.as_chunks_mut::<128>().0 {
        if (0xe0..=0xef).contains(&block[4]) {
            block.fill(0xff);
        }
    }
    std::fs::write(backup, bytes).unwrap();
    let again = run(&store, &path, "refresh");
    assert_eq!(again.nights_skipped, 3);
    let first = store.lock().unwrap().night_detail("2030-01-15").unwrap();
    assert!(first.events.is_empty());
    assert_eq!(first.list.event_counts.central, 0);
    assert_eq!(first.summary.ai, Some(10.9));
    assert!(first.series.is_some());
    let dat = path.join("TEST0000203001/20300116.dat");
    let mut bytes = std::fs::read(&dat).unwrap();
    bytes[0x140 + 0x17] = 5;
    bytes.push(0);
    std::fs::write(dat, bytes).unwrap();
    let changed = run(&store, &path, "changed");
    assert_eq!(changed.nights_updated, 1);
    assert_eq!(changed.nights_skipped, 2);
    let s = store.lock().unwrap();
    let nights = s.list_nights(None, None).unwrap();
    assert!(nights[1].settings_changed);
    assert!(nights[2].settings_changed);
    let history = s.settings_history().unwrap();
    assert_eq!(history.len(), 3);
    assert!(history[1].changed.contains(&"humidifierLevel".into()));
}
#[test]
fn cancel_stops_between_transactions_and_can_resume() {
    let temp = TempDir::new().unwrap();
    let store = db(&temp);
    let cancel = AtomicBool::new(false);
    let mut result = new_result("cancel");
    import_prepared(
        &store,
        prepare_card(&fixture()).unwrap(),
        &cancel,
        &mut result,
        |p| {
            if p.phase == "nights" && p.done == 1 {
                cancel.store(true, Ordering::Release);
            }
        },
    )
    .unwrap();
    assert_eq!(result.status, "cancelled");
    assert_eq!(result.nights_added, 1);
    assert_eq!(store.lock().unwrap().app_info().unwrap().night_count, 1);
    let resume = run(&store, &fixture(), "resume");
    assert_eq!(resume.nights_added, 2);
    assert_eq!(resume.nights_skipped, 1);
}
#[test]
fn bad_night_does_not_abort_other_nights() {
    let temp = TempDir::new().unwrap();
    let path = copy_card(&temp);
    let store = db(&temp);
    let dat = path.join("TEST0000203001/20300115.dat");
    let mut bytes = std::fs::read(&dat).unwrap();
    let end = 0x800 + 599 * 812 + 4;
    bytes[end..end + 2].copy_from_slice(&2100u16.to_le_bytes());
    std::fs::write(dat, bytes).unwrap();
    let result = run(&store, &path, "bad");
    assert_eq!(result.nights_added, 2);
    assert_eq!(result.nights_failed.len(), 1);
    assert_eq!(result.nights_failed[0].date, "2030-01-15");
}
#[test]
fn strict_four_hour_rule_uses_all_calendar_days() {
    let temp = TempDir::new().unwrap();
    let store = db(&temp);
    run(&store, &fixture(), "first");
    let mut s = store.lock().unwrap();
    for (date, usage) in [("2030-01-15", 240.0), ("2030-01-16", 241.0)] {
        let mut night = s.night_detail(date).unwrap();
        night.list.usage_minutes = usage;
        let device = s.device_for_night(date).unwrap();
        s.save_night(&night, ("test-device", &device), None)
            .unwrap();
    }
    let trends = s.trends(Some("2030-01-14"), Some("2030-01-18")).unwrap();
    assert_eq!(trends.valid_nights, 1);
    assert_eq!(trends.compliance_pct, 20.0);
}

#[test]
fn reopened_database_retains_index_and_rejects_newer_schema() {
    let temp = TempDir::new().unwrap();
    let store = db(&temp);
    run(&store, &fixture(), "first");
    drop(store);
    let reopened = db(&temp);
    let repeat = run(&reopened, &fixture(), "reopened");
    assert_eq!(repeat.nights_skipped, 3);
    assert_eq!(
        reopened
            .lock()
            .unwrap()
            .night_detail("2030-01-17")
            .unwrap()
            .series
            .unwrap()
            .pressure
            .len(),
        2401
    );
    drop(reopened);
    let path = temp.path().join("sleeplens.db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.pragma_update(None, "user_version", 99).unwrap();
    drop(conn);
    assert!(Store::open(path).is_err());
}
