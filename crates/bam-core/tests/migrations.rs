use bam_core::store;
use rusqlite::Connection;

const TABLES: &[&str] = &[
    "landing_index_line",
    "package",
    "enrichment",
    "selection",
    "selection_member",
];

fn table_names(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .unwrap();
    stmt.query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn fresh_db_creates_every_table() {
    let conn = store::open(":memory:").unwrap();
    let names = table_names(&conn);
    for table in TABLES {
        assert!(names.contains(&table.to_string()), "missing table {table}");
    }
}

#[test]
fn applying_twice_is_a_noop() {
    let conn = store::open(":memory:").unwrap();
    let version_before: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();

    // A second run must not try to re-create tables that already exist.
    store::apply_migrations(&conn).unwrap();

    let version_after: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version_before, version_after);
}

#[test]
fn db_at_version_n_only_runs_migrations_above_n() {
    let conn = Connection::open(":memory:").unwrap();
    // Run migration 1's DDL directly (bypassing `apply_migrations`, so it
    // isn't counted), then stamp `user_version = 1` and let
    // `apply_migrations` take over. Migration 6 needs `package` to already
    // exist (it `ALTER TABLE`s it), which rules out the simpler "stamp 1,
    // run nothing" version of this test used before migration 6 existed. If
    // `apply_migrations` ignored `user_version` and re-ran migration 1
    // anyway, its `CREATE TABLE package` would collide with the one already
    // here and error — so a plain `unwrap()` below already proves migration
    // 1 was skipped; the table-set assertion proves 2-6 all ran.
    conn.execute_batch(include_str!("../migrations/0001_initial.sql"))
        .unwrap();
    conn.pragma_update(None, "user_version", 1).unwrap();

    store::apply_migrations(&conn).unwrap();

    let mut tables = table_names(&conn);
    tables.sort();
    assert_eq!(
        tables,
        vec![
            "blobs".to_string(),
            "enrichment".to_string(),
            "fetch_queue".to_string(),
            "http_cache".to_string(),
            "landing_index_line".to_string(),
            "landing_readme".to_string(),
            "package".to_string(),
            "package_embedding".to_string(),
            "package_fts".to_string(),
            "package_fts_config".to_string(),
            "package_fts_data".to_string(),
            "package_fts_docsize".to_string(),
            "package_fts_idx".to_string(),
            "selection".to_string(),
            "selection_member".to_string(),
        ]
    );
}
