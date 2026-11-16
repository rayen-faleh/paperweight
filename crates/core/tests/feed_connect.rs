//! Phase 5 gate: the .ics feed is valid iCalendar (and is served on
//! loopback), and the Claude Desktop helper handles a missing file, an
//! invalid file, and a file that already has other servers.

mod common;

use std::io::{Read, Write};

use chrono::Duration;
use common::*;
use paperweight_core::connect::{ConnectOutcome, write_claude_desktop};
use paperweight_core::feed::FeedServer;
use paperweight_core::ics;
use paperweight_core::*;
use serde_json::{Value, json};

fn sample_store() -> (tempfile::TempDir, Store) {
    let (d, mut s) = store();
    let now = tue_afternoon();
    let c = s.clock().unwrap();
    let mut a = NewItem::new(
        "Renew residence card; bring passport, photo",
        c.day_end(date(2026, 11, 26)),
        true,
    );
    a.kind = Kind::Paperwork;
    a.notes = "Immigration office\nGo early, the queue is long. 日本語もOK — long line ".repeat(3);
    a.requirements = vec!["Passport".into(), "Photo 4×3 cm".into()];
    a.steps = vec![NewStep {
        title: "Book slot".into(),
        soft_due_at: None,
    }];
    a.source = Some("https://example.org/form.pdf".into());
    s.add_item(a, now).unwrap();
    let b = s
        .add_item(
            NewItem::new("Submit essay", jst(2026, 11, 18, 1, 0), false),
            now,
        )
        .unwrap();
    let gone = s
        .add_item(
            NewItem::new("Deleted thing", now + Duration::days(1), false),
            now,
        )
        .unwrap();
    s.delete_item(&gone.id, now).unwrap();
    let done = s
        .add_item(
            NewItem::new("Done thing", now + Duration::days(1), false),
            now,
        )
        .unwrap();
    s.complete_item(&done.id, now).unwrap();
    let _ = b;
    (d, s)
}

fn props<'a>(
    c: &'a icalendar::parser::Component<'a>,
    name: &str,
) -> Vec<&'a icalendar::parser::Property<'a>> {
    c.properties
        .iter()
        .filter(|p| p.name.as_str() == name)
        .collect()
}

/// Structural checks from RFC 5545 that a lenient parser does not enforce.
fn assert_rfc5545(body: &str) {
    assert!(body.ends_with("\r\n"));
    for line in body.split("\r\n").filter(|l| !l.is_empty()) {
        assert!(line.len() <= 75, "line longer than 75 octets: {line:?}");
        assert!(
            !line.contains('\n') && !line.contains('\r'),
            "bare newline in {line:?}"
        );
    }
    assert!(
        !body.replace("\r\n", "").contains('\n'),
        "only CRLF line endings"
    );
    let unfolded = icalendar::parser::unfold(body);
    let cal = icalendar::parser::read_calendar_simple(&unfolded).expect("parses as iCalendar");
    assert_eq!(cal.len(), 1, "one VCALENDAR");
    let root = &cal[0];
    assert_eq!(root.name.as_str(), "VCALENDAR");
    assert_eq!(props(root, "VERSION")[0].val.as_str(), "2.0");
    assert_eq!(props(root, "PRODID").len(), 1);
    for ev in &root.components {
        assert_eq!(ev.name.as_str(), "VEVENT");
        for required in ["UID", "DTSTAMP", "DTSTART", "SUMMARY"] {
            assert_eq!(
                props(ev, required).len(),
                1,
                "VEVENT needs exactly one {required}"
            );
        }
    }
}

