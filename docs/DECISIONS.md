# Decisions

Every place this build deviates from the original brief, and every choice the brief left open. Grouped by area; the most consequential first in each.

## Toolchain and libraries

- **Versions checked on 16 Nov 2026, not remembered.** rmcp 3.10.0, Tauri 2.12.6 (tauri-build 2.7.5), tauri-plugin-notification 2.8.1, -autostart 2.7.0, -single-instance 2.5.5, -global-shortcut 2.4.1, -dialog 2.8.4, rusqlite 0.43 (bundled), Svelte 5.57, Vite 8.3, @tauri-apps/api 2.12.5, @anthropic-ai/mcpb 2.1.2. rmcp is at 3.x, far past the 0.x APIs older examples use. The server follows the 3.10 source: `ServerConfig`, `ReadResourceResponse`, and MRTR-aware client calls.
- **Rust 1.94.0 is pinned** in `rust-toolchain.toml` because it was the toolchain installed here. The workspace declares `rust-version = 1.90`, the highest MSRV among its dependencies (Tauri and the notification plugin).
- **TypeScript 6.0.3, not 7.0.** svelte-check 4.7.8 supports TypeScript `^5 || ^6` only.
- **`serde_json` uses `preserve_order`,** so the Claude Desktop config helper keeps the user's key order.

## Architecture

- **`crates/app` is both the Rust crate and the frontend root** (`ui/`, `package.json`, `tauri.conf.json`), with no `src-tauri/` folder. This keeps the workspace at the three crates the brief names.
- **The sidecar is declared in a separate config,** `crates/app/tauri.bundle.conf.json`, and merged only when bundling (`tauri build --config tauri.bundle.conf.json`). `tauri-build` copies `externalBin` at compile time and fails if the file is missing. With the sidecar in the main config, a plain `cargo build --workspace` would fail on a fresh clone. `scripts/prepare-sidecar.sh` builds and places the binary.
- **A plain `cargo build` of the app is a Tauri dev build** that loads the UI from the Vite dev server (`devUrl`). This is standard Tauri behaviour without the `custom-protocol` feature. `tauri build` (with `--debug` or not) embeds the UI.
- **The calendar feed's HTTP server lives in `core`.** It is about 100 lines of std-only code, so core has no HTTP dependency and the feed is testable without Tauri. The app only starts and stops it. It opens the database fresh for each request.
- **The Claude Desktop config logic also lives in `core`** (`connect.rs`). It touches no database; it lives there so it can be tested without building Tauri.
- **No plugin APIs are exposed to the web view.** The capability grants only `core:default`. Notifications, dialogs, autostart and shortcuts are called from Rust commands, which keeps the frontend down to one runtime dependency (`@tauri-apps/api`).
- **Write transactions use `BEGIN IMMEDIATE`.** A deferred transaction that reads and then writes cannot wait for the write lock: if another connection committed in between, SQLite returns "database is locked" at once and ignores `busy_timeout`. That happened in real use when an assistant sent two `add_item` calls in parallel. Taking the lock up front makes concurrent writers queue behind the 5-second busy timeout instead.
- **Logs.** The MCP server logs only to stderr, and `#![deny(clippy::print_stdout)]` guards against stray prints. The app logs to stderr and to `paperweight.log` next to the database.

## Data model and dates

