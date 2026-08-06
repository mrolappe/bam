//! P1.9 — `HttpClient` trait + fetch orchestration. A fake client drives
//! every scenario except the explicitly `#[ignore]`d real-mirror check, so
//! the default run makes no network calls (invariant I8).

use std::io::Write;
use std::sync::Mutex;

use bam_core::http::{HttpClient, HttpError, HttpRequest, HttpResponse};
use bam_core::store::fetch::{FetchError, FetchOutcome, fetch_and_land};
use bam_core::store::normalize::normalize;
use bam_core::store::{self};

const INDEX_URL: &str = "https://ftp.fau.de/aminet/INDEX.gz";
const FETCHED_AT: &str = "2026-08-06T00:00:00Z";

fn read_fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(path).unwrap()
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(bytes).unwrap();
    enc.finish().unwrap()
}

fn landing_line_count(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM landing_index_line", [], |row| {
        row.get(0)
    })
    .unwrap()
}

/// Scripted client: replays queued responses in request order, recording
/// every request it received.
struct FakeClient {
    responses: Mutex<Vec<Result<HttpResponse, HttpError>>>,
    requests: Mutex<Vec<HttpRequest>>,
}

impl FakeClient {
    fn new(responses: Vec<Result<HttpResponse, HttpError>>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().rev().collect()),
            requests: Mutex::new(Vec::new()),
        }
    }
}

impl HttpClient for FakeClient {
    async fn get(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        self.requests.lock().unwrap().push(req);
        self.responses
            .lock()
            .unwrap()
            .pop()
            .expect("no scripted response left")
    }
}

#[tokio::test]
async fn fake_client_drives_full_ingest_with_no_network() {
    let conn = store::open(":memory:").unwrap();
    let body = gzip(&read_fixture("index_sample.txt"));
    let client = FakeClient::new(vec![Ok(HttpResponse {
        status: 200,
        body,
        etag: Some("\"abc\"".into()),
    })]);

    let outcome = fetch_and_land(&conn, &client, INDEX_URL, FETCHED_AT)
        .await
        .unwrap();
    let FetchOutcome::Fetched { landing_ids } = outcome else {
        panic!("expected a fetch, got NotModified");
    };
    assert!(!landing_ids.is_empty());

    normalize(&conn).unwrap();
    let package_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM package", [], |row| row.get(0))
        .unwrap();
    assert!(package_count > 0);
}

#[tokio::test]
async fn stored_etag_is_sent_on_second_request() {
    let conn = store::open(":memory:").unwrap();
    let body = gzip(&read_fixture("index_sample.txt"));
    let client = FakeClient::new(vec![
        Ok(HttpResponse {
            status: 200,
            body,
            etag: Some("\"v1\"".into()),
        }),
        Ok(HttpResponse {
            status: 304,
            body: Vec::new(),
            etag: None,
        }),
    ]);

    fetch_and_land(&conn, &client, INDEX_URL, FETCHED_AT)
        .await
        .unwrap();
    fetch_and_land(&conn, &client, INDEX_URL, FETCHED_AT)
        .await
        .unwrap();

    let requests = client.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].if_none_match, None);
    assert_eq!(requests[1].if_none_match.as_deref(), Some("\"v1\""));
}

#[tokio::test]
async fn not_modified_lands_nothing_and_is_not_an_error() {
    let conn = store::open(":memory:").unwrap();
    let client = FakeClient::new(vec![Ok(HttpResponse {
        status: 304,
        body: Vec::new(),
        etag: None,
    })]);

    let outcome = fetch_and_land(&conn, &client, INDEX_URL, FETCHED_AT)
        .await
        .unwrap();
    assert_eq!(outcome, FetchOutcome::NotModified);
    assert_eq!(landing_line_count(&conn), 0);
}

#[tokio::test]
async fn server_error_surfaces_as_http_error_not_partial_ingest() {
    let conn = store::open(":memory:").unwrap();
    let client = FakeClient::new(vec![Err(HttpError::Request(
        "unexpected status 500".into(),
    ))]);

    let err = fetch_and_land(&conn, &client, INDEX_URL, FETCHED_AT)
        .await
        .unwrap_err();
    assert!(matches!(err, FetchError::Http(_)));
    assert_eq!(landing_line_count(&conn), 0);
}

#[tokio::test]
#[ignore = "hits a real Aminet mirror; run explicitly, never in CI"]
async fn real_mirror_fetch() {
    use bam_core::http::ReqwestClient;

    let conn = store::open(":memory:").unwrap();
    let client = ReqwestClient::new();
    let outcome = fetch_and_land(
        &conn,
        &client,
        "https://ftp.fau.de/aminet/RECENT.gz",
        FETCHED_AT,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, FetchOutcome::Fetched { .. }));
}