#[test]
fn feed_is_valid_icalendar_with_stable_uids() {
    let (_d, s) = sample_store();
    let clock = s.clock().unwrap();
    let items = s.open_items().unwrap();
    let body = ics::calendar(&items, &clock, tue_afternoon());
    assert_rfc5545(&body);

    let unfolded = icalendar::parser::unfold(&body);
    let cal = icalendar::parser::read_calendar_simple(&unfolded).unwrap();
    let events = &cal[0].components;
    assert_eq!(
        events.len(),
        2,
        "only open items; archived and done are left out"
    );

    let card = events
        .iter()
        .find(|e| props(e, "SUMMARY")[0].val.as_str().starts_with("Renew"))
        .unwrap();
    // Escaped text round-trips.
    assert_eq!(
        props(card, "SUMMARY")[0].val.as_str(),
        "Renew residence card; bring passport, photo"
    );
    assert!(body.contains("SUMMARY:Renew residence card\\; bring passport\\, photo"));
    // All-day: a DATE on its logical day, ending the next day.
    let start = props(card, "DTSTART")[0];
    assert!(
        start.params.iter().any(
            |p| p.key.as_str() == "VALUE" && p.val.as_ref().map(|v| v.as_str()) == Some("DATE")
        )
    );
    assert_eq!(start.val.as_str(), "20261126");
    assert_eq!(props(card, "DTEND")[0].val.as_str(), "20261127");
    assert_eq!(
        props(card, "URL")[0].val.as_str(),
        "https://example.org/form.pdf"
    );
    assert!(
        props(card, "DESCRIPTION")[0]
            .val
            .as_str()
            .contains("[ ] Passport")
    );

    // Timed: UTC instant of the deadline (01:00 JST = 16:00Z the day before).
    let essay = events
        .iter()
        .find(|e| props(e, "SUMMARY")[0].val.as_str() == "Submit essay")
        .unwrap();
    assert_eq!(props(essay, "DTSTART")[0].val.as_str(), "20261117T160000Z");

    // Stable UIDs: regenerating later gives the same UIDs, so calendar apps
    // update events instead of duplicating them.
    let uids = |b: &str| {
        let u = icalendar::parser::unfold(b);
        let c = icalendar::parser::read_calendar_simple(&u).unwrap();
        let mut v: Vec<String> = c[0]
            .components
            .iter()
            .map(|e| props(e, "UID")[0].val.as_str().to_string())
            .collect();
        v.sort();
        v
    };
    let later = ics::calendar(
        &s.open_items().unwrap(),
        &clock,
        tue_afternoon() + Duration::hours(5),
    );
    assert_eq!(uids(&body), uids(&later));
    for (u, item) in uids(&body).iter().zip({
        let mut ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
        ids.sort();
        ids
    }) {
        assert_eq!(u, &format!("{item}@paperweight.local"));
    }
}

#[test]
fn folding_never_splits_a_character() {
    let line = format!("SUMMARY:{}", "日本語の書類".repeat(20));
    let folded = ics::fold_line(&line);
    for l in folded.split("\r\n").filter(|l| !l.is_empty()) {
        assert!(l.len() <= 75);
    }
    assert_eq!(folded.replace("\r\n ", "").trim_end(), line);
}

fn http_get(addr: std::net::SocketAddr, path: &str) -> (String, String) {
    let mut s = std::net::TcpStream::connect(addr).unwrap();
    write!(s, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
    let mut resp = String::new();
    s.read_to_string(&mut resp).unwrap();
    let (head, body) = resp.split_once("\r\n\r\n").unwrap();
    (head.to_string(), body.to_string())
}

#[test]
fn feed_is_served_on_loopback_only() {
    let (_d, s) = sample_store();
    let server = FeedServer::start(0, s.path().to_path_buf()).unwrap();
    assert!(server.addr().ip().is_loopback());
    assert!(server.url().starts_with("http://127.0.0.1:"));
    let (head, body) = http_get(server.addr(), "/paperweight.ics");
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert!(head.contains("Content-Type: text/calendar"));
    assert_rfc5545(&body);
    let (head, _) = http_get(server.addr(), "/other");
    assert!(head.starts_with("HTTP/1.1 404"));
    // Changes show up on the next request.
    let mut s = s;
    s.add_item(
        NewItem::new("Fresh", tue_afternoon() + Duration::days(2), false),
        tue_afternoon(),
    )
    .unwrap();
    let (_, body) = http_get(server.addr(), "/paperweight.ics");
    assert!(body.contains("SUMMARY:Fresh"));
    server.stop();
}

// ---------- Connect helper ----------

fn bin() -> std::path::PathBuf {
    std::path::PathBuf::from("/opt/Paperweight/paperweight-mcp")
}

fn backups(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().contains("paperweight-backup"))
        .collect()
}

#[test]
fn connect_missing_file_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("claude_desktop_config.json");
    match write_claude_desktop(&path, &bin(), tue_afternoon()) {
        ConnectOutcome::Manual { reason, steps } => {
            assert!(reason.contains("not found"), "{reason}");
            assert!(
                steps.iter().any(|s| s.contains("\"paperweight\"")
                    && s.contains("/opt/Paperweight/paperweight-mcp"))
            );
        }
        other => panic!("expected manual steps, got {other:?}"),
    }
    assert!(!path.exists(), "never creates the file");
    assert!(backups(dir.path()).is_empty());
}

