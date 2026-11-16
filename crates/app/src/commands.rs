//! Commands the web view calls. Each opens the database fresh through
//! `paperweight-core`; no SQL lives here.

use chrono::Utc;
use paperweight_core::brief::{Brief, ItemView, brief as make_brief};
use paperweight_core::connect::{self, ConnectOutcome};
use paperweight_core::dates::{parse_due, parse_quick_add};
use paperweight_core::model::{
    ItemPatch, Kind, NewItem, NewStep, Recurrence, RoutineBlock, Settings, Status,
};
use paperweight_core::planner::WeekPlan;
use paperweight_core::{Export, ItemFilter};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_dialog::DialogExt;

use crate::AppState;

type Res<T> = Result<T, String>;

fn e(err: impl std::fmt::Display) -> String {
    err.to_string()
}

fn views(state: &AppState, items: Vec<paperweight_core::Item>) -> Res<Vec<ItemView>> {
    let store = state.store()?;
    let clock = store.clock().map_err(e)?;
    let now = Utc::now();
    Ok(items
        .into_iter()
        .map(|i| ItemView::new(i, &clock, now))
        .collect())
}

fn one(state: &AppState, item: paperweight_core::Item) -> Res<ItemView> {
    Ok(views(state, vec![item])?.remove(0))
}

#[tauri::command]
pub fn brief(state: State<AppState>) -> Res<Brief> {
    let store = state.store()?;
    let clock = store.clock().map_err(e)?;
    Ok(make_brief(
        store.open_items().map_err(e)?,
        &clock,
        Utc::now(),
    ))
}

/// `status`: open (default), done, all (open and done), archived.
#[tauri::command]
pub fn list_items(
    state: State<AppState>,
    status: Option<String>,
    kind: Option<String>,
    query: Option<String>,
) -> Res<Vec<ItemView>> {
    let statuses = match status.as_deref() {
        None | Some("open") => vec![Status::Open],
        Some("done") => vec![Status::Done],
        Some("all") => vec![Status::Open, Status::Done],
        Some("archived") => vec![Status::Archived],
        Some(other) => return Err(format!("unknown status {other}")),
    };
    let kinds = kind
        .as_deref()
        .filter(|k| !k.is_empty())
        .and_then(Kind::parse)
        .into_iter()
        .collect();
    let store = state.store()?;
    let mut items = store
        .list_items(&ItemFilter {
            statuses,
            kinds,
            text: query,
            ..Default::default()
        })
        .map_err(e)?;
    if status.as_deref() == Some("archived") {
        items.sort_by(|a, b| b.archived_at.cmp(&a.archived_at));
    }
    views(&state, items)
}

#[tauri::command]
pub fn get_item(state: State<AppState>, id: String) -> Res<ItemView> {
    let item = state.store()?.get_item(&id).map_err(e)?;
    one(&state, item)
}

#[derive(Serialize)]
pub struct QuickPreview {
    title: String,
    due_label: String,
    due_local: String,
    all_day: bool,
}

#[tauri::command]
pub fn preview_quick_add(state: State<AppState>, text: String) -> Res<QuickPreview> {
    let store = state.store()?;
    let clock = store.clock().map_err(e)?;
    let now = Utc::now();
    let (title, due) = parse_quick_add(&text, now, &clock).map_err(e)?;
    Ok(QuickPreview {
        title,
        due_label: clock.due_label(due.at, due.all_day, now),
        due_local: clock.local(due.at).to_rfc3339(),
        all_day: due.all_day,
    })
}

#[derive(Serialize)]
pub struct DuePreview {
    due_label: String,
    due_local: String,
    all_day: bool,
}

/// What a typed date means, before saving it.
#[tauri::command]
pub fn preview_due(state: State<AppState>, text: String) -> Res<DuePreview> {
    let store = state.store()?;
    let clock = store.clock().map_err(e)?;
    let now = Utc::now();
    let due = parse_due(&text, now, &clock).map_err(e)?;
    Ok(DuePreview {
        due_label: clock.due_label(due.at, due.all_day, now),
        due_local: clock.local(due.at).to_rfc3339(),
        all_day: due.all_day,
    })
}

#[tauri::command]
pub fn quick_add(state: State<AppState>, text: String, kind: Option<String>) -> Res<ItemView> {
    let mut store = state.store()?;
    let clock = store.clock().map_err(e)?;
    let now = Utc::now();
    let (title, due) = parse_quick_add(&text, now, &clock).map_err(e)?;
    let mut new = NewItem::new(title, due.at, due.all_day);
    new.kind = kind.as_deref().and_then(Kind::parse).unwrap_or(Kind::Other);
    let item = store.add_item(new, now).map_err(e)?;
    one(&state, item)
}

