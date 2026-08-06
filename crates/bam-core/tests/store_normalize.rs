//! P1.6 — normalizer tests requiring a database: idempotency, offline
//! rebuild, and sanity of the derived dates and `date_precision`.

use bam_core::store::{self, normalize::normalize, tables::*};
use rusqlite::Connection;

type PackageRow = (
    String,
    String,
    String,
    Option<String>,
    Option<i64>,
    Option<String>,
    String,
    Option<String>,
);

fn fixture_lines() -> Vec<Vec<u8>> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/index_sample.txt");
    let bytes = std::fs::read(path).unwrap();
    let mut lines: Vec<Vec<u8>> = bytes.split(|&b| b == b'\n').map(<[u8]>::to_vec).collect();
    if lines.last().is_some_and(Vec::is_empty) {
        lines.pop();
    }
    lines
}

fn ingest_fixture(conn: &Connection) {
    for (i, raw) in fixture_lines().iter().enumerate() {
        insert_landing_index_line(
            conn,
            &LandingIndexLine {
                id: 0,
                fetched_at: "2026-08-06T00:00:00Z".into(),
                source_url: "https://ftp.fau.de/aminet/INDEX".into(),
                line_no: i as i64 + 1,
                raw: raw.clone(),
            },
        )
        .unwrap();
    }
}

fn package_rows(conn: &Connection) -> Vec<PackageRow> {
    let mut stmt = conn
        .prepare(
            "SELECT dir, file, name, version, size_bytes, uploaded_on, date_precision, description
             FROM package ORDER BY id",
        )
        .unwrap();
    stmt.query_map([], |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
            row.get(7)?,
        ))
    })
    .unwrap()
    .collect::<rusqlite::Result<_>>()
    .unwrap()
}

#[test]
fn normalize_is_idempotent() {
    let conn = store::open(":memory:").unwrap();
    ingest_fixture(&conn);

    normalize(&conn).unwrap();
    let first = package_rows(&conn);

    normalize(&conn).unwrap();
    let second = package_rows(&conn);

    assert_eq!(first, second);
    assert!(!first.is_empty());
}

#[test]
fn offline_rebuild_restores_identically() {
    let conn = store::open(":memory:").unwrap();
    ingest_fixture(&conn);
    normalize(&conn).unwrap();
    let before = package_rows(&conn);

    // "Network unavailable" isn't representable in-process; the point being
    // proven is that rebuilding needs nothing but `landing_index_line`, so
    // dropping and recreating `package` (same DDL as migrations/0001) and
    // re-normalizing is the faithful in-process version of that claim.
    conn.execute("DROP TABLE package", []).unwrap();
    conn.execute_batch(
        "CREATE TABLE package (
           id             INTEGER PRIMARY KEY,
           dir            TEXT NOT NULL,
           file           TEXT NOT NULL,
           name           TEXT NOT NULL,
           version        TEXT,
           size_bytes     INTEGER,
           uploaded_on    TEXT,
           date_precision TEXT NOT NULL,
           description    TEXT,
           landing_id     INTEGER NOT NULL REFERENCES landing_index_line(id),
           UNIQUE(dir, file)
         )",
    )
    .unwrap();

    normalize(&conn).unwrap();
    let after = package_rows(&conn);

    assert_eq!(before, after);
}

#[test]
fn every_row_has_week_precision() {
    let conn = store::open(":memory:").unwrap();
    ingest_fixture(&conn);
    normalize(&conn).unwrap();

    for row in package_rows(&conn) {
        assert_eq!(row.6, "week");
    }
}

#[test]
fn age_zero_and_max_observed_age_produce_sane_dates() {
    use bam_core::ingest::normalize::date_from_age_weeks;

    // Age 0: uploaded on the day the INDEX was fetched.
    assert_eq!(
        date_from_age_weeks("2026-08-06T00:00:00Z", 0).as_deref(),
        Some("2026-08-06")
    );

    // 999 is the maximum age observed in the fixture (see PROGRESS.md Round
    // 4) — Aminet's real sentinel for "very old / unknown exact age", still
    // expected to resolve to a plausible calendar date rather than
    // overflowing or underflowing.
    assert_eq!(
        date_from_age_weeks("2026-08-06T00:00:00Z", 999).as_deref(),
        Some("2007-06-14")
    );
}
