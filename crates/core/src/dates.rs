//! Parsing the `due` field shared by the MCP server and Quick add.
//!
//! Accepted, and nothing else:
//! - ISO 8601: `2026-11-20` (all-day), `2026-11-20T17:00`, `2026-11-20 17:00`,
//!   with optional seconds, and with an optional `Z`/`+09:00` offset. Without
//!   an offset the time is in the user's timezone.
//! - Phrases: `today`, `tonight`, `tomorrow`, a weekday name, `next <weekday>`,
//!   `in N days`, `in N weeks`, each optionally followed by a time
//!   (`17:00`, `5pm`, `5:30pm`, optionally preceded by `at`).
//!
//! Phrases use the logical day (see [`crate::time`]). Without a time a phrase
//! means all day; `tonight` without a time means "by the end of tonight" (the
//! day rollover). A time before the rollover (e.g. `01:00`) belongs to the
//! night of that day, so it falls on the next calendar date.
//!
//! `<weekday>` is the first such day on or after today; `next <weekday>` is the
//! first such day strictly after today.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Utc};

use crate::error::{Error, Result};
use crate::time::{Clock, weekday_from_name};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Due {
    pub at: DateTime<Utc>,
    pub all_day: bool,
}

pub fn parse_due(input: &str, now: DateTime<Utc>, clock: &Clock) -> Result<Due> {
    let text = input.trim();
    if text.is_empty() {
        return Err(date_error(input, now, clock));
    }
    if let Some(d) = parse_iso(text, clock) {
        return Ok(d);
    }
    parse_phrase(text, now, clock).ok_or_else(|| date_error(input, now, clock))
}

/// The error text the model sees. It states the current local date and time,
/// and lists exactly what is accepted.
pub fn date_error(input: &str, now: DateTime<Utc>, clock: &Clock) -> Error {
    let local = clock.local(now);
    let today = clock.today(now);
    Error::Date(format!(
        "Could not understand the date \"{input}\". It is now {} ({}); the current day ends at {} so \"today\" is {}. \
         Send the date in ISO 8601 instead, for example \"{}\" for an all-day deadline or \"{}T17:00\" for a specific time \
         (no offset means {} time). Also accepted: today, tonight, tomorrow, a weekday name, next <weekday>, \
         in N days, in N weeks, each optionally followed by a time such as 17:00 or 5pm.",
        local.format("%A %Y-%m-%d %H:%M"),
        clock.tz.name(),
        clock.rollover.format("%H:%M"),
        today.format("%A %Y-%m-%d"),
        (today + Duration::days(1)).format("%Y-%m-%d"),
        (today + Duration::days(1)).format("%Y-%m-%d"),
        clock.tz.name(),
    ))
}

fn parse_iso(s: &str, clock: &Clock) -> Option<Due> {
    if s.len() == 10
        && let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d")
    {
        return Some(Due {
            at: clock.day_end(d),
            all_day: true,
        });
    }
    let normalized = s.replacen(' ', "T", 1);
    let upper = normalized.to_ascii_uppercase();
    if let Ok(dt) = DateTime::parse_from_rfc3339(&upper) {
        return Some(Due {
            at: crate::time::trunc(dt.with_timezone(&Utc)),
            all_day: false,
        });
    }
    for fmt in [
        "%Y-%m-%dT%H:%M%:z",
        "%Y-%m-%dT%H:%M%z",
        "%Y-%m-%dT%H:%M:%S%z",
    ] {
        if let Ok(dt) = DateTime::parse_from_str(&upper, fmt) {
            return Some(Due {
                at: dt.with_timezone(&Utc),
                all_day: false,
            });
        }
    }
    if let Some(rest) = upper.strip_suffix('Z') {
        for fmt in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"] {
            if let Ok(ndt) = NaiveDateTime::parse_from_str(rest, fmt) {
                return Some(Due {
                    at: ndt.and_utc(),
                    all_day: false,
                });
            }
        }
    }
    for fmt in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%dT%H:%M:%S%.f",
    ] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(&upper, fmt) {
            return Some(Due {
                at: crate::time::trunc(clock.resolve(ndt)),
                all_day: false,
            });
        }
    }
    None
}

