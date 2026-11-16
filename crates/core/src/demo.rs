//! `--seed-demo`: a realistic week for Takaya, relative to now.

use chrono::{DateTime, Duration, NaiveTime, Utc};

use crate::error::Result;
use crate::model::{Freq, Kind, NewItem, NewStep, Recurrence, Settings};
use crate::store::Store;

fn t(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).expect("valid time")
}

/// Delete everything in `path` and fill it with the demo week.
pub fn reset_demo(path: &std::path::Path, now: DateTime<Utc>) -> Result<Store> {
    for suffix in ["", "-wal", "-shm"] {
        let mut p = path.as_os_str().to_owned();
        p.push(suffix);
        let _ = std::fs::remove_file(std::path::PathBuf::from(p));
    }
    let mut store = Store::open(path)?;
    seed(&mut store, now)?;
    Ok(store)
}

pub fn seed(store: &mut Store, now: DateTime<Utc>) -> Result<()> {
    let settings: Settings = store.settings()?;
    store.save_settings(&settings)?;
    let clock = store.clock()?;
    let today = clock.today(now);
    let day = |n: i64| today + Duration::days(n);
    // Items are "created" a couple of weeks ago, as if he had been using it.
    let created = now - Duration::days(14);

    let mut items = vec![];

    let mut residence = NewItem::new("Renew residence card", clock.day_end(day(9)), true);
    residence.kind = Kind::Paperwork;
    residence.estimated_minutes = Some(120);
    residence.notes = "Immigration office opens 9:00–12:00 and 13:00–16:00 on weekdays. Go early; the queue is long.".into();
    residence.source = Some("~/Documents/paperwork/residence-card-extension-form.pdf".into());
    residence.requirements = vec![
        "Passport".into(),
        "Current residence card".into(),
        "One photo, 4 cm × 3 cm, taken in the last 3 months".into(),
        "Application form for extension of period of stay (filled in)".into(),
        "Certificate of enrollment from the university office".into(),
        "Revenue stamp for the processing fee (buy at the office)".into(),
    ];
    residence.steps = vec![
        NewStep {
            title: "Ask the student office for a certificate of enrollment".into(),
            soft_due_at: Some(clock.day_end(day(2))),
        },
        NewStep {
            title: "Get the photo taken".into(),
            soft_due_at: Some(clock.day_end(day(4))),
        },
        NewStep {
            title: "Fill in the application form".into(),
            soft_due_at: None,
        },
        NewStep {
            title: "Go to the immigration office".into(),
            soft_due_at: None,
        },
    ];
    items.push(residence);

    let mut scholarship = NewItem::new(
        "Scholarship continuation form",
        clock.at(day(3), t(17, 0)),
        false,
    );
    scholarship.kind = Kind::Paperwork;
    scholarship.estimated_minutes = Some(60);
    scholarship.notes =
        "Hand in at the student support desk, 2nd floor of the admin building.".into();
    scholarship.requirements = vec![
        "Form B-2, signed".into(),
        "Last semester's transcript".into(),
        "Supervisor's comment (one paragraph)".into(),
    ];
    scholarship.steps = vec![
        NewStep {
            title: "Email supervisor for the comment".into(),
            soft_due_at: Some(clock.day_end(day(1))),
        },
        NewStep {
            title: "Print transcript".into(),
            soft_due_at: None,
        },
    ];
    items.push(scholarship);

    let mut rent = NewItem::new("Rent", clock.day_end(day(5)), true);
    rent.kind = Kind::Payment;
    rent.estimated_minutes = Some(15);
    rent.recurrence = Recurrence {
        freq: Freq::Monthly,
        interval: 1,
    };
    rent.notes = "Bank transfer to the landlord's account. ¥68,000.".into();
    items.push(rent);

    let mut stats = NewItem::new(
        "Statistics problem set 4",
        clock.at(day(2), t(13, 0)),
        false,
    );
    stats.kind = Kind::Assignment;
    stats.estimated_minutes = Some(180);
    stats.source = Some("https://lms.example.edu/courses/stat201/assignments/4".into());
    stats.steps = vec![
        NewStep {
            title: "Questions 1–3".into(),
            soft_due_at: None,
        },
        NewStep {
            title: "Questions 4–6 (regression)".into(),
            soft_due_at: None,
        },
        NewStep {
            title: "Write up and upload PDF".into(),
            soft_due_at: None,
        },
    ];
    items.push(stats);

    let mut essay = NewItem::new(
        "Research methods essay: first draft",
        clock.at(day(6), t(17, 0)),
        false,
    );
    essay.kind = Kind::Assignment;
    essay.estimated_minutes = Some(300);
    essay.notes = "2,500 words. Needs a 5-hour block; none of the routine blocks is that long, so the planner flags it.".into();
    items.push(essay);

    let mut dinner = NewItem::new(
        "Dinner with Kenta and Mio",
        clock.at(day(4), t(19, 30)),
        false,
    );
    dinner.kind = Kind::Social;
    dinner.estimated_minutes = Some(0);
    dinner.notes = "Ramen place near the station. Bring the book Mio lent me.".into();
    items.push(dinner);

    let mut insurance = NewItem::new(
        "Pay national health insurance slip",
        clock.day_end(day(-2)),
        true,
    );
    insurance.kind = Kind::Payment;
    insurance.estimated_minutes = Some(20);
    insurance.notes = "Payment slip from the city office. Pay at any convenience store.".into();
    items.push(insurance);

    for new in items {
        store.add_item(new, created)?;
    }
    Ok(())
}
