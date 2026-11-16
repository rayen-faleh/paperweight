# Verification

What was checked, how, and what was not. Run on 16 November 2026 on **Linux only**: Ubuntu 24.04 under WSL2, Rust 1.94.0, Node 24.21. The GUI ran under Xvfb (`:99`) with its own D-Bus session, the **dunst** notification daemon, and a small StatusNotifierWatcher written for the test (Xvfb has no tray). `XDG_CONFIG_HOME` and `PAPERWEIGHT_DB` pointed at a scratch directory, so the real database was never used.

Status key: ✅ verified (seen working) · ⚠️ partly verified · ❌ not verified (reason given).

## Build, lint, tests

| Check | Status | Evidence |
|---|---|---|
| `cargo build --workspace` | ✅ | 0 warnings, 0 errors |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ | 0 warnings, 0 errors |
| `cargo test --workspace` | ✅ | 57 tests pass: 29 data model, 1 concurrency, 1 env override, 8 feed and connect, 17 reminders and planner, 1 MCP stdio integration |
| `cargo fmt --all --check` | ✅ | clean |
| `npm run check` (svelte-check, `--fail-on-warnings`) | ✅ | 0 errors, 0 warnings |
| Cargo.lock committed | ✅ | in git (along with both `package-lock.json` files) |

## Phase gates

| Gate | Status | Evidence |
|---|---|---|
| 1. Unit tests for every data model rule, incl. the 01:00 case and month-end recurrence | ✅ | `crates/core/tests/data_model.rs`, for example `one_am_wednesday_belongs_to_tuesday_night` and `completing_a_monthly_item_on_the_31st_walks_through_short_months` (31 Jan → 28 Feb → 31 Mar → 30 Apr → 31 May) |
| 2. Integration test starts the real binary, connects with rmcp's client over stdio, calls every tool and the error paths, and asserts stdout held only protocol messages | ✅ | `crates/mcp/tests/stdio.rs` routes the child's stdio through a recorder: every stdout line parses as JSON-RPC 2.0, logs reach stderr, and the server survives bad types, an unknown tool, bad ids, bad dates and bad step ids |
| 3. Tests for quiet hours, digest, catch-up, snooze, no duplicate sends, and plan_week with an item that does not fit | ✅ | `crates/core/tests/reminders_planner.rs`, 17 tests, all driven by an explicit `now` (minute-by-minute simulated runs) |
| 4. With the app open, an item added through MCP appears in Today within 2 s | ✅ | `add_item` via `paperweight-mcp` returned at 14:07:27.410Z; the app's watcher logged the change at 27.434Z (24 ms later); a screenshot taken 1.59 s after the call shows the item under "Tomorrow" |
| 5. Feed parses as valid iCalendar in a test; helper tested against a missing file, an invalid file, and a file with other servers | ✅ | `crates/core/tests/feed_connect.rs`. Also parsed the live feed with an independent library (Python `icalendar` 7.3.0): 8 events, 8 unique UIDs, no errors |
| 6. The `.mcpb` passes the CLI's validation | ✅ | `mcpb validate` (@anthropic-ai/mcpb 2.1.2): "Manifest schema validation passes!". Packed to `dist-mcpb/paperweight-0.1.0-linux-x64.mcpb`, unpacked again: the binary keeps its executable bit and serves all 8 tools |
| 7. Use it | ✅ (Linux) | See below |

## "Verification" section of the brief

