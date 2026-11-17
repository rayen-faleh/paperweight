//! Concurrent writers, as when an assistant calls several tools at once or
//! the MCP server and the app write at the same moment. Each writer has its
//! own connection; none may fail with "database is locked".

mod common;

use std::sync::{Arc, Barrier};

use chrono::Duration;
use common::*;
use paperweight_core::*;

const THREADS: usize = 8;
const EACH: usize = 15;

#[test]
fn concurrent_writes_from_separate_connections_all_succeed() {
    let (_d, mut s) = store();
    let path = s.path().to_path_buf();
    let now = tue_afternoon();
    let shared = s
        .add_item(
            NewItem::new("Shared item", now + Duration::days(3), false),
            now,
        )
        .unwrap();

    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let (path, barrier, shared) = (path.clone(), barrier.clone(), shared.id.clone());
            std::thread::spawn(move || -> Result<()> {
                let mut s = Store::open(&path)?;
                barrier.wait();
                for i in 0..EACH {
                    let mut n =
                        NewItem::new(format!("t{t} item {i}"), now + Duration::days(2), false);
                    n.requirements = vec!["Passport".into()];
                    n.steps = vec![NewStep {
                        title: "Step".into(),
                        soft_due_at: None,
                    }];
                    let item = s.add_item(n, now)?;
                    s.update_item(
                        &item.id,
                        ItemPatch {
                            notes: Some(format!("edited {i}")),
                            ..Default::default()
                        },
                        now,
                    )?;
                    s.update_item(
                        &shared,
                        ItemPatch {
                            add_requirements: vec![format!("from t{t} #{i}")],
                            ..Default::default()
                        },
                        now,
                    )?;
                    s.record_sent(&[(item.id.clone(), "7d".into())], now)?;
                }
                Ok(())
            })
        })
        .collect();
    for h in handles {
        h.join()
            .unwrap()
            .expect("no writer may fail with database is locked");
    }
    assert_eq!(s.open_items().unwrap().len(), 1 + THREADS * EACH);
    assert_eq!(
        s.get_item(&shared.id).unwrap().requirements.len(),
        THREADS * EACH
    );
    assert_eq!(s.reminder_log().unwrap().len(), THREADS * EACH);
    let _ = &mut s;
}

#[test]
fn a_writer_waits_past_the_busy_timeout_instead_of_failing() {
    let (_d, mut s) = store();
    let path = s.path().to_path_buf();
    let (locked_tx, locked_rx) = std::sync::mpsc::channel();
    // Another process holds the write lock for longer than busy_timeout (5 s).
    let holder = std::thread::spawn(move || {
        let other = Store::open(&path).unwrap();
        other.conn().execute_batch("BEGIN IMMEDIATE").unwrap();
        locked_tx.send(()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(6500));
        other.conn().execute_batch("COMMIT").unwrap();
    });
    locked_rx.recv().unwrap();
    let start = std::time::Instant::now();
    let item = s.add_item(
        NewItem::new("Waited for the lock", tue_afternoon(), false),
        tue_afternoon(),
    );
    let waited = start.elapsed();
    holder.join().unwrap();
    assert!(
        item.is_ok(),
        "write failed instead of waiting: {:?}",
        item.err()
    );
    assert!(
        waited.as_secs_f64() > 5.5,
        "it really waited past busy_timeout ({waited:?})"
    );
}
