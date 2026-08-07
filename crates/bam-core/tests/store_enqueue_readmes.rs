//! `Session::enqueue_readmes` (P4.7, §7): every package surviving a query's
//! filters gets its readme queued, with the visible window boosted over the
//! rest.

use std::sync::atomic::{AtomicU64, Ordering};

use bam_core::ingest::readme::readme_url;
use bam_core::query::ir::{CmpOp, FieldId, Predicate, Value};
use bam_core::store::fetch_queue;
use bam_core::store::session::{README_PRIORITY_BACKGROUND, README_PRIORITY_VISIBLE, Session};
use bam_core::store::tables::{self, LandingIndexLine, LandingReadme, Package};

fn temp_db_path() -> std::path::PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "bam-store-enqueue-readmes-test-{}-{n}.sqlite",
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
/// `Session` plus the packages' urls in `id` order.
fn seeded_session(n: i64) -> (Session, std::path::PathBuf) {
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
    (Session::open(&path).unwrap(), path)
}

fn queue_urls(path: &std::path::Path) -> Vec<String> {
    let conn = bam_core::store::open(path).unwrap();
    let mut stmt = conn
        .prepare("SELECT url FROM fetch_queue ORDER BY url")
        .unwrap();
    stmt.query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn running_a_query_enqueues_exactly_its_result_set() {
    let (session, path) = seeded_session(10);
    session.enqueue_readmes(&all_dir_predicate(), 0, 5).unwrap();

    let expected: Vec<String> = (0..10)
        .map(|i| readme_url("games/action", &format!("pkg{i}.lha")))
        .collect();
    let mut expected = expected;
    expected.sort();
    assert_eq!(queue_urls(&path), expected);
}

#[test]
fn visible_window_rows_carry_higher_priority_than_the_rest() {
    let (session, path) = seeded_session(10);
    session.enqueue_readmes(&all_dir_predicate(), 2, 3).unwrap();

    let conn = bam_core::store::open(&path).unwrap();
    for i in 0..10 {
        let url = readme_url("games/action", &format!("pkg{i}.lha"));
        let item = fetch_queue::get(&conn, &url).unwrap().unwrap();
        let expected = if (2..5).contains(&i) {
            README_PRIORITY_VISIBLE
        } else {
            README_PRIORITY_BACKGROUND
        };
        assert_eq!(item.priority, expected, "url {url}");
    }
}

#[test]
fn rerunning_the_same_query_does_not_duplicate_queue_entries() {
    let (session, path) = seeded_session(10);
    let pred = all_dir_predicate();
    session.enqueue_readmes(&pred, 0, 5).unwrap();
    session.enqueue_readmes(&pred, 0, 5).unwrap();

    let conn = bam_core::store::open(&path).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM fetch_queue", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 10);
}

#[test]
fn already_fetched_readmes_are_not_reenqueued() {
    let (session, path) = seeded_session(3);
    let conn = bam_core::store::open(&path).unwrap();
    let fetched_url = readme_url("games/action", "pkg1.lha");
    let package_id: i64 = conn
        .query_row("SELECT id FROM package WHERE file = 'pkg1.lha'", [], |r| {
            r.get(0)
        })
        .unwrap();
    tables::insert_landing_readme(
        &conn,
        &LandingReadme {
            id: 0,
            package_id,
            url: fetched_url.clone(),
            fetched_at: "2026-01-01T00:00:00Z".into(),
            raw: b"already fetched".to_vec(),
            detected_encoding: "utf-8".into(),
        },
    )
    .unwrap();
    drop(conn);

    session.enqueue_readmes(&all_dir_predicate(), 0, 3).unwrap();

    let urls = queue_urls(&path);
    assert!(!urls.contains(&fetched_url));
    assert_eq!(urls.len(), 2);
}
