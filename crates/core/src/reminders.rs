//! Reminder logic as pure functions. The tray app calls [`evaluate`] once a
//! minute with the current time and records what it returns as sent.
//!
//! Rules:
//! - Each item has triggers from the configured offsets: N days/hours before
//!   the deadline, the start of the due day, and once a day while overdue.
//! - Snoozed items produce nothing until the snooze ends.
//! - Nothing is delivered during quiet hours. Reminders that came due during
//!   quiet hours are delivered together as one digest when they end.
//! - After the computer was off or asleep (`catch_up`), everything pending is
//!   one catch-up digest, never a flood.
//! - Per item, only the most recent pending trigger is shown; older pending
//!   ones are marked sent with it. Triggers that fell before the item existed
//!   are marked sent silently.
//! - Every trigger is identified by `(item_id, key)` and the caller records
//!   it, so nothing is ever sent twice.

use std::collections::{BTreeMap, HashSet};

use chrono::{DateTime, Duration, NaiveTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{Result, invalid};
use crate::model::{Item, Settings, Status, parse_hhmm};
use crate::time::{Clock, fmt_utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offset {
    Before(Duration),
    DayStart,
    Overdue,
}

impl Offset {
    pub fn parse(s: &str) -> Result<Self> {
        let t = s.trim().to_ascii_lowercase();
        match t.as_str() {
            "day" => return Ok(Offset::DayStart),
            "overdue" => return Ok(Offset::Overdue),
            _ => {}
        }
        let (num, unit) = t.split_at(t.len().saturating_sub(1));
        let n: i64 = num.parse().map_err(|_| bad_offset(s))?;
        if !(1..=365).contains(&n) {
            return Err(bad_offset(s));
        }
        match unit {
            "d" => Ok(Offset::Before(Duration::days(n))),
            "h" => Ok(Offset::Before(Duration::hours(n))),
            "m" => Ok(Offset::Before(Duration::minutes(n))),
            _ => Err(bad_offset(s)),
        }
    }
}

fn bad_offset(s: &str) -> crate::error::Error {
    invalid(format!(
        "\"{s}\" is not a reminder offset. Use \"Nd\" (days before), \"Nh\" (hours before), \"Nm\" (minutes before), \"day\" (start of the due day) or \"overdue\"."
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    pub item_id: String,
    pub key: String,
    pub at: DateTime<Utc>,
    pub offset: String,
}

/// All triggers of an item that are at or before `now`.
pub fn due_triggers(
    item: &Item,
    offsets: &[Offset],
    clock: &Clock,
    now: DateTime<Utc>,
) -> Vec<Trigger> {
    let mut out = vec![];
    if item.status != Status::Open {
        return out;
    }
    for off in offsets {
        let (key, at, label) = match off {
            Offset::Before(d) => {
                let key = if d.num_minutes() % (24 * 60) == 0 {
                    format!("{}d", d.num_days())
                } else if d.num_minutes() % 60 == 0 {
                    format!("{}h", d.num_hours())
                } else {
                    format!("{}m", d.num_minutes())
                };
                (key.clone(), item.due_at - *d, key)
            }
            Offset::DayStart => (
                "day".to_string(),
                clock.day_start(clock.deadline_day(item.due_at)),
                "day".to_string(),
            ),
            Offset::Overdue => {
                if item.due_at > now {
                    continue;
                }
                let today = clock.today(now);
                let at = item.due_at.max(clock.day_start(today));
                (format!("overdue:{today}"), at, "overdue".to_string())
            }
        };
        if at <= now {
            out.push(Trigger {
                item_id: item.id.clone(),
                key,
                at,
                offset: label,
            });
        }
    }
    out
}

pub fn in_quiet_hours(local: NaiveTime, start: NaiveTime, end: NaiveTime) -> bool {
    if start == end {
        false
    } else if start < end {
        local >= start && local < end
    } else {
        local >= start || local < end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryKind {
    /// One reminder for one item.
    Single,
    /// Reminders held back by quiet hours, or several at once.
    Digest,
    /// The first check after start, sleep or shutdown.
    CatchUp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeliveryLine {
    pub item_id: String,
    pub title: String,
    pub due_label: String,
    pub offset: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Delivery {
    pub kind: DeliveryKind,
    pub title: String,
    pub body: String,
    /// The item to open on click (single reminders only).
    pub item_id: Option<String>,
    pub lines: Vec<DeliveryLine>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outcome {
    pub delivery: Option<Delivery>,
    /// `(item_id, key)` pairs to record in the reminder log.
    pub mark_sent: Vec<(String, String)>,
    /// True when quiet hours held everything back.
    pub quiet: bool,
}

pub struct Input<'a> {
    pub now: DateTime<Utc>,
    pub items: &'a [Item],
    pub settings: &'a Settings,
    pub sent: &'a HashSet<(String, String)>,
    /// First check after start or after a gap (sleep/shutdown).
    pub catch_up: bool,
}

pub fn evaluate(input: &Input) -> Outcome {
    let clock = input.settings.clock();
    let now = input.now;
    let quiet_start = parse_hhmm(&input.settings.quiet_start).unwrap_or(NaiveTime::MIN);
    let quiet_end = parse_hhmm(&input.settings.quiet_end).unwrap_or(NaiveTime::MIN);
    let is_quiet =
        |t: DateTime<Utc>| in_quiet_hours(clock.local_naive(t).time(), quiet_start, quiet_end);
    if is_quiet(now) {
        return Outcome {
            quiet: true,
            ..Default::default()
        };
    }
    let offsets: Vec<Offset> = input
        .settings
        .reminder_offsets
        .iter()
        .filter_map(|o| Offset::parse(o).ok())
        .collect();

    let mut mark = vec![];
    // item index -> latest pending trigger
    let mut latest: BTreeMap<usize, Trigger> = BTreeMap::new();
    for (idx, item) in input.items.iter().enumerate() {
        if item.status != Status::Open || item.is_snoozed(now) {
            continue;
        }
        for trig in due_triggers(item, &offsets, &clock, now) {
            if input
                .sent
                .contains(&(trig.item_id.clone(), trig.key.clone()))
            {
                continue;
            }
            mark.push((trig.item_id.clone(), trig.key.clone()));
            if trig.at < item.created_at {
                // Fell before the item existed: the person just added it.
                continue;
            }
            match latest.get(&idx) {
                Some(cur) if cur.at >= trig.at => {}
                _ => {
                    latest.insert(idx, trig);
                }
            }
        }
    }
    if latest.is_empty() {
        return Outcome {
            delivery: None,
            mark_sent: mark,
            quiet: false,
        };
    }

    let mut lines: Vec<(DateTime<Utc>, DeliveryLine)> = latest
        .iter()
        .map(|(idx, trig)| {
            let item = &input.items[*idx];
            (
                item.due_at,
                DeliveryLine {
                    item_id: item.id.clone(),
                    title: item.title.clone(),
                    due_label: clock.due_label(item.due_at, item.all_day, now),
                    offset: trig.offset.clone(),
                },
            )
        })
        .collect();
    lines.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.title.cmp(&b.1.title)));
    let lines: Vec<DeliveryLine> = lines.into_iter().map(|(_, l)| l).collect();
    let held_by_quiet = latest.values().any(|t| is_quiet(t.at));

    let kind = if input.catch_up {
        DeliveryKind::CatchUp
    } else if lines.len() > 1 || held_by_quiet {
        DeliveryKind::Digest
    } else {
        DeliveryKind::Single
    };

    let delivery = match kind {
        DeliveryKind::Single => {
            let l = &lines[0];
            Delivery {
                kind,
                title: l.title.clone(),
                body: single_body(l),
                item_id: Some(l.item_id.clone()),
                lines,
            }
        }
        _ => {
            let n = lines.len();
            let title = match kind {
                DeliveryKind::CatchUp => format!(
                    "While you were away: {n} reminder{}",
                    if n == 1 { "" } else { "s" }
                ),
                _ => format!(
                    "Paperweight: {n} thing{} to look at",
                    if n == 1 { "" } else { "s" }
                ),
            };
            let body = lines
                .iter()
                .map(|l| format!("• {} — {}", l.title, l.due_label))
                .collect::<Vec<_>>()
                .join("\n");
            Delivery {
                kind,
                title,
                body,
                item_id: if n == 1 {
                    Some(lines[0].item_id.clone())
                } else {
                    None
                },
                lines,
            }
        }
    };
    Outcome {
        delivery: Some(delivery),
        mark_sent: mark,
        quiet: false,
    }
}

fn single_body(l: &DeliveryLine) -> String {
    match l.offset.as_str() {
        "overdue" => format!(
            "Overdue — was due {}",
            l.due_label
                .trim_start_matches("overdue (")
                .trim_end_matches(')')
        ),
        _ => format!("Due {}", l.due_label),
    }
}

/// Scheduler bookkeeping: is this check a catch-up? True on the first check
/// after start, or when the previous check was more than `gap` ago (sleep,
/// shutdown).
pub fn is_catch_up(
    last_check: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    first_since_start: bool,
) -> bool {
    first_since_start || last_check.is_none_or(|t| now - t > Duration::minutes(3))
}

pub fn fmt_check(t: DateTime<Utc>) -> String {
    fmt_utc(t)
}
