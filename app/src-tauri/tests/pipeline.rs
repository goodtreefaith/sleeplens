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
    let original_stages = store
        .lock()
        .unwrap()
        .night_detail("2030-01-15")
        .unwrap()
        .stages;
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
    assert_eq!(first.stages, original_stages);
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
    assert!(!nights[1].prescription_changed);
    assert!(!nights[2].prescription_changed);
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

#[test]
fn fixture_v2_stages_tips_and_calendar_period() {
    let temp = TempDir::new().unwrap();
    let store = db(&temp);
    run(&store, &fixture(), "v2");
    let s = store.lock().unwrap();
    assert_eq!(s.app_info().unwrap().needs_reread, 0);
    for item in s.list_nights(None, None).unwrap() {
        let night = s.night_detail(&item.date).unwrap();
        // These fixture nights are shorter than the contract's 30-minute minimum.
        assert!(night.score.is_none());
        assert!(item.sleep_score.is_none());
        assert!(item.excess_leak_p95.is_some());
        assert_eq!(item.interruptions, night.sessions.len().saturating_sub(1));
        let stages = night.stages.unwrap();
        assert_eq!(stages.epoch_seconds, 30);
        assert!(!stages.stages.is_empty());
        assert_eq!(
            stages.estimator_version,
            sleeplens_core::sleep_stages::ESTIMATOR_VERSION
        );
        assert!(stages.signal_quality.is_finite());
        assert!(!night.tips.is_empty());
        assert!(night.tips.len() <= 4);
        assert!(night.tips.iter().any(|tip| tip.id == "short-night"));
        println!(
            "{} score {:?}, tips {:?}",
            item.date,
            item.sleep_score,
            night.tips.iter().map(|t| t.id.as_str()).collect::<Vec<_>>()
        );
    }
    let paused = s.night_detail("2030-01-17").unwrap();
    assert!(paused
        .stages
        .unwrap()
        .stages
        .contains(&sleeplens_core::SleepStage::NoData));
    let summary = s.period_summary("week", None).unwrap();
    assert_eq!(summary.from, "2030-01-11");
    assert_eq!(summary.to, "2030-01-17");
    assert_eq!(summary.days_in_range, 7);
    assert_eq!(summary.nights, 3);
    assert_eq!(summary.valid_nights, 0);
    assert_eq!(summary.compliance_pct, 0.0);
    assert_eq!(summary.score_series.len(), 7);
    assert_eq!(
        summary
            .score_series
            .iter()
            .filter(|p| p.usage_minutes > 0.0)
            .count(),
        3
    );
    assert!(summary.score_series.iter().all(|p| p.sleep_score.is_none()));
    assert!(summary.previous.avg_usage_minutes.is_none());
    assert!(summary.previous.compliance_pct.is_none());
    assert!(summary.best_night.is_none());
    assert!(summary.worst_night.is_none());
    assert!(summary.avg_stage_minutes.is_some());
    assert!(!summary.tips.is_empty());
    assert!(summary.tips.len() <= 5);
    assert!(s.period_summary("invalid", None).is_err());
    assert!(s.period_summary("week", Some("bad")).is_err());
    for (period, days) in [("month", 30), ("6months", 182), ("year", 365)] {
        let period = s.period_summary(period, Some("2030-01-17")).unwrap();
        assert_eq!(period.days_in_range, days);
        assert_eq!(period.score_series.len(), days);
    }
}

#[test]
fn stale_estimator_rereads_unchanged_dat_not_just_backup() {
    let temp = TempDir::new().unwrap();
    let store = db(&temp);
    run(&store, &fixture(), "first");
    let conn = rusqlite::Connection::open(temp.path().join("sleeplens.db")).unwrap();
    conn.execute(
        "UPDATE nights SET estimator_version=-1 WHERE date='2030-01-15'",
        [],
    )
    .unwrap();
    assert_eq!(store.lock().unwrap().app_info().unwrap().needs_reread, 1);
    let result = run(&store, &fixture(), "reread");
    assert_eq!(result.nights_updated, 1);
    assert_eq!(result.nights_skipped, 2);
    assert!(result.nights_failed.is_empty());
    assert_eq!(store.lock().unwrap().app_info().unwrap().needs_reread, 0);
}

