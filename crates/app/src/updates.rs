//! In-app updates from GitHub Releases, through tauri-plugin-updater.
//!
//! Paperweight only goes online for this: when you press "Check for updates",
//! or once a day if you turned on automatic checks. Updates are signed; the
//! plugin refuses anything not signed with the key whose public half is in
//! tauri.conf.json.

use std::sync::Mutex;
use std::time::Duration;

use chrono::Utc;
use paperweight_core::time::{fmt_utc, parse_utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::AppState;

const LAST_CHECK: &str = "updates.last_check";
const NOTIFIED: &str = "updates.notified_version";

/// The update found by the last check, kept until it is installed.
#[derive(Default)]
pub struct Pending(pub Mutex<Option<Update>>);

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    /// The newer version, if there is one.
    pub available: Option<String>,
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

/// Ask GitHub whether a newer release exists.
pub async fn check(app: &AppHandle) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();
    let updater = app.updater().map_err(|e| e.to_string())?;
    let found = updater.check().await.map_err(|e| {
        format!("Could not check for updates: {e}. Check your internet connection and try again.")
    })?;
    if let Ok(store) = app.state::<AppState>().store() {
        let _ = store.set_state(LAST_CHECK, &fmt_utc(Utc::now()));
    }
    let info = UpdateInfo {
        current,
        available: found.as_ref().map(|u| u.version.clone()),
        notes: found.as_ref().and_then(|u| u.body.clone()),
        date: found.as_ref().and_then(|u| u.date.map(|d| d.to_string())),
    };
    *app.state::<Pending>().0.lock().unwrap() = found;
    crate::tray::refresh(app);
    Ok(info)
}

/// The version waiting to be installed, if a check found one.
pub fn pending_version(app: &AppHandle) -> Option<String> {
    app.state::<Pending>()
        .0
        .lock()
        .unwrap()
        .as_ref()
        .map(|u| u.version.clone())
}

/// Download, verify, install and restart. On Windows the installer replaces
/// the app's files, so the app exits; running copies of paperweight-mcp
/// (started by an AI client) are stopped first, or Windows would refuse to
/// replace the file. They hold no state, and the client starts them again.
pub async fn install(app: AppHandle) -> Result<(), String> {
    let update = app
        .state::<Pending>()
        .0
        .lock()
        .unwrap()
        .take()
        .ok_or("There is no update waiting. Press \"Check for updates\" first.")?;
    let version = update.version.clone();
    tracing::info!(%version, "downloading update");
    let mut downloaded: u64 = 0;
    let progress_app = app.clone();
    let bytes = update
        .download(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress_app.emit("update-progress", Progress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| {
            format!("The update could not be downloaded or its signature did not match: {e}")
        })?;
    tracing::info!(%version, "installing update");
    stop_mcp_servers();
    update
        .install(bytes)
        .map_err(|e| format!("The update could not be installed: {e}"))?;
    // macOS and Linux replace the files in place; start the new version.
    app.restart();
}

#[cfg(windows)]
fn stop_mcp_servers() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "paperweight-mcp.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

#[cfg(not(windows))]
fn stop_mcp_servers() {
    // Files of a running program can be replaced on macOS and Linux.
}

/// Opt-in background check, at most once a day. Tells the person once per
/// new version; installing is always their choice.
pub fn spawn_daily(app: AppHandle) {
    std::thread::Builder::new()
        .name("paperweight-updates".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_secs(60));
            loop {
                if let Err(e) = tauri::async_runtime::block_on(daily(&app)) {
                    tracing::debug!("update check skipped: {e}");
                }
                std::thread::sleep(Duration::from_secs(60 * 60));
            }
        })
        .expect("spawn update thread");
}

async fn daily(app: &AppHandle) -> Result<(), String> {
    let store = app.state::<AppState>().store()?;
    if !store
        .settings()
        .map_err(|e| e.to_string())?
        .auto_update_check
    {
        return Ok(());
    }
    let last = store.get_state(LAST_CHECK).ok().flatten();
    if last
        .as_deref()
        .and_then(parse_utc)
        .is_some_and(|t| Utc::now() - t < chrono::Duration::hours(23))
    {
        return Ok(());
    }
    let info = check(app).await?;
    if let Some(v) = info.available
        && store.get_state(NOTIFIED).ok().flatten().as_deref() != Some(v.as_str())
    {
        let _ = app
            .notification()
            .builder()
            .title("Paperweight update available")
            .body(format!(
                "Version {v} is ready. Open Settings → Updates to install it."
            ))
            .show();
        let _ = store.set_state(NOTIFIED, &v);
    }
    Ok(())
}
