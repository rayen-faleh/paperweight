//! Phase 1 gate: every rule in the "Data model" section.

mod common;

use chrono::{Duration, NaiveTime};
use common::*;
use paperweight_core::brief::brief;
use paperweight_core::dates::parse_due;
use paperweight_core::db;
use paperweight_core::recurrence::add_months;
use paperweight_core::*;

// ---------- storage, ids, migrations ----------

#[test]
fn ids_are_short_lowercase_and_repeatable() {
    let (_d, mut s) = store();
    let mut seen = std::collections::HashSet::new();
    for i in 0..200 {
        let item = s
            .add_item(
                NewItem::new(format!("thing {i}"), tue_afternoon(), false),
                tue_afternoon(),
            )
            .unwrap();
        assert_eq!(item.id.len(), 7, "{}", item.id);
        assert!(item.id.starts_with("pw-"));
        assert!(
            item.id[3..]
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        );
        assert!(
            !item.id[3..].contains(['0', '1', 'o', 'l', 'i']),
            "no look-alikes: {}",
            item.id
        );
        assert!(seen.insert(item.id.clone()), "duplicate id");
    }
    // Lookups are case-insensitive, so a model that capitalises still works.
    let any = seen.iter().next().unwrap();
    assert_eq!(&s.get_item(&any.to_uppercase()).unwrap().id, any);
}

#[test]
fn pragmas_and_migrations() {
    let (_d, s) = store();
    let c = s.conn();
    let mode: String = c
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    let fk: i64 = c
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 1);
    let bt: i64 = c
        .query_row("PRAGMA busy_timeout", [], |r| r.get(0))
        .unwrap();
    assert_eq!(bt, 5000);
    assert_eq!(db::user_version(c).unwrap(), db::latest_version());
}

#[test]
fn concurrent_first_open_runs_migrations_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("race.db");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let p = path.clone();
            let b = barrier.clone();
            std::thread::spawn(move || {
                b.wait();
                Store::open(&p).map(|s| s.routine().unwrap().len())
            })
        })
        .collect();
    for h in handles {
        // Every opener sees exactly one seeded routine (4 blocks × 7 days).
        assert_eq!(h.join().unwrap().unwrap(), 28);
    }
    let s = Store::open(&path).unwrap();
    assert_eq!(db::user_version(s.conn()).unwrap(), db::latest_version());
}

