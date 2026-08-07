//! P4.3 — background fetch worker. A scripted client drives every scenario
//! except the explicitly `#[ignore]`d real-mirror check, so the default run
//! makes no network calls (invariant I8).

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use bam_core::http::{HttpClient, HttpError, HttpRequest, HttpResponse};
use bam_core::ratelimit::{RateLimitConfig, SystemClock, TokenBucket};
use bam_core::store::fetch_queue::{enqueue, get};
use bam_core::store::fetch_worker::{FetchResult, StepOutcome, step};
use bam_core::store::{self};

const FAR_FUTURE: &str = "2999-01-01T00:00:00Z";

/// Scripted-by-url client: a queued response per url, popped in request
/// order; an unscripted `.../robots.txt` defaults to 404 (no robots.txt —
/// the common case), and any other unscripted url panics, so a test proves
/// "never fetched" by simply not scripting it.
#[derive(Default)]
struct ByUrlClient {
    responses: Mutex<HashMap<String, VecDeque<Result<HttpResponse, HttpError>>>>,
    requests: Mutex<Vec<HttpRequest>>,
}

impl ByUrlClient {
    fn new() -> Self {
        Self::default()
    }

    fn script(&self, url: &str, resp: Result<HttpResponse, HttpError>) {
        self.responses
            .lock()
            .unwrap()
            .entry(url.to_string())
            .or_default()
            .push_back(resp);
    }
}

impl HttpClient for ByUrlClient {
    async fn get(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        self.requests.lock().unwrap().push(req.clone());
        let mut responses = self.responses.lock().unwrap();
        if let Some(next) = responses.get_mut(&req.url).and_then(VecDeque::pop_front) {
            return next;
        }
        if req.url.ends_with("/robots.txt") {
            return Ok(HttpResponse {
                status: 404,
                body: Vec::new(),
                etag: None,
            });
        }
        panic!("unscripted request to {}", req.url);
    }
}

fn ok(status: u16, etag: Option<&str>) -> Result<HttpResponse, HttpError> {
    Ok(HttpResponse {
        status,
        body: Vec::new(),
        etag: etag.map(String::from),
    })
}

fn generous_bucket() -> TokenBucket<SystemClock> {
    let config = RateLimitConfig {
        rate: 1000.0,
        burst: 1000,
    };
    TokenBucket::new(&config, SystemClock).unwrap()
}

fn temp_db_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "bam-fetch-worker-test-{name}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("db.sqlite3")
}

#[tokio::test]
async fn a_429_triggers_backoff_with_increasing_delays() {
    let conn = store::open(":memory:").unwrap();
    enqueue(&conn, "https://example.invalid/a.readme", "readme", 0).unwrap();

    let client = ByUrlClient::new();
    for _ in 0..3 {
        client.script(
            "https://example.invalid/a.readme",
            Err(HttpError::Status(429)),
        );
    }
    let bucket = generous_bucket();
    let mut robots = HashMap::new();

    let mut now_unix = 1_000_000u64;
    let mut delays = Vec::new();
    for _ in 0..3 {
        let outcome = step(&conn, &client, &bucket, &mut robots, now_unix)
            .await
            .unwrap();
        let StepOutcome::Fetched {
            result: FetchResult::Retrying {
                next_attempt_at, ..
            },
            ..
        } = outcome
        else {
            panic!("expected a retrying outcome, got {outcome:?}");
        };
        let next_unix: u64 = {
            // next_attempt_at is one of our own rfc3339-from-unix strings;
            // recompute the same way rather than parsing, to find the delay.
            let mut n = now_unix;
            while bam_core::rfc3339_from_unix(n) != next_attempt_at {
                n += 1;
            }
            n
        };
        delays.push(next_unix - now_unix);
        now_unix = next_unix;
    }

    assert_eq!(
        delays,
        vec![1, 2, 4],
        "delays must strictly increase: {delays:?}"
    );
    let item = get(&conn, "https://example.invalid/a.readme")
        .unwrap()
        .unwrap();
    assert_eq!(item.attempts, 3);
    assert_eq!(item.last_status, Some(429));
}

