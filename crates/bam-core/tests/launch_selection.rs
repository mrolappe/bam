//! P6.4: launching a selection through the `Launcher` registry.

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use bam_core::blob::{BlobHash, BlobStore, FsBlobStore};
use bam_core::cancel::CancellationToken;
use bam_core::launch::{
    Availability, LaunchHandle, LaunchRequest, Launcher, LauncherCaps, LauncherError,
    LauncherRegistry,
};
use bam_core::store::launch_selection::{LaunchSelectionError, launch_selection};
use bam_core::store::tables::{self, LandingIndexLine, Package};

fn temp_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "bam-launch-selection-test-{label}-{}-{n}",
        std::process::id()
    ))
}

/// Seeds `n` packages, each with its own cached archive blob (zip magic
/// bytes so `detect_format` succeeds) — every package except those listed in
/// `uncached` gets an `archive_hash`.
fn seed(
    conn: &rusqlite::Connection,
    store: &FsBlobStore,
    n: usize,
    uncached: &[usize],
) -> Vec<i64> {
    let landing_id = tables::insert_landing_index_line(
        conn,
        &LandingIndexLine {
            id: 0,
            fetched_at: "2026-01-01T00:00:00Z".into(),
            source_url: "test://fixture".into(),
            line_no: 1,
            raw: vec![],
        },
    )
    .unwrap();
    (0..n)
        .map(|i| {
            let id = tables::insert_package(
                conn,
                &Package {
                    id: 0,
                    dir: "dir".into(),
                    file: format!("pkg{i}.zip"),
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
            if !uncached.contains(&i) {
                let mut bytes = b"PK\x03\x04".to_vec();
                bytes.extend_from_slice(format!("payload-{i}").as_bytes());
                let hash = store.put(Cursor::new(bytes.clone())).unwrap();
                bam_core::store::blob_cache::record_blob(
                    conn,
                    &hash,
                    bytes.len() as i64,
                    "2026-01-01T00:00:00Z",
                )
                .unwrap();
                tables::set_archive_hash(conn, id, Some(hash.as_str())).unwrap();
            }
            id
        })
        .collect()
}

struct FakeLauncher {
    calls: Arc<Mutex<Vec<BlobHash>>>,
    cancel_on: Option<(BlobHash, CancellationToken)>,
}

impl Launcher for FakeLauncher {
    fn id(&self) -> &str {
        "fake"
    }

    fn probe(&self) -> Availability {
        Availability::Available
    }

    fn capabilities(&self) -> LauncherCaps {
        LauncherCaps::default()
    }

    fn launch(&self, req: &LaunchRequest) -> Result<LaunchHandle, LauncherError> {
        let blob = req.archive.as_ref().unwrap().blob.clone();
        self.calls.lock().unwrap().push(blob.clone());
        if let Some((trigger, cancel)) = &self.cancel_on {
            if *trigger == blob {
                cancel.cancel();
            }
        }
        Ok(LaunchHandle {
            launcher_id: self.id().to_string(),
            scratch_dir: None,
        })
    }
}

fn registry(calls: Arc<Mutex<Vec<BlobHash>>>) -> LauncherRegistry {
    let mut reg = LauncherRegistry::new();
    reg.register(Box::new(FakeLauncher {
        calls,
        cancel_on: None,
    }));
    reg
}

#[test]
fn three_member_selection_launches_in_order() {
    let db = temp_path("order");
    let blobs = temp_path("order-blobs");
    let conn = bam_core::store::open(&db).unwrap();
    let store = FsBlobStore::new(&blobs).unwrap();
    let ids = seed(&conn, &store, 3, &[]);

    let calls = Arc::new(Mutex::new(Vec::new()));
    let reg = registry(calls.clone());

    let outcome = launch_selection(
        &conn,
        &store,
        &reg,
        None,
        &ids,
        10,
        false,
        &CancellationToken::new(),
    )
    .unwrap();

    assert_eq!(
        outcome
            .launched
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
        ids
    );
    assert!(outcome.failed.is_empty());
    assert!(!outcome.cancelled);
    assert_eq!(calls.lock().unwrap().len(), 3);
}

#[test]
fn selection_above_threshold_requires_confirmation() {
    let db = temp_path("threshold");
    let blobs = temp_path("threshold-blobs");
    let conn = bam_core::store::open(&db).unwrap();
    let store = FsBlobStore::new(&blobs).unwrap();
    let ids = seed(&conn, &store, 3, &[]);

    let reg = registry(Arc::new(Mutex::new(Vec::new())));

    let err = launch_selection(
        &conn,
        &store,
        &reg,
        None,
        &ids,
        2,
        false,
        &CancellationToken::new(),
    )
    .unwrap_err();
    assert_eq!(
        err,
        LaunchSelectionError::ConfirmationRequired {
            count: 3,
            threshold: 2
        }
    );

    // Confirmed, it proceeds.
    let outcome = launch_selection(
        &conn,
        &store,
        &reg,
        None,
        &ids,
        2,
        true,
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(outcome.launched.len(), 3);
}

#[test]
fn failure_on_member_two_is_reported_and_batch_continues() {
    let db = temp_path("failure");
    let blobs = temp_path("failure-blobs");
    let conn = bam_core::store::open(&db).unwrap();
    let store = FsBlobStore::new(&blobs).unwrap();
    let ids = seed(&conn, &store, 3, &[1]); // member index 1 (second) has no cached archive

    let reg = registry(Arc::new(Mutex::new(Vec::new())));

    let outcome = launch_selection(
        &conn,
        &store,
        &reg,
        None,
        &ids,
        10,
        false,
        &CancellationToken::new(),
    )
    .unwrap();

    assert_eq!(
        outcome
            .launched
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
        vec![ids[0], ids[2]]
    );
    assert_eq!(outcome.failed.len(), 1);
    assert_eq!(outcome.failed[0].0, ids[1]);
    assert!(!outcome.cancelled);
}

#[test]
fn cancelling_mid_batch_stops_cleanly_and_reports_how_many_ran() {
    let db = temp_path("cancel");
    let blobs = temp_path("cancel-blobs");
    let conn = bam_core::store::open(&db).unwrap();
    let store = FsBlobStore::new(&blobs).unwrap();
    let ids = seed(&conn, &store, 3, &[]);

    let first_hash = BlobHash::from_hex(tables::get_archive_hash(&conn, ids[0]).unwrap().unwrap());
    let cancel = CancellationToken::new();

    let mut reg = LauncherRegistry::new();
    reg.register(Box::new(FakeLauncher {
        calls: Arc::new(Mutex::new(Vec::new())),
        cancel_on: Some((first_hash, cancel.clone())),
    }));

    let outcome = launch_selection(&conn, &store, &reg, None, &ids, 10, false, &cancel).unwrap();

    assert_eq!(outcome.launched.len(), 1);
    assert_eq!(outcome.launched[0].0, ids[0]);
    assert!(outcome.cancelled);
    assert!(outcome.failed.is_empty());
}
