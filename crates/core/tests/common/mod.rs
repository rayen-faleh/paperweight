#![allow(dead_code)]

use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use paperweight_core::{Clock, Settings, Store};

pub const TZ: &str = "Asia/Tokyo";

/// A store in a temp dir with Tokyo time and default settings.
pub fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Store::open(dir.path().join("test.db")).unwrap();
    let settings = Settings {
        timezone: TZ.into(),
        ..Default::default()
    };
    s.save_settings(&settings).unwrap();
    (dir, s)
}

pub fn tokyo() -> Tz {
    TZ.parse().unwrap()
}

pub fn clock() -> Clock {
    Clock::new(tokyo(), NaiveTime::from_hms_opt(2, 0, 0).unwrap())
}

/// A Tokyo wall-clock time as UTC.
pub fn jst(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
    tokyo()
        .with_ymd_and_hms(y, mo, d, h, mi, 0)
        .single()
        .unwrap()
        .with_timezone(&Utc)
}

pub fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

/// Tuesday 17 November 2026, 15:00 in Tokyo.
pub fn tue_afternoon() -> DateTime<Utc> {
    jst(2026, 11, 17, 15, 0)
}