| Check | Status | Notes |
|---|---|---|
| `--seed-demo` fills a separate demo database with Takaya's week | ✅ | Both `paperweight --seed-demo` and `paperweight-mcp --seed-demo`. It contains a residence card renewal, scholarship form, rent (monthly), two assignments, dinner with friends, and one overdue payment. Tested with `PAPERWEIGHT_DB` pointing at scratch files |
| Run the app and use it like he would | ✅ | Seeded demo; added items through MCP and Quick add; ticked, snoozed, completed and deleted items; restored from the archive; edited settings |
| Connect with the MCP Inspector | ⚠️ | Used Inspector 2.15.0 in **CLI mode** (`--cli`). The web UI needs a browser, which this environment doesn't have. The CLI connected (protocol 2025-11-25), listed the tools, read `paperweight://routine` and got `weekly_review` |
| Add, update, plan, snooze, complete, delete through MCP | ✅ | Through the Inspector CLI, the raw stdio harness and the integration test. The bad-date path returns an error stating the local date and time |
| Watch the GUI update live | ✅ | Items added by the Inspector and by `paperweight-mcp` appeared in Today without a restart (see gate 4) |
| A reminder fires for an item due in a few minutes | ✅ | Due 23:38; the scheduler delivered "Due now — today 23:38" at 23:38:00.9 and dunst displayed it (`screenshots/notification-due-now.png`) |
| Notification Done / Snooze / click | ✅ (Linux) | Pressed through dunst's action menu. **Done** marked the item done and closed the notification. **Snooze till tomorrow** set `snoozed_until` to 12:00 the next day and left the deadline unchanged. A **click** opened the item in the app |
| Reminder sound | ⚠️ | On Linux, the `Notify` call carried `sound-name: message-new-instant` (seen with `dbus-monitor`). No audio device here, so nothing was heard. The Windows sound ("Reminder") and macOS sound ("Glass") were checked only by reading the notification libraries' source, not by running on those systems |
| Parallel writes (assistant sending several tool calls at once) | ✅ | Seen failing in real use on Windows ("database is locked"). Reproduced by `crates/core/tests/concurrency.rs` (8 connections × 15 add/update/log writes), which failed before the fix and passes after; the MCP stdio test now sends 6 tool calls at once |
| Catch-up digest instead of a flood | ✅ | On launch with 6 pending reminders, one "While you were away: 6 reminders" notification |
| Tray shows the count and a menu with the next five items, Open, Quick add, Quit | ✅ | Read over D-Bus from the app's StatusNotifierItem: label `1`, menu lists 5 items plus Open Paperweight, Quick add…, Quit Paperweight. Clicking a menu entry opened that item. The badge icon is in `screenshots/tray-icon-badge.png`. **Not** seen in a real desktop panel |
| Closing the window hides to tray; single instance; global quick-add shortcut | ✅ | Sent `WM_DELETE_WINDOW`: the window hid and the process kept running. `Ctrl+Shift+Space` while hidden opened Quick add; typing "Return library books thursday 18:00" and Enter created it, due Thu 19 Nov 18:00. A second launch exited and re-showed the first instance (1 process) |
| Starts at login | ⚠️ | On first run the app wrote an XDG autostart entry with `--hidden`. A login was not performed, so starting at login itself was not seen. Note: the autostart plugin writes to the real `~/.config/autostart` even when `XDG_CONFIG_HOME` points elsewhere (seen with the AppImage and with a debug build). Test runs therefore created real entries; they were removed after each test |
| Subscribe a calendar app to the feed | ❌ | No calendar application is available in this environment. The served feed was validated with two parsers instead and confirmed to listen on `127.0.0.1:47800` only (`ss -ltnp`) |
| Connect helper writes Claude Desktop's config | ✅ (sandboxed) | Clicked **Add to Claude Desktop** in the GUI against a scratch `claude_desktop_config.json` that already had a `filesystem` server. It made a backup, added `paperweight`, and kept `filesystem` and `globalShortcut` and their order (`screenshots/*-9-connect-result.png`) |
| Claude Desktop actually using the server or the `.mcpb` | ❌ | Claude Desktop is not available on Linux, and none was installed here |
| Screenshots of every main view, light and dark | ✅ | `screenshots/`: Today, All items, Item detail, Quick add (plus the saved result), Routine, Archive, Settings, Connect, Connect result, in `light-*` and `dark-*`. Dark comes from the OS theme (GTK dark variant), not a toggle in the app |
| MCP transcript | ✅ | `docs/mcp-transcript.md`: 42 real request/response pairs captured from the pipes of the integration test, covering every tool, the resource, the prompt and the error paths |
| Release build runs | ✅ | The release AppImage started under Xvfb, rendered the embedded UI, served the feed and sent the catch-up digest |

## Packaging and platforms

| Check | Status | Notes |
|---|---|---|
| Tauri bundles with `paperweight-mcp` as a sidecar | ✅ (Linux) | Built `.deb`, `.rpm` and `.AppImage`. The `.deb` installs `/usr/bin/paperweight` and `/usr/bin/paperweight-mcp`. Not installed with `dpkg` |
| macOS and Windows builds, bundles and runtime | ⚠️ | Built, tested and packaged by CI on both. Runtime: the Windows installer was installed and run by the user, and its MCP server answered from WSL (`get_brief`, `add_item`). The Windows tray, notifications, sound and start at login were not observed by me. macOS runtime is untested |
| GitHub Actions workflow | ✅ | Run [43890776114](https://github.com/rayen-faleh/paperweight/actions/runs/43890776114) on `ccd78cd` succeeded on `ubuntu-24.04`, `macos-latest` and `windows-latest`: fmt, build, clippy, tests, svelte-check, Tauri bundles and the `.mcpb`. Each job uploaded its installers as an artifact |
| Japanese UI | ⚠️ | All keys exist in the `ja` table with empty values and fall back to English, as requested. No translation was done or checked |

## Bugs found and fixed while using it

- Parallel tool calls failed with "database is locked". Write transactions now start with `BEGIN IMMEDIATE`.
- Reminders were silent on Windows. They now play the system reminder sound, with a setting to turn it off.
- Settings failed to load ("The object can not be cloned"): `structuredClone` can't copy a Svelte 5 state proxy. Fixed with `$state.snapshot`.
- Completed items were labelled "overdue (…)". Labels now only say overdue for open items.
- The reminder at the exact deadline said "Overdue — was due …". It now says "Due now — …".
- After Done or Snooze, the notification stayed on screen. It is now removed.
- Snooze confirmations showed UTC timestamps. They now use local phrases ("tomorrow 12:00").
- The detail header repeated the date; US-style dates appeared in the archive. Both fixed.