- **Rollover boundary.** A deadline exactly on a rollover instant belongs to the day that rollover ends. An all-day item for Wednesday is stored as due at 02:00 Thursday and shows as Wednesday, which is how "due at the rollover that ends its day" is implemented.
- **"tonight" without a time** means "by the end of tonight": a timed deadline at the rollover, not all-day. `tonight 9:00` means 21:00. In phrases, a time earlier than the rollover belongs to the night of the given day (`friday 1am` is early Saturday).
- **ISO 8601 without an offset is taken literally** in the user's timezone, with no rollover shift. Only phrases apply the logical day. A bare date (`2026-11-20`) is all-day.
- **Weekday rules.** `friday` is the first Friday on or after today. `next friday` is the first Friday strictly after today, so "next tuesday" said on Monday means tomorrow. The rule is in the tool description, every write echoes the resolved date, and the instructions tell the model to confirm the date back.
- **Small, documented extras in the phrase grammar:** `at` before a time, `5 pm` with a space, `12am`/`12pm`, `in 1 day`, and three-letter weekday names. Everything else is still rejected; nothing is guessed.
- **Snooze with a date and no time** means the start of that day.
- **Recurrence keeps an anchor day** (`recur_anchor_day`), so a monthly series on the 31st goes 31 Jan → 28 Feb → 31 Mar instead of drifting to the 28th. Completing a recurring item skips occurrences already in the past, so completing a daily item a week late gives tomorrow's, not seven overdue copies. The next occurrence copies steps and requirements, unticked, and shifts step soft-due dates by the same interval.
- **Moving a deadline clears that item's reminder log,** so the moved deadline gets a fresh set of reminders.
- **`update_item` can reopen** a done item (`status: "open"`) but cannot archive; deleting goes through `delete_item`.
- **`estimated_minutes: 0` is allowed** and means no preparation is needed (a dinner). The planner skips such items.
- **Ids** are `pw-` plus four characters from an alphabet without look-alikes (no 0/o/1/l/i). Lookups are case-insensitive.
- **Settings** are stored one key per row as JSON; missing keys fall back to defaults. One setting was added: `quick_add_shortcut` (default `CommandOrControl+Shift+Space`).
- **JSON import** replaces items with the same id and adds the rest. It replaces the routine and settings.
- **The archive is purged by the app's scheduler** once a day. The MCP server never deletes data.
- **The seeded routine** is the same every day of the week: 12–14 wake up (not for tasks), 14–18 assignments (for tasks), 18–21 chores (not for tasks), 22–02 free time and admin (for tasks). 21–22 is left free.

## Reminders

- **Trigger keys.** `Nd`/`Nh`/`Nm` fire that long before the deadline. `day` fires at the start of the due day, i.e. its rollover; at 02:00 that falls in quiet hours, so it arrives in the noon digest. `overdue` fires at the deadline itself, then once a day at the start of each later day (`overdue:YYYY-MM-DD`). Minute offsets (`15m`) are an extra.
- **Wording.** The overdue reminder that fires at the deadline reads "Due now — …". Later ones read "Overdue — was due …".
- **One line per item.** When several triggers for one item are pending at once (after sleep, after a snooze), only the most recent is shown and all of them are marked sent.
- **No reminders for the past.** Triggers that fall before the item was created are marked sent silently. Adding something due Friday on Tuesday does not ping the 7-day reminder.
- **When a digest is used:** more than one reminder at once, any reminder held back by quiet hours, or a catch-up. A single fresh reminder is a single notification with Done and Snooze buttons.
- **Catch-up.** The first check after the app starts, or a check more than 3 minutes after the previous one (sleep or shutdown), sends one "While you were away" digest.
- **Quiet hours with the same start and end** mean no quiet hours.
- **Done and Snooze buttons are on all three desktops.** tauri-plugin-notification 2.8.1 documents desktop support for action buttons and click (`tap`) reporting on Linux, Windows and macOS when an `on_action` handler is registered. **Snooze till tomorrow** snoozes until the end of tomorrow's quiet hours (12:00 by default). After Done or Snooze the app removes the notification (the plugin supports this on Linux; elsewhere it is a no-op).
- **Sound.** Reminders play the system's own sound: "Reminder" on Windows, "Glass" on macOS, and the freedesktop theme sound `message-new-instant` on Linux (sent as the `sound-name` hint, which the desktop's notification server may or may not play). A **Play a sound** setting, on by default, turns this off. It is needed because without a sound name, Windows toasts are generated as `<audio silent="true"/>`.
- **Tray count.** The icon is drawn at runtime with a numeric badge, because Windows trays cannot show text next to an icon. On macOS and Linux the count is also set as the tray title. The tooltip is set too, but libayatana-appindicator does not show tooltips.

## Planner

- **"Ends before the deadline" is applied literally:** a work session only goes in a block whose *end* is at or before the deadline. Items are not split across blocks.
- **The horizon** is the 7 logical days starting today. A block already under way starts now, rounded up to five minutes. Items due after the horizon that don't fit go in a separate `later` list rather than `at_risk`.
- **Brief priority order:** earliest deadline first; ties go payment, paperwork, assignment, appointment, social, other, then title.

