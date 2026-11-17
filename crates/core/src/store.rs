//! All database access. Nothing outside this crate writes SQL.

use std::collections::HashSet;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result, invalid};
use crate::model::{
    Freq, Item, ItemPatch, Kind, NewItem, NewStep, Recurrence, ReminderLogEntry, Requirement,
    RoutineBlock, Settings, Status, Step,
};
use crate::recurrence;
use crate::time::{Clock, fmt_utc, parse_utc, trunc};

/// Archived items are purged this long after being archived.
pub const PURGE_AFTER_DAYS: i64 = 30;

pub struct Store {
    conn: Connection,
    path: PathBuf,
}

/// Filter for [`Store::list_items`].
#[derive(Debug, Clone, Default)]
pub struct ItemFilter {
    /// Empty means open items only.
    pub statuses: Vec<Status>,
    pub kinds: Vec<Kind>,
    pub due_from: Option<DateTime<Utc>>,
    pub due_to: Option<DateTime<Utc>>,
    /// Case-insensitive match on title, notes, steps and requirements.
    pub text: Option<String>,
    pub limit: Option<usize>,
}

/// Full data export, also the import format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Export {
    pub format: String,
    pub version: u32,
    pub exported_at: DateTime<Utc>,
    pub items: Vec<Item>,
    pub routine: Vec<RoutineBlock>,
    pub settings: Settings,
}

const ID_ALPHABET: &[u8] = b"23456789abcdefghjkmnpqrstuvwxyz";

fn random_u64() -> u64 {
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    h.write_u32(std::process::id());
    h.finish()
}

/// A short, lowercase id that is easy for a model to repeat: `pw-` and four
/// characters with no look-alikes (no 0/o, 1/l/i).
pub fn new_id() -> String {
    let mut n = random_u64();
    let mut s = String::from("pw-");
    for _ in 0..4 {
        s.push(ID_ALPHABET[(n % ID_ALPHABET.len() as u64) as usize] as char);
        n /= ID_ALPHABET.len() as u64;
    }
    s
}

pub fn not_found(id: &str) -> Error {
    Error::NotFound(format!("No item with id {id}. Call list_items to find it."))
}

fn opt_time(s: Option<String>) -> Option<DateTime<Utc>> {
    s.as_deref().and_then(parse_utc)
}

fn time_col(row: &Row, idx: &str) -> rusqlite::Result<DateTime<Utc>> {
    let s: String = row.get(idx)?;
    parse_utc(&s).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            format!("bad timestamp {s}").into(),
        )
    })
}

fn item_from_row(row: &Row) -> rusqlite::Result<Item> {
    let kind: String = row.get("kind")?;
    let status: String = row.get("status")?;
    let freq: String = row.get("recur_freq")?;
    Ok(Item {
        id: row.get("id")?,
        title: row.get("title")?,
        kind: Kind::parse(&kind).unwrap_or(Kind::Other),
        due_at: time_col(row, "due_at")?,
        all_day: row.get("all_day")?,
        status: Status::parse(&status).unwrap_or(Status::Open),
        notes: row.get("notes")?,
        source: row.get("source")?,
        estimated_minutes: row.get("estimated_minutes")?,
        snoozed_until: opt_time(row.get("snoozed_until")?),
        recurrence: Recurrence {
            freq: Freq::parse(&freq).unwrap_or(Freq::None),
            interval: row.get("recur_interval")?,
        },
        recurrence_anchor_day: row.get("recur_anchor_day")?,
        steps: vec![],
        requirements: vec![],
        created_at: time_col(row, "created_at")?,
        updated_at: time_col(row, "updated_at")?,
        completed_at: opt_time(row.get("completed_at")?),
        archived_at: opt_time(row.get("archived_at")?),
    })
}

fn clean_title(t: &str) -> Result<String> {
    let t = t.trim();
    if t.is_empty() {
        return Err(invalid("title must not be empty"));
    }
    if t.chars().count() > 300 {
        return Err(invalid(
            "title must be at most 300 characters; put details in notes",
        ));
    }
    Ok(t.to_string())
}

