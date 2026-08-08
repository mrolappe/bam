//! P9.2's four runnable acceptance tests (the fifth — "no SQL, no query
//! logic" — is `tests/purity.rs`, mirroring `bam-core`'s P0.4 convention).
//! Each test spins up a real `bam-server` on an ephemeral loopback port
//! against its own temp db file and talks to it with `reqwest`, so these
//! exercise the actual HTTP/SSE wire format a browser would see.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use bam_core::store::tables::{self, LandingIndexLine, Package};
use bam_server::{AppState, app};
use futures_util::StreamExt;
use serde_json::{Value, json};

fn temp_db_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "bam-server-test-{label}-{}-{n}.sqlite",
        std::process::id()
    ))
}

/// Inserts one searchable package, returning its id.
fn seed_package(db_path: &std::path::Path, dir: &str) -> i64 {
    let conn = bam_core::store::open(db_path).unwrap();
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
    tables::insert_package(
        &conn,
        &Package {
            id: 0,
            dir: dir.to_string(),
            file: "pkg.lha".into(),
            name: "pkg".into(),
            version: None,
            size_bytes: Some(1),
            uploaded_on: Some("2026-01-01".into()),
            date_precision: "exact".into(),
            description: None,
            landing_id,
        },
    )
    .unwrap()
}

/// Starts a server on an ephemeral port against a fresh temp db, returning
/// its base URL. The server task is detached — it dies with the test
/// process, and the db file is a fresh path per test so nothing needs
/// explicit teardown beyond it.
async fn start_server(label: &str) -> (String, PathBuf) {
    let db_path = temp_db_path(label);
    let state = Arc::new(AppState::new(db_path.clone()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app(state)).await.unwrap();
    });
    (format!("http://{addr}"), db_path)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap()
}

