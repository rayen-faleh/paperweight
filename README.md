# Paperweight

Paperweight is an open-source, local-first deadline tracker for paperwork, assignments and life admin. You add things by telling your AI assistant in one sentence ("here's the form from city hall, track it"), and Paperweight reminds you, so you never have to remember to open it.

- **One sentence to add.** Your assistant talks to Paperweight through a local MCP server. It can add an item with its checklist of what's needed and its steps in one call, then confirm it back to you.
- **Reminders come to you.** A small tray app sends reminders, with your system's notification sound, a week before, two days before, on the day and daily while overdue. It holds them during quiet hours and sends one summary instead of a flood.
- **Your day, not the clock's.** The day ends at 02:00 by default, so something due at 01:00 Wednesday is "Tuesday night".
- **Everything stays on your computer.** No accounts, no cloud, no telemetry. The only network traffic is on `127.0.0.1` (the calendar feed). The app never calls an LLM.

It is built for one real person first: a university student who wakes around noon, works from 14:00 to 18:00, does admin late at night, and gives up on planners that need effort or need opening.

## What's in the box

| Part | What it is |
|---|---|
| `crates/core` | Domain model, SQLite storage and migrations, date parser, day rollover, recurrence, reminder rules, planner, iCalendar feed. The only crate that touches the database. |
| `crates/mcp` → `paperweight-mcp` | MCP server over stdio (built on [`rmcp`](https://crates.io/crates/rmcp)). Your AI client starts it. |
| `crates/app` → `paperweight` | Tauri 2 tray app with a Svelte UI. Owns the reminder scheduler and the calendar feed. |
| `packaging/mcpb` | Builds the `.mcpb` bundle for one-click install in Claude Desktop. |

The MCP server and the app work independently and share one SQLite database (WAL mode). When the assistant adds something, the open app shows it within about a second.

## Install

Download a release for your system:

- **macOS:** `Paperweight.dmg`
- **Windows:** the `.msi` or `-setup.exe` installer
- **Linux:** the `.deb`, `.rpm` or `.AppImage`

Every package installs both programs: `paperweight` (the app) and `paperweight-mcp` (the MCP server), side by side. Paperweight starts at login and lives in the tray. Closing the window hides it; use **Quit** in the tray menu to stop it.

Then connect your assistant (below).

## Connect your AI assistant

The easiest way: open Paperweight, go to **Settings → Connect your AI assistant**. It finds the installed `paperweight-mcp`, shows the exact config to copy, and can add itself to Claude Desktop for you.

### Claude Desktop

**Option 1, MCP Bundle:** double-click `paperweight-<version>-<platform>.mcpb` (or drag it onto Claude Desktop's Settings → Extensions).

**Option 2, in the app:** Settings → Connect your AI assistant → **Add to Claude Desktop**. It backs up `claude_desktop_config.json` first and changes only the `paperweight` entry. If the file is missing or isn't valid JSON, it changes nothing and shows these manual steps instead.

**Option 3, by hand:** Claude Desktop → Settings → Developer → Edit Config, then add:

```json
{
  "mcpServers": {
    "paperweight": {
      "command": "/Applications/Paperweight.app/Contents/MacOS/paperweight-mcp",
      "args": []
    }
  }
}
```

Use the path for your system:

| System | `command` |
|---|---|
| macOS | `/Applications/Paperweight.app/Contents/MacOS/paperweight-mcp` |
| Windows | the folder Paperweight was installed to, e.g. `C:\\Program Files\\Paperweight\\paperweight-mcp.exe` (MSI) or `%LOCALAPPDATA%\\Paperweight\\paperweight-mcp.exe` (setup.exe); backslashes doubled in JSON |
| Linux (.deb/.rpm) | `/usr/bin/paperweight-mcp` |

Not sure? Settings → Connect your AI assistant shows the exact path on this computer.

The config file is at `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows). Restart Claude Desktop afterwards.

### Cursor, LM Studio and other MCP clients

Any client that runs local stdio servers can use the same entry. Cursor (`~/.cursor/mcp.json`) and LM Studio (its `mcp.json`) both use the `mcpServers` format:

```json
{
  "mcpServers": {
    "paperweight": {
      "command": "/usr/bin/paperweight-mcp",
      "args": []
    }
  }
}
```

Clients that take a single command line can run `paperweight-mcp` directly. It speaks MCP on stdin and stdout and logs only to stderr.

### What the assistant can do

| Tool | Does |
|---|---|
| `add_item` | Track a deadline with notes, source, estimate, repeat rule, steps and the "what's needed" checklist, in one call |
| `update_item` | Change any field; add, tick or remove steps and checklist lines |
| `complete_item` | Mark done; repeating items get their next occurrence |
| `snooze_item` | Silence reminders until a time, without moving the deadline |
| `delete_item` | Move to the archive (restorable for 30 days) |
| `list_items` | Find items by status, kind, date range and text |
| `get_brief` | Overdue, due today, due in the next 7 days, plus the local date and time |
| `plan_week` | Suggest when to work on each item from your weekly routine; flags what won't fit |

There is also a resource, `paperweight://routine` (your weekly routine and settings), and a prompt, `weekly_review`.

Dates can be ISO 8601 (`2026-11-20`, `2026-11-20T17:00`, with or without an offset) or one of these phrases: `today`, `tonight`, `tomorrow`, a weekday name, `next <weekday>`, `in N days` or `in N weeks`. Each can take a time (`17:00`, `5pm`). Anything else is rejected with an error that states the current date and time, so the assistant converts the date itself instead of guessing.

## Using the app

- **Today** (default): overdue, today (with the 02:00 rollover), and the next seven days, plus suggested work sessions from your routine.
- **Quick add**: press `Ctrl+Shift+Space` (`⌘⇧Space` on macOS) from anywhere, or `n` / `Ctrl+K` in the window. Type `City hall form friday 17:00` and press Enter. It uses the same date parser as the MCP server.
- **Overview**: a period you choose with a slider, from one week to one year, shown two ways. **Stacks** groups deadlines into piles by day, week or month, with each pile's work against your free task time. **Near and far** runs time down the page, giving the next few days the most room. Search and the kind filters fade out everything that doesn't match.
- **Item detail**: edit everything, tick the checklist and steps, snooze, complete, delete.
- **Routine**: your weekly blocks; the ones marked "for tasks" feed `plan_week`.
- **Archive**: two tabs. **Done** lists completed items, newest first; open one or reopen it. **Deleted** restores deleted items for 30 days.
- **Settings**: quiet hours, reminder offsets, reminder sound on or off, day rollover, timezone, calendar feed and port, `.ics` export, language, start at login, quick-add shortcut, JSON export and import, and Connect your AI assistant.

### Calendar feed

While the app runs, subscribe to `http://127.0.0.1:47800/paperweight.ics` in your calendar app. It's read-only, reachable only from this computer, and has one event per open item with a stable UID, so edits update events instead of duplicating them. Settings → **Export .ics file** saves a one-time copy.

### Data

The database lives in your OS data directory:

- **Linux:** `~/.local/share/paperweight/paperweight.db`
- **macOS:** `~/Library/Application Support/app.Paperweight.Paperweight/paperweight.db`
- **Windows:** `%APPDATA%\Paperweight\Paperweight\data\paperweight.db`

Settings shows the exact path. Set `PAPERWEIGHT_DB=/path/to/file.db` to use another file; both programs honour it. A log is written next to the database as `paperweight.log`.

### Demo data

```sh
paperweight --seed-demo        # resets a separate demo database to a realistic week and opens it
paperweight-mcp --seed-demo    # same, for the MCP server
paperweight --demo             # use the demo database without resetting it
```

The demo database is `paperweight-demo.db` in the same folder (or `$PAPERWEIGHT_DB` if set). Your real data is never touched.

## Build from source

You need:

- Rust 1.94 (pinned in `rust-toolchain.toml`)
- Node.js 20.19+ or 22.12+
- On Linux, Tauri's system libraries:

```sh
sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libdbus-1-dev libssl-dev build-essential file
```

Build and check:

```sh
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

cd crates/app
npm ci
npm run check          # svelte-check, zero warnings
```

Run:

```sh
# MCP server against a scratch database
PAPERWEIGHT_DB=/tmp/pw.db cargo run -p paperweight-mcp

# The app, with hot reload of the UI
cd crates/app && npx tauri dev
# The app with the UI embedded, as it ships
cd crates/app && npx tauri build --debug --no-bundle && ../../target/debug/paperweight --seed-demo
```

A plain `cargo build` of the app produces a development build that loads the UI from the Vite dev server (`npx tauri dev` starts it). Use `tauri build` for a binary with the UI embedded.

Inspect the MCP server:

```sh
cargo build -p paperweight-mcp
npx @modelcontextprotocol/inspector -e PAPERWEIGHT_DB=/tmp/pw.db ./target/debug/paperweight-mcp
```

## Packaging

```sh
# 1. Build paperweight-mcp in release mode and place it as Tauri's sidecar
./scripts/prepare-sidecar.sh

# 2. Installers for the current OS (.dmg / .msi + .exe / .deb + .rpm + .AppImage)
cd crates/app && npx tauri build --config tauri.bundle.conf.json

# 3. The MCP Bundle, validated with the official mcpb CLI
cd packaging/mcpb && npm ci && npm run build      # → dist-mcpb/paperweight-<version>-<os>-<arch>.mcpb
```

The `.mcpb` contains a native binary, so build one per platform. CI (`.github/workflows/ci.yml`) builds, lints, tests and packages on macOS, Windows and Linux, and uploads the installers and the `.mcpb` as artifacts.

## Project docs

- [`docs/DECISIONS.md`](docs/DECISIONS.md): where this deviates from the original brief, and the choices it left open.
- [`docs/VERIFICATION.md`](docs/VERIFICATION.md): what was checked by hand and what wasn't.
- [`docs/mcp-transcript.md`](docs/mcp-transcript.md): a real request and response for every MCP tool.
- [`screenshots/`](screenshots/): every main view, light and dark.

## License

MIT. See [LICENSE](LICENSE).
