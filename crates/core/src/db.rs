//! Connection setup and migrations.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior};

use crate::error::Result;

/// Environment variable that overrides the database path.
pub const DB_ENV: &str = "PAPERWEIGHT_DB";

/// Numbered migrations. Index 0 is migration 1. Never edit a shipped
/// migration; append a new one.
const MIGRATIONS: &[&str] = &[
    // 1: schema
    r#"
    CREATE TABLE items (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        kind TEXT NOT NULL,
        due_at TEXT NOT NULL,
        all_day INTEGER NOT NULL DEFAULT 0,
        status TEXT NOT NULL DEFAULT 'open',
        notes TEXT NOT NULL DEFAULT '',
        source TEXT,
        estimated_minutes INTEGER NOT NULL DEFAULT 30,
        snoozed_until TEXT,
        recur_freq TEXT NOT NULL DEFAULT 'none',
        recur_interval INTEGER NOT NULL DEFAULT 1,
        recur_anchor_day INTEGER,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        completed_at TEXT,
        archived_at TEXT
    );
    CREATE INDEX items_status_due ON items(status, due_at);

    CREATE TABLE steps (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
        title TEXT NOT NULL,
        done INTEGER NOT NULL DEFAULT 0,
        soft_due_at TEXT,
        position INTEGER NOT NULL
    );
    CREATE INDEX steps_item ON steps(item_id, position);

    CREATE TABLE requirements (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
        text TEXT NOT NULL,
        done INTEGER NOT NULL DEFAULT 0,
        position INTEGER NOT NULL
    );
    CREATE INDEX requirements_item ON requirements(item_id, position);

    CREATE TABLE routine_blocks (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        weekday INTEGER NOT NULL CHECK (weekday BETWEEN 0 AND 6),
        start TEXT NOT NULL,
        "end" TEXT NOT NULL,
        label TEXT NOT NULL,
        available_for_tasks INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    CREATE TABLE reminder_log (
        item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
        key TEXT NOT NULL,
        sent_at TEXT NOT NULL,
        PRIMARY KEY (item_id, key)
    );

    CREATE TABLE app_state (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    "#,
    // 2: Takaya's routine. Weekday 0 = Monday. Same shape every day.
    r#"
    WITH RECURSIVE days(d) AS (SELECT 0 UNION ALL SELECT d + 1 FROM days WHERE d < 6)
    INSERT INTO routine_blocks (weekday, start, "end", label, available_for_tasks)
    SELECT d, '12:00', '14:00', 'Wake up, eat, get going', 0 FROM days
    UNION ALL SELECT d, '14:00', '18:00', 'Assignments and research', 1 FROM days
    UNION ALL SELECT d, '18:00', '21:00', 'Cooking and chores', 0 FROM days
    UNION ALL SELECT d, '22:00', '02:00', 'Free time and admin', 1 FROM days;
    "#,
];

pub fn latest_version() -> i64 {
    MIGRATIONS.len() as i64
}

/// The database path: `$PAPERWEIGHT_DB` if set, otherwise
/// `<OS data dir>/paperweight.db`.
pub fn default_path() -> PathBuf {
    if let Some(p) = std::env::var_os(DB_ENV).filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    data_dir().join("paperweight.db")
}

/// The demo database path, used by `--seed-demo` and `--demo`. Honors
/// `$PAPERWEIGHT_DB` so tests never touch real data.
pub fn demo_path() -> PathBuf {
    if let Some(p) = std::env::var_os(DB_ENV).filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    data_dir().join("paperweight-demo.db")
}

pub fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("app", "Paperweight", "Paperweight")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Open a connection with the required pragmas and bring the schema up to date.
pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut conn = Connection::open(path)?;
    configure(&conn)?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// Open without migrating (for the change watcher, which only reads
/// `data_version`).
pub fn open_raw(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    configure(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<()> {
    conn.busy_timeout(Duration::from_millis(5000))?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    // Switching to WAL needs a brief exclusive lock; when two processes race
    // on a brand-new file one may see SQLITE_BUSY despite the timeout, so retry.
    let mut tries = 0;
    loop {
        match conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get::<_, String>(0)) {
            Ok(_) => break,
            Err(e) if tries < 50 && is_busy(&e) => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(e.into()),
        }
    }
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

fn is_busy(e: &rusqlite::Error) -> bool {
    matches!(
        e.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy) | Some(rusqlite::ErrorCode::DatabaseLocked)
    )
}

/// Apply pending migrations inside one IMMEDIATE transaction. The version is
/// read after the write lock is taken, so two processes starting together
/// apply each migration exactly once.
pub fn migrate(conn: &mut Connection) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if current >= latest_version() {
        return Ok(());
    }
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let current: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let version = i as i64 + 1;
        if version > current {
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", version)?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn user_version(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// `PRAGMA data_version`: changes when another connection commits.
pub fn data_version(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("PRAGMA data_version", [], |r| r.get(0))?)
}
