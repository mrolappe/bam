//! P5.8 — archive inventory enrichment.

use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use bam_core::blob::{BlobStore, FsBlobStore};
use bam_core::store::blob_cache::{evict_to_budget, record_blob};
use bam_core::store::inventory::{InventoryOutcome, enrich_inventory};
use bam_core::store::tables::{
    Enrichment, LandingIndexLine, Package, get_enrichment, insert_landing_index_line,
    insert_package, set_archive_hash, upsert_enrichment,
};
use bam_core::store::{self};
use bam_core::unpack::{ArchiveFormat, Inventory, UnpackerRegistry, ZipUnpacker, detect_format};
use rusqlite::Connection;

fn temp_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "bam-inventory-test-{label}-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut writer = zip::ZipWriter::new(Cursor::new(&mut buf));
    let options: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
    buf
}

fn fresh_db() -> Connection {
    store::open(":memory:").unwrap()
}

/// Inserts a package and a real, DB-tracked blob for it in one step,
/// mirroring `tests/store_blob_cache.rs`'s helper of the same name.
fn seed(conn: &Connection, store: &FsBlobStore, file: &str, contents: &[u8]) -> i64 {
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
    record_blob(conn, &hash, contents.len() as i64, "2026-08-07T00:00:00Z").unwrap();
    set_archive_hash(conn, package_id, Some(hash.as_str())).unwrap();

    package_id
}

/// `scratch_directory_is_removed_including_on_error` scans the shared system
/// temp dir for leftover `bam-inventory-scratch-*` entries, which would
/// otherwise race every other test in this file that transiently creates and
/// removes one of its own via `enrich_inventory`. All four tests take this
/// lock so the scan never observes another test's in-flight scratch dir.
static SCRATCH_LOCK: Mutex<()> = Mutex::new(());

fn scratch_dirs() -> Vec<PathBuf> {
    std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("bam-inventory-scratch-"))
        })
        .collect()
}

#[test]
fn inventory_matches_actual_archive_contents() {
    let _guard = SCRATCH_LOCK.lock().unwrap();
    let dir = temp_dir("contents");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let bytes = build_zip(&[("a.txt", b"hello\n"), ("sub/b.txt", b"world!\n")]);
    let hash = store.put(Cursor::new(&bytes)).unwrap();
    let format = detect_format(&bytes).unwrap();

    let conn = fresh_db();
    let package_id = seed(&conn, &store, "Foo.zip", &bytes);

    let mut registry = UnpackerRegistry::new();
    registry.register(Box::new(ZipUnpacker::new(store)));

    let outcome = enrich_inventory(&conn, &registry, package_id, &hash, format, None).unwrap();
    assert_eq!(outcome, InventoryOutcome::Written);

    let row = get_enrichment(&conn, package_id, "inventory").unwrap();
    let inventory: Inventory = serde_json::from_str(&row.payload).unwrap();
    let mut files = inventory.files;
    files.sort_by(|a, b| a.path.cmp(&b.path));

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].path, "a.txt");
    assert_eq!(files[0].size, 6);
    assert_eq!(files[0].kind, "text");
    assert_eq!(files[1].path, "sub/b.txt");
    assert_eq!(files[1].size, 7);
}

#[test]
fn scratch_directory_is_removed_including_on_error() {
    let _guard = SCRATCH_LOCK.lock().unwrap();
    let dir = temp_dir("cleanup");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    // Not a valid zip — forces `unpack()` to error before extracting.
    let hash = store
        .put(Cursor::new(b"not a zip archive".to_vec()))
        .unwrap();

    let conn = fresh_db();
    let package_id = seed(&conn, &store, "Bad.zip", b"not a zip archive");

    let mut registry = UnpackerRegistry::new();
    registry.register(Box::new(ZipUnpacker::new(store)));

    let before = scratch_dirs();
    let err = enrich_inventory(
        &conn,
        &registry,
        package_id,
        &hash,
        ArchiveFormat::Zip,
        None,
    )
    .unwrap_err();
    assert!(matches!(
        err,
        bam_core::store::inventory::InventoryError::Unpack(_)
    ));
    assert_eq!(
        scratch_dirs(),
        before,
        "scratch dir must not be left behind"
    );
}

#[test]
fn inventory_survives_blob_eviction() {
    let _guard = SCRATCH_LOCK.lock().unwrap();
    let dir = temp_dir("eviction");
    let blobs_dir = dir.join("blobs");
    let store = FsBlobStore::new(&blobs_dir).unwrap();
    let bytes = build_zip(&[("a.txt", b"hi\n")]);
    let hash = store.put(Cursor::new(&bytes)).unwrap();
    let format = detect_format(&bytes).unwrap();

    let conn = fresh_db();
    let package_id = seed(&conn, &store, "Foo.zip", &bytes);

    let mut registry = UnpackerRegistry::new();
    registry.register(Box::new(ZipUnpacker::new(
        FsBlobStore::new(&blobs_dir).unwrap(),
    )));

    enrich_inventory(&conn, &registry, package_id, &hash, format, None).unwrap();

    evict_to_budget(&conn, &store, 0).unwrap();

    let row = get_enrichment(&conn, package_id, "inventory").unwrap();
    let inventory: Inventory = serde_json::from_str(&row.payload).unwrap();
    assert_eq!(inventory.files.len(), 1);
}

#[test]
fn same_producer_version_is_a_no_op_bumping_reprocesses() {
    let _guard = SCRATCH_LOCK.lock().unwrap();
    let dir = temp_dir("reprocess");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let bytes = build_zip(&[("a.txt", b"hi\n")]);
    let hash = store.put(Cursor::new(&bytes)).unwrap();
    let format = detect_format(&bytes).unwrap();

    let conn = fresh_db();
    let package_id = seed(&conn, &store, "Foo.zip", &bytes);

    // Simulate a stale inventory from an older producer version.
    upsert_enrichment(
        &conn,
        &Enrichment {
            package_id,
            kind: "inventory".into(),
            producer_version: 0,
            produced_at: "2026-08-01T00:00:00Z".into(),
            payload: r#"{"files":[]}"#.into(),
        },
    )
    .unwrap();

    let mut registry = UnpackerRegistry::new();
    registry.register(Box::new(ZipUnpacker::new(store)));

    let reprocessed = enrich_inventory(&conn, &registry, package_id, &hash, format, None).unwrap();
    assert_eq!(reprocessed, InventoryOutcome::Written);
    let row = get_enrichment(&conn, package_id, "inventory").unwrap();
    assert_eq!(row.producer_version, 1);

    let no_op = enrich_inventory(&conn, &registry, package_id, &hash, format, None).unwrap();
    assert_eq!(no_op, InventoryOutcome::UpToDate);
}
