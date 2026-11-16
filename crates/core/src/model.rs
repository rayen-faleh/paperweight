//! Domain types. Every timestamp is UTC; conversion to local time happens at
//! the edges (see [`crate::time::Clock`]).

use chrono::{DateTime, NaiveTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Result, invalid};
use crate::time::Clock;

macro_rules! string_enum {
    ($(#[$m:meta])* $name:ident { $($variant:ident => $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
        #[serde(rename_all = "lowercase")]
        pub enum $name { $($variant),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self { $($name::$variant => $s),+ }
            }

            pub fn parse(s: &str) -> Option<Self> {
                match s.trim().to_ascii_lowercase().as_str() {
                    $($s => Some($name::$variant),)+
                    _ => None,
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

string_enum!(
    /// What sort of thing an item is.
    Kind {
        Paperwork => "paperwork",
        Assignment => "assignment",
        Payment => "payment",
        Appointment => "appointment",
        Social => "social",
        Other => "other",
    }
);

string_enum!(
    /// Lifecycle state. `archived` means soft-deleted.
    Status {
        Open => "open",
        Done => "done",
        Archived => "archived",
    }
);

string_enum!(
    /// How often an item repeats.
    Freq {
        None => "none",
        Daily => "daily",
        Weekly => "weekly",
        Monthly => "monthly",
        Yearly => "yearly",
    }
);

fn one() -> u32 {
    1
}

/// Repetition rule: every `interval` days/weeks/months/years.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Recurrence {
    pub freq: Freq,
    /// Repeat every N units (1 = every day/week/month/year). Must be at least 1.
    #[serde(default = "one")]
    pub interval: u32,
}

impl Default for Recurrence {
    fn default() -> Self {
        Recurrence {
            freq: Freq::None,
            interval: 1,
        }
    }
}

impl Recurrence {
    pub fn is_none(&self) -> bool {
        self.freq == Freq::None
    }

    pub fn validate(&self) -> Result<()> {
        if self.interval == 0 || self.interval > 366 {
            return Err(invalid(
                "recurrence.interval must be between 1 and 366 (1 means every day/week/month/year).",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Step {
    pub id: i64,
    pub title: String,
    pub done: bool,
    pub soft_due_at: Option<DateTime<Utc>>,
    pub position: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Requirement {
    pub id: i64,
    pub text: String,
    pub done: bool,
    pub position: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub kind: Kind,
    pub due_at: DateTime<Utc>,
    pub all_day: bool,
    pub status: Status,
    pub notes: String,
    pub source: Option<String>,
    pub estimated_minutes: u32,
    pub snoozed_until: Option<DateTime<Utc>>,
    pub recurrence: Recurrence,
    /// Day of month a monthly/yearly series was first anchored to, so that a
    /// series on the 31st returns to the 31st after a short month.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurrence_anchor_day: Option<u32>,
    pub steps: Vec<Step>,
    pub requirements: Vec<Requirement>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub archived_at: Option<DateTime<Utc>>,
}

impl Item {
    pub fn is_snoozed(&self, now: DateTime<Utc>) -> bool {
        self.snoozed_until.is_some_and(|s| s > now)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewStep {
    pub title: String,
    pub soft_due_at: Option<DateTime<Utc>>,
}

/// Input for creating an item. The due date is already resolved; use
/// [`crate::dates::parse_due`] to turn user text into one.
#[derive(Debug, Clone)]
pub struct NewItem {
    pub title: String,
    pub kind: Kind,
    pub due_at: DateTime<Utc>,
    pub all_day: bool,
    pub notes: String,
    pub source: Option<String>,
    pub estimated_minutes: Option<u32>,
    pub recurrence: Recurrence,
    pub steps: Vec<NewStep>,
    pub requirements: Vec<String>,
}

impl NewItem {
    pub fn new(title: impl Into<String>, due_at: DateTime<Utc>, all_day: bool) -> Self {
        NewItem {
            title: title.into(),
            kind: Kind::Other,
            due_at,
            all_day,
            notes: String::new(),
            source: None,
            estimated_minutes: None,
            recurrence: Recurrence::default(),
            steps: vec![],
            requirements: vec![],
        }
    }
}

/// A partial update. `None` leaves a field unchanged.
#[derive(Debug, Clone, Default)]
pub struct ItemPatch {
    pub title: Option<String>,
    pub kind: Option<Kind>,
    /// New due instant and whether it is all-day.
    pub due: Option<(DateTime<Utc>, bool)>,
    pub notes: Option<String>,
    /// `Some(None)` clears the source.
    pub source: Option<Option<String>>,
    pub estimated_minutes: Option<u32>,
    pub recurrence: Option<Recurrence>,
    /// Only `open` and `done` are accepted here; use delete/restore for archive.
    pub status: Option<Status>,
    /// `Some(None)` clears an active snooze.
    pub snoozed_until: Option<Option<DateTime<Utc>>>,
    pub add_steps: Vec<NewStep>,
    pub set_steps_done: Vec<(i64, bool)>,
    pub rename_steps: Vec<(i64, String)>,
    pub remove_steps: Vec<i64>,
    pub add_requirements: Vec<String>,
    pub set_requirements_done: Vec<(i64, bool)>,
    pub remove_requirements: Vec<i64>,
}

/// One block of the weekly routine. `weekday` is 0 = Monday … 6 = Sunday and
/// refers to the *logical* day (see day rollover), so a 22:00–02:00 block on
/// Tuesday runs into early Wednesday morning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RoutineBlock {
    #[serde(default)]
    pub id: i64,
    pub weekday: u8,
    /// Local start time, "HH:MM".
    pub start: String,
    /// Local end time, "HH:MM". An end at or before the start runs past midnight.
    pub end: String,
    pub label: String,
    pub available_for_tasks: bool,
}

impl RoutineBlock {
    pub fn times(&self) -> Result<(NaiveTime, NaiveTime)> {
        Ok((parse_hhmm(&self.start)?, parse_hhmm(&self.end)?))
    }

    pub fn validate(&self) -> Result<()> {
        if self.weekday > 6 {
            return Err(invalid("weekday must be 0 (Monday) to 6 (Sunday)"));
        }
        if self.label.trim().is_empty() {
            return Err(invalid("routine block label must not be empty"));
        }
        self.times().map(|_| ())
    }
}

pub fn parse_hhmm(s: &str) -> Result<NaiveTime> {
    NaiveTime::parse_from_str(s.trim(), "%H:%M")
        .map_err(|_| invalid(format!("\"{s}\" is not a time in HH:MM (24-hour) form")))
}

/// User settings. Stored key by key; missing keys fall back to these defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Settings {
    /// IANA timezone name (e.g. "Asia/Tokyo"), or "system".
    pub timezone: String,
    /// When the day ends, "HH:MM". Default 02:00.
    pub day_rollover: String,
    pub quiet_start: String,
    pub quiet_end: String,
    /// Reminder offsets: "Nd" (N days before), "Nh" (N hours before),
    /// "day" (start of the due day), "overdue" (when overdue, then daily).
    pub reminder_offsets: Vec<String>,
    pub feed_port: u16,
    /// Interface language: "en" or "ja".
    pub language: String,
    /// Global keyboard shortcut for quick add.
    pub quick_add_shortcut: String,
    /// Play the system's reminder sound with each notification.
    pub reminder_sound: bool,
    /// Check GitHub for a new release once a day. Off by default: without it
    /// Paperweight only goes online when you press "Check for updates".
    pub auto_update_check: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            timezone: "system".into(),
            day_rollover: "02:00".into(),
            quiet_start: "02:00".into(),
            quiet_end: "12:00".into(),
            reminder_offsets: vec!["7d".into(), "2d".into(), "day".into(), "overdue".into()],
            feed_port: 47800,
            language: "en".into(),
            quick_add_shortcut: "CommandOrControl+Shift+Space".into(),
            reminder_sound: true,
            auto_update_check: false,
        }
    }
}

impl Settings {
    pub fn clock(&self) -> Clock {
        Clock::from_settings(self)
    }

    pub fn validate(&self) -> Result<()> {
        parse_hhmm(&self.day_rollover)?;
        parse_hhmm(&self.quiet_start)?;
        parse_hhmm(&self.quiet_end)?;
        if self.timezone != "system" && self.timezone.parse::<chrono_tz::Tz>().is_err() {
            return Err(invalid(format!(
                "\"{}\" is not a known IANA timezone (for example \"Asia/Tokyo\"), and not \"system\"",
                self.timezone
            )));
        }
        for o in &self.reminder_offsets {
            crate::reminders::Offset::parse(o)?;
        }
        if self.feed_port < 1024 {
            return Err(invalid("feed_port must be 1024 or higher"));
        }
        if !["en", "ja"].contains(&self.language.as_str()) {
            return Err(invalid("language must be \"en\" or \"ja\""));
        }
        Ok(())
    }
}

/// A sent reminder. `key` names the trigger ("7d", "day", "overdue:2026-11-18").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReminderLogEntry {
    pub item_id: String,
    pub key: String,
    pub sent_at: DateTime<Utc>,
}