/// Edits from the item detail view. Only present fields change.
#[derive(Debug, Default, Deserialize)]
pub struct Changes {
    title: Option<String>,
    kind: Option<Kind>,
    due: Option<String>,
    notes: Option<String>,
    source: Option<String>,
    estimated_minutes: Option<u32>,
    recurrence: Option<Recurrence>,
    status: Option<Status>,
    clear_snooze: Option<bool>,
    add_step: Option<String>,
    set_step: Option<(i64, bool)>,
    rename_step: Option<(i64, String)>,
    remove_step: Option<i64>,
    add_requirement: Option<String>,
    set_requirement: Option<(i64, bool)>,
    remove_requirement: Option<i64>,
}

#[tauri::command]
pub fn update_item(state: State<AppState>, id: String, changes: Changes) -> Res<ItemView> {
    let mut store = state.store()?;
    let clock = store.clock().map_err(e)?;
    let now = Utc::now();
    let mut patch = ItemPatch {
        title: changes.title,
        kind: changes.kind,
        notes: changes.notes,
        estimated_minutes: changes.estimated_minutes,
        recurrence: changes.recurrence,
        status: changes.status,
        ..Default::default()
    };
    if let Some(d) = changes.due {
        let due = parse_due(&d, now, &clock).map_err(e)?;
        patch.due = Some((due.at, due.all_day));
    }
    if let Some(s) = changes.source {
        patch.source = Some(Some(s).filter(|s| !s.trim().is_empty()));
    }
    if changes.clear_snooze == Some(true) {
        patch.snoozed_until = Some(None);
    }
    if let Some(t) = changes.add_step {
        patch.add_steps.push(NewStep {
            title: t,
            soft_due_at: None,
        });
    }
    patch.set_steps_done.extend(changes.set_step);
    patch.rename_steps.extend(changes.rename_step);
    patch.remove_steps.extend(changes.remove_step);
    patch.add_requirements.extend(changes.add_requirement);
    patch.set_requirements_done.extend(changes.set_requirement);
    patch.remove_requirements.extend(changes.remove_requirement);
    let item = store.update_item(&id, patch, now).map_err(e)?;
    one(&state, item)
}

#[derive(Serialize)]
pub struct Completed {
    item: ItemView,
    next: Option<ItemView>,
}

#[tauri::command]
pub fn complete_item(state: State<AppState>, id: String) -> Res<Completed> {
    let (done, next) = state.store()?.complete_item(&id, Utc::now()).map_err(e)?;
    Ok(Completed {
        item: one(&state, done)?,
        next: next.map(|n| one(&state, n)).transpose()?,
    })
}

#[tauri::command]
pub fn snooze_item(state: State<AppState>, id: String, until: String) -> Res<ItemView> {
    let mut store = state.store()?;
    let clock = store.clock().map_err(e)?;
    let now = Utc::now();
    let due = parse_due(&until, now, &clock).map_err(e)?;
    let at = if due.all_day {
        clock.day_start(clock.deadline_day(due.at))
    } else {
        due.at
    };
    let item = store.snooze_item(&id, at, now).map_err(e)?;
    one(&state, item)
}

#[tauri::command]
pub fn delete_item(state: State<AppState>, id: String) -> Res<ItemView> {
    let item = state.store()?.delete_item(&id, Utc::now()).map_err(e)?;
    one(&state, item)
}

#[tauri::command]
pub fn restore_item(state: State<AppState>, id: String) -> Res<ItemView> {
    let item = state.store()?.restore_item(&id, Utc::now()).map_err(e)?;
    one(&state, item)
}

#[tauri::command]
pub fn get_routine(state: State<AppState>) -> Res<Vec<RoutineBlock>> {
    state.store()?.routine().map_err(e)
}

#[tauri::command]
pub fn set_routine(state: State<AppState>, blocks: Vec<RoutineBlock>) -> Res<Vec<RoutineBlock>> {
    state.store()?.set_routine(&blocks).map_err(e)
}

#[derive(Serialize)]
pub struct SettingsInfo {
    settings: Settings,
    effective_timezone: String,
    db_path: String,
    log_path: String,
    feed_url: String,
    feed_error: Option<String>,
    shortcut_error: Option<String>,
    autostart: bool,
}

