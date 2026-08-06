//! P1.10 — `bam ingest` orchestration. Exercised at the `run_ingest` level
//! rather than by spawning the compiled binary: the CLI (`bam-tui`) is a
//! thin arg-parsing/rendering wrapper, and `--offline`/`--rebuild-normalized`
//! map 1:1 onto `IngestMode` variants tested here.

use bam_core::http::{HttpClient, HttpError, HttpRequest, HttpResponse};
use bam_core::progress::{OperationId, Outcome, ProgressEvent, ProgressSink};
use bam_core::store::ingest::{IngestMode, run_ingest};
use bam_core::store::land::land_lines;
use bam_core::store::{self};

const FETCHED_AT: &str = "2026-08-06T00:00:00Z";

/// Never actually called in the scenarios that use it — its purpose is to
/// panic if `run_ingest` ever reaches the network in a mode that promises
/// not to.
struct PanicOnCallClient;

impl HttpClient for PanicOnCallClient {
    async fn get(&self, _req: HttpRequest) -> Result<HttpResponse, HttpError> {
        panic!("HttpClient::get called in a mode that must not touch the network");
    }
}

#[derive(Default)]
struct RecordingSink(Vec<ProgressEvent>);

impl ProgressSink for RecordingSink {
    fn emit(&mut self, event: ProgressEvent) {
        self.0.push(event);
    }
}

#[tokio::test]
async fn recording_sink_captures_event_sequence_for_fixture_ingest() {
    let conn = store::open(":memory:").unwrap();
    let mut sink = RecordingSink::default();

    run_ingest(
        &conn,
        &PanicOnCallClient,
        &mut sink,
        IngestMode::Offline,
        FETCHED_AT,
    )
    .await
    .unwrap();

    let operation = OperationId(0);
    assert_eq!(
        sink.0,
        vec![
            ProgressEvent::Started {
                operation,
                total: Some(2),
            },
            ProgressEvent::Advanced { operation, done: 1 },
            ProgressEvent::Advanced { operation, done: 2 },
            ProgressEvent::Finished {
                operation,
                outcome: Outcome::Success,
            },
        ]
    );
}

#[test]
fn progress_event_round_trips_through_serde() {
    let events = [
        ProgressEvent::Started {
            operation: OperationId(7),
            total: Some(2),
        },
        ProgressEvent::Advanced {
            operation: OperationId(7),
            done: 1,
        },
        ProgressEvent::Finished {
            operation: OperationId(7),
            outcome: Outcome::Failed {
                message: "boom".into(),
            },
        },
    ];
    for event in events {
        let json = serde_json::to_string(&event).unwrap();
        let back: ProgressEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }
}

#[tokio::test]
async fn offline_populates_db_from_fixtures() {
    let conn = store::open(":memory:").unwrap();
    let mut sink = RecordingSink::default();

    let outcome = run_ingest(
        &conn,
        &PanicOnCallClient,
        &mut sink,
        IngestMode::Offline,
        FETCHED_AT,
    )
    .await
    .unwrap();

    assert!(outcome.package_count > 0);
    let package_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM package", [], |row| row.get(0))
        .unwrap();
    assert_eq!(package_count, outcome.package_count as i64);
}

#[tokio::test]
async fn rebuild_normalized_never_touches_the_network() {
    let conn = store::open(":memory:").unwrap();
    // Pre-seed landing directly, bypassing fetch, the way an earlier
    // `--offline` or real fetch run would have left the DB.
    let body = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/index_sample.txt"),
    )
    .unwrap();
    land_lines(&conn, "seed", FETCHED_AT, &body).unwrap();

    let mut sink = RecordingSink::default();
    let outcome = run_ingest(
        &conn,
        &PanicOnCallClient,
        &mut sink,
        IngestMode::RebuildNormalized,
        FETCHED_AT,
    )
    .await
    .unwrap();

    assert!(outcome.package_count > 0);
    let operation = OperationId(0);
    assert_eq!(
        sink.0,
        vec![
            ProgressEvent::Started {
                operation,
                total: Some(1),
            },
            ProgressEvent::Advanced { operation, done: 1 },
            ProgressEvent::Finished {
                operation,
                outcome: Outcome::Success,
            },
        ]
    );
}