#[tokio::test]
async fn a_stored_etag_is_sent_and_a_304_marks_success_permanently() {
    let conn = store::open(":memory:").unwrap();
    enqueue(&conn, "https://example.invalid/a.readme", "readme", 0).unwrap();
    // Simulate an item that already carries a known ETag (e.g. seeded from
    // an earlier era) without going through a first successful fetch.
    conn.execute(
        "UPDATE fetch_queue SET etag = 'etag-1' WHERE url = 'https://example.invalid/a.readme'",
        [],
    )
    .unwrap();

    let client = ByUrlClient::new();
    client.script("https://example.invalid/a.readme", ok(304, None));
    let bucket = generous_bucket();
    let mut robots = HashMap::new();

    let outcome = step(&conn, &client, &bucket, &mut robots, 1_000_000)
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        StepOutcome::Fetched {
            result: FetchResult::NotModified,
            ..
        }
    ));

    let requests = client.requests.lock().unwrap();
    // requests[0] is the robots.txt probe; requests[1] is the readme itself.
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].if_none_match.as_deref(), Some("etag-1"));

    let item = get(&conn, "https://example.invalid/a.readme")
        .unwrap()
        .unwrap();
    assert_eq!(
        item.etag.as_deref(),
        Some("etag-1"),
        "304 leaves the etag untouched"
    );
    assert_eq!(item.last_status, Some(304));
    assert_eq!(
        item.next_attempt_at.as_deref(),
        Some(FAR_FUTURE),
        "a confirmed-unchanged fetch is complete, same as a fresh 200"
    );
}

#[tokio::test]
async fn robots_txt_disallowing_a_path_prevents_the_fetch() {
    let conn = store::open(":memory:").unwrap();
    enqueue(
        &conn,
        "https://example.invalid/aminet/foo.readme",
        "readme",
        0,
    )
    .unwrap();

    let client = ByUrlClient::new();
    client.script(
        "https://example.invalid/robots.txt",
        Ok(HttpResponse {
            status: 200,
            body: b"User-agent: *\nDisallow: /aminet/\n".to_vec(),
            etag: None,
        }),
    );
    // Deliberately not scripting the readme url itself: the client panics
    // if it's ever requested, proving the fetch never happens.
    let bucket = generous_bucket();
    let mut robots = HashMap::new();

    let outcome = step(&conn, &client, &bucket, &mut robots, 1_000_000)
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        StepOutcome::Fetched {
            result: FetchResult::RobotsDisallowed,
            ..
        }
    ));

    let item = get(&conn, "https://example.invalid/aminet/foo.readme")
        .unwrap()
        .unwrap();
    assert_eq!(item.next_attempt_at.as_deref(), Some(FAR_FUTURE));
    assert!(item.claimed_at.is_none());
}

#[tokio::test]
async fn interrupting_mid_run_and_restarting_does_not_re_fetch_completed_items() {
    let path = temp_db_path("restart");
    let _ = std::fs::remove_file(&path);
    {
        let conn = store::open(&path).unwrap();
        // Higher priority than b, so without an exclusion mechanism a's own
        // completion wouldn't stop it from being reclaimed ahead of b.
        enqueue(&conn, "https://example.invalid/a.readme", "readme", 10).unwrap();
        enqueue(&conn, "https://example.invalid/b.readme", "readme", 1).unwrap();

        let client = ByUrlClient::new();
        client.script("https://example.invalid/a.readme", ok(200, Some("etag-a")));
        let bucket = generous_bucket();
        let mut robots = HashMap::new();
        let outcome = step(&conn, &client, &bucket, &mut robots, 1_000_000)
            .await
            .unwrap();
        assert!(matches!(
            outcome,
            StepOutcome::Fetched {
                result: FetchResult::Success { status: 200 },
                ..
            }
        ));
        // The connection (and, standing in for a crashed process, this
        // worker's whole in-memory state — the robots cache) is dropped here.
    }

    // "Restart": a fresh connection to the same on-disk database, fresh
    // in-memory state.
    let conn = store::open(&path).unwrap();
    let client = ByUrlClient::new();
    client.script("https://example.invalid/b.readme", ok(200, Some("etag-b")));
    // No script for a.readme: the client panics if it's requested again.
    let bucket = generous_bucket();
    let mut robots = HashMap::new();

    let outcome = step(&conn, &client, &bucket, &mut robots, 1_000_001)
        .await
        .unwrap();
    match outcome {
        StepOutcome::Fetched { url, result } => {
            assert_eq!(url, "https://example.invalid/b.readme");
            assert!(matches!(result, FetchResult::Success { status: 200 }));
        }
        other => panic!("expected b.readme to be fetched, got {other:?}"),
    }
}

