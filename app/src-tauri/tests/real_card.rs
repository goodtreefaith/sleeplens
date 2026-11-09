//! Opt-in import of a real card. Skipped unless SLEEPLENS_REAL_CARD points at a
//! card folder. Prints counts and the last night only (no serial). Never commit
//! the output.
//!
//!   SLEEPLENS_REAL_CARD="/Volumes/NO NAME" cargo test -p sleeplens-app --release \
//!     --test real_card -- --ignored --nocapture

use sleeplens_app::{
    importer::{import_prepared, new_result, prepare_card},
    store::Store,
};
use std::{
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::Instant,
};

fn import(store: &Arc<Mutex<Store>>, card: &Path, id: &str) -> sleeplens_app::types::ImportResult {
    store.lock().unwrap().begin_import(id).unwrap();
    let mut result = new_result(id);
    import_prepared(
        store,
        prepare_card(card).unwrap(),
        &AtomicBool::new(false),
        &mut result,
        |_| {},
    )
    .unwrap();
    store.lock().unwrap().finish_import(&result).unwrap();
    result
}

#[test]
#[ignore]
fn import_real_card() {
    let Ok(card) = std::env::var("SLEEPLENS_REAL_CARD") else {
        return;
    };
    let card = PathBuf::from(card);
    let temp = tempfile::TempDir::new().unwrap();
    let db_path = temp.path().join("sleeplens.db");
    let store = Arc::new(Mutex::new(Store::open(&db_path).unwrap()));

    let t = Instant::now();
    let first = import(&store, &card, "first");
    println!(
        "first import: added {} updated {} skipped {} failed {} in {:.1}s",
        first.nights_added,
        first.nights_updated,
        first.nights_skipped,
        first.nights_failed.len(),
        t.elapsed().as_secs_f32()
    );
    for f in first.nights_failed.iter().take(5) {
        println!("  failed {}: {}", f.date, f.reason);
    }
    println!(
        "db size: {:.1} MB",
        std::fs::metadata(&db_path).unwrap().len() as f64 / 1e6
    );

    let s = store.lock().unwrap();
    let nights = s.list_nights(None, None).unwrap();
    let last = nights.last().unwrap();
    println!(
        "nights {} | last {} usage {} ({:?}) mean {:?} p90 {:?} ({:?}) ahi {:?} largeLeak {:?} excessLeak {:?}",
        nights.len(),
        last.date,
        last.usage_minutes,
        last.usage_source,
        last.pressure_mean,
        last.pressure_p90,
        last.pressure_source,
        last.ahi,
        last.large_leak_minutes,
        last.excess_leak_median
    );
    let d = s.night_detail(&last.date).unwrap();
    println!(
        "summary ai {:?} cai {:?} hi {:?} sni {:?} | series {}",
        d.summary.ai,
        d.summary.cai,
        d.summary.hi,
        d.summary.sni,
        d.series.as_ref().map(|x| x.pressure.len()).unwrap_or(0)
    );
    let info = s.app_info().unwrap();
    println!(
        "device {:?}",
        info.device
            .map(|d| (d.model, d.firmware, d.serial_masked, d.pressure_unit))
    );
    drop(s);

    let t = Instant::now();
    let second = import(&store, &card, "second");
    println!(
        "re-import: added {} updated {} skipped {} in {:.1}s",
        second.nights_added,
        second.nights_updated,
        second.nights_skipped,
        t.elapsed().as_secs_f32()
    );
}
