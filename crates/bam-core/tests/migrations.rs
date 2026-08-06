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
    // Pretend the schema is already at the latest version, without ever
    // running the DDL. If `apply_migrations` ignored `user_version` and
    // re-ran migration 1 anyway, the (nonexistent) CREATE TABLE would still
    // succeed trivially — so instead prove it by checking no tables exist
    // after a call that should have been a no-op.
    conn.pragma_update(None, "user_version", 1).unwrap();

    store::apply_migrations(&conn).unwrap();

    assert!(table_names(&conn).is_empty());
}
