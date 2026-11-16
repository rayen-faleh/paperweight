//! Phase 3 gate: reminders (quiet hours, digest, catch-up, snooze, no
//! duplicates) and plan_week, all driven by an explicit "now".

mod common;

use chrono::{DateTime, Duration, Utc};
use common::*;
use paperweight_core::planner::plan_week;
use paperweight_core::reminders::{self, Delivery, DeliveryKind, Input, Offset, in_quiet_hours};
use paperweight_core::*;

/// One scheduler tick, exactly as the tray app runs it: evaluate, then record.
fn tick(s: &mut Store, now: DateTime<Utc>, catch_up: bool) -> Option<Delivery> {
    let items = s.open_items().unwrap();
    let settings = s.settings().unwrap();
    let sent = s.sent_reminders().unwrap();
    let out = reminders::evaluate(&Input {
        now,
        items: &items,
        settings: &settings,
        sent: &sent,
        catch_up,
    });
    s.record_sent(&out.mark_sent, now).unwrap();
    out.delivery
}

/// Run ticks every minute over a span and collect deliveries.
fn run(s: &mut Store, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<(DateTime<Utc>, Delivery)> {
    let mut out = vec![];
    let mut t = from;
    while t <= to {
        if let Some(d) = tick(s, t, false) {
            out.push((t, d));
        }
        t += Duration::minutes(1);
    }
    out
}

fn add(s: &mut Store, title: &str, due: DateTime<Utc>, created: DateTime<Utc>) -> Item {
    s.add_item(NewItem::new(title, due, false), created)
        .unwrap()
}

#[test]
fn offsets_parse_and_reject() {
    assert_eq!(
        Offset::parse("7d").unwrap(),
        Offset::Before(Duration::days(7))
    );
    assert_eq!(
        Offset::parse("3h").unwrap(),
        Offset::Before(Duration::hours(3))
    );
    assert_eq!(
        Offset::parse("15m").unwrap(),
        Offset::Before(Duration::minutes(15))
    );
    assert_eq!(Offset::parse("day").unwrap(), Offset::DayStart);
    assert_eq!(Offset::parse("overdue").unwrap(), Offset::Overdue);
    for bad in ["", "7", "d", "0d", "7w", "-1d"] {
        assert!(Offset::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn quiet_hours_window() {
    let t = |h, m| chrono::NaiveTime::from_hms_opt(h, m, 0).unwrap();
    // Default 02:00–12:00.
    assert!(in_quiet_hours(t(2, 0), t(2, 0), t(12, 0)));
    assert!(in_quiet_hours(t(11, 59), t(2, 0), t(12, 0)));
    assert!(!in_quiet_hours(t(12, 0), t(2, 0), t(12, 0)));
    assert!(!in_quiet_hours(t(1, 59), t(2, 0), t(12, 0)));
    // Across midnight.
    assert!(in_quiet_hours(t(23, 0), t(22, 0), t(7, 0)));
    assert!(in_quiet_hours(t(6, 59), t(22, 0), t(7, 0)));
    assert!(!in_quiet_hours(t(7, 0), t(22, 0), t(7, 0)));
    // Equal start and end means no quiet hours.
    assert!(!in_quiet_hours(t(3, 0), t(3, 0), t(3, 0)));
}

#[test]
fn a_reminder_fires_on_time_once() {
    let (_d, mut s) = store();
    let created = jst(2026, 11, 10, 15, 0);
    let due = jst(2026, 11, 20, 17, 0); // Friday 17:00
    let item = add(&mut s, "Scholarship form", due, created);
    // The 7-day reminder is at Friday 13 Nov 17:00, outside quiet hours.
    let at = jst(2026, 11, 13, 17, 0);
    assert!(tick(&mut s, at - Duration::minutes(1), false).is_none());
    let d = tick(&mut s, at, false).expect("7-day reminder");
    assert_eq!(d.kind, DeliveryKind::Single);
    assert_eq!(d.item_id.as_deref(), Some(item.id.as_str()));
    assert_eq!(d.title, "Scholarship form");
    assert!(d.body.starts_with("Due "), "{}", d.body);
    // Never twice.
    for m in 1..120 {
        assert!(tick(&mut s, at + Duration::minutes(m), false).is_none());
    }
    let log = s.reminder_log().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].key, "7d");
}

#[test]
fn no_duplicate_sends_even_if_recorded_twice() {
    let (_d, mut s) = store();
    let item = add(
        &mut s,
        "x",
        jst(2026, 11, 20, 17, 0),
        jst(2026, 11, 1, 12, 0),
    );
    let entry = vec![(item.id.clone(), "7d".to_string())];
    assert_eq!(s.record_sent(&entry, tue_afternoon()).unwrap(), 1);
    assert_eq!(s.record_sent(&entry, tue_afternoon()).unwrap(), 0);
    assert_eq!(s.reminder_log().unwrap().len(), 1);
    // Two schedulers racing on the same database: only one log row per trigger.
    let path = s.path().to_path_buf();
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let p = path.clone();
            let e = vec![(item.id.clone(), "2d".to_string())];
            std::thread::spawn(move || {
                Store::open(&p)
                    .unwrap()
                    .record_sent(&e, tue_afternoon())
                    .unwrap()
            })
        })
        .collect();
    let inserted: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    assert_eq!(inserted, 1);
}