#[test]
fn connect_invalid_json_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("claude_desktop_config.json");
    let broken = "{ \"mcpServers\": { \"other\": { \"command\": \"x\" }, }";
    std::fs::write(&path, broken).unwrap();
    match write_claude_desktop(&path, &bin(), tue_afternoon()) {
        ConnectOutcome::Manual { reason, .. } => {
            assert!(reason.contains("not valid JSON"), "{reason}")
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
    assert!(backups(dir.path()).is_empty());

    // Valid JSON of the wrong shape is also left alone.
    std::fs::write(&path, "[1, 2]").unwrap();
    assert!(matches!(
        write_claude_desktop(&path, &bin(), tue_afternoon()),
        ConnectOutcome::Manual { .. }
    ));
    std::fs::write(&path, "{\"mcpServers\": 3}").unwrap();
    assert!(matches!(
        write_claude_desktop(&path, &bin(), tue_afternoon()),
        ConnectOutcome::Manual { .. }
    ));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "{\"mcpServers\": 3}"
    );
}

#[test]
fn connect_merges_without_touching_other_servers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("claude_desktop_config.json");
    let original = r#"{
  "globalShortcut": "Ctrl+Space",
  "mcpServers": {
    "filesystem": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "/home/t"] },
    "zeta": { "command": "/usr/bin/zeta", "env": { "TOKEN": "abc" } }
  },
  "theme": "dark"
}"#;
    std::fs::write(&path, original).unwrap();
    let out = write_claude_desktop(&path, &bin(), tue_afternoon());
    let ConnectOutcome::Written {
        backup,
        result,
        replaced,
        ..
    } = out
    else {
        panic!("{out:?}")
    };
    assert!(!replaced);
    // Backed up first, byte for byte.
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);
    // The file on disk is what was shown.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), result);

    let before: Value = serde_json::from_str(original).unwrap();
    let after: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(after["globalShortcut"], before["globalShortcut"]);
    assert_eq!(after["theme"], before["theme"]);
    assert_eq!(
        after["mcpServers"]["filesystem"],
        before["mcpServers"]["filesystem"]
    );
    assert_eq!(after["mcpServers"]["zeta"], before["mcpServers"]["zeta"]);
    assert_eq!(
        after["mcpServers"]["paperweight"],
        json!({"command": "/opt/Paperweight/paperweight-mcp", "args": []})
    );
    // Key order is kept.
    let keys: Vec<&String> = after.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["globalShortcut", "mcpServers", "theme"]);
    let servers: Vec<&String> = after["mcpServers"].as_object().unwrap().keys().collect();
    assert_eq!(servers, ["filesystem", "zeta", "paperweight"]);

    // Running it again replaces only our entry.
    let again = write_claude_desktop(
        &path,
        std::path::Path::new("/new/paperweight-mcp"),
        tue_afternoon() + Duration::seconds(1),
    );
    let ConnectOutcome::Written {
        replaced, result, ..
    } = again
    else {
        panic!()
    };
    assert!(replaced);
    let v: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(
        v["mcpServers"]["paperweight"]["command"],
        "/new/paperweight-mcp"
    );
    assert_eq!(v["mcpServers"]["zeta"], before["mcpServers"]["zeta"]);
    assert_eq!(backups(dir.path()).len(), 2);
}

#[test]
fn connect_adds_mcp_servers_when_absent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("claude_desktop_config.json");
    std::fs::write(&path, "{}").unwrap();
    let ConnectOutcome::Written { result, .. } =
        write_claude_desktop(&path, &bin(), tue_afternoon())
    else {
        panic!()
    };
    let v: Value = serde_json::from_str(&result).unwrap();
    assert_eq!(
        v["mcpServers"]["paperweight"]["command"],
        "/opt/Paperweight/paperweight-mcp"
    );
}

#[test]
fn config_paths_per_os() {
    use paperweight_core::connect::claude_desktop_config_path_for as p;
    assert!(
        p("macos")
            .unwrap()
            .ends_with("Library/Application Support/Claude/claude_desktop_config.json")
    );
    assert!(
        p("windows")
            .unwrap()
            .ends_with("Claude/claude_desktop_config.json")
    );
    assert!(
        p("linux")
            .unwrap()
            .ends_with("Claude/claude_desktop_config.json")
    );
}
