//! Device-local wall clock timestamps, without an assumed time zone.
use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalTime {
    pub iso: String,
    pub epoch_sec: i64,
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}
fn from_datetime(dt: NaiveDateTime) -> LocalTime {
    LocalTime {
        iso: dt.format("%Y-%m-%dT%H:%M:%S").to_string(),
        epoch_sec: dt.and_utc().timestamp(),
        year: dt.year(),
        month: dt.month(),
        day: dt.day(),
        hour: dt.hour(),
        minute: dt.minute(),
        second: dt.second(),
    }
}
pub fn make_local_time(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> Option<LocalTime> {
    if !(2000..=2100).contains(&year) {
        return None;
    }
    Some(from_datetime(
        NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(hour, minute, second)?,
    ))
}
pub fn read_timestamp(buf: &[u8], offset: usize) -> Option<LocalTime> {
    let b = buf.get(offset..offset.checked_add(7)?)?;
    make_local_time(
        u16::from_le_bytes([b[0], b[1]]) as i32,
        b[2] as u32,
        b[3] as u32,
        b[4] as u32,
        b[5] as u32,
        b[6] as u32,
    )
}
pub fn add_seconds(time: &LocalTime, seconds: i64) -> LocalTime {
    from_datetime(
        chrono::DateTime::from_timestamp(time.epoch_sec + seconds, 0)
            .expect("device timestamp is within chrono range")
            .naive_utc(),
    )
}
pub fn date_key(time: &LocalTime) -> String {
    format!("{:04}{:02}{:02}", time.year, time.month, time.day)
}
pub fn read_ascii(buf: &[u8], offset: usize, max_len: usize) -> String {
    buf.get(offset..)
        .unwrap_or_default()
        .iter()
        .take(max_len)
        .take_while(|&&b| b != 0 && b != 255)
        .map(|&b| char::from(b))
        .collect()
}
