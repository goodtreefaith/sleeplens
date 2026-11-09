use crate::types::*;
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use rusqlite::{params, Connection, OptionalExtension};
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
        if version > 1 {
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
        Ok(Self {
            path,
            conn: Some(conn),
        })
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
        self.read_night(date, true)
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
        self.db()?.query_row("SELECT EXISTS(SELECT 1 FROM file_index f JOIN nights n ON f.date=n.date WHERE f.path=?1 AND f.date=?2 AND f.device=?3 AND n.device=?3 AND f.size=?4 AND f.mtime=?5)", params![key,date,device,size,mtime], |r| r.get(0)).map_err(err)
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
        tx.execute("INSERT INTO nights VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
          ON CONFLICT(date) DO UPDATE SET device=excluded.device,start=excluded.start,end=excluded.end,usage_minutes=excluded.usage_minutes,usage_source=excluded.usage_source,ahi=excluded.ahi,pressure_mean=excluded.pressure_mean,pressure_p90=excluded.pressure_p90,pressure_source=excluded.pressure_source,excess_leak_median=excluded.excess_leak_median,large_leak_minutes=excluded.large_leak_minutes,event_counts=excluded.event_counts,settings_changed=excluded.settings_changed,summary=excluded.summary,settings=excluded.settings,list_json=excluded.list_json",
          params![n.date,device.0,n.start,n.end,n.usage_minutes,serde_json::to_string(&n.usage_source).map_err(err)?,n.ahi,n.pressure_mean,n.pressure_p90,serde_json::to_string(&n.pressure_source).map_err(err)?,n.excess_leak_median,n.large_leak_minutes,serde_json::to_string(&n.event_counts).map_err(err)?,n.settings_changed,serde_json::to_string(&night.summary).map_err(err)?,night.settings.as_ref().map(serde_json::to_string).transpose().map_err(err)?,serde_json::to_string(n).map_err(err)?]).map_err(err)?;
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
            if settings.is_some() {
                prev = settings;
            }
            tx.execute(
                "UPDATE nights SET settings_changed=?2,list_json=?3 WHERE date=?1",
                params![
                    n.date,
                    n.settings_changed,
                    serde_json::to_string(&n).map_err(err)?
                ],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)
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
            points: items
                .into_iter()
                .map(|n| TrendPoint {
                    date: n.date,
                    usage_minutes: n.usage_minutes,
                    ahi: n.ahi,
                    pressure_p90: n.pressure_p90,
                    excess_leak_median: n.excess_leak_median,
                    settings_changed: n.settings_changed,
                })
                .collect(),
        })
    }
}
fn average(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    let values: Vec<_> = values.flatten().collect();
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}
fn validate_date(date: &str) -> AppResult<chrono::NaiveDate> {
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