#[test]
fn quiet_hours_hold_reminders_until_a_digest_at_noon() {
    let (_d, mut s) = store();
    let created = jst(2026, 11, 1, 15, 0);
    // Two deadlines whose 2-day reminders fall at 03:00 and 09:00.
    let a = add(&mut s, "Pay phone bill", jst(2026, 11, 20, 3, 0), created);
    let b = add(&mut s, "Library books", jst(2026, 11, 20, 9, 0), created);
    let night = jst(2026, 11, 18, 2, 0);
    let noon = jst(2026, 11, 18, 12, 0);
    // Nothing during quiet hours, and nothing is marked as sent.
    let quiet = run(&mut s, night, noon - Duration::minutes(1));
    assert!(quiet.is_empty(), "{quiet:?}");
    assert!(s.reminder_log().unwrap().is_empty());
    // One digest when quiet hours end.
    let d = tick(&mut s, noon, false).expect("digest at noon");
    assert_eq!(d.kind, DeliveryKind::Digest);
    let ids: Vec<&str> = d.lines.iter().map(|l| l.item_id.as_str()).collect();
    assert_eq!(ids, vec![a.id.as_str(), b.id.as_str()]);
    assert!(d.body.contains("Pay phone bill") && d.body.contains("Library books"));
    assert!(tick(&mut s, noon + Duration::minutes(1), false).is_none());
}

#[test]
fn a_single_held_reminder_is_still_a_digest() {
    let (_d, mut s) = store();
    // All-day item for Friday: "start of due day" is Friday 02:00, inside quiet hours.
    let c = s.clock().unwrap();
    let mut n = NewItem::new("Rent", c.day_end(date(2026, 11, 20)), true);
    n.kind = Kind::Payment;
    s.add_item(n, jst(2026, 11, 19, 15, 0)).unwrap();
    assert!(tick(&mut s, jst(2026, 11, 20, 2, 0), false).is_none());
    let d = tick(&mut s, jst(2026, 11, 20, 12, 0), false).expect("noon");
    assert_eq!(d.kind, DeliveryKind::Digest);
    assert_eq!(d.lines.len(), 1);
    assert_eq!(d.lines[0].due_label, "today");
}

#[test]
fn simultaneous_reminders_become_one_digest() {
    let (_d, mut s) = store();
    let created = jst(2026, 11, 1, 15, 0);
    let due = jst(2026, 11, 20, 18, 0);
    for t in ["One", "Two", "Three"] {
        add(&mut s, t, due, created);
    }
    let d = tick(&mut s, jst(2026, 11, 18, 18, 0), false).expect("2-day reminders");
    assert_eq!(d.kind, DeliveryKind::Digest);
    assert_eq!(d.lines.len(), 3);
    assert!(d.title.contains('3'));
}

