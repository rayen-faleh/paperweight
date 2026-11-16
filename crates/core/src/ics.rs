//! iCalendar (RFC 5545) generation for the feed and the one-time export.

use chrono::{DateTime, Duration, Utc};

use crate::model::{Item, Status};
use crate::time::Clock;

const PRODID: &str = "-//Paperweight//Paperweight 0.1//EN";

/// Escape a TEXT value (RFC 5545 §3.3.11).
pub fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

/// Fold a content line at 75 octets without splitting a UTF-8 character
/// (RFC 5545 §3.1), and terminate it with CRLF.
pub fn fold_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len() + 8);
    let mut count = 0;
    for c in line.chars() {
        let len = c.len_utf8();
        if count + len > 75 {
            out.push_str("\r\n ");
            count = 1;
        }
        out.push(c);
        count += len;
    }
    out.push_str("\r\n");
    out
}

fn utc_stamp(t: DateTime<Utc>) -> String {
    t.format("%Y%m%dT%H%M%SZ").to_string()
}

/// The stable UID for an item, so calendar apps update rather than duplicate.
pub fn uid(item: &Item) -> String {
    format!("{}@paperweight.local", item.id)
}

fn description(item: &Item) -> String {
    let mut parts = vec![];
    if !item.notes.trim().is_empty() {
        parts.push(item.notes.trim().to_string());
    }
    if !item.requirements.is_empty() {
        let mut s = String::from("What's needed:");
        for r in &item.requirements {
            s.push_str(&format!(
                "\n[{}] {}",
                if r.done { "x" } else { " " },
                r.text
            ));
        }
        parts.push(s);
    }
    if !item.steps.is_empty() {
        let mut s = String::from("Steps:");
        for st in &item.steps {
            s.push_str(&format!(
                "\n[{}] {}",
                if st.done { "x" } else { " " },
                st.title
            ));
        }
        parts.push(s);
    }
    if let Some(src) = &item.source {
        parts.push(format!("Source: {src}"));
    }
    parts.push(format!("Paperweight id: {}", item.id));
    parts.join("\n\n")
}

/// Build a VCALENDAR with one VEVENT per open item. All-day items are
/// all-day events on their logical day; timed items are 15-minute events
/// that start at the deadline.
pub fn calendar(items: &[Item], clock: &Clock, now: DateTime<Utc>) -> String {
    let mut lines: Vec<String> = vec![
        "BEGIN:VCALENDAR".into(),
        "VERSION:2.0".into(),
        format!("PRODID:{PRODID}"),
        "CALSCALE:GREGORIAN".into(),
        "METHOD:PUBLISH".into(),
        "X-WR-CALNAME:Paperweight".into(),
        format!("X-WR-TIMEZONE:{}", clock.tz.name()),
        "REFRESH-INTERVAL;VALUE=DURATION:PT15M".into(),
        "X-PUBLISHED-TTL:PT15M".into(),
    ];
    for item in items.iter().filter(|i| i.status == Status::Open) {
        lines.push("BEGIN:VEVENT".into());
        lines.push(format!("UID:{}", uid(item)));
        lines.push(format!("DTSTAMP:{}", utc_stamp(now)));
        lines.push(format!("CREATED:{}", utc_stamp(item.created_at)));
        lines.push(format!("LAST-MODIFIED:{}", utc_stamp(item.updated_at)));
        // SEQUENCE grows with each edit so clients accept the update.
        lines.push(format!(
            "SEQUENCE:{}",
            (item.updated_at - item.created_at).num_seconds().max(0) / 60
        ));
        if item.all_day {
            let day = clock.deadline_day(item.due_at);
            lines.push(format!("DTSTART;VALUE=DATE:{}", day.format("%Y%m%d")));
            lines.push(format!(
                "DTEND;VALUE=DATE:{}",
                (day + Duration::days(1)).format("%Y%m%d")
            ));
            lines.push("TRANSP:TRANSPARENT".into());
        } else {
            lines.push(format!("DTSTART:{}", utc_stamp(item.due_at)));
            lines.push(format!(
                "DTEND:{}",
                utc_stamp(item.due_at + Duration::minutes(15))
            ));
            lines.push("TRANSP:TRANSPARENT".into());
        }
        lines.push(format!("SUMMARY:{}", escape_text(&item.title)));
        lines.push(format!("DESCRIPTION:{}", escape_text(&description(item))));
        lines.push(format!("CATEGORIES:{}", escape_text(item.kind.as_str())));
        if let Some(src) = item
            .source
            .as_deref()
            .filter(|s| s.starts_with("http://") || s.starts_with("https://"))
        {
            lines.push(format!("URL:{}", src.replace(['\r', '\n'], "")));
        }
        lines.push("STATUS:CONFIRMED".into());
        lines.push("END:VEVENT".into());
    }
    lines.push("END:VCALENDAR".into());
    lines.iter().map(|l| fold_line(l)).collect()
}
