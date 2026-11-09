use crate::{
    store::{AppResult, Store},
    types::*,
};
use sha2::{Digest, Sha256};
use sleeplens_core::{
    detect::As2xDetection, detect_as2x, refresh_backup_fields, stream_card_with_backup, CardSource,
    FsCardSource,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::UNIX_EPOCH,
};

pub struct PreparedCard {
    source: FsCardSource,
    root: PathBuf,
    detection: As2xDetection,
}
pub fn prepare_card(path: &Path) -> AppResult<PreparedCard> {
    let root = path
        .canonicalize()
        .map_err(|_| "The selected folder could not be opened.".to_string())?;
    let source = FsCardSource::new(&root);
    let names = source
        .list()
        .map_err(|_| "The selected folder could not be listed.".to_string())?;
    let detection = detect_as2x(&names);
    if !detection.detected {
        return Err("No supported Aeonmed AS2X card was found in this folder.".into());
    }
    Ok(PreparedCard {
        source,
        root,
        detection,
    })
}
pub fn lock_store(store: &Mutex<Store>) -> AppResult<std::sync::MutexGuard<'_, Store>> {
    store
        .lock()
        .map_err(|_| "The database is unavailable. Restart SleepLens.".into())
}
pub fn new_result(id: &str) -> ImportResult {
    ImportResult {
        import_id: id.into(),
        status: "ok".into(),
        nights_added: 0,
        nights_updated: 0,
        nights_skipped: 0,
        nights_failed: Vec::new(),
        message: None,
    }
}
pub fn progress(phase: &str, done: usize, total: usize, date: Option<String>) -> ImportProgress {
    ImportProgress {
        phase: phase.into(),
        done,
        total,
        current_date: date,
        message: None,
    }
}
/// No GUI or app-data paths: callers supply an isolated store, card and event sink.
pub fn import_prepared(
    store: &Arc<Mutex<Store>>,
    card: PreparedCard,
    cancel: &AtomicBool,
    result: &mut ImportResult,
    mut emit: impl FnMut(ImportProgress),
) -> AppResult<()> {
    emit(progress("scanning", 0, 0, None));
    let total = card.detection.nights.len();
    emit(progress("summaries", 0, total, None));
    let backup_bytes = card
        .detection
        .bkp
        .as_ref()
        .map(|p| card.source.read(p))
        .transpose()
        .map_err(|_| "The card backup log could not be read.".to_string())?;
    let backup = backup_bytes.as_deref().map(sleeplens_core::bkp::parse_bkp);
    let device_id = format!(
        "{:x}",
        Sha256::digest(card.detection.serial.as_deref().unwrap_or("").as_bytes())
    );
    let mut done = 0;
    for file in &card.detection.nights {
        if cancel.load(Ordering::Acquire) {
            result.status = "cancelled".into();
            break;
        }
        let date = format!(
            "{}-{}-{}",
            &file.date[..4],
            &file.date[4..6],
            &file.date[6..]
        );
        emit(progress("nights", done, total, Some(date.clone())));
        let outcome = (|| -> AppResult<()> {
            let meta = std::fs::metadata(card.root.join(&file.path))
                .map_err(|_| "Night file metadata could not be read.".to_string())?;
            let mtime = meta
                .modified()
                .map_err(|_| "Night modification time is unavailable.".to_string())?
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "Night modification time is invalid.".to_string())?
                .as_nanos()
                .to_string();
            // Preserve the relative layout but replace its serial-bearing folder prefix.
            let key = format!("{device_id}/{}/{}.dat", &file.date[..6], file.date);
            let mut db = lock_store(store)?;
            if db.unchanged(&key, &date, &device_id, meta.len(), &mtime)? {
                if let Some(backup) = &backup {
                    let mut cached = db.cached_night(&date)?;
                    if refresh_backup_fields(&mut cached, backup) {
                        let device = db.device_for_night(&date)?;
                        db.save_night(&cached, (&device_id, &device), None)?;
                    }
                }
                result.nights_skipped += 1;
                return Ok(());
            }
            let existed = db.exists(&date)?;
            drop(db);
            let mut one = card.detection.clone();
            one.nights = vec![file.clone()];
            let mut parsed = None;
            let metadata = stream_card_with_backup(&card.source, &one, backup.as_ref(), |night| {
                parsed = Some(night)
            })
            .map_err(|_| "Night data could not be parsed.".to_string())?;
            let night = parsed.ok_or("No night data was decoded.")?;
            let device = metadata.device.ok_or("No device was decoded.")?;
            lock_store(store)?.save_night(
                &night,
                (&device_id, &device),
                Some((&key, meta.len(), &mtime)),
            )?;
            if existed {
                result.nights_updated += 1;
            } else {
                result.nights_added += 1;
            }
            Ok(())
        })();
        if let Err(reason) = outcome {
            result.nights_failed.push(NightFailure {
                date: date.clone(),
                reason,
            });
        }
        done += 1;
        emit(progress("nights", done, total, Some(date)));
    }
    if cancel.load(Ordering::Acquire) {
        result.status = "cancelled".into();
    }
    lock_store(store)?.reconcile_settings()?;
    Ok(())
}
