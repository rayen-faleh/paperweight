//! Local time and the day rollover.
//!
//! Takaya's day ends at 02:00, not midnight. Every "today", "tonight", brief,
//! view and reminder uses the *logical day*: the local calendar date of
//! `local_time - rollover`. So 01:00 on Wednesday belongs to Tuesday.
//!
//! A deadline that falls exactly on a rollover instant belongs to the day that
//! rollover ends (an all-day item for Wednesday is stored as due at 02:00
//! Thursday, and still shows as Wednesday).

use chrono::{
    DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc,
    Weekday,
};
use chrono_tz::Tz;

use crate::model::{Settings, parse_hhmm};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clock {
    pub tz: Tz,
    pub rollover: NaiveTime,
}

/// The system timezone, falling back to UTC when it cannot be determined.
pub fn system_tz() -> Tz {
    iana_time_zone::get_timezone()
        .ok()
        .and_then(|name| name.parse::<Tz>().ok())
        .unwrap_or(Tz::UTC)
}

impl Clock {
    pub fn new(tz: Tz, rollover: NaiveTime) -> Self {
        Clock { tz, rollover }
    }

    pub fn from_settings(s: &Settings) -> Self {
        let tz = if s.timezone == "system" {
            system_tz()
        } else {
            s.timezone.parse().unwrap_or_else(|_| system_tz())
        };
        let rollover = parse_hhmm(&s.day_rollover)
            .unwrap_or_else(|_| NaiveTime::from_hms_opt(2, 0, 0).unwrap());
        Clock { tz, rollover }
    }

    fn rollover_offset(&self) -> Duration {
        self.rollover.signed_duration_since(NaiveTime::MIN)
    }

    pub fn local(&self, t: DateTime<Utc>) -> DateTime<Tz> {
        t.with_timezone(&self.tz)
    }

    pub fn local_naive(&self, t: DateTime<Utc>) -> NaiveDateTime {
        self.local(t).naive_local()
    }

    /// The logical day an instant belongs to.
    pub fn logical_day(&self, t: DateTime<Utc>) -> NaiveDate {
        (self.local_naive(t) - self.rollover_offset()).date()
    }

    /// The logical day a deadline belongs to. An instant exactly on a rollover
    /// counts as the end of the previous day.
    pub fn deadline_day(&self, due: DateTime<Utc>) -> NaiveDate {
        self.logical_day(due - Duration::seconds(1))
    }

    /// The instant a logical day starts (its rollover).
    pub fn day_start(&self, day: NaiveDate) -> DateTime<Utc> {
        self.resolve(day.and_time(self.rollover))
    }

    /// The instant a logical day ends: the next rollover. This is where an
    /// all-day item for `day` is due.
    pub fn day_end(&self, day: NaiveDate) -> DateTime<Utc> {
        self.day_start(day + Duration::days(1))
    }

    /// The naive local datetime of wall-clock time `t` on logical day `day`.
    /// Times before the rollover fall on the next calendar date.
    pub fn naive_on(&self, day: NaiveDate, t: NaiveTime) -> NaiveDateTime {
        if t < self.rollover {
            (day + Duration::days(1)).and_time(t)
        } else {
            day.and_time(t)
        }
    }

    /// The instant of wall-clock time `t` on logical day `day`.
    pub fn at(&self, day: NaiveDate, t: NaiveTime) -> DateTime<Utc> {
        self.resolve(self.naive_on(day, t))
    }

    /// Convert a local wall-clock datetime to UTC. Ambiguous times (DST end)
    /// take the earlier instant; skipped times (DST start) move forward an hour.
    pub fn resolve(&self, ndt: NaiveDateTime) -> DateTime<Utc> {
        match self.tz.from_local_datetime(&ndt) {
            LocalResult::Single(t) => t.with_timezone(&Utc),
            LocalResult::Ambiguous(a, _) => a.with_timezone(&Utc),
            LocalResult::None => match self.tz.from_local_datetime(&(ndt + Duration::hours(1))) {
                LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => t.with_timezone(&Utc),
                LocalResult::None => Utc.from_utc_datetime(&ndt),
            },
        }
    }

    pub fn today(&self, now: DateTime<Utc>) -> NaiveDate {
        self.logical_day(now)
    }

    /// Short human label for a deadline relative to `now`, e.g.
    /// "today", "tonight 01:00", "tomorrow 17:00", "Fri 21 Nov", "overdue (Mon 16 Nov)".
    pub fn due_label(&self, due: DateTime<Utc>, all_day: bool, now: DateTime<Utc>) -> String {
        let day = self.deadline_day(due);
        let today = self.today(now);
        let time = if all_day {
            String::new()
        } else {
            format!(" {}", self.local(due).format("%H:%M"))
        };
        let late_night = !all_day && self.local_naive(due).date() != day;
        let diff = (day - today).num_days();
        let base = match diff {
            0 if late_night => format!("tonight{time}"),
            0 => format!("today{time}"),
            1 if late_night => format!("tomorrow night{time}"),
            1 => format!("tomorrow{time}"),
            2..=6 => format!("{}{}", day.format("%a %-d %b"), time),
            _ => format!("{}{}", day.format("%a %-d %b"), time),
        };
        if due <= now {
            format!("overdue ({base})")
        } else {
            base
        }
    }
}

pub fn weekday_index(d: NaiveDate) -> u8 {
    d.weekday().num_days_from_monday() as u8
}

pub fn weekday_from_name(s: &str) -> Option<Weekday> {
    Some(match s {
        "monday" | "mon" => Weekday::Mon,
        "tuesday" | "tue" | "tues" => Weekday::Tue,
        "wednesday" | "wed" => Weekday::Wed,
        "thursday" | "thu" | "thurs" => Weekday::Thu,
        "friday" | "fri" => Weekday::Fri,
        "saturday" | "sat" => Weekday::Sat,
        "sunday" | "sun" => Weekday::Sun,
        _ => return None,
    })
}

/// Format a UTC instant for storage: second precision, `Z` suffix, sortable.
pub fn fmt_utc(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn parse_utc(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// Truncate to whole seconds, matching storage precision.
pub fn trunc(t: DateTime<Utc>) -> DateTime<Utc> {
    DateTime::from_timestamp(t.timestamp(), 0).unwrap_or(t)
}
