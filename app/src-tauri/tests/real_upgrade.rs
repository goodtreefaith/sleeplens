//! Opt-in upgrade check on a COPY of a real v1 database, then a re-read of the
//! real card. Skipped unless both env vars are set. Prints aggregates only (no
//! serial). Never point SLEEPLENS_DB_COPY at the live database.
//!
//!   SLEEPLENS_DB_COPY=/tmp/copy/sleeplens.db SLEEPLENS_REAL_CARD="/Volumes/NO NAME" \
//!     cargo test -p sleeplens-app --release --test real_upgrade -- --ignored --nocapture

use sleeplens_app::{
    importer::{import_prepared, new_result, prepare_card},
    store::Store,
};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::Instant,
};

fn report(s: &Store, label: &str) {
    let info = s.app_info().unwrap();
    let nights = s.list_nights(None, None).unwrap();
    let last = nights.last().unwrap();
    let d = s.night_detail(&last.date).unwrap();
    println!(
        "[{label}] nights {} needsReread {} | last {} score {:?} leakP95 {:?} interruptions {}",
        nights.len(),
        info.needs_reread,
        last.date,
        d.score.as_ref().map(|x| (x.total, x.label.clone())),
        last.excess_leak_p95,
        last.interruptions
    );
    if let Some(score) = &d.score {
        for c in &score.components {
            println!(
                "    {} {}/{}: {}",
                c.label, c.points, c.max_points, c.detail
            );
        }
    }
    if let Some(st) = &d.stages {
        println!(
            "    stages v{} wake {:.0} light {:.0} deep {:.0} rem {:.0} uncertain {:.0} min, quality {:.2}",
            st.estimator_version,
            st.minutes.wake,
            st.minutes.light,
            st.minutes.deep,
            st.minutes.rem,
            st.minutes.uncertain,
            st.signal_quality
        );
    }
    for t in &d.tips {
        println!(
            "    tip [{}/{}] {}: {} | evidence {:?} dates {:?}",
            t.category, t.severity, t.title, t.body, t.evidence, t.evidence_dates
        );
    }
    for period in ["week", "month", "6months", "year"] {
        let p = s.period_summary(period, None).unwrap();
        println!(
            "    {period}: {}..{} nights {} compliance {:.0}% avgScore {:?} avgAhi {:?} prevScore {:?} best {:?} worst {:?} tips {:?}",
            p.from,
            p.to,
            p.nights,
            p.compliance_pct,
            p.avg_sleep_score.map(|v| (v * 10.0).round() / 10.0),
            p.avg_ahi.map(|v| (v * 100.0).round() / 100.0),
            p.previous.avg_sleep_score.map(|v| (v * 10.0).round() / 10.0),
            p.best_night.as_ref().map(|n| (n.date.clone(), n.sleep_score)),
            p.worst_night.as_ref().map(|n| (n.date.clone(), n.sleep_score)),
            p.tips.iter().map(|t| t.id.clone()).collect::<Vec<_>>()
        );
        println!("    pressure {:?}", p.pressure);
        for tip in p.tips.iter().filter(|t| t.category == "pressure") {
            println!(
                "    pressure tip [{}] {}: {} | evidence {:?} dates {:?}",
                tip.severity, tip.title, tip.body, tip.evidence, tip.evidence_dates
            );
        }
    }
}

#[test]
#[ignore]
fn upgrade_real_database_then_reread() {
    let (Ok(db), Ok(card)) = (
        std::env::var("SLEEPLENS_DB_COPY"),
        std::env::var("SLEEPLENS_REAL_CARD"),
    ) else {
        return;
    };
    let t = Instant::now();
    let store = Arc::new(Mutex::new(Store::open(PathBuf::from(&db)).unwrap()));
    println!("opened + migrated in {:.1}s", t.elapsed().as_secs_f32());
    report(&store.lock().unwrap(), "after migration");

    let t = Instant::now();
    let id = format!(
        "reread-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    store.lock().unwrap().begin_import(&id).unwrap();
    let mut result = new_result(&id);
    import_prepared(
        &store,
        prepare_card(&PathBuf::from(card)).unwrap(),
        &AtomicBool::new(false),
        &mut result,
        |_| {},
    )
    .unwrap();
    store.lock().unwrap().finish_import(&result).unwrap();
    println!(
        "re-read: added {} updated {} skipped {} failed {} in {:.1}s",
        result.nights_added,
        result.nights_updated,
        result.nights_skipped,
        result.nights_failed.len(),
        t.elapsed().as_secs_f32()
    );
    report(&store.lock().unwrap(), "after re-read");
}