## Calendar feed

- **Event shape.** Timed items are 15-minute events starting at the deadline, in UTC. All-day items are `VALUE=DATE` events on their logical day. All events are `TRANSPARENT` so they don't block free/busy.
- **Contents.** Only open items appear. Requirements and steps are listed in `DESCRIPTION`, and an http(s) `source` becomes `URL`.
- **Updates.** `UID` is `<id>@paperweight.local`, and `SEQUENCE` grows with `updated_at` so clients accept edits.

## GUI

- **"All items" became "Overview".** A flat list was hard to take in, so the tab now shows a chosen period (1 week to 1 year, on a logarithmic slider) in one of two designs the user switches between: **Stacks** (piles of slips per day, week or month, slip height by estimated effort, a bar of work against free task time from the routine) and **Near and far** (time runs down the page on a log scale, nearer items larger and darker). Three designs were prototyped first; the third (a horizontal timeline with labelled landmarks) was not chosen. Search and kind filters dim non-matching items instead of removing them, so nothing jumps around while filtering.
- **Stacks picks its grouping by width as well as period.** Days only while each day column stays about 78 px wide; otherwise weeks. When weeks or months don't fit in one row, they wrap onto balanced shelves (for example two rows of months for a year) instead of shrinking until titles disappear.
- **Near and far compresses empty time.** A gap between consecutive rows keeps its first 44 px and a fifth of the rest, so long quiet stretches don't leave blank space; the time guides go through the same mapping so they stay between the right rows.
- **Animation.** Items are keyed and morphed with FLIP: moves glide from where they were (also when a slip moves to another column, or when switching designs, where every item travels from its place in one design to its place in the other), new items fade up, and items leaving the period fade out where they were. While dragging the slider moves are short and unstaggered so items follow the hand; on release, a click on a stop, or a design switch, they settle with a short stagger and an ease with about 2% overshoot. Releasing the slider near a named stop (2 weeks, 1 month…) snaps to it. Reduced-motion settings turn all of this off.
- **The chosen design and period are remembered per device** (the web view's local storage), not in the database: they're view preferences, not data.
- **No separate "new item" form.** Items are created by the assistant or by Quick add; everything else is edited in the item detail panel.
- **Today also shows plan_week's suggestions** ("When to work on it" and "Won't fit before the deadline"). This was not in the brief, but it serves someone who finds multitasking hard.
- **Japanese table.** `ja` has every key, with empty strings that fall back to English: ready to fill in, as asked.
- **Start at login** is turned on automatically on first run (once, recorded in the database) and can be turned off in Settings. The autostart entry passes `--hidden`.
- **Extra keyboard shortcuts:** `n` and `Ctrl/⌘+K` open Quick add inside the window; the global shortcut works anywhere.

## Connect helper and packaging

- **Claude Desktop on Linux** has no official build. The helper uses `$XDG_CONFIG_HOME/Claude/claude_desktop_config.json`, where community builds look.
- **The helper also refuses** when the JSON root isn't an object or `mcpServers` isn't an object. It writes through a temporary file and a rename, so a crash can't leave half a config. Backups are named `claude_desktop_config.json.paperweight-backup-<UTC timestamp>`.
- **Finding the binary.** The helper looks for `paperweight-mcp` next to the running executable (where Tauri installs sidecars), then on `PATH`.
- **MCP Bundle.** `manifest_version` is `0.3`, which the spec calls current (0.4 only adds the `uv` server type). The bundle holds a native binary, so one is built per platform with `compatibility.platforms` set to match. Tools are listed; `prompts_generated: true` because the server serves `weekly_review` itself. Bundles are unsigned. The bundled server uses the same default database as the app, so the two share data.
- **`--seed-demo` and `--demo`** exist on both binaries. The demo database is `paperweight-demo.db` in the data directory, or `$PAPERWEIGHT_DB` if set.
- **CI** runs fmt, build, clippy, tests and svelte-check, builds the Tauri bundles and the `.mcpb`, and uploads them as artifacts. It does not sign or notarize.