#[test]
fn v1_database_migrates_without_losing_nights() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("sleeplens.db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE devices(id TEXT PRIMARY KEY,json TEXT NOT NULL);
        CREATE TABLE nights(date TEXT PRIMARY KEY,device TEXT NOT NULL REFERENCES devices(id),start TEXT,end TEXT,
        usage_minutes REAL NOT NULL,usage_source TEXT NOT NULL,ahi REAL,pressure_mean REAL,pressure_p90 REAL,
        pressure_source TEXT NOT NULL,excess_leak_median REAL,large_leak_minutes REAL,event_counts TEXT NOT NULL,
        settings_changed INTEGER NOT NULL,summary TEXT NOT NULL,settings TEXT,list_json TEXT NOT NULL);
        CREATE TABLE sessions(date TEXT NOT NULL REFERENCES nights(date) ON DELETE CASCADE,position INTEGER NOT NULL,json TEXT NOT NULL,PRIMARY KEY(date,position));
        CREATE TABLE events(date TEXT NOT NULL REFERENCES nights(date) ON DELETE CASCADE,position INTEGER NOT NULL,json TEXT NOT NULL,PRIMARY KEY(date,position));
        CREATE TABLE series(date TEXT PRIMARY KEY REFERENCES nights(date) ON DELETE CASCADE,blob BLOB NOT NULL);
        CREATE TABLE imports(id TEXT PRIMARY KEY,started_at TEXT NOT NULL,finished_at TEXT,status TEXT NOT NULL,counts TEXT);
        CREATE TABLE file_index(path TEXT PRIMARY KEY,date TEXT NOT NULL REFERENCES nights(date) ON DELETE CASCADE,device TEXT NOT NULL,size INTEGER NOT NULL,mtime TEXT NOT NULL);
        PRAGMA user_version=1;").unwrap();
    let list = serde_json::json!({"date":"2030-01-15","start":"2030-01-15T22:00:00","end":"2030-01-16T05:10:00",
        "usageMinutes":420.0,"usageSource":"machine","ahi":1.0,"pressureMean":6.4,"pressureP90":8.0,
        "pressureSource":"machine","excessLeakMedian":0.0,"largeLeakMinutes":0.0,
        "eventCounts":{"obstructive":0,"central":0,"hypopnea":0},"settingsChanged":false});
    let device = serde_json::json!({"brand":"Aeonmed","model":"AS100A","family":"AS2X","firmware":null,
        "hardware":null,"serialMasked":"•••00","pressureUnit":"hPa"});
    conn.execute(
        "INSERT INTO devices VALUES('device',?1)",
        [device.to_string()],
    )
    .unwrap();
    conn.execute("INSERT INTO nights VALUES('2030-01-15','device','2030-01-15T22:00:00','2030-01-16T05:10:00',420,'\"machine\"',1,6.4,8,'\"machine\"',0,0,'{\"obstructive\":0,\"central\":0,\"hypopnea\":0}',0,'{}',NULL,?1)", [list.to_string()]).unwrap();
    for (position, start, end) in [
        (0, "2030-01-15T22:00:00", "2030-01-16T01:30:00"),
        (1, "2030-01-16T01:40:00", "2030-01-16T05:10:00"),
    ] {
        conn.execute(
            "INSERT INTO sessions VALUES('2030-01-15',?1,?2)",
            rusqlite::params![
                position,
                serde_json::json!({"start":start,"end":end,"minutes":210.0}).to_string()
            ],
        )
        .unwrap();
    }
    drop(conn);
    let store = Store::open(&path).unwrap();
    assert_eq!(store.app_info().unwrap().night_count, 1);
    assert_eq!(store.app_info().unwrap().needs_reread, 1);
    let night = store.night_detail("2030-01-15").unwrap();
    assert!(night.stages.is_none());
    assert_eq!(night.sessions.len(), 2);
    assert_eq!(night.list.interruptions, 1);
    let score = night.score.unwrap();
    assert!(score.total > 90);
    assert!(!score.components.iter().any(|c| c.key == "sleepStages"));
    assert_eq!(night.list.sleep_score, Some(score.total));
    let conn = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn scored_period_compares_previous_and_marks_only_prescription_changes() {
    let temp = TempDir::new().unwrap();
    let store = db(&temp);
    run(&store, &fixture(), "first");
    let mut s = store.lock().unwrap();
    let mut template = s.night_detail("2030-01-15").unwrap();
    let device = s.device_for_night("2030-01-15").unwrap();
    for (date, usage, ahi) in [
        ("2030-01-01", 240.0, 15.0),
        ("2030-01-08", 420.0, 1.0),
        ("2030-01-09", 300.0, 5.0),
    ] {
        template.list.date = date.into();
        template.list.usage_minutes = usage;
        template.list.ahi = Some(ahi);
        template.list.large_leak_minutes = Some(0.0);
        template.list.excess_leak_p95 = Some(0.0);
        template.list.interruptions = 0;
        template.stages = None;
        sleeplens_core::score::refresh_score(&mut template);
        if date == "2030-01-09" {
            template.settings.as_mut().unwrap().max_pressure += 1.0;
        }
        s.save_night(&template, ("test-device", &device), None)
            .unwrap();
    }
    s.reconcile_settings().unwrap();
    let summary = s.period_summary("week", Some("2030-01-14")).unwrap();
    assert_eq!(summary.nights, 2);
    assert_eq!(summary.valid_nights, 2);
    assert_eq!(summary.avg_usage_minutes, Some(360.0));
    assert_eq!(summary.avg_ahi, Some(3.0));
    assert_eq!(summary.previous.avg_usage_minutes, Some(240.0));
    assert_eq!(summary.previous.compliance_pct, Some(0.0));
    assert_eq!(summary.best_night.as_ref().unwrap().date, "2030-01-08");
    assert_eq!(summary.worst_night.as_ref().unwrap().date, "2030-01-09");
    assert!(summary.avg_sleep_score.unwrap() > summary.previous.avg_sleep_score.unwrap());
    let trends = s.trends(Some("2030-01-08"), Some("2030-01-09")).unwrap();
    assert!(!trends.points[0].prescription_changed);
    assert!(trends.points[1].prescription_changed);
    assert!(trends.avg_sleep_score.is_some());
    assert_eq!(trends.avg_excess_leak_p95, Some(0.0));
    let empty = s.period_summary("week", Some("2029-01-01")).unwrap();
    assert_eq!(empty.nights, 0);
    assert!(empty.avg_usage_minutes.is_none());
    assert!(empty.avg_stage_minutes.is_none());
    assert_eq!(empty.score_series.len(), 7);
}

#[test]
fn prescription_reconciliation_preserves_intrinsic_changes_and_clears_stale_comparisons() {
    let temp = TempDir::new().unwrap();
    let store = db(&temp);
    run(&store, &fixture(), "first");
    let mut s = store.lock().unwrap();
    let mut first = s.night_detail("2030-01-15").unwrap();
    let mut second = s.night_detail("2030-01-16").unwrap();
    let device = s.device_for_night("2030-01-15").unwrap();
    second.settings.as_mut().unwrap().max_pressure += 1.0;
    s.save_night(&second, ("test-device", &device), None)
        .unwrap();
    s.reconcile_settings().unwrap();
    assert!(
        s.night_detail("2030-01-16")
            .unwrap()
            .list
            .prescription_changed
    );
    first.settings = second.settings.clone();
    first.intranight_prescription_changed = true;
    s.save_night(&first, ("test-device", &device), None)
        .unwrap();
    s.reconcile_settings().unwrap();
    assert!(
        s.night_detail("2030-01-15")
            .unwrap()
            .list
            .prescription_changed
    );
    assert!(
        !s.night_detail("2030-01-16")
            .unwrap()
            .list
            .prescription_changed
    );
    let wire = serde_json::to_value(s.night_detail("2030-01-15").unwrap()).unwrap();
    assert!(wire.get("intranightPrescriptionChanged").is_none());
}

#[test]
fn period_tips_use_previous_nights_for_repeated_ahi_context() {
    let temp = TempDir::new().unwrap();
    let store = db(&temp);
    run(&store, &fixture(), "first");
    let mut s = store.lock().unwrap();
    let mut template = s.night_detail("2030-01-15").unwrap();
    let device = s.device_for_night("2030-01-15").unwrap();
    template.list.ahi = Some(6.0);
    template.summary.cai = None;
    template.list.event_counts = sleeplens_core::EventCounts::default();
    for date in ["2029-12-01", "2029-12-20", "2030-01-01"] {
        template.list.date = date.into();
        s.save_night(&template, ("test-device", &device), None)
            .unwrap();
    }
    let summary = s.period_summary("week", Some("2030-01-07")).unwrap();
    assert_eq!(summary.nights, 1);
    assert!(summary.tips.iter().any(|tip| tip.id == "ahi-talk"));
}
