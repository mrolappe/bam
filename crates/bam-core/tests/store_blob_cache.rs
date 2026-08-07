use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use bam_core::blob::{BlobHash, BlobStore, FsBlobStore};
use bam_core::store::blob_cache::{EvictionError, evict_to_budget, record_blob, set_pinned};
use bam_core::store::tables::*;
use bam_core::store::{self};
use rusqlite::Connection;

fn temp_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "bam-blob-cache-test-{label}-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn fresh_db() -> Connection {
    store::open(":memory:").unwrap()
}

/// Inserts a package and a real, DB-tracked blob for it in one step.
fn seed(
    conn: &Connection,
    store: &FsBlobStore,
    file: &str,
    contents: &[u8],
    last_used: &str,
) -> (i64, BlobHash) {
    let landing_id = insert_landing_index_line(
        conn,
        &LandingIndexLine {
            id: 0,
            fetched_at: "2026-08-07T00:00:00Z".into(),
            source_url: "https://ftp.fau.de/aminet/INDEX".into(),
            line_no: 1,
            raw: format!("util/misc  {file}  10K  a test package").into_bytes(),
        },
    )
    .unwrap();
    let package_id = insert_package(
        conn,
        &Package {
            id: 0,
            dir: "util/misc".into(),
            file: file.into(),
            name: file.into(),
            version: None,
            size_bytes: Some(contents.len() as i64),
            uploaded_on: Some("2026-08-01".into()),
            date_precision: "week".into(),
            description: Some("a test package".into()),
            landing_id,
        },
    )
    .unwrap();

    let hash = store.put(Cursor::new(contents.to_vec())).unwrap();
    record_blob(conn, &hash, contents.len() as i64, last_used).unwrap();
    set_archive_hash(conn, package_id, Some(hash.as_str())).unwrap();

    (package_id, hash)
}

#[test]
fn evicting_to_a_small_budget_removes_unpinned_and_keeps_pinned() {
    let conn = fresh_db();
    let store = FsBlobStore::new(temp_dir("evict-basic")).unwrap();

    let (_, keep) = seed(
        &conn,
        &store,
        "Keep.lha",
        b"pinned bytes",
        "2026-08-01T00:00:00Z",
    );
    let (_, evict) = seed(
        &conn,
        &store,
        "Evict.lha",
        b"unpinned bytes",
        "2026-08-02T00:00:00Z",
    );
    set_pinned(&conn, &keep, true).unwrap();

    // Budget only the pinned blob's own size — the unpinned one must go
    // entirely, and eviction must stop there rather than touch the pinned one.
    let report = evict_to_budget(&conn, &store, "pinned bytes".len() as i64).unwrap();

    assert_eq!(report.evicted, vec![evict.clone()]);
    assert!(store.get(&keep).is_ok());
    assert!(store.get(&evict).is_err());
}

#[test]
fn every_enrichment_row_survives_eviction() {
    let conn = fresh_db();
    let store = FsBlobStore::new(temp_dir("evict-enrichment")).unwrap();

    let (package_id, _evict) = seed(
        &conn,
        &store,
        "Foo.lha",
        b"contents",
        "2026-08-01T00:00:00Z",
    );
    insert_enrichment(
        &conn,
        &Enrichment {
            package_id,
            kind: "readme_header".into(),
            producer_version: 1,
            produced_at: "2026-08-07T00:00:00Z".into(),
            payload: "{}".into(),
        },
    )
    .unwrap();

    evict_to_budget(&conn, &store, 0).unwrap();

    let enrichment = get_enrichment(&conn, package_id, "readme_header").unwrap();
    assert_eq!(enrichment.payload, "{}");
}

#[test]
fn package_rows_survive_only_archive_hash_is_cleared() {
    let conn = fresh_db();
    let store = FsBlobStore::new(temp_dir("evict-package")).unwrap();

    let (package_id, hash) = seed(
        &conn,
        &store,
        "Foo.lha",
        b"contents",
        "2026-08-01T00:00:00Z",
    );

    evict_to_budget(&conn, &store, 0).unwrap();

    let package = get_package(&conn, package_id).unwrap();
    assert_eq!(package.file, "Foo.lha");
    assert_eq!(get_archive_hash(&conn, package_id).unwrap(), None);
    assert_ne!(
        get_archive_hash(&conn, package_id).unwrap(),
        Some(hash.as_str().to_string())
    );
}

#[test]
fn eviction_order_is_least_recently_used() {
    let conn = fresh_db();
    let store = FsBlobStore::new(temp_dir("evict-lru")).unwrap();

    let (_, oldest) = seed(
        &conn,
        &store,
        "A.lha",
        b"aaaaaaaaaa",
        "2026-08-01T00:00:00Z",
    );
    let (_, middle) = seed(
        &conn,
        &store,
        "B.lha",
        b"bbbbbbbbbb",
        "2026-08-02T00:00:00Z",
    );
    let (_, newest) = seed(
        &conn,
        &store,
        "C.lha",
        b"cccccccccc",
        "2026-08-03T00:00:00Z",
    );

    // Budget for exactly one blob's worth (10 bytes) — two must go, oldest first.
    let report = evict_to_budget(&conn, &store, 10).unwrap();

    assert_eq!(report.evicted, vec![oldest, middle]);
    assert!(store.get(&newest).is_ok());
}

#[test]
fn evicting_with_everything_pinned_reports_budget_not_met_rather_than_evicting() {
    let conn = fresh_db();
    let store = FsBlobStore::new(temp_dir("evict-all-pinned")).unwrap();

    let (_, only) = seed(
        &conn,
        &store,
        "Foo.lha",
        b"contents",
        "2026-08-01T00:00:00Z",
    );
    set_pinned(&conn, &only, true).unwrap();

    let err = evict_to_budget(&conn, &store, 0).unwrap_err();

    assert!(matches!(err, EvictionError::BudgetNotMet { .. }));
    assert!(store.get(&only).is_ok(), "pinned blob must not be evicted");
}
