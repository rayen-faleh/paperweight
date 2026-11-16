//! `PAPERWEIGHT_DB` overrides the database path. Alone in its own test binary
//! because it changes the process environment.

#[test]
fn env_var_overrides_db_path() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("override.db");
    // SAFETY: this test binary has a single test, so no other thread reads
    // the environment concurrently.
    unsafe { std::env::set_var(paperweight_core::db::DB_ENV, &p) };
    assert_eq!(paperweight_core::db::default_path(), p);
    assert_eq!(paperweight_core::db::demo_path(), p);
    let s = paperweight_core::Store::open_default().unwrap();
    assert_eq!(s.path(), p);
    assert!(p.exists());
    unsafe { std::env::remove_var(paperweight_core::db::DB_ENV) };
    assert!(paperweight_core::db::default_path().ends_with("paperweight.db"));
}
