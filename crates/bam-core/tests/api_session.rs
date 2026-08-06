//! P2.6's five test groups, against [`bam_core::api`] and its underlying
//! [`bam_core::store::session::Session`]. The purity/no-stdout bullet is
//! `tests/purity.rs`, re-verified green rather than duplicated here (same
//! convention P1.10's own fifth bullet used, Round 6).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use bam_core::api::types::{
    GetPackageRequest, GetPackageResponse, ListCategoriesResponse, ListSelectionsResponse,
    LoadRequest, OperationStatusRequest, OperationStatusResponse, SaveAsRequest,
    SearchPackagesRequest, SearchPackagesResponse, SelectByQueryRequest, SelectByQueryResponse,
};
use bam_core::api::{self, CancellationToken, OperationStatus};
use bam_core::http::{HttpClient, HttpError, HttpRequest, HttpResponse};
use bam_core::progress::{OperationId, ProgressEvent, ProgressSink};
use bam_core::query::ir::{FieldId, Predicate};
use bam_core::store::ingest::IngestMode;
use bam_core::store::session::{SelectionMode, Session};
use bam_core::store::tables::{self, LandingIndexLine, Package};

/// A unique on-disk path per test: an in-memory DB is a fresh, private
/// database per `Connection::open` call, which can't demonstrate "two
/// sessions share a database but not each other's session state" — that
/// needs both sessions actually pointed at the same file.
fn temp_db_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "bam-api-session-test-{label}-{}-{n}.sqlite",
        std::process::id()
    ))
}

/// Inserts one package with `dir` set (searchable), returning its id.
fn seed_package(session_path: &std::path::Path, dir: &str) -> i64 {
    let conn = bam_core::store::open(session_path).unwrap();
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

fn dir_predicate(dir: &str) -> Predicate {
    Predicate::Compare {
        field: FieldId::new("dir"),
        op: bam_core::query::ir::CmpOp::Eq,
        value: bam_core::query::ir::Value::Text(dir.to_string()),
    }
}

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

/// Every request/response type round-trips serde and produces a JSON
/// schema (invariant I5 rule 3).
#[test]
fn request_response_types_round_trip_and_have_a_schema() {
    fn check<T>(value: T)
    where
        T: Clone
            + PartialEq
            + std::fmt::Debug
            + serde::Serialize
            + serde::de::DeserializeOwned
            + schemars::JsonSchema,
    {
        let json = serde_json::to_string(&value).unwrap();
        let back: T = serde_json::from_str(&json).unwrap();
        assert_eq!(value, back);
        // Schema generation itself is the check: a type with a field that
        // doesn't implement `JsonSchema` fails to compile, not to run — so
        // this call just proves one exists and doesn't panic building it.
        let _schema = schemars::r#gen::SchemaGenerator::default().into_root_schema_for::<T>();
    }

    check(SearchPackagesRequest {
        predicate: dir_predicate("mods"),
    });
    check(SearchPackagesResponse { packages: vec![] });
    check(GetPackageRequest { id: 1 });
    check(GetPackageResponse { package: None });
    check(ListCategoriesResponse {
        categories: vec!["mods".into()],
    });
    check(SelectByQueryRequest {
        predicate: dir_predicate("mods"),
        mode: SelectionMode::Replace,
    });
    check(SelectByQueryResponse { member_count: 3 });
    check(SaveAsRequest {
        name: "tracker candidates".into(),
    });
    check(LoadRequest {
        name: "tracker candidates".into(),
    });
    check(ListSelectionsResponse { selections: vec![] });
    check(OperationStatusRequest {
        operation: OperationId(1),
    });
    check(OperationStatusResponse {
        status: Some(OperationStatus::Cancelled),
    });
}

/// Two sessions, same underlying database, do not observe each other's
/// working selection or operation table.
#[test]
fn two_sessions_do_not_observe_each_others_state() {
    let path = temp_db_path("isolation");
    let id = seed_package(&path, "mods/tracker");

    let a = Session::open(&path).unwrap();
    let b = Session::open(&path).unwrap();

    a.mark(id).unwrap();
    assert!(a.is_marked(id).unwrap());
    assert!(!b.is_marked(id).unwrap(), "b must not see a's mark");

    drop(a);
    drop(b);
    let _ = std::fs::remove_file(&path);
}

/// A cancelled operation stops within a bounded time and reports
/// cancellation rather than an error.
#[tokio::test]
async fn cancelled_ingest_reports_cancellation_not_an_error() {
    let session = Session::in_memory().unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let mut sink = RecordingSink::default();

    let operation = api::start_ingest(
        &session,
        &PanicOnCallClient,
        &api::StartIngestRequest {
            mode: IngestMode::Offline,
        },
        &cancel,
        &mut sink,
    )
    .await
    .expect("cancellation is reported as a status, not an Err");

    assert!(
        sink.0.is_empty(),
        "a pre-cancelled run must not touch the network path nor emit progress"
    );
    let status = api::operation_status(&session, &OperationStatusRequest { operation });
    assert_eq!(
        status,
        OperationStatusResponse {
            status: Some(OperationStatus::Cancelled),
        }
    );
}

/// An `OperationId` returned by a long call can be used to query its
/// status, including after the call has returned.
#[tokio::test]
async fn operation_id_from_a_long_call_queries_its_status() {
    let session = Session::in_memory().unwrap();
    let cancel = CancellationToken::new();
    let mut sink = RecordingSink::default();

    let operation = api::start_ingest(
        &session,
        &PanicOnCallClient,
        &api::StartIngestRequest {
            mode: IngestMode::Offline,
        },
        &cancel,
        &mut sink,
    )
    .await
    .unwrap();

    let status = api::operation_status(&session, &OperationStatusRequest { operation });
    match status.status {
        Some(OperationStatus::Finished(bam_core::progress::Outcome::Success)) => {}
        other => panic!("expected a finished-success status, got {other:?}"),
    }

    // A second, unrelated session must not see the first one's operation.
    let other = Session::in_memory().unwrap();
    let status = api::operation_status(&other, &OperationStatusRequest { operation });
    assert_eq!(status, OperationStatusResponse { status: None });
}

/// `search_packages`/`get_package`/`list_categories` (P2.6's three named
/// use cases) against a seeded database, exercised through `api::`.
#[tokio::test]
async fn search_get_and_list_categories_work_through_the_api() {
    let path = temp_db_path("query");
    let id = seed_package(&path, "mods/tracker");
    let session = Session::open(&path).unwrap();

    let found = api::search_packages(
        &session,
        &SearchPackagesRequest {
            predicate: dir_predicate("mods/tracker"),
        },
    )
    .unwrap();
    assert_eq!(found.packages.len(), 1);
    assert_eq!(found.packages[0].id, id);

    let got = api::get_package(&session, &GetPackageRequest { id }).unwrap();
    assert_eq!(got.package.map(|p| p.id), Some(id));

    let missing = api::get_package(&session, &GetPackageRequest { id: id + 999 }).unwrap();
    assert_eq!(missing.package, None);

    let categories = api::list_categories(&session).unwrap();
    assert_eq!(categories.categories, vec!["mods/tracker".to_string()]);

    drop(session);
    let _ = std::fs::remove_file(&path);
}
