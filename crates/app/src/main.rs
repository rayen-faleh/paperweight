//! Paperweight desktop app. It is the only long-running process, so it owns
//! the reminder scheduler, the calendar feed and the tray icon. All data
//! access goes through `paperweight-core`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod badge;
mod commands;
mod scheduler;
mod tray;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use paperweight_core::Store;
use paperweight_core::feed::FeedServer;
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_notification::NotificationExt;

pub struct AppState {
    pub db_path: PathBuf,
    pub feed: Mutex<Option<FeedServer>>,
    pub feed_error: Mutex<Option<String>>,
    pub shortcut: Mutex<Option<String>>,
    pub shortcut_error: Mutex<Option<String>>,
    /// Notification id → item id, for clicks and actions.
    pub notified: Mutex<HashMap<i32, String>>,
}

impl AppState {
    pub fn store(&self) -> Result<Store, String> {
        Store::open(&self.db_path).map_err(|e| e.to_string())
    }
}

struct Args {
    db: Option<PathBuf>,
    demo: bool,
    seed: bool,
    hidden: bool,
}

fn parse_args() -> Args {
    let mut a = Args {
        db: None,
        demo: false,
        seed: false,
        hidden: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--db" => a.db = it.next().map(PathBuf::from),
            "--demo" => a.demo = true,
            "--seed-demo" => {
                a.demo = true;
                a.seed = true;
            }
            "--hidden" => a.hidden = true,
            "--help" | "-h" => {
                eprintln!(
                    "paperweight — local-first deadline tracker\n\nUsage: paperweight [--hidden] [--demo | --seed-demo] [--db PATH]\n\n  --hidden     start in the tray without opening the window (used at login)\n  --demo       use the separate demo database\n  --seed-demo  reset the demo database to a realistic week, then use it\n  --db PATH    use this database (default: $PAPERWEIGHT_DB or the OS data directory)"
                );
                std::process::exit(0);
            }
            _ => {}
        }
    }
    a
}