#[test]
fn catch_up_after_sleep_is_one_digest_not_a_flood() {
    let (_d, mut s) = store();
    let created = jst(2026, 11, 1, 15, 0);
    let a = add(&mut s, "Residence card", jst(2026, 11, 20, 12, 0), created);
    let b = add(&mut s, "Scholarship", jst(2026, 11, 17, 17, 0), created);
    let c = add(&mut s, "Old bill", jst(2026, 11, 15, 17, 0), created);
    // The computer was off from 1 Nov to Wednesday 18 Nov 15:00. Item a has
    // pending 7d and 2d, b has 7d, 2d, day and overdue, c has all of them.
    let now = jst(2026, 11, 18, 15, 0);
    assert!(reminders::is_catch_up(Some(created), now, false));
    let d = tick(&mut s, now, true).expect("catch-up");
    assert_eq!(d.kind, DeliveryKind::CatchUp);
    assert!(d.title.starts_with("While you were away"), "{}", d.title);
    // One line per item: the most recent trigger, not every missed one.
    let ids: Vec<&str> = d.lines.iter().map(|l| l.item_id.as_str()).collect();
    assert_eq!(ids, vec![c.id.as_str(), b.id.as_str(), a.id.as_str()]);
    assert_eq!(d.lines[0].offset, "overdue");
    assert_eq!(d.lines[2].offset, "2d");
    // Every missed trigger is marked, so none of them fires later.
    let keys: Vec<String> = s
        .reminder_log()
        .unwrap()
        .into_iter()
        .filter(|e| e.item_id == a.id)
        .map(|e| e.key)
        .collect();
    assert_eq!(keys.len(), 2, "{keys:?}");
    assert!(run(&mut s, now + Duration::minutes(1), now + Duration::hours(3)).is_empty());
}

#[test]
fn catch_up_during_quiet_hours_waits_for_noon() {
    let (_d, mut s) = store();
    add(
        &mut s,
        "x",
        jst(2026, 11, 17, 17, 0),
        jst(2026, 11, 1, 15, 0),
    );
    let out = {
        let items = s.open_items().unwrap();
        let settings = s.settings().unwrap();
        let sent = s.sent_reminders().unwrap();
        reminders::evaluate(&Input {
            now: jst(2026, 11, 18, 4, 0),
            items: &items,
            settings: &settings,
            sent: &sent,
            catch_up: true,
        })
    };
    assert!(out.quiet && out.delivery.is_none() && out.mark_sent.is_empty());
}

#[test]
fn snoozed_items_send_nothing_until_the_snooze_ends() {
    let (_d, mut s) = store();
    let created = jst(2026, 11, 1, 15, 0);
    let item = add(&mut s, "Form", jst(2026, 11, 20, 17, 0), created);
    // Snoozed past both the 7d (13 Nov 17:00) and 2d (18 Nov 17:00) reminders.
    s.snooze_item(&item.id, jst(2026, 11, 19, 13, 0), created)
        .unwrap();
    let fired = run(&mut s, jst(2026, 11, 13, 16, 0), jst(2026, 11, 19, 1, 59));
    assert!(fired.is_empty(), "{fired:?}");
    assert!(s.reminder_log().unwrap().is_empty());
    // After the snooze: one reminder (the latest missed one), not two.
    let d = tick(&mut s, jst(2026, 11, 19, 13, 0), false).expect("after snooze");
    assert_eq!(d.lines.len(), 1);
    assert_eq!(d.lines[0].offset, "2d");
    assert!(tick(&mut s, jst(2026, 11, 19, 13, 1), false).is_none());
    // Snooze did not move the deadline.
    assert_eq!(
        s.get_item(&item.id).unwrap().due_at,
        jst(2026, 11, 20, 17, 0)
    );
}