/// Parse a clock time: "17:00", "5pm", "5:30pm", "5 pm", "12am".
pub fn parse_time(tokens: &[&str]) -> Option<NaiveTime> {
    let joined: String = tokens.concat();
    let s = joined.as_str();
    let (body, meridiem) = if let Some(b) = s.strip_suffix("am") {
        (b, Some(false))
    } else if let Some(b) = s.strip_suffix("pm") {
        (b, Some(true))
    } else {
        (s, None)
    };
    let (h, m) = match body.split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, {
            if m.len() != 2 {
                return None;
            }
            m.parse::<u32>().ok()?
        }),
        None if meridiem.is_some() => (body.parse::<u32>().ok()?, 0),
        None => return None,
    };
    if m > 59 {
        return None;
    }
    let h = match meridiem {
        Some(pm) => {
            if !(1..=12).contains(&h) {
                return None;
            }
            match (h, pm) {
                (12, false) => 0,
                (12, true) => 12,
                (h, true) => h + 12,
                (h, false) => h,
            }
        }
        None => {
            if h > 23 {
                return None;
            }
            h
        }
    };
    NaiveTime::from_hms_opt(h, m, 0)
}

fn parse_phrase(text: &str, now: DateTime<Utc>, clock: &Clock) -> Option<Due> {
    let lower = text.to_lowercase();
    let tokens: Vec<&str> = lower.split_whitespace().collect();

    // Split off a trailing time (1 or 2 tokens), with an optional "at".
    let mut date_tokens: &[&str] = &tokens;
    let mut time: Option<NaiveTime> = None;
    for take in [2usize, 1] {
        if tokens.len() > take {
            let tail = &tokens[tokens.len() - take..];
            if take == 2 && !matches!(tail[1], "am" | "pm") {
                continue;
            }
            if let Some(t) = parse_time(tail) {
                time = Some(t);
                date_tokens = &tokens[..tokens.len() - take];
                break;
            }
        }
    }
    if time.is_some() && date_tokens.last() == Some(&"at") {
        date_tokens = &date_tokens[..date_tokens.len() - 1];
    }

    let today = clock.today(now);
    let mut tonight = false;
    let day: NaiveDate = match date_tokens {
        ["today"] => today,
        ["tonight"] => {
            tonight = true;
            today
        }
        ["tomorrow"] => today + Duration::days(1),
        [name] => {
            let wd = weekday_from_name(name)?;
            let ahead = (wd.num_days_from_monday() as i64
                - today.weekday().num_days_from_monday() as i64)
                .rem_euclid(7);
            today + Duration::days(ahead)
        }
        ["next", name] => {
            let wd = weekday_from_name(name)?;
            let mut ahead = (wd.num_days_from_monday() as i64
                - today.weekday().num_days_from_monday() as i64)
                .rem_euclid(7);
            if ahead == 0 {
                ahead = 7;
            }
            today + Duration::days(ahead)
        }
        ["in", n, unit] => {
            let n: i64 = n.parse().ok().filter(|n| (0..=3650).contains(n))?;
            match *unit {
                "day" | "days" => today + Duration::days(n),
                "week" | "weeks" => today + Duration::days(7 * n),
                _ => return None,
            }
        }
        _ => return None,
    };

    match time {
        None if tonight => Some(Due {
            at: clock.day_end(day),
            all_day: false,
        }),
        None => Some(Due {
            at: clock.day_end(day),
            all_day: true,
        }),
        Some(mut t) => {
            // "tonight 9:00" means 21:00.
            if tonight && t >= clock.rollover && t < NaiveTime::from_hms_opt(12, 0, 0)? {
                t += Duration::hours(12);
            }
            Some(Due {
                at: clock.at(day, t),
                all_day: false,
            })
        }
    }
}

/// Quick add: "Scholarship form friday 17:00" → ("Scholarship form", Friday
/// 17:00). The date is the longest run of trailing words that [`parse_due`]
/// accepts; an optional "by", "due", "on" or "," before it is dropped. Uses
/// the same parser as the MCP server, so the same phrases work in both.
pub fn parse_quick_add(text: &str, now: DateTime<Utc>, clock: &Clock) -> Result<(String, Due)> {
    let words: Vec<&str> = text.split_whitespace().collect();
    // At most 4 trailing words form a date phrase ("next friday at 5pm").
    for start in (1..words.len()).filter(|s| words.len() - s <= 4) {
        let phrase = words[start..].join(" ");
        if let Ok(due) = parse_due(&phrase, now, clock) {
            let mut title_words = &words[..start];
            if let Some(last) = title_words.last()
                && matches!(
                    last.to_lowercase().as_str(),
                    "by" | "due" | "on" | "-" | "—" | "until"
                )
            {
                title_words = &title_words[..title_words.len() - 1];
            }
            let title = title_words
                .join(" ")
                .trim_end_matches([',', ':', ' '])
                .to_string();
            if title.is_empty() {
                break;
            }
            return Ok((title, due));
        }
    }
    let base = date_error(text, now, clock).to_string();
    Err(Error::Date(format!(
        "Add a date at the end, for example \"Scholarship form friday 17:00\" or \"Rent 2026-11-30\". {base}"
    )))
}
