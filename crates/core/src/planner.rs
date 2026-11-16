//! `plan_week`: a deterministic suggestion of when to work on each open item.
//!
//! Open items are sorted by deadline. Each is placed, whole, in the earliest
//! routine block marked `available_for_tasks` in the next 7 days that still
//! has `estimated_minutes` of room and *ends* before the item's deadline.
//! Items due within the week that fit nowhere are "at risk". Items due later
//! that fit nowhere this week are listed as "later". Items with an estimate
//! of 0 need no work time and are skipped.

use chrono::{DateTime, Duration, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::{Item, RoutineBlock, Status};
use crate::time::{Clock, weekday_index};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlannedSession {
    pub item_id: String,
    pub title: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    /// Local start, e.g. "Tue 18 Nov 14:00".
    pub start_local: String,
    pub end_local: String,
    pub block_label: String,
    /// Logical day of the session.
    pub day: NaiveDate,
    pub due_at: DateTime<Utc>,
    pub minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Unplaced {
    pub item_id: String,
    pub title: String,
    pub due_at: DateTime<Utc>,
    pub estimated_minutes: u32,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WeekPlan {
    pub generated_at: DateTime<Utc>,
    pub horizon_start: DateTime<Utc>,
    pub horizon_end: DateTime<Utc>,
    pub sessions: Vec<PlannedSession>,
    /// Items due within the week (or overdue) that do not fit before their deadline.
    pub at_risk: Vec<Unplaced>,
    /// Items due after this week that did not fit this week; nothing to worry about yet.
    pub later: Vec<Unplaced>,
}

#[derive(Debug, Clone)]
struct Slot {
    day: NaiveDate,
    label: String,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    used_minutes: i64,
}

impl Slot {
    fn free_minutes(&self) -> i64 {
        (self.end - self.start).num_minutes() - self.used_minutes
    }
}

fn block_instances(
    now: DateTime<Utc>,
    routine: &[RoutineBlock],
    clock: &Clock,
    days: i64,
) -> Vec<Slot> {
    let today = clock.today(now);
    let mut slots = vec![];
    for offset in 0..days {
        let day = today + Duration::days(offset);
        for b in routine
            .iter()
            .filter(|b| b.available_for_tasks && b.weekday == weekday_index(day))
        {
            let Ok((s, e)) = b.times() else { continue };
            let start_n = clock.naive_on(day, s);
            let mut end_n = start_n.date().and_time(e);
            if end_n <= start_n {
                end_n += Duration::days(1);
            }
            let start = clock.resolve(start_n);
            let end = clock.resolve(end_n);
            if end <= now {
                continue;
            }
            // A block already under way starts now, rounded up to 5 minutes.
            let start = if start < now {
                let secs = now.timestamp();
                let rounded = (secs + 299) / 300 * 300;
                DateTime::from_timestamp(rounded, 0).unwrap_or(now)
            } else {
                start
            };
            if end <= start {
                continue;
            }
            slots.push(Slot {
                day,
                label: b.label.clone(),
                start,
                end,
                used_minutes: 0,
            });
        }
    }
    slots.sort_by_key(|s| s.start);
    slots
}

pub fn plan_week(
    now: DateTime<Utc>,
    items: &[Item],
    routine: &[RoutineBlock],
    clock: &Clock,
) -> WeekPlan {
    let today = clock.today(now);
    let horizon_end = clock.day_end(today + Duration::days(6));
    let mut slots = block_instances(now, routine, clock, 7);
    let largest = |slots: &[Slot]| slots.iter().map(|s| s.free_minutes()).max().unwrap_or(0);

    let mut open: Vec<&Item> = items
        .iter()
        .filter(|i| i.status == Status::Open && i.estimated_minutes > 0)
        .collect();
    open.sort_by(|a, b| {
        a.due_at
            .cmp(&b.due_at)
            .then(a.created_at.cmp(&b.created_at))
            .then(a.id.cmp(&b.id))
    });

    let mut sessions = vec![];
    let mut at_risk = vec![];
    let mut later = vec![];
    for item in open {
        let need = item.estimated_minutes as i64;
        let unplaced = |reason: String| Unplaced {
            item_id: item.id.clone(),
            title: item.title.clone(),
            due_at: item.due_at,
            estimated_minutes: item.estimated_minutes,
            reason,
        };
        if item.due_at <= now {
            at_risk.push(unplaced("Already overdue.".into()));
            continue;
        }
        let found = slots
            .iter_mut()
            .find(|s| s.end <= item.due_at && s.free_minutes() >= need);
        match found {
            Some(slot) => {
                let start = slot.start + Duration::minutes(slot.used_minutes);
                let end = start + Duration::minutes(need);
                slot.used_minutes += need;
                sessions.push(PlannedSession {
                    item_id: item.id.clone(),
                    title: item.title.clone(),
                    start,
                    end,
                    start_local: clock.local(start).format("%a %-d %b %H:%M").to_string(),
                    end_local: clock.local(end).format("%a %-d %b %H:%M").to_string(),
                    block_label: slot.label.clone(),
                    day: slot.day,
                    due_at: item.due_at,
                    minutes: item.estimated_minutes,
                });
            }
            None if item.due_at > horizon_end => later.push(unplaced(
                "Due after this week; no room for it this week.".into(),
            )),
            None => {
                let usable: Vec<&Slot> = slots.iter().filter(|s| s.end <= item.due_at).collect();
                let biggest = usable.iter().map(|s| s.free_minutes()).max().unwrap_or(0);
                let reason = if usable.is_empty() {
                    format!(
                        "No task block ends before the deadline ({}).",
                        clock.due_label(item.due_at, item.all_day, now)
                    )
                } else {
                    format!(
                        "Needs {need} min in one block, but the most free time left in any block before the deadline ({}) is {biggest} min (largest free block this week: {} min).",
                        clock.due_label(item.due_at, item.all_day, now),
                        largest(&slots)
                    )
                };
                at_risk.push(unplaced(reason));
            }
        }
    }
    sessions.sort_by_key(|s| s.start);
    WeekPlan {
        generated_at: now,
        horizon_start: now,
        horizon_end,
        sessions,
        at_risk,
        later,
    }
}
