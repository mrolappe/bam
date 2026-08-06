//! `Session::search_window` (P3.4): a page of matches plus the total count,
//! the primitive the TUI's virtualized list queries instead of
//! `search_packages`, which would materialize every match.

use std::sync::atomic::{AtomicU64, Ordering};

use bam_core::query::ir::{CmpOp, FieldId, Predicate, Value};
use bam_core::store::session::Session;
use bam_core::store::tables::{self, LandingIndexLine, Package};

fn temp_db_path() -> std::path::PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "bam-store-session-window-test-{}-{n}.sqlite",
        std::process::id()
    ))
}

fn all_dir_predicate() -> Predicate {
    Predicate::Compare {
        field: FieldId("dir".into()),
        op: CmpOp::Eq,
        value: Value::Text("games/action".into()),
    }
}

/// Opens a fresh on-disk DB, inserts `n` packages sharing one `dir` (so a
/// single field-compare predicate matches them all), then hands back a
/// `Session` opened against the same file.
fn seeded_session(n: i64) -> Session {
    let path = temp_db_path();
    let conn = bam_core::store::open(&path).unwrap();
    let landing_id = tables::insert_landing_index_line(
        &conn,
        &LandingIndexLine {
            id: 0,
            fetched_at: "2026-01-01T00:00:00Z".into(),
            source_url: "test://fixture".into(),
            line_no: 1,
            raw: vec![],
        },
    )
    .unwrap();
    for i in 0..n {
        tables::insert_package(
            &conn,
            &Package {
                id: 0,
                dir: "games/action".into(),
                file: format!("pkg{i}.lha"),
                name: format!("pkg{i}"),
                version: None,
                size_bytes: Some(1),
                uploaded_on: Some("2026-01-01".into()),
                date_precision: "exact".into(),
                description: None,
                landing_id,
            },
        )
        .unwrap();
    }
    drop(conn);
    Session::open(&path).unwrap()
}

#[test]
fn window_returns_a_page_and_the_total_match_count() {
    let session = seeded_session(25);
    let pred = all_dir_predicate();

    let (page, total) = session.search_window(&pred, 10, 5).unwrap();
    assert_eq!(total, 25);
    assert_eq!(page.len(), 5);

    let (full, _) = session.search_window(&pred, 0, 25).unwrap();
    let expected: Vec<_> = full[10..15].iter().map(|p| p.id).collect();
    let got: Vec<_> = page.iter().map(|p| p.id).collect();
    assert_eq!(got, expected);
}

#[test]
fn window_past_the_end_is_empty_but_total_is_still_reported() {
    let session = seeded_session(5);
    let pred = all_dir_predicate();

    let (page, total) = session.search_window(&pred, 100, 10).unwrap();
    assert_eq!(total, 5);
    assert!(page.is_empty());
}
