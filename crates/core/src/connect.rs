//! "Connect your AI assistant": the MCP config entry, and a careful writer for
//! Claude Desktop's config file.
//!
//! The writer never creates the file and never writes over a file it cannot
//! parse. It backs the file up first, then changes only
//! `mcpServers.paperweight`, keeping every other key and server as it was
//! (including key order).

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Map, Value, json};

pub const SERVER_NAME: &str = "paperweight";

/// Where Claude Desktop keeps its config on this OS.
pub fn claude_desktop_config_path() -> Option<PathBuf> {
    claude_desktop_config_path_for(std::env::consts::OS)
}

pub fn claude_desktop_config_path_for(os: &str) -> Option<PathBuf> {
    let base = directories::BaseDirs::new()?;
    match os {
        "macos" => Some(
            base.home_dir()
                .join("Library/Application Support/Claude/claude_desktop_config.json"),
        ),
        // %APPDATA%\Claude\claude_desktop_config.json
        "windows" => Some(
            base.config_dir()
                .join("Claude")
                .join("claude_desktop_config.json"),
        ),
        // Claude Desktop has no official Linux build; this is where community
        // builds look ($XDG_CONFIG_HOME/Claude).
        _ => Some(
            base.config_dir()
                .join("Claude")
                .join("claude_desktop_config.json"),
        ),
    }
}

/// The server entry for any MCP client that uses the `mcpServers` format.
pub fn server_entry(binary: &Path) -> Value {
    json!({
        "command": binary.to_string_lossy(),
        "args": []
    })
}

/// The full snippet to paste into a client config.
pub fn config_snippet(binary: &Path) -> String {
    let v = json!({ "mcpServers": { SERVER_NAME: server_entry(binary) } });
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ConnectOutcome {
    /// The file was backed up and updated.
    Written {
        path: PathBuf,
        backup: PathBuf,
        /// The file as written.
        result: String,
        /// Whether a previous paperweight entry was replaced.
        replaced: bool,
    },
    /// Nothing was written; show these manual steps instead.
    Manual { reason: String, steps: Vec<String> },
}

pub fn manual_steps(path: &Path, binary: &Path) -> Vec<String> {
    vec![
        "Open Claude Desktop, then Settings → Developer → Edit Config. This opens claude_desktop_config.json.".into(),
        format!("The file lives at: {}", path.display()),
        "If the file is empty, paste the whole snippet below. If it already has an \"mcpServers\" section, add the \"paperweight\" entry inside it.".into(),
        config_snippet(binary),
        "Save the file and restart Claude Desktop.".into(),
    ]
}

/// Merge the paperweight entry into Claude Desktop's config at `path`.
pub fn write_claude_desktop(path: &Path, binary: &Path, now: DateTime<Utc>) -> ConnectOutcome {
    let manual = |reason: String| ConnectOutcome::Manual {
        reason,
        steps: manual_steps(path, binary),
    };
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return manual(format!(
                "Claude Desktop's config file was not found at {}. Is Claude Desktop installed and opened at least once?",
                path.display()
            ));
        }
        Err(e) => return manual(format!("Could not read {}: {e}", path.display())),
    };
    let mut root: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            return manual(format!(
                "{} is not valid JSON ({e}). Paperweight did not change it.",
                path.display()
            ));
        }
    };
    let Some(obj) = root.as_object_mut() else {
        return manual(format!(
            "{} does not contain a JSON object. Paperweight did not change it.",
            path.display()
        ));
    };
    let servers = obj
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(servers) = servers.as_object_mut() else {
        return manual(
            "\"mcpServers\" in the config is not an object. Paperweight did not change it.".into(),
        );
    };
    let replaced = servers.contains_key(SERVER_NAME);
    servers.insert(SERVER_NAME.into(), server_entry(binary));

    let backup = path.with_file_name(format!(
        "{}.paperweight-backup-{}",
        path.file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default(),
        now.format("%Y%m%dT%H%M%SZ")
    ));
    if let Err(e) = std::fs::copy(path, &backup) {
        return manual(format!(
            "Could not back up the config to {}: {e}. Nothing was written.",
            backup.display()
        ));
    }
    let mut result = match serde_json::to_string_pretty(&root) {
        Ok(s) => s,
        Err(e) => return manual(format!("Could not serialize the config: {e}")),
    };
    result.push('\n');
    // Write to a temp file then rename, so a crash never leaves half a file.
    let tmp = path.with_extension("json.paperweight-tmp");
    if let Err(e) = std::fs::write(&tmp, &result).and_then(|_| std::fs::rename(&tmp, path)) {
        let _ = std::fs::remove_file(&tmp);
        return manual(format!(
            "Could not write {}: {e}. Your backup is at {}.",
            path.display(),
            backup.display()
        ));
    }
    ConnectOutcome::Written {
        path: path.to_path_buf(),
        backup,
        result,
        replaced,
    }
}

/// Find the installed `paperweight-mcp` binary: next to the running
/// executable (how Tauri installs sidecars), then on PATH.
pub fn find_mcp_binary(current_exe: Option<PathBuf>) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "paperweight-mcp.exe"
    } else {
        "paperweight-mcp"
    };
    if let Some(exe) = current_exe {
        let dir = exe.parent()?.to_path_buf();
        let candidates = [
            dir.join(name),
            // macOS bundle: Contents/MacOS holds both binaries; this covers
            // running from a dev build of the app as well.
            dir.join("../MacOS").join(name),
        ];
        for c in candidates {
            if c.is_file() {
                return Some(c.canonicalize().unwrap_or(c));
            }
        }
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|p| p.join(name))
        .find(|p| p.is_file())
}
