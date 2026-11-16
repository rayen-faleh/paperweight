//! The reminder scheduler: once a minute, evaluate the pure reminder rules
//! from `paperweight-core` and deliver what they return as a notification.

use std::time::Duration;

use chrono::{DateTime, Utc};
use paperweight_core::reminders::{self, Delivery, DeliveryKind, Input};
use paperweight_core::time::{fmt_utc, parse_utc};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::{Action, ActionPerformed, ActionType, NotificationExt};

use crate::AppState;

const LAST_CHECK: &str = "scheduler.last_check";
const LAST_PURGE: &str = "scheduler.last_purge";
const ACTION_TYPE: &str = "reminder";

pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("paperweight-scheduler".into())
        .spawn(move || {
            let mut first = true;
            loop {
                if let Err(e) = tick(&app, Utc::now(), first) {
                    tracing::warn!("reminder check failed: {e}");
                }
                first = false;
                crate::tray::refresh(&app);
                // Wake just after the next minute boundary.
                let secs = 60 - (Utc::now().timestamp() % 60) as u64;
                std::thread::sleep(Duration::from_secs(secs.max(1)) + Duration::from_millis(200));
            }
        })
        .expect("spawn scheduler thread");
}

fn tick(app: &AppHandle, now: DateTime<Utc>, first: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut store = state.store()?;
    let last = store
        .get_state(LAST_CHECK)
        .map_err(|e| e.to_string())?
        .as_deref()
        .and_then(parse_utc);
    let catch_up = reminders::is_catch_up(last, now, first);

    // Purge the archive once a day.
    let last_purge = store
        .get_state(LAST_PURGE)
        .ok()
        .flatten()
        .as_deref()
        .and_then(parse_utc);
    if last_purge.is_none_or(|t| now - t > chrono::Duration::hours(24)) {
        match store.purge_archived(now) {
            Ok(n) if n > 0 => tracing::info!(purged = n, "purged old archived items"),
            _ => {}
        }
        let _ = store.set_state(LAST_PURGE, &fmt_utc(now));
    }

    let items = store.open_items().map_err(|e| e.to_string())?;
    let settings = store.settings().map_err(|e| e.to_string())?;
    let sent = store.sent_reminders().map_err(|e| e.to_string())?;
    let out = reminders::evaluate(&Input {
        now,
        items: &items,
        settings: &settings,
        sent: &sent,
        catch_up,
    });
    if let Some(d) = &out.delivery {
        deliver(app, d);
    }
    if !out.mark_sent.is_empty() {
        store
            .record_sent(&out.mark_sent, now)
            .map_err(|e| e.to_string())?;
    }
    // Reminders held by quiet hours stay pending (they are not marked sent),
    // so they still arrive together as one digest when quiet hours end.
    store
        .set_state(LAST_CHECK, &fmt_utc(now))
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn register_actions(app: &AppHandle) {
    let t = ActionType::builder(ACTION_TYPE)
        .actions(vec![
            Action::builder("done", "Done").build(),
            Action::builder("snooze", "Snooze till tomorrow").build(),
        ])
        .build();
    if let Err(e) = app.notification().register_action_types(vec![t]) {
        tracing::warn!("could not register notification actions: {e}");
    }
}

fn next_notification_id() -> i32 {
    (Utc::now().timestamp_millis() % i32::MAX as i64) as i32
}

pub fn deliver(app: &AppHandle, d: &Delivery) {
    let id = next_notification_id();
    let state = app.state::<AppState>();
    let mut builder = app
        .notification()
        .builder()
        .id(id)
        .title(&d.title)
        .body(&d.body);
    if let Some(item) = &d.item_id {
        state.notified.lock().unwrap().insert(id, item.clone());
        builder = builder.extra("item_id", item.clone());
        if d.kind == DeliveryKind::Single {
            // Done and Snooze buttons (supported by the notification plugin on
            // Linux, Windows and macOS when an action handler is registered).
            builder = builder.action_type_id(ACTION_TYPE);
        }
    }
    match builder.show() {
        Ok(()) => {
            tracing::info!(kind = ?d.kind, title = %d.title, body = %d.body, "reminder delivered")
        }
        Err(e) => tracing::warn!("could not show notification: {e}"),
    }
}

fn item_of(app: &AppHandle, performed: &ActionPerformed) -> Option<String> {
    if let Some(n) = performed.notification() {
        if let Some(id) = n.extra().get("item_id").and_then(|v| v.as_str()) {
            return Some(id.to_string());
        }
        if let Some(id) = app
            .state::<AppState>()
            .notified
            .lock()
            .unwrap()
            .get(&n.id())
        {
            return Some(id.clone());
        }
    }
    None
}

/// A click (`tap`) opens the item (or Today for a digest); Done completes it;
/// Snooze silences it until quiet hours end tomorrow.
pub fn on_action(app: &AppHandle, performed: &ActionPerformed) {
    let item = item_of(app, performed);
    tracing::info!(action = performed.action_id(), item = ?item, "notification action");
    let state = app.state::<AppState>();
    // The buttons did their job; take the notification off the screen.
    if matches!(performed.action_id(), "done" | "snooze")
        && let Some(n) = performed.notification()
    {
        let _ = app.notification().remove_active(vec![n.id()]);
    }
    match (performed.action_id(), item) {
        ("done", Some(id)) => {
            if let Ok(mut store) = state.store()
                && let Err(e) = store.complete_item(&id, Utc::now())
            {
                tracing::warn!("done from notification failed: {e}");
            }
        }
        ("snooze", Some(id)) => {
            if let Ok(mut store) = state.store() {
                let now = Utc::now();
                let until = store.settings().ok().and_then(|s| {
                    let clock = s.clock();
                    let end = paperweight_core::model::parse_hhmm(&s.quiet_end).ok()?;
                    Some(clock.at(clock.today(now) + chrono::Duration::days(1), end))
                });
                if let Some(until) = until
                    && let Err(e) = store.snooze_item(&id, until, now)
                {
                    tracing::warn!("snooze from notification failed: {e}");
                }
            }
        }
        ("tap", Some(id)) => crate::open_item(app, &id),
        ("tap", None) => crate::show_main(app),
        _ => {}
    }
}
