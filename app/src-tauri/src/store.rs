use crate::types::*;
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use rusqlite::{params, Connection, OptionalExtension};
use sleeplens_core::sleep_stages::{
    SleepStage, SleepStageEstimate, StageMinutes, ESTIMATOR_VERSION,
};
use sleeplens_core::{Device, Night, NightListItem, Series, Settings};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub type AppResult<T> = Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn now() -> String {
    crate::platform::timestamp()
}

pub struct Store {
    path: PathBuf,
    conn: Option<Connection>,
}
impl Store {
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(err)?;
        }
        let conn = Connection::open(&path).map_err(err)?;
        conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA secure_delete=ON;")
            .map_err(err)?;
        let version: i32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(err)?;
        if version > 4 {
            return Err("This database needs a newer version of SleepLens.".into());
        }
        if version == 0 {
            conn.execute_batch("BEGIN;
              CREATE TABLE devices(id TEXT PRIMARY KEY, json TEXT NOT NULL);
              CREATE TABLE nights(date TEXT PRIMARY KEY, device TEXT NOT NULL REFERENCES devices(id),
                start TEXT, end TEXT, usage_minutes REAL NOT NULL, usage_source TEXT NOT NULL,
                ahi REAL, pressure_mean REAL, pressure_p90 REAL, pressure_source TEXT NOT NULL,
                excess_leak_median REAL, large_leak_minutes REAL, event_counts TEXT NOT NULL,
                settings_changed INTEGER NOT NULL, summary TEXT NOT NULL, settings TEXT, list_json TEXT NOT NULL);
              CREATE TABLE sessions(date TEXT NOT NULL REFERENCES nights(date) ON DELETE CASCADE, position INTEGER NOT NULL, json TEXT NOT NULL, PRIMARY KEY(date,position));
              CREATE TABLE events(date TEXT NOT NULL REFERENCES nights(date) ON DELETE CASCADE, position INTEGER NOT NULL, json TEXT NOT NULL, PRIMARY KEY(date,position));
              CREATE TABLE series(date TEXT PRIMARY KEY REFERENCES nights(date) ON DELETE CASCADE, blob BLOB NOT NULL);
              CREATE TABLE imports(id TEXT PRIMARY KEY, started_at TEXT NOT NULL, finished_at TEXT, status TEXT NOT NULL, counts TEXT);
              CREATE TABLE file_index(path TEXT PRIMARY KEY, date TEXT NOT NULL REFERENCES nights(date) ON DELETE CASCADE, device TEXT NOT NULL, size INTEGER NOT NULL, mtime TEXT NOT NULL);
              PRAGMA user_version=1;
              COMMIT;").map_err(err)?;
        }
        let mut store = Self {
            path,
            conn: Some(conn),
        };
        if version < 2 {
            store.migrate_v2()?;
        }
        if version < 3 {
            store.migrate_scores(3)?;
        }
        if version < 4 {
            store.migrate_scores(4)?;
        }
        Ok(store)
    }
    fn migrate_scores(&mut self, schema_version: u32) -> AppResult<()> {
        // Formula changes use the imported fields and stored estimates. They do
        // not invalidate stages or require the original card or waveform series.
        self.db()?.execute_batch("BEGIN;").map_err(err)?;
        let result =
            (|| -> AppResult<()> {
                for item in self.list_nights(None, None)? {
                    let mut night = self.cached_night(&item.date)?;
                    sleeplens_core::score::refresh_score(&mut night);
                    let score_json = night
                        .score
                        .as_ref()
                        .map(serde_json::to_string)
                        .transpose()
                        .map_err(err)?;
                    let list_json = serde_json::to_string(&night.list).map_err(err)?;
                    self.db()?.execute(
                    "UPDATE nights SET sleep_score=?2,score_json=?3,list_json=?4 WHERE date=?1",
                    params![night.list.date, night.list.sleep_score, score_json, list_json],
                ).map_err(err)?;
                }
                self.db()?
                    .execute_batch(&format!("PRAGMA user_version={schema_version}; COMMIT;"))
                    .map_err(err)
            })();
        if result.is_err() {
            let _ = self.db()?.execute_batch("ROLLBACK;");
        }
        result
    }
    fn migrate_v2(&mut self) -> AppResult<()> {
        self.db()?.execute_batch("BEGIN;
            ALTER TABLE nights ADD COLUMN sleep_score REAL;
            ALTER TABLE nights ADD COLUMN excess_leak_p95 REAL;
            ALTER TABLE nights ADD COLUMN interruptions INTEGER NOT NULL DEFAULT 0;
            ALTER TABLE nights ADD COLUMN prescription_changed INTEGER NOT NULL DEFAULT 0;
            ALTER TABLE nights ADD COLUMN intranight_prescription_changed INTEGER NOT NULL DEFAULT 0;
            ALTER TABLE nights ADD COLUMN estimator_version INTEGER NOT NULL DEFAULT 0;
            ALTER TABLE nights ADD COLUMN score_json TEXT;
            CREATE TABLE stages(date TEXT PRIMARY KEY REFERENCES nights(date) ON DELETE CASCADE, blob BLOB NOT NULL);").map_err(err)?;
        let result = (|| -> AppResult<()> {
            for item in self.list_nights(None, None)? {
                let mut night = self.cached_night(&item.date)?;
                night.list.interruptions = night.sessions.len().saturating_sub(1);
                sleeplens_core::score::refresh_score(&mut night);
                self.db()?.execute("UPDATE nights SET sleep_score=?2,interruptions=?3,score_json=?4,list_json=?5 WHERE date=?1", params![night.list.date,night.list.sleep_score,night.list.interruptions,night.score.as_ref().map(serde_json::to_string).transpose().map_err(err)?,serde_json::to_string(&night.list).map_err(err)?]).map_err(err)?;
            }
            self.db()?
                .execute_batch("PRAGMA user_version=2; COMMIT;")
                .map_err(err)
        })();
        if result.is_err() {
            let _ = self.db()?.execute_batch("ROLLBACK;");
        }
        result?;
        self.reconcile_settings()
    }
    fn db(&self) -> AppResult<&Connection> {
        self.conn
            .as_ref()
            .ok_or_else(|| "Database is closed. Retry deleting all data.".into())
    }
    pub fn delete_all_data(&mut self, confirm: bool) -> AppResult<()> {
        if !confirm {
            return Err("Confirm deletion before removing all data.".into());
        }
        if let Some(conn) = self.conn.take() {
            conn.close().map_err(|(_, e)| err(e))?;
        }
        for suffix in ["", "-journal", "-wal", "-shm"] {
            let path = PathBuf::from(format!("{}{suffix}", self.path.to_string_lossy()));
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(err(e)),
            }
        }
        *self = Self::open(&self.path)?;
        Ok(())
    }
    pub fn app_info(&self) -> AppResult<AppInfo> {
        let db = self.db()?;
        let (night_count, first_date, last_date) = db
            .query_row("SELECT count(*),min(date),max(date) FROM nights", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .map_err(err)?;
        let device_json: Option<String> = db.query_row("SELECT d.json FROM devices d JOIN nights n ON d.id=n.device ORDER BY n.date DESC LIMIT 1", [], |r| r.get(0)).optional().map_err(err)?;
        let last_import_at = db
            .query_row(
                "SELECT max(finished_at) FROM imports WHERE finished_at IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        Ok(AppInfo {
            version: env!("CARGO_PKG_VERSION"),
            data_dir: self
                .path
                .parent()
                .unwrap_or(Path::new("."))
                .to_string_lossy()
                .into_owned(),
            db_size_bytes: std::fs::metadata(&self.path).map_err(err)?.len(),
            device: device_json
                .map(|j| serde_json::from_str(&j))
                .transpose()
                .map_err(err)?,
            night_count,
            needs_reread: db.query_row("SELECT count(*) FROM nights n WHERE n.estimator_version < ?1 OR NOT EXISTS(SELECT 1 FROM stages s WHERE s.date=n.date)", [ESTIMATOR_VERSION], |r| r.get(0)).map_err(err)?,
            first_date,
            last_date,
            last_import_at,
        })
    }
    pub fn list_nights(
        &self,
        from: Option<&str>,
        to: Option<&str>,
    ) -> AppResult<Vec<NightListItem>> {
        validate_range(from, to)?;
        let mut stmt = self.db()?.prepare("SELECT list_json FROM nights WHERE (?1 IS NULL OR date>=?1) AND (?2 IS NULL OR date<=?2) ORDER BY date").map_err(err)?;
        let rows = stmt
            .query_map(params![from, to], |r| r.get::<_, String>(0))
            .map_err(err)?;
        rows.map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
            .collect()
    }
    pub fn night_detail(&self, date: &str) -> AppResult<Night> {
        let mut night = self.read_night(date, true)?;
        let recent = self
            .list_nights(None, Some(date))?
            .iter()
            .rev()
            .take(7)
            .map(|n| self.cached_night(&n.date))
            .collect::<AppResult<Vec<_>>>()?;
        night.tips = sleeplens_core::tips::night_tips(&night, &recent);
        Ok(night)
    }
    pub fn cached_night(&self, date: &str) -> AppResult<Night> {
        self.read_night(date, false)
    }
    fn read_night(&self, date: &str, with_series: bool) -> AppResult<Night> {
        validate_date(date)?;
        let db = self.db()?;
        let (list, summary, settings): (String, String, Option<String>) = db
            .query_row(
                "SELECT list_json,summary,settings FROM nights WHERE date=?1",
                [date],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(err)?
            .ok_or("No imported night for this date.")?;
        let mut sessions = db
            .prepare("SELECT json FROM sessions WHERE date=?1 ORDER BY position")
            .map_err(err)?;
        let sessions = sessions
            .query_map([date], |r| r.get::<_, String>(0))
            .map_err(err)?
            .map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
            .collect::<AppResult<_>>()?;
        let mut events = db
            .prepare("SELECT json FROM events WHERE date=?1 ORDER BY position")
            .map_err(err)?;
        let events = events
            .query_map([date], |r| r.get::<_, String>(0))
            .map_err(err)?
            .map(|r| serde_json::from_str(&r.map_err(err)?).map_err(err))
            .collect::<AppResult<_>>()?;
        let series = if with_series {
            let blob: Option<Vec<u8>> = db
                .query_row("SELECT blob FROM series WHERE date=?1", [date], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(err)?;
            blob.map(|b| decode_series(&b)).transpose()?
        } else {
            None
        };
        Ok(Night {
            list: serde_json::from_str(&list).map_err(err)?,
            summary: serde_json::from_str(&summary).map_err(err)?,
            settings: settings
                .map(|s| serde_json::from_str(&s))
                .transpose()
                .map_err(err)?,
            sessions,
            events,
            series,
            score: db
                .query_row("SELECT score_json FROM nights WHERE date=?1", [date], |r| {
                    r.get::<_, Option<String>>(0)
                })
                .map_err(err)?
                .map(|j| serde_json::from_str(&j))
                .transpose()
                .map_err(err)?,
            stages: db
                .query_row("SELECT blob FROM stages WHERE date=?1", [date], |r| {
                    r.get::<_, Vec<u8>>(0)
                })
                .optional()
                .map_err(err)?
                .map(|b| decode_stages(&b))
                .transpose()?,
            tips: Vec::new(),
            intranight_prescription_changed: db
                .query_row(
                    "SELECT intranight_prescription_changed FROM nights WHERE date=?1",
                    [date],
                    |r| r.get(0),
                )
                .map_err(err)?,
        })
    }
    pub fn device_for_night(&self, date: &str) -> AppResult<Device> {
        let json: String = self
            .db()?
            .query_row(
                "SELECT d.json FROM devices d JOIN nights n ON d.id=n.device WHERE n.date=?1",
                [date],
                |r| r.get(0),
            )
            .map_err(err)?;
        serde_json::from_str(&json).map_err(err)
    }
    pub fn exists(&self, date: &str) -> AppResult<bool> {
        self.db()?
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM nights WHERE date=?1)",
                [date],
                |r| r.get(0),
            )
            .map_err(err)
    }
    pub fn unchanged(
        &self,
        key: &str,
        date: &str,
        device: &str,
        size: u64,
        mtime: &str,
    ) -> AppResult<bool> {
        self.db()?.query_row("SELECT EXISTS(SELECT 1 FROM file_index f JOIN nights n ON f.date=n.date WHERE f.path=?1 AND f.date=?2 AND f.device=?3 AND n.device=?3 AND f.size=?4 AND f.mtime=?5 AND n.estimator_version>=?6 AND EXISTS(SELECT 1 FROM stages s WHERE s.date=n.date))", params![key,date,device,size,mtime,ESTIMATOR_VERSION], |r| r.get(0)).map_err(err)
    }
    pub fn save_night(
        &mut self,
        night: &Night,
        device: (&str, &Device),
        file: Option<(&str, u64, &str)>,
    ) -> AppResult<()> {
        let compressed = night.series.as_ref().map(encode_series).transpose()?;
        let tx = self
            .conn
            .as_mut()
            .ok_or("Database is closed.")?
            .transaction()
            .map_err(err)?;
        tx.execute("INSERT INTO devices(id,json) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET json=excluded.json", params![device.0,serde_json::to_string(device.1).map_err(err)?]).map_err(err)?;
        let n = &night.list;
        tx.execute("INSERT INTO nights(date,device,start,end,usage_minutes,usage_source,ahi,pressure_mean,pressure_p90,pressure_source,excess_leak_median,large_leak_minutes,event_counts,settings_changed,summary,settings,list_json,sleep_score,excess_leak_p95,interruptions,prescription_changed,estimator_version,score_json,intranight_prescription_changed) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24)
          ON CONFLICT(date) DO UPDATE SET device=excluded.device,start=excluded.start,end=excluded.end,usage_minutes=excluded.usage_minutes,usage_source=excluded.usage_source,ahi=excluded.ahi,pressure_mean=excluded.pressure_mean,pressure_p90=excluded.pressure_p90,pressure_source=excluded.pressure_source,excess_leak_median=excluded.excess_leak_median,large_leak_minutes=excluded.large_leak_minutes,event_counts=excluded.event_counts,settings_changed=excluded.settings_changed,summary=excluded.summary,settings=excluded.settings,list_json=excluded.list_json,sleep_score=excluded.sleep_score,excess_leak_p95=excluded.excess_leak_p95,interruptions=excluded.interruptions,prescription_changed=excluded.prescription_changed,estimator_version=excluded.estimator_version,score_json=excluded.score_json,intranight_prescription_changed=excluded.intranight_prescription_changed",
          params![n.date,device.0,n.start,n.end,n.usage_minutes,serde_json::to_string(&n.usage_source).map_err(err)?,n.ahi,n.pressure_mean,n.pressure_p90,serde_json::to_string(&n.pressure_source).map_err(err)?,n.excess_leak_median,n.large_leak_minutes,serde_json::to_string(&n.event_counts).map_err(err)?,n.settings_changed,serde_json::to_string(&night.summary).map_err(err)?,night.settings.as_ref().map(serde_json::to_string).transpose().map_err(err)?,serde_json::to_string(n).map_err(err)?,n.sleep_score,n.excess_leak_p95,n.interruptions,n.prescription_changed,night.stages.as_ref().map_or(0, |s| s.estimator_version),night.score.as_ref().map(serde_json::to_string).transpose().map_err(err)?,night.intranight_prescription_changed]).map_err(err)?;
        for table in ["events", "sessions"] {
            tx.execute(&format!("DELETE FROM {table} WHERE date=?1"), [&n.date])
                .map_err(err)?;
        }
        for (i, event) in night.events.iter().enumerate() {
            tx.execute(
                "INSERT INTO events VALUES(?1,?2,?3)",
                params![n.date, i, serde_json::to_string(event).map_err(err)?],
            )
            .map_err(err)?;
        }
        for (i, session) in night.sessions.iter().enumerate() {
            tx.execute(
                "INSERT INTO sessions VALUES(?1,?2,?3)",
                params![n.date, i, serde_json::to_string(session).map_err(err)?],
            )
            .map_err(err)?;
        }
        if let Some(stages) = &night.stages {
            tx.execute("INSERT INTO stages VALUES(?1,?2) ON CONFLICT(date) DO UPDATE SET blob=excluded.blob", params![n.date,encode_stages(stages)?]).map_err(err)?;
        } else {
            tx.execute("DELETE FROM stages WHERE date=?1", [&n.date])
                .map_err(err)?;
        }
        if let Some(blob) = compressed {
            tx.execute("INSERT INTO series VALUES(?1,?2) ON CONFLICT(date) DO UPDATE SET blob=excluded.blob", params![n.date,blob]).map_err(err)?;
        }
        if file.is_some() && night.series.is_none() {
            tx.execute("DELETE FROM series WHERE date=?1", [&n.date])
                .map_err(err)?;
        }
        if let Some((key, size, mtime)) = file {
            tx.execute("INSERT INTO file_index VALUES(?1,?2,?3,?4,?5) ON CONFLICT(path) DO UPDATE SET date=excluded.date,device=excluded.device,size=excluded.size,mtime=excluded.mtime", params![key,n.date,device.0,size,mtime]).map_err(err)?;
        }
        tx.commit().map_err(err)
    }
    pub fn begin_import(&self, id: &str) -> AppResult<()> {
        self.db()?
            .execute(
                "INSERT INTO imports(id,started_at,status) VALUES(?1,?2,'running')",
                params![id, now()],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn finish_import(&self, result: &ImportResult) -> AppResult<()> {
        self.db()?
            .execute(
                "UPDATE imports SET finished_at=?2,status=?3,counts=?4 WHERE id=?1",
                params![
                    result.import_id,
                    now(),
                    result.status,
                    serde_json::to_string(result).map_err(err)?
                ],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn settings_history(&self) -> AppResult<Vec<SettingsChange>> {
        let mut stmt = self
            .db()?
            .prepare(
                "SELECT date,start,settings FROM nights WHERE settings IS NOT NULL ORDER BY date",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(err)?;
        let mut previous: Option<serde_json::Value> = None;
        let mut history = Vec::new();
        for row in rows {
            let (date, time, json) = row.map_err(err)?;
            let settings: Settings = serde_json::from_str(&json).map_err(err)?;
            let value = serde_json::to_value(&settings).map_err(err)?;
            let changed: Vec<_> = value
                .as_object()
                .ok_or("Invalid settings.")?
                .iter()
                .filter(|(k, v)| previous.as_ref().and_then(|p| p.get(*k)) != Some(*v))
                .map(|(k, _)| k.clone())
                .collect();
            if !changed.is_empty() {
                history.push(SettingsChange {
                    date,
                    time,
                    settings,
                    changed,
                });
            }
            previous = Some(value);
        }
        Ok(history)
    }
    pub fn reconcile_settings(&mut self) -> AppResult<()> {
        let nights = self.list_nights(None, None)?;
        let mut prev: Option<Settings> = None;
        let tx = self
            .conn
            .as_mut()
            .ok_or("Database is closed.")?
            .transaction()
            .map_err(err)?;
        for mut n in nights {
            let settings: Option<String> = tx
                .query_row(
                    "SELECT settings FROM nights WHERE date=?1",
                    [&n.date],
                    |r| r.get(0),
                )
                .map_err(err)?;
            let settings: Option<Settings> = settings
                .map(|s| serde_json::from_str(&s))
                .transpose()
                .map_err(err)?;
            n.settings_changed = prev
                .as_ref()
                .is_some_and(|p| settings.as_ref().is_some_and(|s| p != s));
            let intrinsic: bool = tx
                .query_row(
                    "SELECT intranight_prescription_changed FROM nights WHERE date=?1",
                    [&n.date],
                    |r| r.get(0),
                )
                .map_err(err)?;
            n.prescription_changed = intrinsic
                || prev.as_ref().is_some_and(|p| {
                    settings.as_ref().is_some_and(|s| {
                        p.mode != s.mode
                            || p.start_pressure != s.start_pressure
                            || p.min_pressure != s.min_pressure
                            || p.max_pressure != s.max_pressure
                            || p.fixed_pressure != s.fixed_pressure
                    })
                });
            if settings.is_some() {
                prev = settings;
            }
            tx.execute(
                "UPDATE nights SET settings_changed=?2,list_json=?3,prescription_changed=?4 WHERE date=?1",
                params![
                    n.date,
                    n.settings_changed,
                    serde_json::to_string(&n).map_err(err)?, n.prescription_changed
                ],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)
    }
    pub fn period_summary(
        &self,
        period: &str,
        end: Option<&str>,
    ) -> AppResult<sleeplens_core::PeriodSummary> {
        use sleeplens_core::{PeriodSummary, PreviousPeriod, ScorePoint, ScoredNight};
        let days: usize = match period {
            "week" => 7,
            "month" => 30,
            "6months" => 182,
            "year" => 365,
            _ => return Err("Choose week, month, 6months, or year.".into()),
        };
        let info = self.app_info()?;
        let end = end
            .or(info.last_date.as_deref())
            .ok_or("Import nights before viewing a period summary.")?;
        let to = validate_date(end)?;
        let requested_from = to
            .checked_sub_signed(chrono::Duration::days(days as i64 - 1))
            .ok_or("Date range is out of bounds.")?;
        let first = info.first_date.as_deref().map(validate_date).transpose()?;
        // An explicit end before the first import remains an empty historical
        // range rather than producing an inverted date range.
        let from = first
            .filter(|first| *first <= to)
            .map_or(requested_from, |first| requested_from.max(first));
        let clamped_to_first_night = from != requested_from;
        let days_in_range = (to - from).num_days() as usize + 1;
        let previous_to = requested_from
            .pred_opt()
            .ok_or("Date range is out of bounds.")?;
        let previous_from = requested_from
            .checked_sub_signed(chrono::Duration::days(days as i64))
            .ok_or("Date range is out of bounds.")?;
        if previous_from < chrono::NaiveDate::from_ymd_opt(1, 1, 1).expect("valid date") {
            return Err("Date range is out of bounds.".into());
        }
        let from_text = from.format("%Y-%m-%d").to_string();
        let to_text = to.format("%Y-%m-%d").to_string();
        let items = self.list_nights(Some(&from_text), Some(&to_text))?;
        let previous = if first.is_some_and(|first| previous_from < first) {
            Vec::new()
        } else {
            self.list_nights(
                Some(&previous_from.format("%Y-%m-%d").to_string()),
                Some(&previous_to.format("%Y-%m-%d").to_string()),
            )?
        };
        // Read at most one waveform at a time. A year of full 1 Hz series
        // would otherwise stay in memory just to calculate pressure facts.
        let mut nights = Vec::with_capacity(items.len());
        let mut pressure_facts = Vec::with_capacity(items.len());
        for item in &items {
            let mut night = self.read_night(&item.date, true)?;
            pressure_facts.push(sleeplens_core::pressure::night_pressure_facts(&night));
            night.series = None;
            nights.push(night);
        }
        let stage_minutes: Vec<_> = nights
            .iter()
            .filter_map(|n| n.stages.as_ref().map(|s| &s.minutes))
            .collect();
        let avg_stage_minutes = (!stage_minutes.is_empty()).then(|| {
            let count = stage_minutes.len() as f32;
            StageMinutes {
                wake: stage_minutes.iter().map(|m| m.wake).sum::<f32>() / count,
                light: stage_minutes.iter().map(|m| m.light).sum::<f32>() / count,
                deep: stage_minutes.iter().map(|m| m.deep).sum::<f32>() / count,
                rem: stage_minutes.iter().map(|m| m.rem).sum::<f32>() / count,
                uncertain: stage_minutes.iter().map(|m| m.uncertain).sum::<f32>() / count,
            }
        });
        let valid_nights = items.iter().filter(|n| n.usage_minutes > 240.0).count();
        let used = items.iter().filter(|n| n.usage_minutes > 0.0).count();
        let mut scored: Vec<_> = items
            .iter()
            .filter_map(|n| n.sleep_score.map(|score| (score, n.date.clone())))
            .collect();
        scored.sort();
        let scored_night = |value: &(u32, String)| ScoredNight {
            date: value.1.clone(),
            sleep_score: value.0,
        };
        let mut summary = PeriodSummary {
            period: period.into(),
            from: from_text,
            to: to_text,
            days_in_range,
            clamped_to_first_night,
            headline: String::new(),
            nights: used,
            valid_nights,
            compliance_pct: valid_nights as f64 / days_in_range as f64 * 100.0,
            avg_usage_minutes: average(
                items
                    .iter()
                    .filter(|n| n.usage_minutes > 0.0)
                    .map(|n| Some(n.usage_minutes)),
            ),
            avg_ahi: average(items.iter().map(|n| n.ahi)),
            avg_sleep_score: average(items.iter().map(|n| n.sleep_score.map(f64::from))),
            avg_excess_leak_p95: average(items.iter().map(|n| n.excess_leak_p95)),
            avg_pressure_p90: average(items.iter().map(|n| n.pressure_p90)),
            avg_stage_minutes,
            pressure: sleeplens_core::pressure::pressure_summary_with_facts(
                &nights,
                &pressure_facts,
            ),
            previous: PreviousPeriod {
                avg_usage_minutes: average(
                    previous
                        .iter()
                        .filter(|n| n.usage_minutes > 0.0)
                        .map(|n| Some(n.usage_minutes)),
                ),
                avg_ahi: average(previous.iter().map(|n| n.ahi)),
                avg_sleep_score: average(previous.iter().map(|n| n.sleep_score.map(f64::from))),
                compliance_pct: (!previous.is_empty()).then(|| {
                    previous.iter().filter(|n| n.usage_minutes > 240.0).count() as f64 / days as f64
                        * 100.0
                }),
            },
            best_night: scored.last().map(scored_night),
            worst_night: scored.first().map(scored_night),
            score_series: (0..days_in_range)
                .map(|offset| {
                    let date = (from + chrono::Duration::days(offset as i64))
                        .format("%Y-%m-%d")
                        .to_string();
                    let night = items.iter().find(|n| n.date == date);
                    ScorePoint {
                        date,
                        sleep_score: night.and_then(|n| n.sleep_score),
                        usage_minutes: night.map_or(0.0, |n| n.usage_minutes),
                    }
                })
                .collect(),
            tips: Vec::new(),
        };
        summary.headline = period_headline(&summary, &items);
        // Earlier imported nights provide recurrence context only. Period metrics above
        // and tip emission dates remain restricted to the selected range.
        let mut tip_nights = self
            .list_nights(None, Some(&previous_to.format("%Y-%m-%d").to_string()))?
            .iter()
            .rev()
            .take(6)
            .map(|n| self.cached_night(&n.date))
            .collect::<AppResult<Vec<_>>>()?;
        let pressure_tips =
            sleeplens_core::pressure::period_pressure_tips_with_facts(&nights, &pressure_facts);
        tip_nights.extend(nights);
        summary.tips =
            sleeplens_core::tips::period_tips_with_pressure(&summary, &tip_nights, pressure_tips);
        Ok(summary)
    }
    pub fn trends(&self, from: Option<&str>, to: Option<&str>) -> AppResult<Trends> {
        let items = self.list_nights(from, to)?;
        let start = from
            .or_else(|| items.first().map(|n| n.date.as_str()))
            .unwrap_or("");
        let end = to
            .or_else(|| items.last().map(|n| n.date.as_str()))
            .unwrap_or("");
        let days = if start.is_empty() || end.is_empty() {
            0
        } else {
            let a = validate_date(start)?;
            let b = validate_date(end)?;
            if b < a {
                return Err("The end date must be on or after the start date.".into());
            }
            (b - a).num_days() + 1
        };
        let nights = items.iter().filter(|n| n.usage_minutes > 0.0).count();
        let valid = items.iter().filter(|n| n.usage_minutes > 240.0).count();
        Ok(Trends {
            from: start.into(),
            to: end.into(),
            nights,
            days_in_range: days,
            valid_nights: valid,
            compliance_pct: if days > 0 {
                valid as f64 / days as f64 * 100.0
            } else {
                0.0
            },
            avg_usage_minutes: if nights > 0 {
                items.iter().map(|n| n.usage_minutes).sum::<f64>() / nights as f64
            } else {
                0.0
            },
            avg_ahi: average(items.iter().map(|n| n.ahi)),
            avg_pressure_p90: average(items.iter().map(|n| n.pressure_p90)),
            avg_excess_leak: average(items.iter().map(|n| n.excess_leak_median)),
            avg_sleep_score: average(items.iter().map(|n| n.sleep_score.map(f64::from))),
            avg_excess_leak_p95: average(items.iter().map(|n| n.excess_leak_p95)),
            points: items
                .into_iter()
                .map(|n| TrendPoint {
                    date: n.date,
                    usage_minutes: n.usage_minutes,
                    ahi: n.ahi,
                    pressure_p90: n.pressure_p90,
                    excess_leak_median: n.excess_leak_median,
                    settings_changed: n.settings_changed,
                    sleep_score: n.sleep_score,
                    excess_leak_p95: n.excess_leak_p95,
                    prescription_changed: n.prescription_changed,
                })
                .collect(),
        })
    }
}
fn period_headline(summary: &sleeplens_core::PeriodSummary, items: &[NightListItem]) -> String {
    let breathing = match summary.avg_ahi {
        Some(ahi) if ahi < 5.0 => "average breathing events were below 5 per hour",
        Some(ahi) if ahi <= 15.0 => "average breathing events were between 5 and 15 per hour",
        Some(_) => "average breathing events were above 15 per hour",
        None => "breathing events were not recorded",
    };
    let seal_nights = items
        .iter()
        .filter(|night| {
            night.excess_leak_p95.is_some_and(|leak| leak > 24.0)
                || night.large_leak_minutes.is_some_and(|leak| {
                    night.usage_minutes > 0.0 && leak / night.usage_minutes > 0.05
                })
        })
        .count();
    let seal = if seal_nights > 0 {
        format!(
            "mask leaks were higher on {seal_nights} {}",
            if seal_nights == 1 { "night" } else { "nights" }
        )
    } else if items
        .iter()
        .any(|night| night.excess_leak_p95.is_some() || night.large_leak_minutes.is_some())
    {
        "recorded mask leaks stayed low".into()
    } else {
        "mask seal was not recorded".into()
    };
    let harder = items
        .iter()
        .filter(|night| night.sleep_score.is_some_and(|score| score < 70))
        .count();
    format!(
        "You used it on {} of {} nights, {breathing}, {seal}, and {harder} {} harder.",
        summary.nights,
        summary.days_in_range,
        if harder == 1 {
            "night was"
        } else {
            "nights were"
        }
    )
}

fn average(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    let values: Vec<_> = values.flatten().collect();
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}
fn validate_date(date: &str) -> AppResult<chrono::NaiveDate> {
    if date.len() != 10 || date.starts_with("0000") {
        return Err("Use a date in YYYY-MM-DD format.".into());
    }
    let parsed = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| "Use a date in YYYY-MM-DD format.".to_string())?;
    if parsed.format("%Y-%m-%d").to_string() != date {
        return Err("Use a date in YYYY-MM-DD format.".into());
    }
    Ok(parsed)
}
fn validate_range(from: Option<&str>, to: Option<&str>) -> AppResult<()> {
    let a = from.map(validate_date).transpose()?;
    let b = to.map(validate_date).transpose()?;
    if a.zip(b).is_some_and(|(a, b)| a > b) {
        return Err("The end date must be on or after the start date.".into());
    }
    Ok(())
}
// Versioned zlib stream: timestamp length + timestamp, sample count, five f32 LE channels.
// NaN represents a gap on disk and is restored to None before serialization.
fn encode_series(s: &Series) -> AppResult<Vec<u8>> {
    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    z.write_all(&[1]).map_err(err)?;
    z.write_all(&(s.series_start.len() as u32).to_le_bytes())
        .map_err(err)?;
    z.write_all(s.series_start.as_bytes()).map_err(err)?;
    let length = s.pressure.len();
    z.write_all(&(length as u32).to_le_bytes()).map_err(err)?;
    for channel in [
        &s.pressure,
        &s.excess_leak,
        &s.total_leak,
        &s.flow_amplitude,
        &s.snore,
    ] {
        if channel.len() != length {
            return Err("Series channels must have equal lengths.".into());
        }
        for value in channel {
            z.write_all(&value.unwrap_or(f32::NAN).to_le_bytes())
                .map_err(err)?;
        }
    }
    z.finish().map_err(err)
}
fn decode_series(blob: &[u8]) -> AppResult<Series> {
    let mut z = ZlibDecoder::new(blob);
    let mut version = [0];
    z.read_exact(&mut version).map_err(err)?;
    if version != [1] {
        return Err("Unsupported stored series version.".into());
    }
    fn u32_le(r: &mut impl Read) -> AppResult<usize> {
        let mut b = [0; 4];
        r.read_exact(&mut b).map_err(err)?;
        Ok(u32::from_le_bytes(b) as usize)
    }
    let len = u32_le(&mut z)?;
    if len > 64 {
        return Err("Invalid series timestamp.".into());
    }
    let mut start = vec![0; len];
    z.read_exact(&mut start).map_err(err)?;
    let count = u32_le(&mut z)?;
    if count > 172800 {
        return Err("Invalid series length.".into());
    }
    let mut channel = || -> AppResult<Vec<Option<f32>>> {
        (0..count)
            .map(|_| {
                let mut b = [0; 4];
                z.read_exact(&mut b).map_err(err)?;
                let f = f32::from_le_bytes(b);
                Ok(f.is_finite().then_some(f))
            })
            .collect()
    };
    Ok(Series {
        series_start: String::from_utf8(start).map_err(err)?,
        pressure: channel()?,
        excess_leak: channel()?,
        total_leak: channel()?,
        flow_amplitude: channel()?,
        snore: channel()?,
    })
}

// Header stores the estimate metadata; the remaining bytes are exactly one per epoch.
fn encode_stages(stages: &SleepStageEstimate) -> AppResult<Vec<u8>> {
    let mut metadata = stages.clone();
    metadata.stages.clear();
    let header = serde_json::to_vec(&metadata).map_err(err)?;
    let mut blob = Vec::with_capacity(9 + header.len() + stages.stages.len());
    blob.push(2);
    blob.extend_from_slice(&(header.len() as u32).to_le_bytes());
    blob.extend_from_slice(&(stages.stages.len() as u32).to_le_bytes());
    blob.extend_from_slice(&header);
    blob.extend(stages.stages.iter().map(|stage| match stage {
        SleepStage::Wake => 0,
        SleepStage::Light => 1,
        SleepStage::Deep => 2,
        SleepStage::Rem => 3,
        SleepStage::NoData => 4,
        SleepStage::Uncertain => 5,
    }));
    Ok(blob)
}
fn decode_stages(blob: &[u8]) -> AppResult<SleepStageEstimate> {
    if blob.len() < 9 || !matches!(blob[0], 1 | 2) {
        return Err("Unsupported stored stages version.".into());
    }
    let length = u32::from_le_bytes(blob[1..5].try_into().map_err(err)?) as usize;
    let count = u32::from_le_bytes(blob[5..9].try_into().map_err(err)?) as usize;
    let header_end = 9usize
        .checked_add(length)
        .filter(|end| *end <= blob.len())
        .ok_or("Invalid stored stages header.")?;
    if blob.len() - header_end != count {
        return Err("Invalid stored stages length.".into());
    }
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&blob[9..header_end]).map_err(err)?;
    // v1 blobs predate uncertainty and confidence. Preserve their original
    // estimates until the card is re-read with estimator version 3.
    if let Some(minutes) = metadata.get_mut("minutes").and_then(|v| v.as_object_mut()) {
        minutes.entry("uncertain").or_insert(serde_json::json!(0.0));
    }
    if let Some(fields) = metadata.as_object_mut() {
        fields.entry("confidence").or_insert(serde_json::json!({
            "sleepWake": "moderate", "rem": "low", "deep": "veryLow"
        }));
    }
    let mut estimate: SleepStageEstimate = serde_json::from_value(metadata).map_err(err)?;
    estimate.stages = blob[header_end..]
        .iter()
        .map(|stage| match stage {
            0 => Ok(SleepStage::Wake),
            1 => Ok(SleepStage::Light),
            2 => Ok(SleepStage::Deep),
            3 => Ok(SleepStage::Rem),
            4 => Ok(SleepStage::NoData),
            5 if blob[0] >= 2 => Ok(SleepStage::Uncertain),
            _ => Err("Invalid stored stage.".into()),
        })
        .collect::<AppResult<_>>()?;
    Ok(estimate)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stage_blob_preserves_metadata_and_encodes_one_byte_per_epoch() {
        let estimate = SleepStageEstimate {
            start: "2026-11-01T22:00:00".into(),
            epoch_seconds: 30,
            stages: vec![
                SleepStage::Wake,
                SleepStage::Light,
                SleepStage::Deep,
                SleepStage::Rem,
                SleepStage::NoData,
                SleepStage::Uncertain,
            ],
            minutes: StageMinutes {
                wake: 0.5,
                light: 0.5,
                deep: 0.5,
                rem: 0.5,
                uncertain: 0.5,
            },
            confidence: Default::default(),
            signal_quality: 0.8,
            estimator_version: ESTIMATOR_VERSION,
        };
        let blob = encode_stages(&estimate).unwrap();
        assert_eq!(blob[0], 2);
        let metadata_len = u32::from_le_bytes(blob[1..5].try_into().unwrap()) as usize;
        assert_eq!(blob.len(), 9 + metadata_len + estimate.stages.len());
        assert_eq!(&blob[9 + metadata_len..], &[0, 1, 2, 3, 4, 5]);
        assert_eq!(decode_stages(&blob).unwrap(), estimate);
    }
    #[test]
    fn legacy_stage_blob_decodes_without_uncertainty_or_confidence() {
        let header = serde_json::to_vec(&serde_json::json!({
            "start": "2026-11-01T22:00:00", "epochSeconds": 30,
            "stages": [], "minutes": {"wake": 0., "light": 0.5, "deep": 0., "rem": 0.},
            "signalQuality": 0.8, "estimatorVersion": 2
        }))
        .unwrap();
        let mut blob = vec![1];
        blob.extend_from_slice(&(header.len() as u32).to_le_bytes());
        blob.extend_from_slice(&1u32.to_le_bytes());
        blob.extend_from_slice(&header);
        blob.push(1);
        let estimate = decode_stages(&blob).unwrap();
        assert_eq!(estimate.stages, [SleepStage::Light]);
        assert_eq!(estimate.minutes.uncertain, 0.);
        assert_eq!(estimate.estimator_version, 2);
        let json = serde_json::to_value(estimate).unwrap();
        assert_eq!(json["confidence"]["sleepWake"], "moderate");
        *blob.last_mut().unwrap() = 5;
        assert!(decode_stages(&blob).is_err());
    }
    #[test]
    fn stage_blob_rejects_truncation_and_unknown_codes() {
        assert!(decode_stages(&[]).is_err());
        assert!(decode_stages(&[1, 255, 255, 255, 255]).is_err());
        let estimate = SleepStageEstimate {
            start: "2026-11-01T22:00:00".into(),
            epoch_seconds: 30,
            stages: vec![SleepStage::Light],
            minutes: StageMinutes::default(),
            confidence: Default::default(),
            signal_quality: 0.0,
            estimator_version: ESTIMATOR_VERSION,
        };
        let mut blob = encode_stages(&estimate).unwrap();
        assert!(decode_stages(&blob[..blob.len() - 1]).is_err());
        let mut extra = blob.clone();
        extra.push(0);
        assert!(decode_stages(&extra).is_err());
        *blob.last_mut().unwrap() = 255;
        assert!(decode_stages(&blob).is_err());
    }
    #[test]
    fn period_summary_rejects_invalid_inputs_and_date_underflow() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("test.sqlite")).unwrap();
        assert!(store.period_summary("week", None).is_err());
        assert!(store.period_summary("daily", Some("2026-11-01")).is_err());
        assert!(store.period_summary("week", Some("2026-02-30")).is_err());
        assert!(store.period_summary("year", Some("0001-01-01")).is_err());
        let summary = store.period_summary("week", Some("2026-11-01")).unwrap();
        assert_eq!(summary.score_series.len(), 7);
        assert_eq!(summary.avg_usage_minutes, None);
        assert_eq!(summary.previous.compliance_pct, None);
    }
}