#[tokio::test]
async fn a_high_priority_item_enqueued_mid_run_is_served_before_the_backlog() {
    let conn = store::open(":memory:").unwrap();
    for i in 0..5 {
        enqueue(
            &conn,
            &format!("https://example.invalid/backlog-{i}"),
            "readme",
            1,
        )
        .unwrap();
    }

    let client = ByUrlClient::new();
    for i in 0..5 {
        client.script(
            &format!("https://example.invalid/backlog-{i}"),
            ok(200, None),
        );
    }
    client.script("https://example.invalid/urgent", ok(200, None));
    let bucket = generous_bucket();
    let mut robots = HashMap::new();

    // Drain one backlog item first, same as an ordinary bulk run in progress.
    step(&conn, &client, &bucket, &mut robots, 1_000_000)
        .await
        .unwrap();

    // The user now looks at something new; it gets queued with a boost.
    enqueue(&conn, "https://example.invalid/urgent", "readme", 100).unwrap();

    let outcome = step(&conn, &client, &bucket, &mut robots, 1_000_001)
        .await
        .unwrap();
    match outcome {
        StepOutcome::Fetched { url, .. } => {
            assert_eq!(url, "https://example.invalid/urgent");
        }
        other => panic!("expected the urgent item to be served next, got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "hits a real Aminet mirror; run explicitly, never in CI"]
async fn real_mirror_harvest_observes_rate_and_no_429s() {
    use bam_core::http::ReqwestClient;
    use bam_core::ingest::index::parse_index_line;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    let fixture = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/index_sample.txt"),
    )
    .unwrap();

    let conn = store::open(":memory:").unwrap();
    let mut count = 0;
    for line in fixture.split(|&b| b == b'\n') {
        if line.is_empty() {
            continue;
        }
        if let Ok(record) = parse_index_line(line) {
            let dir = String::from_utf8_lossy(record.dir);
            let file = String::from_utf8_lossy(record.file);
            // Real convention (confirmed against ftp.fau.de's own listings,
            // P4.5), now shared with P4.7's `Session::enqueue_readmes` via
            // `bam_core::ingest::readme::readme_url` — a `.tar.bz2` file's
            // readme is one 404 short of correct, acceptable for a
            // manual-only check.
            let url = bam_core::ingest::readme::readme_url(&dir, &file);
            enqueue(&conn, &url, "readme", 0).unwrap();
            count += 1;
            if count >= 1000 {
                break;
            }
        }
    }

    let client = ReqwestClient::new();
    let config = RateLimitConfig::default();
    let bucket = TokenBucket::new(&config, SystemClock).unwrap();
    let mut robots = HashMap::new();
    let mut retryable_429s = 0;
    let mut fetched = 0;
    let start = Instant::now();

    loop {
        let now_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        match step(&conn, &client, &bucket, &mut robots, now_unix)
            .await
            .unwrap()
        {
            StepOutcome::Empty => break,
            StepOutcome::RateLimited(wait) => tokio::time::sleep(wait).await,
            StepOutcome::Fetched { result, .. } => {
                fetched += 1;
                if matches!(
                    result,
                    FetchResult::Retrying {
                        status: Some(429),
                        ..
                    }
                ) {
                    retryable_429s += 1;
                }
            }
        }
    }

    let elapsed = start.elapsed();
    assert_eq!(
        retryable_429s, 0,
        "mirror should never rate-limit a polite crawl"
    );
    let expected_min = Duration::from_secs_f64(
        (fetched.max(config.burst as usize) - config.burst as usize) as f64 / config.rate,
    );
    assert!(
        elapsed >= expected_min.mul_f64(0.5),
        "elapsed {elapsed:?} suspiciously fast for {fetched} fetches at rate {}",
        config.rate
    );
}