#[tauri::command]
pub fn get_settings(app: AppHandle, state: State<AppState>) -> Res<SettingsInfo> {
    let settings = state.store()?.settings().map_err(e)?;
    let clock = settings.clock();
    Ok(SettingsInfo {
        effective_timezone: clock.tz.name().to_string(),
        db_path: state.db_path.display().to_string(),
        log_path: state
            .db_path
            .with_file_name("paperweight.log")
            .display()
            .to_string(),
        feed_url: paperweight_core::feed::feed_url(settings.feed_port),
        feed_error: state.feed_error.lock().unwrap().clone(),
        shortcut_error: state.shortcut_error.lock().unwrap().clone(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        settings,
    })
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<AppState>,
    settings: Settings,
) -> Res<SettingsInfo> {
    let old = state.store()?.settings().map_err(e)?;
    state.store()?.save_settings(&settings).map_err(e)?;
    if old.feed_port != settings.feed_port {
        crate::restart_feed(&app);
    }
    if old.quick_add_shortcut != settings.quick_add_shortcut {
        crate::register_shortcut(&app);
    }
    crate::tray::refresh(&app);
    get_settings(app.clone(), state)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Res<bool> {
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(e)?;
    al.is_enabled().map_err(e)
}

#[tauri::command]
pub fn plan_week(state: State<AppState>) -> Res<WeekPlan> {
    let store = state.store()?;
    let clock = store.clock().map_err(e)?;
    Ok(paperweight_core::planner::plan_week(
        Utc::now(),
        &store.open_items().map_err(e)?,
        &store.routine().map_err(e)?,
        &clock,
    ))
}

fn file_path(p: tauri_plugin_dialog::FilePath) -> Res<std::path::PathBuf> {
    p.into_path().map_err(e)
}

// Dialog commands are async so the blocking dialog runs off the main thread.

#[tauri::command]
pub async fn export_json(app: AppHandle) -> Res<Option<String>> {
    let Some(path) = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .set_file_name(format!(
            "paperweight-export-{}.json",
            Utc::now().format("%Y-%m-%d")
        ))
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = file_path(path)?;
    let state = app.state::<AppState>();
    let export = state.store()?.export(Utc::now()).map_err(e)?;
    std::fs::write(&path, serde_json::to_string_pretty(&export).map_err(e)?).map_err(e)?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
pub async fn import_json(app: AppHandle) -> Res<Option<usize>> {
    let Some(path) = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(file_path(path)?).map_err(e)?;
    let data: Export = serde_json::from_str(&text)
        .map_err(|err| format!("This is not a Paperweight export: {err}"))?;
    let state = app.state::<AppState>();
    let n = state.store()?.import(&data).map_err(e)?;
    Ok(Some(n))
}

#[tauri::command]
pub async fn export_ics(app: AppHandle) -> Res<Option<String>> {
    let Some(path) = app
        .dialog()
        .file()
        .add_filter("iCalendar", &["ics"])
        .set_file_name("paperweight.ics")
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = file_path(path)?;
    let state = app.state::<AppState>();
    let body = paperweight_core::feed::render(&state.db_path).map_err(e)?;
    std::fs::write(&path, body).map_err(e)?;
    Ok(Some(path.display().to_string()))
}

#[derive(Serialize)]
pub struct ConnectInfo {
    binary: Option<String>,
    snippet: Option<String>,
    claude_config_path: Option<String>,
    claude_config_exists: bool,
    db_path: String,
}

#[tauri::command]
pub fn connect_info(state: State<AppState>) -> Res<ConnectInfo> {
    let binary = connect::find_mcp_binary(std::env::current_exe().ok());
    let cfg = connect::claude_desktop_config_path();
    Ok(ConnectInfo {
        snippet: binary.as_deref().map(connect::config_snippet),
        binary: binary.map(|b| b.display().to_string()),
        claude_config_exists: cfg.as_deref().is_some_and(|p| p.is_file()),
        claude_config_path: cfg.map(|p| p.display().to_string()),
        db_path: state.db_path.display().to_string(),
    })
}

#[tauri::command]
pub fn connect_claude_desktop() -> Res<ConnectOutcome> {
    let binary = connect::find_mcp_binary(std::env::current_exe().ok())
        .ok_or("Could not find the paperweight-mcp program next to Paperweight. Reinstall Paperweight, or build it with `cargo build -p paperweight-mcp`.")?;
    let path = connect::claude_desktop_config_path()
        .ok_or("Could not work out where Claude Desktop keeps its config on this system.")?;
    Ok(connect::write_claude_desktop(&path, &binary, Utc::now()))
}

#[tauri::command]
pub fn test_notification(app: AppHandle) -> Res<()> {
    crate::scheduler::deliver(
        &app,
        &paperweight_core::reminders::Delivery {
            kind: paperweight_core::reminders::DeliveryKind::Digest,
            title: "Paperweight".into(),
            body: "Reminders will look like this.".into(),
            item_id: None,
            lines: vec![],
        },
    );
    Ok(())
}