#[test]
fn overdue_reminds_once_a_day() {
    let (_d, mut s) = store();
    let created = jst(2026, 11, 16, 13, 0);
    let item = add(&mut s, "Insurance slip", jst(2026, 11, 17, 18, 0), created);
    // Tuesday 18:00: day reminder already went at noon digest; the overdue one at 18:00.
    let tue = run(&mut s, jst(2026, 11, 17, 12, 0), jst(2026, 11, 17, 23, 59));
    let offsets: Vec<String> = tue.iter().map(|(_, d)| d.lines[0].offset.clone()).collect();
    assert_eq!(offsets, vec!["day", "now"]);
    assert!(
        tue[1].1.body.starts_with("Due now — today 18:00"),
        "{}",
        tue[1].1.body
    );
    assert_eq!(tue[1].0, jst(2026, 11, 17, 18, 0));
    // Wednesday: once, at noon (02:00 is inside quiet hours).
    let wed = run(&mut s, jst(2026, 11, 18, 0, 0), jst(2026, 11, 18, 23, 59));
    assert_eq!(wed.len(), 1);
    assert_eq!(wed[0].0, jst(2026, 11, 18, 12, 0));
    assert!(wed[0].1.lines[0].due_label.starts_with("overdue"));
    // Thursday: once again.
    assert_eq!(
        run(&mut s, jst(2026, 11, 19, 0, 0), jst(2026, 11, 19, 23, 59)).len(),
        1
    );
    // Completed items stop reminding.
    s.complete_item(&item.id, jst(2026, 11, 19, 23, 59))
        .unwrap();
    assert!(run(&mut s, jst(2026, 11, 20, 0, 0), jst(2026, 11, 20, 23, 59)).is_empty());
}

#[test]
fn triggers_before_an_item_existed_are_silent() {
    let (_d, mut s) = store();
    // Added on Tuesday for Friday: the 7-day reminder is already in the past.
    let now = tue_afternoon();
    add(&mut s, "City hall form", jst(2026, 11, 20, 17, 0), now);
    assert!(tick(&mut s, now + Duration::minutes(1), false).is_none());
    assert_eq!(s.reminder_log().unwrap().len(), 1, "7d marked silently");
    // The 2-day reminder still comes.
    assert!(tick(&mut s, jst(2026, 11, 18, 17, 0), false).is_some());
}

#[test]
fn custom_offsets_and_day_rollover_in_reminders() {
    let (_d, mut s) = store();
    let mut settings = s.settings().unwrap();
    settings.reminder_offsets = vec!["15m".into()];
    s.save_settings(&settings).unwrap();
    let created = jst(2026, 11, 17, 13, 0);
    // Due 01:00 Wednesday = Tuesday night; 15m before is 00:45, not quiet.
    add(&mut s, "Submit essay", jst(2026, 11, 18, 1, 0), created);
    let fired = run(&mut s, jst(2026, 11, 17, 23, 0), jst(2026, 11, 18, 1, 30));
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].0, jst(2026, 11, 18, 0, 45));
    assert_eq!(fired[0].1.body, "Due tonight 01:00");
}

// ---------- plan_week ----------