fn init_logging(db_path: &Path) {
    use tracing_subscriber::fmt::writer::MakeWriterExt;
    let log_path = db_path.with_file_name("paperweight.log");
    let filter = tracing_subscriber::EnvFilter::try_from_env("PAPERWEIGHT_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        Ok(file) => tracing_subscriber::fmt()
            .with_ansi(false)
            .with_env_filter(filter)
            .with_writer(std::io::stderr.and(Mutex::new(file)))
            .init(),
        Err(_) => tracing_subscriber::fmt()
            .with_ansi(false)
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .init(),
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Bring the window up and tell the UI to open an item.
pub fn open_item(app: &AppHandle, id: &str) {
    show_main(app);
    let _ = app.emit("open-item", id.to_string());
}

pub fn open_quick_add(app: &AppHandle) {
    show_main(app);
    let _ = app.emit("quick-add", ());
}

/// (Re)start the calendar feed on the configured port.
pub fn restart_feed(app: &AppHandle) {
    let state = app.state::<AppState>();
    let port = state
        .store()
        .and_then(|s| s.settings().map_err(|e| e.to_string()))
        .map(|s| s.feed_port)
        .unwrap_or(47800);
    let mut feed = state.feed.lock().unwrap();
    if let Some(old) = feed.take() {
        old.stop();
    }
    match FeedServer::start(port, state.db_path.clone()) {
        Ok(f) => {
            tracing::info!(url = %f.url(), "calendar feed listening");
            *feed = Some(f);
            *state.feed_error.lock().unwrap() = None;
        }
        Err(e) => {
            tracing::warn!(port, "calendar feed could not start: {e}");
            *state.feed_error.lock().unwrap() = Some(format!("Port {port} is not available: {e}"));
        }
    }
}

/// Register the global quick-add shortcut from settings.
pub fn register_shortcut(app: &AppHandle) {
    let state = app.state::<AppState>();
    let wanted = state
        .store()
        .and_then(|s| s.settings().map_err(|e| e.to_string()))
        .map(|s| s.quick_add_shortcut)
        .unwrap_or_default();
    let gs = app.global_shortcut();
    let mut current = state.shortcut.lock().unwrap();
    if current.as_deref() == Some(wanted.as_str()) {
        return;
    }
    let _ = gs.unregister_all();
    *current = None;
    if wanted.trim().is_empty() {
        *state.shortcut_error.lock().unwrap() = None;
        return;
    }
    match gs.register(wanted.as_str()) {
        Ok(()) => {
            tracing::info!(shortcut = %wanted, "quick add shortcut registered");
            *current = Some(wanted);
            *state.shortcut_error.lock().unwrap() = None;
        }
        Err(e) => {
            tracing::warn!(shortcut = %wanted, "could not register shortcut: {e}");
            *state.shortcut_error.lock().unwrap() =
                Some(format!("Could not register {wanted}: {e}"));
        }
    }
}

/// Poll `PRAGMA data_version` once a second. When another connection (the
/// MCP server, or a command) commits, tell the UI and refresh the tray.
fn spawn_watcher(app: AppHandle, db_path: PathBuf) {
    std::thread::Builder::new()
        .name("paperweight-watcher".into())
        .spawn(move || {
            let conn = loop {
                match paperweight_core::db::open_raw(&db_path) {
                    Ok(c) => break c,
                    Err(e) => {
                        tracing::warn!("watcher cannot open database: {e}");
                        std::thread::sleep(Duration::from_secs(5));
                    }
                }
            };
            let mut last = paperweight_core::db::data_version(&conn).ok();
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let now = paperweight_core::db::data_version(&conn).ok();
                if now.is_some() && now != last {
                    last = now;
                    tracing::debug!("database changed; refreshing");
                    let _ = app.emit("data-changed", ());
                    tray::refresh(&app);
                }
            }
        })
        .expect("spawn watcher thread");
}

fn main() {
    let args = parse_args();
    let db_path = match (&args.db, args.demo) {
        (Some(p), _) => p.clone(),
        (None, true) => paperweight_core::db::demo_path(),
        (None, false) => paperweight_core::db::default_path(),
    };
    if args.seed {
        if let Err(e) = paperweight_core::demo::reset_demo(&db_path, chrono::Utc::now()) {
            eprintln!(
                "Could not seed the demo database at {}: {e}",
                db_path.display()
            );
            std::process::exit(1);
        }
    } else if let Err(e) = Store::open(&db_path) {
        eprintln!("Could not open the database at {}: {e}", db_path.display());
        std::process::exit(1);
    }
    init_logging(&db_path);
    tracing::info!(db = %db_path.display(), "paperweight starting");
    let start_hidden = args.hidden;

    let app = tauri::Builder::default()
        // Must be first: a second launch focuses the running app instead.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if !argv.iter().any(|a| a == "--hidden") {
                show_main(app);
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--hidden"])
                .app_name("Paperweight")
                .build(),
        )
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        open_quick_add(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            db_path: db_path.clone(),
            feed: Mutex::new(None),
            feed_error: Mutex::new(None),
            shortcut: Mutex::new(None),
            shortcut_error: Mutex::new(None),
            notified: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            commands::brief,
            commands::list_items,
            commands::get_item,
            commands::preview_quick_add,
            commands::preview_due,
            commands::quick_add,
            commands::update_item,
            commands::complete_item,
            commands::snooze_item,
            commands::delete_item,
            commands::restore_item,
            commands::get_routine,
            commands::set_routine,
            commands::get_settings,
            commands::save_settings,
            commands::set_autostart,
            commands::plan_week,
            commands::export_json,
            commands::import_json,
            commands::export_ics,
            commands::connect_info,
            commands::connect_claude_desktop,
            commands::test_notification,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            tray::create(&handle)?;
            restart_feed(&handle);
            register_shortcut(&handle);

            // Start at login by default, once. The user can turn it off in Settings.
            let state = handle.state::<AppState>();
            if let Ok(store) = state.store()
                && store
                    .get_state("autostart.initialized")
                    .ok()
                    .flatten()
                    .is_none()
            {
                match handle.autolaunch().enable() {
                    Ok(()) => tracing::info!("start at login enabled"),
                    Err(e) => tracing::warn!("could not enable start at login: {e}"),
                }
                let _ = store.set_state("autostart.initialized", "1");
            }

            scheduler::register_actions(&handle);
            let h = handle.clone();
            handle
                .notification()
                .on_action(move |performed| scheduler::on_action(&h, performed))?;

            spawn_watcher(handle.clone(), db_path.clone());
            scheduler::spawn(handle.clone());

            if !start_hidden {
                show_main(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window hides it; the app keeps running in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Paperweight");

    app.run(|_app, event| {
        if let RunEvent::ExitRequested { api, code, .. } = event
            && code.is_none()
        {
            // No windows left is not a reason to quit; only "Quit" is.
            api.prevent_exit();
        }
    });
}