#[test]
fn timestamps_are_stored_as_utc_iso_strings() {
    let (_d, mut s) = store();
    let item = s
        .add_item(
            NewItem::new("x", jst(2026, 11, 20, 17, 0), false),
            tue_afternoon(),
        )
        .unwrap();
    let (due, created): (String, String) = s
        .conn()
        .query_row(
            "SELECT due_at, created_at FROM items WHERE id = ?1",
            [&item.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(due, "2026-11-20T08:00:00Z");
    assert_eq!(created, "2026-11-17T06:00:00Z");
}

#[test]
fn item_defaults_and_children() {
    let (_d, mut s) = store();
    let mut new = NewItem::new("Residence card", jst(2026, 11, 26, 2, 0), true);
    new.kind = Kind::Paperwork;
    new.steps = vec![
        NewStep {
            title: "Photo".into(),
            soft_due_at: None,
        },
        NewStep {
            title: "Form".into(),
            soft_due_at: None,
        },
    ];
    new.requirements = vec!["Passport".into(), "Card".into()];
    let item = s.add_item(new, tue_afternoon()).unwrap();
    assert_eq!(item.estimated_minutes, 30, "default estimate");
    assert_eq!(item.status, Status::Open);
    assert_eq!(item.kind, Kind::Paperwork);
    assert_eq!(
        item.steps.iter().map(|s| s.position).collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(item.requirements.len(), 2);

    let photo = item.steps[0].id;
    let passport = item.requirements[0].id;
    let patch = ItemPatch {
        set_steps_done: vec![(photo, true)],
        set_requirements_done: vec![(passport, true)],
        add_steps: vec![NewStep {
            title: "Book appointment".into(),
            soft_due_at: None,
        }],
        remove_requirements: vec![item.requirements[1].id],
        add_requirements: vec!["1 photo 4x3".into()],
        ..Default::default()
    };
    let item = s.update_item(&item.id, patch, tue_afternoon()).unwrap();
    assert!(item.steps[0].done);
    assert_eq!(item.steps.len(), 3);
    assert_eq!(item.steps[2].position, 2);
    assert!(item.requirements[0].done);
    assert_eq!(
        item.requirements
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Passport", "1 photo 4x3"]
    );

    // A step id from another item is rejected with a useful message.
    let err = s
        .update_item(
            &item.id,
            ItemPatch {
                set_steps_done: vec![(9999, true)],
                ..Default::default()
            },
            tue_afternoon(),
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("no step with id 9999"), "{err}");
    assert!(err.contains("Photo"), "lists real steps: {err}");
}

#[test]
fn not_found_message_tells_the_model_what_to_do() {
    let (_d, s) = store();
    let err = s.get_item("pw-xxxx").unwrap_err().to_string();
    assert_eq!(err, "No item with id pw-xxxx. Call list_items to find it.");
}

#[test]
fn routine_is_seeded_with_takayas_week() {
    let (_d, s) = store();
    let r = s.routine().unwrap();
    assert_eq!(r.len(), 28);
    for wd in 0..7u8 {
        let day: Vec<_> = r.iter().filter(|b| b.weekday == wd).collect();
        let spans: Vec<(&str, &str, bool)> = day
            .iter()
            .map(|b| (b.start.as_str(), b.end.as_str(), b.available_for_tasks))
            .collect();
        assert_eq!(
            spans,
            vec![
                ("12:00", "14:00", false),
                ("14:00", "18:00", true),
                ("18:00", "21:00", false),
                ("22:00", "02:00", true)
            ]
        );
    }
}

#[test]
fn settings_defaults() {
    let s = Settings::default();
    assert_eq!(s.timezone, "system");
    assert_eq!(s.day_rollover, "02:00");
    assert_eq!(
        (s.quiet_start.as_str(), s.quiet_end.as_str()),
        ("02:00", "12:00")
    );
    assert_eq!(s.reminder_offsets, vec!["7d", "2d", "day", "overdue"]);
    assert_eq!(s.feed_port, 47800);
    let (_d, mut st) = store();
    let mut bad = st.settings().unwrap();
    bad.timezone = "Mars/Olympus".into();
    assert!(st.save_settings(&bad).is_err());
}

// ---------- day rollover ----------

#[test]
fn one_am_wednesday_belongs_to_tuesday_night() {
    let c = clock();
    let due = jst(2026, 11, 18, 1, 0); // Wednesday 01:00
    assert_eq!(c.logical_day(due), date(2026, 11, 17));
    assert_eq!(c.deadline_day(due), date(2026, 11, 17));
    // Seen from Tuesday afternoon it is "tonight".
    assert_eq!(c.due_label(due, false, tue_afternoon()), "tonight 01:00");

    // In the brief it is due *today* (Tuesday), not tomorrow.
    let (_d, mut s) = store();
    let item = s
        .add_item(NewItem::new("Submit form", due, false), tue_afternoon())
        .unwrap();
    let b = brief(
        s.open_items().unwrap(),
        &s.clock().unwrap(),
        tue_afternoon(),
    );
    assert_eq!(b.today, date(2026, 11, 17));
    assert_eq!(b.due_today.len(), 1);
    assert_eq!(b.due_today[0].item.id, item.id);
    assert_eq!(b.due_today[0].due_day, date(2026, 11, 17));

    // At 00:30 Wednesday it is still Tuesday, and still "today".
    let late = jst(2026, 11, 18, 0, 30);
    assert_eq!(c.today(late), date(2026, 11, 17));
    let b = brief(s.open_items().unwrap(), &s.clock().unwrap(), late);
    assert_eq!(b.due_today.len(), 1);
    // At 02:00 it is Wednesday and the item is overdue.
    let b = brief(
        s.open_items().unwrap(),
        &s.clock().unwrap(),
        jst(2026, 11, 18, 2, 0),
    );
    assert_eq!(b.today, date(2026, 11, 18));
    assert_eq!(b.overdue.len(), 1);
}

#[test]
fn today_and_tonight_use_the_rollover() {
    let c = clock();
    let half_past_midnight_wed = jst(2026, 11, 18, 0, 30);
    // "today" at 00:30 Wednesday is still Tuesday.
    let d = parse_due("today", half_past_midnight_wed, &c).unwrap();
    assert!(d.all_day);
    assert_eq!(c.deadline_day(d.at), date(2026, 11, 17));
    // "tomorrow" is Wednesday.
    let d = parse_due("tomorrow", half_past_midnight_wed, &c).unwrap();
    assert_eq!(c.deadline_day(d.at), date(2026, 11, 18));
    // "tonight 1am" from Tuesday afternoon is Wednesday 01:00.
    let d = parse_due("tonight 1am", tue_afternoon(), &c).unwrap();
    assert_eq!(d.at, jst(2026, 11, 18, 1, 0));
    assert!(!d.all_day);
    // "today 23:30" stays on Tuesday.
    let d = parse_due("today 23:30", tue_afternoon(), &c).unwrap();
    assert_eq!(d.at, jst(2026, 11, 17, 23, 30));
}

#[test]
fn all_day_item_is_due_at_the_rollover_that_ends_its_day() {
    let c = clock();
    let d = parse_due("2026-11-20", tue_afternoon(), &c).unwrap();
    assert!(d.all_day);
    assert_eq!(d.at, jst(2026, 11, 21, 2, 0));
    assert_eq!(c.deadline_day(d.at), date(2026, 11, 20));
    // At 01:59 Saturday it is not yet overdue; at 02:00 it is.
    assert!(d.at > jst(2026, 11, 21, 1, 59));
    assert_eq!(c.due_label(d.at, true, tue_afternoon()), "Fri 20 Nov");
}

#[test]
fn custom_rollover_setting_is_respected() {
    let c = Clock::new(tokyo(), NaiveTime::from_hms_opt(4, 0, 0).unwrap());
    assert_eq!(c.logical_day(jst(2026, 11, 18, 3, 0)), date(2026, 11, 17));
    let midnight = Clock::new(tokyo(), NaiveTime::MIN);
    assert_eq!(
        midnight.logical_day(jst(2026, 11, 18, 1, 0)),
        date(2026, 11, 18)
    );
}

// ---------- date parser ----------

#[test]
fn parser_accepts_iso_8601() {
    let c = clock();
    let now = tue_afternoon();
    let cases = [
        ("2026-11-20T17:00", jst(2026, 11, 20, 17, 0)),
        ("2026-11-20 17:00", jst(2026, 11, 20, 17, 0)),
        ("2026-11-20T17:00:00", jst(2026, 11, 20, 17, 0)),
        ("2026-11-20T08:00:00Z", jst(2026, 11, 20, 17, 0)),
        ("2026-11-20T17:00:00+09:00", jst(2026, 11, 20, 17, 0)),
        ("2026-11-20T09:00+01:00", jst(2026, 11, 20, 17, 0)),
        ("2026-11-20T08:00Z", jst(2026, 11, 20, 17, 0)),
    ];
    for (input, want) in cases {
        let d = parse_due(input, now, &c).unwrap_or_else(|e| panic!("{input}: {e}"));
        assert_eq!(d.at, want, "{input}");
        assert!(!d.all_day, "{input}");
    }
}

#[test]
fn parser_accepts_the_fixed_phrases() {
    let c = clock();
    let now = tue_afternoon(); // Tuesday 17 Nov
    let day = |s: &str| {
        let d = parse_due(s, now, &c).unwrap_or_else(|e| panic!("{s}: {e}"));
        (c.deadline_day(d.at), d.all_day)
    };
    assert_eq!(day("today"), (date(2026, 11, 17), true));
    assert_eq!(day("Tomorrow"), (date(2026, 11, 18), true));
    assert_eq!(day("friday"), (date(2026, 11, 20), true));
    assert_eq!(
        day("tuesday"),
        (date(2026, 11, 17), true),
        "bare weekday includes today"
    );
    assert_eq!(
        day("next tuesday"),
        (date(2026, 11, 24), true),
        "next <weekday> is strictly after today"
    );
    assert_eq!(day("next friday"), (date(2026, 11, 20), true));
    assert_eq!(day("in 3 days"), (date(2026, 11, 20), true));
    assert_eq!(day("in 1 day"), (date(2026, 11, 18), true));
    assert_eq!(day("in 2 weeks"), (date(2026, 12, 1), true));
    // tonight without a time = by the end of tonight (the rollover), timed.
    let d = parse_due("tonight", now, &c).unwrap();
    assert_eq!((d.at, d.all_day), (jst(2026, 11, 18, 2, 0), false));

    let at = |s: &str| {
        parse_due(s, now, &c)
            .unwrap_or_else(|e| panic!("{s}: {e}"))
            .at
    };
    assert_eq!(at("friday 17:00"), jst(2026, 11, 20, 17, 0));
    assert_eq!(at("friday at 5pm"), jst(2026, 11, 20, 17, 0));
    assert_eq!(at("tomorrow 9:30am"), jst(2026, 11, 18, 9, 30));
    assert_eq!(at("tomorrow 12pm"), jst(2026, 11, 18, 12, 0));
    assert_eq!(at("in 2 days 5 pm"), jst(2026, 11, 19, 17, 0));
    assert_eq!(at("tonight 9:00"), jst(2026, 11, 17, 21, 0));
    assert_eq!(at("tonight 23:00"), jst(2026, 11, 17, 23, 0));
    assert_eq!(at("next monday 14:00"), jst(2026, 11, 23, 14, 0));
}

#[test]
fn parser_never_guesses() {
    let c = clock();
    for bad in [
        "",
        "soon",
        "end of month",
        "next week",
        "20/11/2026",
        "Nov 20",
        "friday 5",
        "in three days",
        "tomorrow 25:00",
        "2026-13-01",
        "this friday",
    ] {
        let err = parse_due(bad, tue_afternoon(), &c).unwrap_err().to_string();
        assert!(
            err.contains("Tuesday 2026-11-17 15:00"),
            "error states local now: {err}"
        );
        assert!(err.contains("ISO 8601"), "asks for ISO: {err}");
    }
}

#[test]
fn parser_handles_dst_gaps() {
    let ny: chrono_tz::Tz = "America/New_York".parse().unwrap();
    let c = Clock::new(ny, NaiveTime::from_hms_opt(2, 0, 0).unwrap());
    // 2026-03-08 02:30 does not exist in New York; it moves forward an hour.
    let d = parse_due("2026-03-08T02:30", tue_afternoon(), &c).unwrap();
    assert_eq!(d.at.to_rfc3339(), "2026-03-08T07:30:00+00:00");
}

// ---------- soft delete ----------

#[test]
fn delete_is_soft_hidden_restorable_and_purged_after_30_days() {
    let (_d, mut s) = store();
    let now = tue_afternoon();
    let item = s
        .add_item(
            NewItem::new("Old form", now + Duration::days(3), false),
            now,
        )
        .unwrap();
    let deleted = s.delete_item(&item.id, now).unwrap();
    assert_eq!(deleted.status, Status::Archived);
    assert!(deleted.archived_at.is_some());

    // Hidden everywhere.
    assert!(s.open_items().unwrap().is_empty());
    let all_visible = s
        .list_items(&ItemFilter {
            statuses: vec![Status::Open, Status::Done],
            ..Default::default()
        })
        .unwrap();
    assert!(all_visible.is_empty());
    assert!(
        brief(
            s.list_items(&ItemFilter::default()).unwrap(),
            &s.clock().unwrap(),
            now
        )
        .due_today
        .is_empty()
    );
    let err = s
        .update_item(&item.id, ItemPatch::default(), now)
        .unwrap_err()
        .to_string();
    assert!(err.contains("archive"), "{err}");
    assert!(s.complete_item(&item.id, now).is_err());

    // Restorable.
    let restored = s.restore_item(&item.id, now).unwrap();
    assert_eq!(restored.status, Status::Open);
    assert_eq!(s.open_items().unwrap().len(), 1);

    // Purged only after 30 days.
    s.delete_item(&item.id, now).unwrap();
    assert_eq!(s.purge_archived(now + Duration::days(29)).unwrap(), 0);
    assert!(s.get_item(&item.id).is_ok());
    assert_eq!(s.purge_archived(now + Duration::days(31)).unwrap(), 1);
    assert!(s.get_item(&item.id).is_err());
}

// ---------- recurrence ----------

#[test]
fn month_end_recurrence_clamps_and_returns_to_the_anchor() {
    assert_eq!(add_months(date(2027, 1, 31), 1, 31), date(2027, 2, 28));
    assert_eq!(add_months(date(2028, 1, 31), 1, 31), date(2028, 2, 29));
    assert_eq!(add_months(date(2027, 2, 28), 1, 31), date(2027, 3, 31));
    assert_eq!(add_months(date(2027, 3, 31), 1, 31), date(2027, 4, 30));
    assert_eq!(add_months(date(2027, 1, 30), 1, 30), date(2027, 2, 28));
    assert_eq!(add_months(date(2027, 1, 29), 1, 29), date(2027, 2, 28));
    assert_eq!(add_months(date(2028, 1, 29), 1, 29), date(2028, 2, 29));
    assert_eq!(add_months(date(2026, 12, 31), 2, 31), date(2027, 2, 28));
    // Yearly on 29 Feb.
    assert_eq!(add_months(date(2028, 2, 29), 12, 29), date(2029, 2, 28));
    assert_eq!(add_months(date(2029, 2, 28), 36, 29), date(2032, 2, 29));
}

#[test]
fn completing_a_monthly_item_on_the_31st_walks_through_short_months() {
    let (_d, mut s) = store();
    let c = s.clock().unwrap();
    let mut new = NewItem::new("Rent", c.day_end(date(2027, 1, 31)), true);
    new.kind = Kind::Payment;
    new.recurrence = Recurrence {
        freq: Freq::Monthly,
        interval: 1,
    };
    new.requirements = vec!["Bank app".into()];
    new.steps = vec![NewStep {
        title: "Transfer".into(),
        soft_due_at: None,
    }];
    let created = jst(2027, 1, 20, 12, 0);
    let mut item = s.add_item(new, created).unwrap();
    item = s
        .update_item(
            &item.id,
            ItemPatch {
                set_steps_done: vec![(item.steps[0].id, true)],
                ..Default::default()
            },
            created,
        )
        .unwrap();
    assert!(item.steps[0].done);

    let mut days = vec![];
    let mut now = created;
    for _ in 0..4 {
        let (done, next) = s.complete_item(&item.id, now).unwrap();
        assert_eq!(done.status, Status::Done);
        assert!(done.completed_at.is_some());
        let next = next.expect("monthly item recurs");
        assert_eq!(next.status, Status::Open);
        assert!(next.all_day);
        assert_eq!(next.recurrence, item.recurrence);
        assert!(!next.steps[0].done, "steps are reset");
        assert_eq!(next.requirements[0].text, "Bank app");
        assert_ne!(next.id, item.id);
        days.push(c.deadline_day(next.due_at));
        now = next.due_at - Duration::days(1);
        item = next;
    }
    assert_eq!(
        days,
        vec![
            date(2027, 2, 28),
            date(2027, 3, 31),
            date(2027, 4, 30),
            date(2027, 5, 31)
        ]
    );
}

#[test]
fn recurrence_kinds_and_intervals() {
    let (_d, mut s) = store();
    let base = jst(2026, 11, 17, 18, 0);
    let mk = |s: &mut Store, freq, interval| {
        let mut n = NewItem::new("r", base, false);
        n.recurrence = Recurrence { freq, interval };
        s.add_item(n, base - Duration::days(1)).unwrap()
    };
    let now = base - Duration::hours(1);
    let d = mk(&mut s, Freq::Daily, 1);
    assert_eq!(
        s.complete_item(&d.id, now).unwrap().1.unwrap().due_at,
        jst(2026, 11, 18, 18, 0)
    );
    let w = mk(&mut s, Freq::Weekly, 2);
    assert_eq!(
        s.complete_item(&w.id, now).unwrap().1.unwrap().due_at,
        jst(2026, 12, 1, 18, 0)
    );
    let y = mk(&mut s, Freq::Yearly, 1);
    assert_eq!(
        s.complete_item(&y.id, now).unwrap().1.unwrap().due_at,
        jst(2027, 11, 17, 18, 0)
    );
    let none = s.add_item(NewItem::new("once", base, false), now).unwrap();
    assert!(s.complete_item(&none.id, now).unwrap().1.is_none());
    // Completing a long-overdue daily item skips missed days instead of
    // creating something already overdue.
    let late = mk(&mut s, Freq::Daily, 1);
    let next = s
        .complete_item(&late.id, jst(2026, 11, 25, 12, 0))
        .unwrap()
        .1
        .unwrap();
    assert_eq!(next.due_at, jst(2026, 11, 25, 18, 0));
    // Completing twice is an error that says how to reopen.
    let err = s.complete_item(&none.id, now).unwrap_err().to_string();
    assert!(err.contains("already done"), "{err}");
}

#[test]
fn late_night_recurring_item_stays_late_night() {
    let (_d, mut s) = store();
    let mut n = NewItem::new("Weekly review", jst(2026, 11, 18, 1, 0), false); // Tue night
    n.recurrence = Recurrence {
        freq: Freq::Weekly,
        interval: 1,
    };
    let i = s.add_item(n, tue_afternoon()).unwrap();
    let next = s.complete_item(&i.id, tue_afternoon()).unwrap().1.unwrap();
    assert_eq!(next.due_at, jst(2026, 11, 25, 1, 0));
    assert_eq!(
        s.clock().unwrap().deadline_day(next.due_at),
        date(2026, 11, 24)
    );
}

// ---------- snooze ----------

#[test]
fn snooze_silences_without_moving_the_deadline() {
    let (_d, mut s) = store();
    let now = tue_afternoon();
    let due = jst(2026, 11, 19, 17, 0);
    let item = s.add_item(NewItem::new("Form", due, false), now).unwrap();
    let until = jst(2026, 11, 18, 12, 0);
    let snoozed = s.snooze_item(&item.id, until, now).unwrap();
    assert_eq!(snoozed.due_at, due);
    assert_eq!(snoozed.snoozed_until, Some(until));
    assert!(snoozed.is_snoozed(now));
    assert!(!snoozed.is_snoozed(until));
    assert!(
        s.snooze_item(&item.id, now - Duration::hours(1), now)
            .is_err()
    );
}

// ---------- updates and export ----------

#[test]
fn moving_a_deadline_resets_its_reminders() {
    let (_d, mut s) = store();
    let now = tue_afternoon();
    let item = s
        .add_item(NewItem::new("x", now + Duration::days(3), false), now)
        .unwrap();
    s.record_sent(&[(item.id.clone(), "2d".into())], now)
        .unwrap();
    assert_eq!(s.sent_reminders().unwrap().len(), 1);
    s.update_item(
        &item.id,
        ItemPatch {
            due: Some((now + Duration::days(10), false)),
            ..Default::default()
        },
        now,
    )
    .unwrap();
    assert!(s.sent_reminders().unwrap().is_empty());
}

#[test]
fn export_import_round_trip() {
    let (_d, mut s) = store();
    let now = tue_afternoon();
    let mut n = NewItem::new("Scholarship", now + Duration::days(2), false);
    n.requirements = vec!["Form B-2".into()];
    s.add_item(n, now).unwrap();
    let gone = s.add_item(NewItem::new("gone", now, false), now).unwrap();
    s.delete_item(&gone.id, now).unwrap();
    let export = s.export(now).unwrap();
    let json = serde_json::to_string(&export).unwrap();

    let (_d2, mut other) = store();
    let parsed: Export = serde_json::from_str(&json).unwrap();
    assert_eq!(other.import(&parsed).unwrap(), 2);
    assert_eq!(other.open_items().unwrap().len(), 1);
    assert_eq!(
        other.open_items().unwrap()[0].requirements[0].text,
        "Form B-2"
    );
    assert_eq!(other.get_item(&gone.id).unwrap().status, Status::Archived);
    assert_eq!(other.settings().unwrap().timezone, TZ);
}

#[test]
fn text_search_covers_children() {
    let (_d, mut s) = store();
    let now = tue_afternoon();
    let mut n = NewItem::new("Residence card", now, false);
    n.requirements = vec!["Passport photo 4x3".into()];
    s.add_item(n, now).unwrap();
    s.add_item(NewItem::new("Rent", now, false), now).unwrap();
    let hits = s
        .list_items(&ItemFilter {
            text: Some("PASSPORT".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "Residence card");
    // LIKE wildcards in the query are literal.
    assert!(
        s.list_items(&ItemFilter {
            text: Some("%".into()),
            ..Default::default()
        })
        .unwrap()
        .is_empty()
    );
}

#[test]
fn demo_seed_has_the_promised_week() {
    let dir = tempfile::tempdir().unwrap();
    let store = demo::reset_demo(&dir.path().join("demo.db"), tue_afternoon()).unwrap();
    let items = store.open_items().unwrap();
    let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
    for want in [
        "Renew residence card",
        "Scholarship continuation form",
        "Rent",
        "Dinner with Kenta and Mio",
    ] {
        assert!(titles.contains(&want), "{titles:?}");
    }
    assert_eq!(
        items.iter().filter(|i| i.kind == Kind::Assignment).count(),
        2
    );
    assert_eq!(
        items.iter().filter(|i| i.due_at <= tue_afternoon()).count(),
        1,
        "one overdue"
    );
}

#[test]
fn quick_add_splits_title_and_date_with_the_same_parser() {
    use paperweight_core::dates::parse_quick_add;
    let c = clock();
    let now = tue_afternoon();
    let (t, d) = parse_quick_add("City hall form by friday 17:00", now, &c).unwrap();
    assert_eq!(
        (t.as_str(), d.at, d.all_day),
        ("City hall form", jst(2026, 11, 20, 17, 0), false)
    );
    let (t, d) = parse_quick_add("Rent 2026-11-30", now, &c).unwrap();
    assert_eq!(t, "Rent");
    assert!(d.all_day);
    let (t, d) =
        parse_quick_add("Email the professor about next week, tonight 1am", now, &c).unwrap();
    assert_eq!(t, "Email the professor about next week");
    assert_eq!(d.at, jst(2026, 11, 18, 1, 0));
    let (t, _) = parse_quick_add("Read chapter 4 in 3 days", now, &c).unwrap();
    assert_eq!(t, "Read chapter 4");
    let (t, d) = parse_quick_add("Dinner next friday at 7:30pm", now, &c).unwrap();
    assert_eq!((t.as_str(), d.at), ("Dinner", jst(2026, 11, 20, 19, 30)));
    assert!(
        parse_quick_add("Something sometime", now, &c)
            .unwrap_err()
            .to_string()
            .contains("Add a date")
    );
    assert!(
        parse_quick_add("tomorrow", now, &c).is_err(),
        "needs a title"
    );
}

#[test]
fn finished_items_are_not_labelled_overdue() {
    let (_d, mut s) = store();
    let now = tue_afternoon();
    let item = s
        .add_item(NewItem::new("x", now - Duration::hours(2), false), now)
        .unwrap();
    let c = s.clock().unwrap();
    assert!(
        ItemView::new(item.clone(), &c, now)
            .due_label
            .starts_with("overdue")
    );
    let (done, _) = s.complete_item(&item.id, now).unwrap();
    let v = ItemView::new(done, &c, now);
    assert_eq!(v.due_label, "today 13:00");
    assert!(!v.overdue);
}
