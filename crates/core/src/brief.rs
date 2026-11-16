//! The brief (what's coming up) and the presentation form of an item.

use chrono::{DateTime, Duration, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::{Item, Kind, Status};
use crate::time::Clock;

/// An item plus local-time fields computed with the day rollover, so neither
/// the model nor the GUI has to do timezone arithmetic.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ItemView {
    #[serde(flatten)]
    pub item: Item,
    /// Local due date and time, ISO 8601 with offset.
    pub due_local: String,
    /// The logical day the deadline belongs to (rollover-aware), YYYY-MM-DD.
    pub due_day: NaiveDate,
    /// Short phrase, e.g. "tonight 01:00", "tomorrow", "Fri 21 Nov", "overdue (Mon 16 Nov)".
    pub due_label: String,
    pub overdue: bool,
    pub snoozed: bool,
    pub steps_done: usize,
    pub requirements_done: usize,
}

impl ItemView {
    pub fn new(item: Item, clock: &Clock, now: DateTime<Utc>) -> Self {
        let due_local = clock.local(item.due_at).to_rfc3339();
        let due_day = clock.deadline_day(item.due_at);
        let due_label = clock.due_label(item.due_at, item.all_day, now);
        let overdue = item.status == Status::Open && item.due_at <= now;
        let snoozed = item.is_snoozed(now);
        let steps_done = item.steps.iter().filter(|s| s.done).count();
        let requirements_done = item.requirements.iter().filter(|r| r.done).count();
        ItemView {
            item,
            due_local,
            due_day,
            due_label,
            overdue,
            snoozed,
            steps_done,
            requirements_done,
        }
    }

    /// One line for summaries: `pw-7k3f "Renew residence card" — Fri 21 Nov (paperwork)`.
    pub fn one_line(&self) -> String {
        format!(
            "{} \"{}\" — {} ({})",
            self.item.id, self.item.title, self.due_label, self.item.kind
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Brief {
    /// Current local date and time, ISO 8601 with offset.
    pub now_local: String,
    pub timezone: String,
    /// Today's logical date (the day ends at `day_rollover`, not midnight).
    pub today: NaiveDate,
    pub day_rollover: String,
    pub overdue: Vec<ItemView>,
    pub due_today: Vec<ItemView>,
    pub next_7_days: Vec<ItemView>,
}

/// Lower ranks first when deadlines tie: paperwork and payments have real
/// consequences when missed.
fn kind_rank(k: Kind) -> u8 {
    match k {
        Kind::Payment => 0,
        Kind::Paperwork => 1,
        Kind::Assignment => 2,
        Kind::Appointment => 3,
        Kind::Social => 4,
        Kind::Other => 5,
    }
}

/// Priority order: earliest deadline first, then kind, then title.
pub fn sort_by_priority(items: &mut [ItemView]) {
    items.sort_by(|a, b| {
        a.item
            .due_at
            .cmp(&b.item.due_at)
            .then(kind_rank(a.item.kind).cmp(&kind_rank(b.item.kind)))
            .then(a.item.title.cmp(&b.item.title))
    });
}

/// Build the brief from open items. Archived and done items are ignored.
pub fn brief(items: Vec<Item>, clock: &Clock, now: DateTime<Utc>) -> Brief {
    let today = clock.today(now);
    let horizon = today + Duration::days(7);
    let mut overdue = vec![];
    let mut due_today = vec![];
    let mut next = vec![];
    for item in items.into_iter().filter(|i| i.status == Status::Open) {
        let v = ItemView::new(item, clock, now);
        if v.overdue {
            overdue.push(v);
        } else if v.due_day == today {
            due_today.push(v);
        } else if v.due_day <= horizon {
            next.push(v);
        }
    }
    sort_by_priority(&mut overdue);
    sort_by_priority(&mut due_today);
    sort_by_priority(&mut next);
    Brief {
        now_local: clock.local(now).to_rfc3339(),
        timezone: clock.tz.name().to_string(),
        today,
        day_rollover: clock.rollover.format("%H:%M").to_string(),
        overdue,
        due_today,
        next_7_days: next,
    }
}
