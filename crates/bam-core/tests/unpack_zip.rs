//! P5.5 — `zip` backend.

use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use bam_core::blob::{BlobStore, FsBlobStore};
use bam_core::unpack::{
    Availability, ExtractedFile, UnarUnpacker, UnpackError, Unpacker, UnpackerRegistry,
    ZipUnpacker, detect_format,
};

fn temp_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bam-zip-test-{label}-{}-{n}", std::process::id()));
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

fn expected_sample_files() -> Vec<ExtractedFile> {
    let mut files = vec![
        ExtractedFile {
            path: PathBuf::from("a.txt"),
            size: 6,
        },
        ExtractedFile {
            path: PathBuf::from("sub/b.txt"),
            size: 6,
        },
    ];
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

fn sorted(mut files: Vec<ExtractedFile>) -> Vec<ExtractedFile> {
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

#[test]
fn zip_fixture_extracts_to_expected_file_list() {
    let dir = temp_dir("sample");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let bytes = build_zip(&[("a.txt", b"hello\n"), ("sub/b.txt", b"world\n")]);
    let hash = store.put(Cursor::new(bytes)).unwrap();
    let unpacker = ZipUnpacker::new(store);
    assert_eq!(unpacker.probe(), Availability::Available);

    let dest = dir.join("dest");
    let files = unpacker.unpack(&hash, &dest).unwrap();

    assert_eq!(sorted(files), expected_sample_files());
    assert_eq!(std::fs::read(dest.join("a.txt")).unwrap(), b"hello\n");
    assert_eq!(std::fs::read(dest.join("sub/b.txt")).unwrap(), b"world\n");
}

#[test]
fn probe_always_reports_available() {
    let dir = temp_dir("probe");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    assert_eq!(ZipUnpacker::new(store).probe(), Availability::Available);
}

#[test]
fn registry_routes_zip_to_zip_and_lha_to_unar() {
    let dir = temp_dir("registry");
    let blobs = dir.join("blobs");

    let mut registry = UnpackerRegistry::new();
    registry.register(Box::new(ZipUnpacker::new(
        FsBlobStore::new(blobs.clone()).unwrap(),
    )));
    registry.register(Box::new(UnarUnpacker::new(
        FsBlobStore::new(blobs).unwrap(),
    )));

    let zip_bytes = build_zip(&[("a.txt", b"hi\n")]);
    let zip_format = detect_format(&zip_bytes).unwrap();
    assert_eq!(registry.select(zip_format, None).unwrap().id(), "zip");

    let lha_bytes = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/archives/sample.lha"),
    )
    .unwrap();
    let lha_format = detect_format(&lha_bytes).unwrap();
    assert_eq!(registry.select(lha_format, None).unwrap().id(), "unar");
}

#[test]
fn traversal_entry_is_rejected_without_partial_extraction() {
    let dir = temp_dir("traversal");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let bytes = build_zip(&[("safe.txt", b"ok\n"), ("../evil.txt", b"pwned\n")]);
    let hash = store.put(Cursor::new(bytes)).unwrap();
    let unpacker = ZipUnpacker::new(store);

    let dest = dir.join("dest");
    let err = unpacker.unpack(&hash, &dest).unwrap_err();
    assert!(
        matches!(err, UnpackError::PathTraversal { .. }),
        "expected PathTraversal, got {err:?}"
    );
    assert!(!dest.exists() || std::fs::read_dir(&dest).unwrap().count() == 0);
}