/// Every API operation is reachable over HTTP and round-trips its types.
#[tokio::test]
async fn every_operation_is_reachable_and_round_trips() {
    let (base, db_path) = start_server("roundtrip").await;
    let id = seed_package(&db_path, "mods/tracker");
    let c = client();

    let predicate =
        json!({"Compare": {"field": "dir", "op": "Eq", "value": {"Text": "mods/tracker"}}});

    let parsed: Value = c
        .post(format!("{base}/api/parse-query"))
        .json(&json!({"src": "dir:mods/tracker"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(parsed.get("predicate").is_some());

    let search: Value = c
        .post(format!("{base}/api/search-packages"))
        .json(&json!({"predicate": predicate}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(search["packages"].as_array().unwrap().len(), 1);

    let get: Value = c
        .post(format!("{base}/api/get-package"))
        .json(&json!({"id": id}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(get["package"]["id"], id);

    let filtered: Value = c
        .post(format!("{base}/api/filter-ids"))
        .json(&json!({"predicate": predicate, "ids": [id, id + 1]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(filtered["ids"].as_array().unwrap(), &vec![json!(id)]);

    let categories: Value = c
        .post(format!("{base}/api/list-categories"))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!categories["categories"].as_array().unwrap().is_empty());

    let marked: Value = c
        .post(format!("{base}/api/toggle"))
        .json(&json!({"package_id": id}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(marked["marked"], true);

    let is_marked: Value = c
        .post(format!("{base}/api/is-marked"))
        .json(&json!({"package_id": id}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(is_marked["marked"], true);

    let selected: Value = c
        .post(format!("{base}/api/select-by-query"))
        .json(&json!({"predicate": predicate, "mode": "Replace"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(selected["member_count"], 1);

    let save = c
        .post(format!("{base}/api/save-as"))
        .json(&json!({"name": "tracker candidates"}))
        .send()
        .await
        .unwrap();
    assert_eq!(save.status(), 204);

    let selections: Value = c
        .post(format!("{base}/api/list-selections"))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(selections["selections"][0]["name"], "tracker candidates");

    let load = c
        .post(format!("{base}/api/load"))
        .json(&json!({"name": "tracker candidates"}))
        .send()
        .await
        .unwrap();
    assert_eq!(load.status(), 204);

    let delete = c
        .post(format!("{base}/api/delete-selection"))
        .json(&json!({"name": "tracker candidates"}))
        .send()
        .await
        .unwrap();
    assert_eq!(delete.status(), 204);

    let unmark = c
        .post(format!("{base}/api/unmark"))
        .json(&json!({"package_id": id}))
        .send()
        .await
        .unwrap();
    assert_eq!(unmark.status(), 204);

    let clear = c.post(format!("{base}/api/clear")).send().await.unwrap();
    assert_eq!(clear.status(), 204);
}

/// Two concurrent sessions do not observe each other's working selection.
#[tokio::test]
async fn two_sessions_do_not_observe_each_others_state() {
    let (base, db_path) = start_server("isolation").await;
    let id = seed_package(&db_path, "mods/tracker");

    let a = client();
    let b = client();

    let mark = a
        .post(format!("{base}/api/mark"))
        .json(&json!({"package_id": id}))
        .send()
        .await
        .unwrap();
    assert_eq!(mark.status(), 204);

    let a_marked: Value = a
        .post(format!("{base}/api/is-marked"))
        .json(&json!({"package_id": id}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(a_marked["marked"], true);

    let b_marked: Value = b
        .post(format!("{base}/api/is-marked"))
        .json(&json!({"package_id": id}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(b_marked["marked"], false, "b must not see a's mark");
}

/// Reads SSE `data:` frames from a `text/event-stream` response until the
/// body ends, parsing each as JSON.
async fn read_sse_events(resp: reqwest::Response) -> Vec<Value> {
    let mut buf = String::new();
    let mut events = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        buf.push_str(&String::from_utf8_lossy(&chunk.unwrap()));
        while let Some(idx) = buf.find("\n\n") {
            let frame: String = buf.drain(..idx + 2).collect();
            for line in frame.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    events.push(serde_json::from_str(data).unwrap());
                }
            }
        }
    }
    events
}

/// SSE delivers progress events for a long operation.
#[tokio::test]
async fn sse_delivers_progress_events() {
    let (base, _db_path) = start_server("sse-progress").await;
    let c = client();

    let started: Value = c
        .post(format!("{base}/api/start-ingest"))
        .json(&json!({"mode": "Offline"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let operation = started["operation"].clone();

    let resp = c
        .get(format!(
            "{base}/api/progress/{}",
            operation.as_u64().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let events = tokio::time::timeout(Duration::from_secs(10), read_sse_events(resp))
        .await
        .expect("sse stream did not terminate");

    assert!(!events.is_empty(), "expected at least one progress event");
    let last = events.last().unwrap();
    assert!(
        last.get("Finished").is_some(),
        "stream must end in Finished: {last:?}"
    );
}

/// A client disconnecting and reconnecting with the same `OperationId`
/// re-attaches to the still-running (or just-finished) operation rather
/// than orphaning it — the reconnect either observes the live tail of
/// events or a synthesized terminal one, but never hangs and never leaves
/// the ingest itself unfinished.
#[tokio::test]
async fn reconnecting_sse_reattaches_instead_of_orphaning() {
    let (base, _db_path) = start_server("sse-reconnect").await;
    let c = client();

    let started: Value = c
        .post(format!("{base}/api/start-ingest"))
        .json(&json!({"mode": "Offline"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let operation = started["operation"].as_u64().unwrap();

    // First connection: connect and immediately drop it (simulates a
    // client that disconnects before the operation finishes).
    let first = c
        .get(format!("{base}/api/progress/{operation}"))
        .send()
        .await
        .unwrap();
    drop(first);

    // Reconnect with the same operation id.
    let second = c
        .get(format!("{base}/api/progress/{operation}"))
        .send()
        .await
        .unwrap();
    let events = tokio::time::timeout(Duration::from_secs(10), read_sse_events(second))
        .await
        .expect("reconnecting sse stream did not terminate");
    let last = events
        .last()
        .expect("reconnecting client must observe at least the terminal event");
    assert!(
        last.get("Finished").is_some(),
        "must resolve to Finished: {last:?}"
    );

    // The ingest itself must have actually completed, proving it kept
    // running independent of the first connection dropping.
    let status: Value = c
        .post(format!("{base}/api/operation-status"))
        .json(&json!({"operation": operation}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        status["status"].get("Finished").is_some(),
        "ingest must have completed, not been orphaned: {status:?}"
    );
}
