//! Next-occurrence calculation for recurring items.

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};

use crate::model::{Freq, Item, Recurrence};
use crate::time::Clock;

pub fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(28)
}

/// Add `months` to a date, using `anchor_day` as the target day of month and
/// clamping to the last day of shorter months (31 Jan + 1 month = 28/29 Feb;
/// with anchor 31, 28 Feb + 1 month = 31 Mar).
pub fn add_months(date: NaiveDate, months: u32, anchor_day: u32) -> NaiveDate {
    let total = date.year() * 12 + date.month0() as i32 + months as i32;
    let year = total.div_euclid(12);
    let month = total.rem_euclid(12) as u32 + 1;
    let day = anchor_day.min(days_in_month(year, month)).max(1);
    NaiveDate::from_ymd_opt(year, month, day).expect("clamped date is valid")
}

pub fn step_date(date: NaiveDate, rec: Recurrence, anchor_day: u32) -> NaiveDate {
    let n = rec.interval.max(1);
    match rec.freq {
        Freq::None => date,
        Freq::Daily => date + Duration::days(n as i64),
        Freq::Weekly => date + Duration::days(7 * n as i64),
        Freq::Monthly => add_months(date, n, anchor_day),
        Freq::Yearly => add_months(date, 12 * n, anchor_day),
    }
}

/// The calendar date a recurrence is anchored to: the deadline's logical day
/// for all-day items, or the local calendar date for timed items.
fn base_date(item: &Item, clock: &Clock) -> NaiveDate {
    if item.all_day {
        clock.deadline_day(item.due_at)
    } else {
        clock.local_naive(item.due_at).date()
    }
}

/// The anchor day an item's series uses.
pub fn anchor_day(item: &Item, clock: &Clock) -> u32 {
    item.recurrence_anchor_day
        .unwrap_or_else(|| base_date(item, clock).day())
}

/// The due instant of the occurrence after `item`. Missed occurrences are
/// skipped: the result is the first occurrence after both the current due
/// date and `now`. Returns `None` for non-recurring items.
pub fn next_due(item: &Item, now: DateTime<Utc>, clock: &Clock) -> Option<DateTime<Utc>> {
    if item.recurrence.is_none() {
        return None;
    }
    let anchor = anchor_day(item, clock);
    let time = clock.local_naive(item.due_at).time();
    let mut date = base_date(item, clock);
    for _ in 0..10_000 {
        date = step_date(date, item.recurrence, anchor);
        let due = if item.all_day {
            clock.day_end(date)
        } else {
            clock.resolve(date.and_time(time))
        };
        if due > now && due > item.due_at {
            return Some(due);
        }
    }
    None
}