#[test]
fn plan_week_places_items_in_deadline_order_and_flags_what_does_not_fit() {
    let (_d, mut s) = store();
    let now = jst(2026, 11, 17, 13, 0); // Tuesday 13:00
    let mk = |s: &mut Store, title: &str, due, minutes| {
        let mut n = NewItem::new(title, due, false);
        n.estimated_minutes = Some(minutes);
        s.add_item(n, now - Duration::days(1)).unwrap()
    };
    let stats = mk(&mut s, "Stats problem set", jst(2026, 11, 19, 13, 0), 180);
    let form = mk(&mut s, "Scholarship form", jst(2026, 11, 18, 17, 0), 60);
    let essay = mk(&mut s, "Essay draft", jst(2026, 11, 23, 17, 0), 300);
    let rent = mk(&mut s, "Rent", jst(2026, 11, 22, 23, 0), 15);
    let tonight = mk(&mut s, "Email professor", jst(2026, 11, 17, 21, 0), 30);
    let dinner = mk(&mut s, "Dinner", jst(2026, 11, 20, 19, 30), 0);
    let later = mk(&mut s, "Visa photo", jst(2026, 12, 20, 17, 0), 240);
    let overdue = mk(&mut s, "Insurance", jst(2026, 11, 16, 17, 0), 20);

    let clock = s.clock().unwrap();
    let plan = plan_week(now, &s.open_items().unwrap(), &s.routine().unwrap(), &clock);

    let session = |id: &str| plan.sessions.iter().find(|x| x.item_id == id).cloned();
    // Earliest deadline first: "Email professor" (due 21:00) takes Tuesday 14:00
    // because the 14–18 block ends before 21:00.
    let e = session(&tonight.id).unwrap();
    assert_eq!(
        (e.start, e.end),
        (jst(2026, 11, 17, 14, 0), jst(2026, 11, 17, 14, 30))
    );
    assert_eq!(e.block_label, "Assignments and research");
    // The form (due Wed 17:00) continues in the same block.
    let f = session(&form.id).unwrap();
    assert_eq!(f.start, jst(2026, 11, 17, 14, 30));
    // Stats (180 min, due Thu 13:00): Tuesday 14–18 has 150 min left, so it
    // goes to the 22:00–02:00 block that night (ends Wed 02:00, before Thu 13:00).
    let st = session(&stats.id).unwrap();
    assert_eq!(st.start, jst(2026, 11, 17, 22, 0));
    assert_eq!(st.end, jst(2026, 11, 18, 1, 0));
    assert_eq!(
        st.day,
        date(2026, 11, 17),
        "a late-night block belongs to its logical day"
    );
    // Rent fits in Tuesday's afternoon block remainder.
    assert_eq!(session(&rent.id).unwrap().start, jst(2026, 11, 17, 15, 30));
    // 0-minute items need no time.
    assert!(session(&dinner.id).is_none());
    assert!(!plan.at_risk.iter().any(|r| r.item_id == dinner.id));
    // The 300-minute essay fits in no 240-minute block: at risk, with a reason.
    let risk = plan
        .at_risk
        .iter()
        .find(|r| r.item_id == essay.id)
        .expect("essay at risk");
    assert!(risk.reason.contains("300"), "{}", risk.reason);
    // Overdue items are at risk.
    assert!(
        plan.at_risk
            .iter()
            .any(|r| r.item_id == overdue.id && r.reason.contains("overdue"))
    );
    // A 240-minute item due next month fits in this week; not at risk either way.
    assert!(session(&later.id).is_some() || plan.later.iter().any(|r| r.item_id == later.id));
    assert!(!plan.at_risk.iter().any(|r| r.item_id == later.id));

    // Deterministic.
    let again = plan_week(now, &s.open_items().unwrap(), &s.routine().unwrap(), &clock);
    assert_eq!(plan, again);
}

#[test]
fn plan_week_requires_the_block_to_end_before_the_deadline() {
    let (_d, mut s) = store();
    let now = jst(2026, 11, 17, 13, 0);
    let mut n = NewItem::new("Due mid-block", jst(2026, 11, 17, 17, 0), false);
    n.estimated_minutes = Some(30);
    let item = s.add_item(n, now).unwrap();
    let plan = plan_week(
        now,
        &s.open_items().unwrap(),
        &s.routine().unwrap(),
        &s.clock().unwrap(),
    );
    // Tuesday's 14–18 block ends after 17:00 and no earlier block is free.
    let r = plan
        .at_risk
        .iter()
        .find(|r| r.item_id == item.id)
        .expect("at risk");
    assert!(
        r.reason.contains("No task block ends before the deadline"),
        "{}",
        r.reason
    );
}

#[test]
fn plan_week_starts_from_now_inside_a_running_block() {
    let (_d, mut s) = store();
    let now = jst(2026, 11, 17, 16, 2); // inside 14–18
    let mut n = NewItem::new("Quick task", jst(2026, 11, 19, 17, 0), false);
    n.estimated_minutes = Some(60);
    let item = s.add_item(n, now).unwrap();
    let plan = plan_week(
        now,
        &s.open_items().unwrap(),
        &s.routine().unwrap(),
        &s.clock().unwrap(),
    );
    let sess = plan.sessions.iter().find(|x| x.item_id == item.id).unwrap();
    // Rounded up to 16:05; 115 minutes remain, so it fits today.
    assert_eq!(sess.start, jst(2026, 11, 17, 16, 5));
}

#[test]
fn plan_week_with_an_empty_routine_puts_everything_at_risk() {
    let (_d, mut s) = store();
    s.set_routine(&[]).unwrap();
    let now = tue_afternoon();
    let item = add(&mut s, "x", now + Duration::days(2), now);
    let plan = plan_week(
        now,
        &s.open_items().unwrap(),
        &s.routine().unwrap(),
        &s.clock().unwrap(),
    );
    assert!(plan.sessions.is_empty());
    assert_eq!(plan.at_risk[0].item_id, item.id);
}