fn check_minutes(m: u32) -> Result<u32> {
    if m > 7 * 24 * 60 {
        return Err(invalid(
            "estimated_minutes must be between 0 and 10080 (one week). Use 0 for things that need no preparation.",
        ));
    }
    Ok(m)
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let conn = crate::db::open(&path)?;
        Ok(Store { conn, path })
    }

    pub fn open_default() -> Result<Self> {
        Self::open(crate::db::default_path())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Start a write transaction with `BEGIN IMMEDIATE`. A deferred
    /// transaction that reads first and writes later cannot wait for the lock:
    /// if another connection wrote in between, SQLite returns "database is
    /// locked" at once, ignoring the busy timeout. Taking the write lock up
    /// front makes concurrent writers queue behind `busy_timeout` instead.
    ///
    /// `busy_timeout` (5 s) bounds a single wait, and SQLite's busy handler is
    /// not fair: under heavy contention one writer can keep losing the race.
    /// So a busy start is retried, for up to 30 s in all, instead of failing.
    fn write_tx(&mut self) -> Result<rusqlite::Transaction<'_>> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut attempt: u64 = 0;
        loop {
            match rusqlite::Transaction::new_unchecked(
                &self.conn,
                rusqlite::TransactionBehavior::Immediate,
            ) {
                Ok(tx) => return Ok(tx),
                Err(e) if crate::db::is_busy(&e) && std::time::Instant::now() < deadline => {
                    attempt += 1;
                    // Short, slightly varied pause so retrying writers spread out.
                    let jitter = (std::process::id() as u64 + attempt * 7) % 23;
                    std::thread::sleep(std::time::Duration::from_millis(10 + jitter));
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    pub fn data_version(&self) -> Result<i64> {
        crate::db::data_version(&self.conn)
    }

    // ---------- settings ----------

    pub fn settings(&self) -> Result<Settings> {
        let mut map = match serde_json::to_value(Settings::default())? {
            serde_json::Value::Object(m) => m,
            _ => unreachable!("settings serialize to an object"),
        };
        let mut stmt = self.conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (k, v) = row?;
            if map.contains_key(&k)
                && let Ok(v) = serde_json::from_str(&v)
            {
                map.insert(k, v);
            }
        }
        let s: Settings =
            serde_json::from_value(serde_json::Value::Object(map)).unwrap_or_default();
        Ok(s)
    }

    pub fn save_settings(&mut self, s: &Settings) -> Result<()> {
        s.validate()?;
        let map = match serde_json::to_value(s)? {
            serde_json::Value::Object(m) => m,
            _ => unreachable!(),
        };
        let tx = self.write_tx()?;
        for (k, v) in map {
            tx.execute(
                "INSERT INTO settings(key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![k, v.to_string()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn clock(&self) -> Result<Clock> {
        Ok(self.settings()?.clock())
    }

    // ---------- app state (scheduler bookkeeping) ----------

    pub fn get_state(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM app_state WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn set_state(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO app_state(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // ---------- items ----------

    fn load_children(&self, item: &mut Item) -> Result<()> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, title, done, soft_due_at, position FROM steps WHERE item_id = ?1 ORDER BY position, id",
        )?;
        item.steps = stmt
            .query_map([&item.id], |r| {
                Ok(Step {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    done: r.get(2)?,
                    soft_due_at: opt_time(r.get(3)?),
                    position: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, text, done, position FROM requirements WHERE item_id = ?1 ORDER BY position, id",
        )?;
        item.requirements = stmt
            .query_map([&item.id], |r| {
                Ok(Requirement {
                    id: r.get(0)?,
                    text: r.get(1)?,
                    done: r.get(2)?,
                    position: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(())
    }

    /// Fetch any item, including archived ones.
    pub fn get_item(&self, id: &str) -> Result<Item> {
        let id = id.trim().to_ascii_lowercase();
        let mut item = self
            .conn
            .query_row("SELECT * FROM items WHERE id = ?1", [&id], item_from_row)
            .optional()?
            .ok_or_else(|| not_found(&id))?;
        self.load_children(&mut item)?;
        Ok(item)
    }

    /// Fetch an item that is not archived. Archived items are hidden everywhere
    /// except the archive view.
    pub fn get_visible(&self, id: &str) -> Result<Item> {
        let item = self.get_item(id)?;
        if item.status == Status::Archived {
            return Err(Error::NotFound(format!(
                "Item {} was deleted (it is in the archive and can be restored from the Paperweight app). Call list_items to find current items.",
                item.id
            )));
        }
        Ok(item)
    }

    pub fn add_item(&mut self, new: NewItem, now: DateTime<Utc>) -> Result<Item> {
        let title = clean_title(&new.title)?;
        new.recurrence.validate()?;
        let minutes = check_minutes(new.estimated_minutes.unwrap_or(30))?;
        let now = trunc(now);
        let clock = self.clock()?;
        let anchor = match new.recurrence.freq {
            Freq::Monthly | Freq::Yearly => Some(if new.all_day {
                use chrono::Datelike;
                clock.deadline_day(new.due_at).day()
            } else {
                use chrono::Datelike;
                clock.local_naive(new.due_at).day()
            }),
            _ => None,
        };
        let tx = self.write_tx()?;
        let mut id = new_id();
        for _ in 0..50 {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM items WHERE id = ?1)",
                [&id],
                |r| r.get(0),
            )?;
            if !exists {
                break;
            }
            id = new_id();
        }
        tx.execute(
            "INSERT INTO items (id, title, kind, due_at, all_day, status, notes, source, estimated_minutes,
                recur_freq, recur_interval, recur_anchor_day, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
            params![
                id,
                title,
                new.kind.as_str(),
                fmt_utc(new.due_at),
                new.all_day,
                new.notes.trim(),
                new.source.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                minutes,
                new.recurrence.freq.as_str(),
                new.recurrence.interval,
                anchor,
                fmt_utc(now),
            ],
        )?;
        for (i, s) in new.steps.iter().enumerate() {
            insert_step(&tx, &id, s, i as i64)?;
        }
        for (i, r) in new.requirements.iter().enumerate() {
            insert_requirement(&tx, &id, r, i as i64)?;
        }
        tx.commit()?;
        self.get_item(&id)
    }

    pub fn update_item(&mut self, id: &str, patch: ItemPatch, now: DateTime<Utc>) -> Result<Item> {
        let item = self.get_visible(id)?;
        let id = item.id.clone();
        let now = trunc(now);
        let step_ids: HashSet<i64> = item.steps.iter().map(|s| s.id).collect();
        let req_ids: HashSet<i64> = item.requirements.iter().map(|r| r.id).collect();
        let check_step = |sid: i64| -> Result<()> {
            if step_ids.contains(&sid) {
                Ok(())
            } else {
                Err(invalid(format!(
                    "Item {id} has no step with id {sid}. Its steps are: {}.",
                    describe_children(item.steps.iter().map(|s| (s.id, s.title.as_str())))
                )))
            }
        };
        let check_req = |rid: i64| -> Result<()> {
            if req_ids.contains(&rid) {
                Ok(())
            } else {
                Err(invalid(format!(
                    "Item {id} has no requirement with id {rid}. Its requirements are: {}.",
                    describe_children(item.requirements.iter().map(|r| (r.id, r.text.as_str())))
                )))
            }
        };
        for (sid, _) in &patch.set_steps_done {
            check_step(*sid)?;
        }
        for (sid, _) in &patch.rename_steps {
            check_step(*sid)?;
        }
        for sid in &patch.remove_steps {
            check_step(*sid)?;
        }
        for (rid, _) in &patch.set_requirements_done {
            check_req(*rid)?;
        }
        for rid in &patch.remove_requirements {
            check_req(*rid)?;
        }
        if let Some(r) = &patch.recurrence {
            r.validate()?;
        }
        if let Some(Status::Archived) = patch.status {
            return Err(invalid(
                "To delete an item use delete_item. update_item accepts status \"open\" or \"done\".",
            ));
        }
        let title = patch.title.as_deref().map(clean_title).transpose()?;
        let minutes = patch.estimated_minutes.map(check_minutes).transpose()?;
        let clock = self.clock()?;

        let tx = self.write_tx()?;
        if let Some(t) = title {
            tx.execute("UPDATE items SET title = ?2 WHERE id = ?1", params![id, t])?;
        }
        if let Some(k) = patch.kind {
            tx.execute(
                "UPDATE items SET kind = ?2 WHERE id = ?1",
                params![id, k.as_str()],
            )?;
        }
        if let Some((due, all_day)) = patch.due {
            tx.execute(
                "UPDATE items SET due_at = ?2, all_day = ?3 WHERE id = ?1",
                params![id, fmt_utc(due), all_day],
            )?;
            // A moved deadline gets a fresh set of reminders.
            tx.execute("DELETE FROM reminder_log WHERE item_id = ?1", [&id])?;
        }
        if let Some(n) = &patch.notes {
            tx.execute(
                "UPDATE items SET notes = ?2 WHERE id = ?1",
                params![id, n.trim()],
            )?;
        }
        if let Some(src) = &patch.source {
            let src = src.as_deref().map(str::trim).filter(|s| !s.is_empty());
            tx.execute(
                "UPDATE items SET source = ?2 WHERE id = ?1",
                params![id, src],
            )?;
        }
        if let Some(m) = minutes {
            tx.execute(
                "UPDATE items SET estimated_minutes = ?2 WHERE id = ?1",
                params![id, m],
            )?;
        }
        if let Some(r) = patch.recurrence {
            let due = patch.due.map(|d| d.0).unwrap_or(item.due_at);
            let all_day = patch.due.map(|d| d.1).unwrap_or(item.all_day);
            let anchor = match r.freq {
                Freq::Monthly | Freq::Yearly => {
                    use chrono::Datelike;
                    Some(if all_day {
                        clock.deadline_day(due).day()
                    } else {
                        clock.local_naive(due).day()
                    })
                }
                _ => None,
            };
            tx.execute(
                "UPDATE items SET recur_freq = ?2, recur_interval = ?3, recur_anchor_day = ?4 WHERE id = ?1",
                params![id, r.freq.as_str(), r.interval, anchor],
            )?;
        }
        if let Some(s) = patch.snoozed_until {
            tx.execute(
                "UPDATE items SET snoozed_until = ?2 WHERE id = ?1",
                params![id, s.map(fmt_utc)],
            )?;
        }
        match patch.status {
            Some(Status::Open) => {
                tx.execute(
                    "UPDATE items SET status = 'open', completed_at = NULL WHERE id = ?1",
                    [&id],
                )?;
            }
            Some(Status::Done) => {
                tx.execute(
                    "UPDATE items SET status = 'done', completed_at = ?2 WHERE id = ?1",
                    params![id, fmt_utc(now)],
                )?;
            }
            _ => {}
        }
        for (sid, done) in &patch.set_steps_done {
            tx.execute(
                "UPDATE steps SET done = ?2 WHERE id = ?1",
                params![sid, done],
            )?;
        }
        for (sid, t) in &patch.rename_steps {
            tx.execute(
                "UPDATE steps SET title = ?2 WHERE id = ?1",
                params![sid, clean_title(t)?],
            )?;
        }
        for sid in &patch.remove_steps {
            tx.execute("DELETE FROM steps WHERE id = ?1", [sid])?;
        }
        if !patch.add_steps.is_empty() {
            let mut pos: i64 = tx.query_row(
                "SELECT COALESCE(MAX(position) + 1, 0) FROM steps WHERE item_id = ?1",
                [&id],
                |r| r.get(0),
            )?;
            for s in &patch.add_steps {
                insert_step(&tx, &id, s, pos)?;
                pos += 1;
            }
        }
        for (rid, done) in &patch.set_requirements_done {
            tx.execute(
                "UPDATE requirements SET done = ?2 WHERE id = ?1",
                params![rid, done],
            )?;
        }
        for rid in &patch.remove_requirements {
            tx.execute("DELETE FROM requirements WHERE id = ?1", [rid])?;
        }
        if !patch.add_requirements.is_empty() {
            let mut pos: i64 = tx.query_row(
                "SELECT COALESCE(MAX(position) + 1, 0) FROM requirements WHERE item_id = ?1",
                [&id],
                |r| r.get(0),
            )?;
            for r in &patch.add_requirements {
                insert_requirement(&tx, &id, r, pos)?;
                pos += 1;
            }
        }
        tx.execute(
            "UPDATE items SET updated_at = ?2 WHERE id = ?1",
            params![id, fmt_utc(now)],
        )?;
        tx.commit()?;
        self.get_item(&id)
    }

    /// Mark an item done. If it recurs, create and return the next occurrence
    /// (with steps and requirements copied and unticked).
    pub fn complete_item(&mut self, id: &str, now: DateTime<Utc>) -> Result<(Item, Option<Item>)> {
        let item = self.get_visible(id)?;
        if item.status == Status::Done {
            return Err(invalid(format!(
                "Item {} is already done (completed {}). To reopen it, call update_item with status \"open\".",
                item.id,
                item.completed_at.map(fmt_utc).unwrap_or_default()
            )));
        }
        let now = trunc(now);
        let clock = self.clock()?;
        let next_due = recurrence::next_due(&item, now, &clock);
        self.conn.execute(
            "UPDATE items SET status = 'done', completed_at = ?2, updated_at = ?2, snoozed_until = NULL WHERE id = ?1",
            params![item.id, fmt_utc(now)],
        )?;
        let next = match next_due {
            Some(due) => {
                let mut new = NewItem::new(item.title.clone(), due, item.all_day);
                new.kind = item.kind;
                new.notes = item.notes.clone();
                new.source = item.source.clone();
                new.estimated_minutes = Some(item.estimated_minutes);
                new.recurrence = item.recurrence;
                new.steps = item
                    .steps
                    .iter()
                    .map(|s| NewStep {
                        title: s.title.clone(),
                        soft_due_at: s.soft_due_at.map(|t| t + (due - item.due_at)),
                    })
                    .collect();
                new.requirements = item.requirements.iter().map(|r| r.text.clone()).collect();
                let created = self.add_item(new, now)?;
                // Keep the series anchored to its original day of month.
                let anchor = recurrence::anchor_day(&item, &clock);
                if matches!(item.recurrence.freq, Freq::Monthly | Freq::Yearly) {
                    self.conn.execute(
                        "UPDATE items SET recur_anchor_day = ?2 WHERE id = ?1",
                        params![created.id, anchor],
                    )?;
                }
                Some(self.get_item(&created.id)?)
            }
            None => None,
        };
        Ok((self.get_item(&item.id)?, next))
    }

    pub fn snooze_item(
        &mut self,
        id: &str,
        until: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<Item> {
        let item = self.get_visible(id)?;
        if until <= now {
            return Err(invalid(
                "The snooze time is in the past. Give a time after now; snooze only silences reminders and does not move the deadline.",
            ));
        }
        self.conn.execute(
            "UPDATE items SET snoozed_until = ?2, updated_at = ?3 WHERE id = ?1",
            params![item.id, fmt_utc(trunc(until)), fmt_utc(trunc(now))],
        )?;
        self.get_item(&item.id)
    }

    /// Soft delete: the item moves to the archive and is purged after 30 days.
    pub fn delete_item(&mut self, id: &str, now: DateTime<Utc>) -> Result<Item> {
        let item = self.get_visible(id)?;
        let now = fmt_utc(trunc(now));
        self.conn.execute(
            "UPDATE items SET status = 'archived', archived_at = ?2, updated_at = ?2 WHERE id = ?1",
            params![item.id, now],
        )?;
        self.get_item(&item.id)
    }

    /// Bring an archived item back as open (or done, if it was completed).
    pub fn restore_item(&mut self, id: &str, now: DateTime<Utc>) -> Result<Item> {
        let item = self.get_item(id)?;
        if item.status != Status::Archived {
            return Err(invalid(format!("Item {} is not in the archive.", item.id)));
        }
        let status = if item.completed_at.is_some() {
            "done"
        } else {
            "open"
        };
        self.conn.execute(
            "UPDATE items SET status = ?2, archived_at = NULL, updated_at = ?3 WHERE id = ?1",
            params![item.id, status, fmt_utc(trunc(now))],
        )?;
        self.get_item(&item.id)
    }

    /// Permanently remove items archived more than 30 days ago.
    pub fn purge_archived(&mut self, now: DateTime<Utc>) -> Result<usize> {
        let cutoff = fmt_utc(now - Duration::days(PURGE_AFTER_DAYS));
        Ok(self.conn.execute(
            "DELETE FROM items WHERE status = 'archived' AND archived_at IS NOT NULL AND archived_at < ?1",
            [cutoff],
        )?)
    }

    pub fn list_items(&self, filter: &ItemFilter) -> Result<Vec<Item>> {
        let statuses: Vec<Status> = if filter.statuses.is_empty() {
            vec![Status::Open]
        } else {
            filter.statuses.clone()
        };
        let mut sql = String::from("SELECT * FROM items WHERE status IN (");
        let mut args: Vec<String> = vec![];
        for (i, s) in statuses.iter().enumerate() {
            if i > 0 {
                sql.push(',');
            }
            args.push(s.as_str().to_string());
            sql.push_str(&format!("?{}", args.len()));
        }
        sql.push(')');
        if !filter.kinds.is_empty() {
            sql.push_str(" AND kind IN (");
            for (i, k) in filter.kinds.iter().enumerate() {
                if i > 0 {
                    sql.push(',');
                }
                args.push(k.as_str().to_string());
                sql.push_str(&format!("?{}", args.len()));
            }
            sql.push(')');
        }
        if let Some(f) = filter.due_from {
            args.push(fmt_utc(f));
            sql.push_str(&format!(" AND due_at >= ?{}", args.len()));
        }
        if let Some(t) = filter.due_to {
            args.push(fmt_utc(t));
            sql.push_str(&format!(" AND due_at <= ?{}", args.len()));
        }
        if let Some(q) = filter
            .text
            .as_deref()
            .map(str::trim)
            .filter(|q| !q.is_empty())
        {
            let like = format!(
                "%{}%",
                q.to_lowercase()
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            );
            args.push(like);
            let n = args.len();
            sql.push_str(&format!(
                " AND (lower(title) LIKE ?{n} ESCAPE '\\' OR lower(notes) LIKE ?{n} ESCAPE '\\'
                   OR lower(COALESCE(source, '')) LIKE ?{n} ESCAPE '\\'
                   OR EXISTS (SELECT 1 FROM steps s WHERE s.item_id = items.id AND lower(s.title) LIKE ?{n} ESCAPE '\\')
                   OR EXISTS (SELECT 1 FROM requirements r WHERE r.item_id = items.id AND lower(r.text) LIKE ?{n} ESCAPE '\\'))"
            ));
        }
        sql.push_str(" ORDER BY due_at, created_at, id");
        if let Some(l) = filter.limit {
            sql.push_str(&format!(" LIMIT {l}"));
        }
        let mut stmt = self.conn.prepare(&sql)?;
        let mut items: Vec<Item> = stmt
            .query_map(rusqlite::params_from_iter(args.iter()), item_from_row)?
            .collect::<rusqlite::Result<_>>()?;
        for item in &mut items {
            self.load_children(item)?;
        }
        Ok(items)
    }

    pub fn open_items(&self) -> Result<Vec<Item>> {
        self.list_items(&ItemFilter::default())
    }

    // ---------- routine ----------

    pub fn routine(&self) -> Result<Vec<RoutineBlock>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, weekday, start, "end", label, available_for_tasks FROM routine_blocks ORDER BY weekday, start, id"#,
        )?;
        let blocks = stmt
            .query_map([], |r| {
                Ok(RoutineBlock {
                    id: r.get(0)?,
                    weekday: r.get(1)?,
                    start: r.get(2)?,
                    end: r.get(3)?,
                    label: r.get(4)?,
                    available_for_tasks: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(blocks)
    }

    /// Replace the whole routine.
    pub fn set_routine(&mut self, blocks: &[RoutineBlock]) -> Result<Vec<RoutineBlock>> {
        for b in blocks {
            b.validate()?;
        }
        let tx = self.write_tx()?;
        tx.execute("DELETE FROM routine_blocks", [])?;
        for b in blocks {
            let (s, e) = b.times()?;
            tx.execute(
                r#"INSERT INTO routine_blocks (weekday, start, "end", label, available_for_tasks) VALUES (?1, ?2, ?3, ?4, ?5)"#,
                params![
                    b.weekday,
                    s.format("%H:%M").to_string(),
                    e.format("%H:%M").to_string(),
                    b.label.trim(),
                    b.available_for_tasks
                ],
            )?;
        }
        tx.commit()?;
        self.routine()
    }

    // ---------- reminder log ----------

    pub fn sent_reminders(&self) -> Result<HashSet<(String, String)>> {
        let mut stmt = self.conn.prepare("SELECT item_id, key FROM reminder_log")?;
        let set = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(set)
    }

    pub fn reminder_log(&self) -> Result<Vec<ReminderLogEntry>> {
        let mut stmt = self
            .conn
            .prepare("SELECT item_id, key, sent_at FROM reminder_log ORDER BY sent_at, item_id")?;
        let v = stmt
            .query_map([], |r| {
                Ok(ReminderLogEntry {
                    item_id: r.get(0)?,
                    key: r.get(1)?,
                    sent_at: time_col(r, "sent_at")?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
    }

    /// Record reminders as sent. Already-recorded pairs are ignored, so a
    /// reminder is never logged (or sent) twice.
    pub fn record_sent(
        &mut self,
        entries: &[(String, String)],
        now: DateTime<Utc>,
    ) -> Result<usize> {
        let tx = self.write_tx()?;
        let mut n = 0;
        for (item, key) in entries {
            n += tx.execute(
                "INSERT OR IGNORE INTO reminder_log (item_id, key, sent_at)
                 SELECT ?1, ?2, ?3 WHERE EXISTS (SELECT 1 FROM items WHERE id = ?1)",
                params![item, key, fmt_utc(now)],
            )?;
        }
        tx.commit()?;
        Ok(n)
    }

    // ---------- export / import ----------

    pub fn export(&self, now: DateTime<Utc>) -> Result<Export> {
        Ok(Export {
            format: "paperweight-export".into(),
            version: 1,
            exported_at: trunc(now),
            items: self.list_items(&ItemFilter {
                statuses: Status::ALL.to_vec(),
                ..Default::default()
            })?,
            routine: self.routine()?,
            settings: self.settings()?,
        })
    }

    /// Import an export. Items whose id already exists are replaced; others
    /// are added. The routine and settings are replaced. Returns the number of
    /// items imported.
    pub fn import(&mut self, data: &Export) -> Result<usize> {
        if data.format != "paperweight-export" {
            return Err(invalid("This file is not a Paperweight export."));
        }
        data.settings.validate()?;
        let tx = self.write_tx()?;
        for item in &data.items {
            clean_title(&item.title)?;
            tx.execute("DELETE FROM items WHERE id = ?1", [&item.id])?;
            tx.execute(
                "INSERT INTO items (id, title, kind, due_at, all_day, status, notes, source, estimated_minutes,
                    snoozed_until, recur_freq, recur_interval, recur_anchor_day, created_at, updated_at, completed_at, archived_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    item.id.to_ascii_lowercase(),
                    item.title,
                    item.kind.as_str(),
                    fmt_utc(item.due_at),
                    item.all_day,
                    item.status.as_str(),
                    item.notes,
                    item.source,
                    item.estimated_minutes,
                    item.snoozed_until.map(fmt_utc),
                    item.recurrence.freq.as_str(),
                    item.recurrence.interval.max(1),
                    item.recurrence_anchor_day,
                    fmt_utc(item.created_at),
                    fmt_utc(item.updated_at),
                    item.completed_at.map(fmt_utc),
                    item.archived_at.map(fmt_utc),
                ],
            )?;
            for s in &item.steps {
                tx.execute(
                    "INSERT INTO steps (item_id, title, done, soft_due_at, position) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![item.id, s.title, s.done, s.soft_due_at.map(fmt_utc), s.position],
                )?;
            }
            for r in &item.requirements {
                tx.execute(
                    "INSERT INTO requirements (item_id, text, done, position) VALUES (?1, ?2, ?3, ?4)",
                    params![item.id, r.text, r.done, r.position],
                )?;
            }
        }
        tx.commit()?;
        self.set_routine(&data.routine)?;
        self.save_settings(&data.settings)?;
        Ok(data.items.len())
    }
}

fn describe_children<'a>(it: impl Iterator<Item = (i64, &'a str)>) -> String {
    let v: Vec<String> = it.map(|(id, t)| format!("{id} \"{t}\"")).collect();
    if v.is_empty() {
        "none".into()
    } else {
        v.join(", ")
    }
}

fn insert_step(tx: &rusqlite::Transaction, item_id: &str, s: &NewStep, pos: i64) -> Result<()> {
    let title = clean_title(&s.title)?;
    tx.execute(
        "INSERT INTO steps (item_id, title, done, soft_due_at, position) VALUES (?1, ?2, 0, ?3, ?4)",
        params![item_id, title, s.soft_due_at.map(fmt_utc), pos],
    )?;
    Ok(())
}

fn insert_requirement(
    tx: &rusqlite::Transaction,
    item_id: &str,
    text: &str,
    pos: i64,
) -> Result<()> {
    let text = text.trim();
    if text.is_empty() {
        return Err(invalid("requirement text must not be empty"));
    }
    tx.execute(
        "INSERT INTO requirements (item_id, text, done, position) VALUES (?1, ?2, 0, ?3)",
        params![item_id, text, pos],
    )?;
    Ok(())
}
