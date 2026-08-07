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
    // Pretend migration 1 already ran, without ever running its DDL. If
    // `apply_migrations` ignored `user_version` and re-ran migration 1
    // anyway, its tables would appear alongside migrations 2 and 3's — so
    // instead prove migration 1 was skipped by checking only the later
    // migrations' tables (`http_cache`, `fetch_queue`) exist afterwards.
    conn.pragma_update(None, "user_version", 1).unwrap();

    store::apply_migrations(&conn).unwrap();

    let mut tables = table_names(&conn);
    tables.sort();
    assert_eq!(
        tables,
        vec!["fetch_queue".to_string(), "http_cache".to_string()]
    );
}
