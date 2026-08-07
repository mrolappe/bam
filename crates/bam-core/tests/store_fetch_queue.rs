//! P4.1 — `fetch_queue` schema and atomic claim.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;

use bam_core::store::fetch_queue::{claim_next, enqueue, get, mark_failure, mark_success};

const FAR_FUTURE: &str = "2999-01-01T00:00:00Z";
const NEVER_STALE: &str = "1970-01-01T00:00:00Z";

fn temp_db_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "bam-fetch-queue-test-{name}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("db.sqlite3")
}

#[test]
fn two_threads_claiming_simultaneously_receive_different_rows() {
    let path = temp_db_path("concurrent");
    let _ = std::fs::remove_file(&path);
    {
        let conn = bam_core::store::open(&path).unwrap();
        for i in 0..40 {
            enqueue(&conn, &format!("https://example.invalid/{i}"), "readme", 0).unwrap();
        }
    }

    let claimed: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let path = path.clone();
        let claimed = Arc::clone(&claimed);
        handles.push(thread::spawn(move || {
            let conn = bam_core::store::open(&path).unwrap();
            while let Some(item) = claim_next(&conn, FAR_FUTURE, NEVER_STALE).unwrap() {
                claimed.lock().unwrap().push(item.url);
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    let claimed = claimed.lock().unwrap();
    assert_eq!(claimed.len(), 40, "every row claimed exactly once total");
    let unique: HashSet<_> = claimed.iter().collect();
    assert_eq!(unique.len(), 40, "no row claimed twice");
}

#[test]
fn items_with_future_next_attempt_at_are_not_claimed() {
    let path = temp_db_path("future");
    let _ = std::fs::remove_file(&path);
    let conn = bam_core::store::open(&path).unwrap();
    enqueue(&conn, "https://example.invalid/a", "readme", 0).unwrap();
    mark_failure(&conn, "https://example.invalid/a", Some(500), FAR_FUTURE).unwrap();

    let claimed = claim_next(&conn, "2026-01-01T00:00:00Z", NEVER_STALE).unwrap();
    assert!(claimed.is_none());
}

#[test]
fn higher_priority_is_claimed_first() {
    let path = temp_db_path("priority");
    let _ = std::fs::remove_file(&path);
    let conn = bam_core::store::open(&path).unwrap();
    enqueue(&conn, "https://example.invalid/low", "readme", 1).unwrap();
    enqueue(&conn, "https://example.invalid/high", "readme", 10).unwrap();

    let claimed = claim_next(&conn, FAR_FUTURE, NEVER_STALE).unwrap().unwrap();
    assert_eq!(claimed.url, "https://example.invalid/high");
}

#[test]
fn marking_failure_increments_attempts_and_sets_a_future_next_attempt_at() {
    let path = temp_db_path("failure");
    let _ = std::fs::remove_file(&path);
    let conn = bam_core::store::open(&path).unwrap();
    enqueue(&conn, "https://example.invalid/a", "readme", 0).unwrap();
    claim_next(&conn, FAR_FUTURE, NEVER_STALE).unwrap().unwrap();

    mark_failure(&conn, "https://example.invalid/a", Some(503), FAR_FUTURE).unwrap();

    let item = get(&conn, "https://example.invalid/a").unwrap().unwrap();
    assert_eq!(item.attempts, 1);
    assert_eq!(item.last_status, Some(503));
    assert_eq!(item.next_attempt_at.as_deref(), Some(FAR_FUTURE));
    assert!(item.claimed_at.is_none(), "failure releases the claim");
}

#[test]
fn a_claim_abandoned_by_a_crashed_worker_becomes_reclaimable_after_a_timeout() {
    let path = temp_db_path("abandoned");
    let _ = std::fs::remove_file(&path);
    let conn = bam_core::store::open(&path).unwrap();
    enqueue(&conn, "https://example.invalid/a", "readme", 0).unwrap();

    let claimed_at = "2026-01-01T00:00:00Z";
    let first = claim_next(&conn, claimed_at, NEVER_STALE).unwrap().unwrap();
    assert_eq!(first.url, "https://example.invalid/a");

    // No mark_success/mark_failure call — the worker "crashed" mid-fetch.
    // Reclaiming with a stale_before earlier than the claim is refused...
    let still_claimed = claim_next(&conn, "2026-01-01T00:00:05Z", "2025-12-31T23:59:59Z").unwrap();
    assert!(still_claimed.is_none());

    // ...but a stale_before after the claim's timestamp reclaims it.
    let reclaimed = claim_next(&conn, "2026-01-01T01:00:00Z", "2026-01-01T00:00:01Z")
        .unwrap()
        .unwrap();
    assert_eq!(reclaimed.url, "https://example.invalid/a");
}

#[test]
fn mark_success_clears_the_claim_and_updates_the_etag() {
    let path = temp_db_path("success");
    let _ = std::fs::remove_file(&path);
    let conn = bam_core::store::open(&path).unwrap();
    enqueue(&conn, "https://example.invalid/a", "readme", 0).unwrap();
    claim_next(&conn, FAR_FUTURE, NEVER_STALE).unwrap().unwrap();

    mark_success(
        &conn,
        "https://example.invalid/a",
        200,
        Some("etag-1"),
        None,
    )
    .unwrap();
    let item = get(&conn, "https://example.invalid/a").unwrap().unwrap();
    assert_eq!(item.last_status, Some(200));
    assert_eq!(item.etag.as_deref(), Some("etag-1"));
    assert!(item.claimed_at.is_none());

    // A subsequent 304 (no new ETag) leaves the stored one untouched.
    claim_next(&conn, FAR_FUTURE, NEVER_STALE).unwrap().unwrap();
    mark_success(&conn, "https://example.invalid/a", 304, None, None).unwrap();
    let item = get(&conn, "https://example.invalid/a").unwrap().unwrap();
    assert_eq!(item.last_status, Some(304));
    assert_eq!(item.etag.as_deref(), Some("etag-1"));
}
