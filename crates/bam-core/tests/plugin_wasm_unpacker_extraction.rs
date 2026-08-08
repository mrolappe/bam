//! P8.4 — a WASM-backed unpacker doing real archive extraction, proving
//! I4's registry claim against a second, non-trivial implementation (P8.2's
//! echo-unpacker only ever bounced bytes back). Uses the fixture plugin
//! under `tests/fixtures/plugins/zip-unpacker/`: a genuine ZIP reader
//! compiled to WASM via the `zip` crate. `tests/fixtures/plugins/
//! unavailable-unpacker/` always reports itself unavailable, for the
//! probe-is-honoured test.

use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use bam_core::blob::{BlobStore, FsBlobStore};
use bam_core::plugin::WasmUnpacker;
use bam_core::unpack::{
    ArchiveFormat, Availability, ExtractedFile, UnpackError, Unpacker, UnpackerRegistry,
    detect_format,
};

fn temp_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "bam-wasm-zip-unpacker-test-{label}-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/plugins")
        .join(name)
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

fn sorted(mut files: Vec<ExtractedFile>) -> Vec<ExtractedFile> {
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

#[test]
fn wasm_unpacker_extracts_a_fixture_archive_correctly() {
    let dir = temp_dir("extract");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let bytes = build_zip(&[("a.txt", b"hello\n"), ("sub/b.txt", b"world\n")]);
    let hash = store.put(Cursor::new(bytes)).unwrap();

    let plugin = WasmUnpacker::load(&fixture_dir("zip-unpacker"), store).unwrap();
    let dest = dir.join("dest");
    let files = plugin.unpack(&hash, &dest).unwrap();

    assert_eq!(
        sorted(files),
        sorted(vec![
            ExtractedFile {
                path: PathBuf::from("a.txt"),
                size: 6,
            },
            ExtractedFile {
                path: PathBuf::from("sub/b.txt"),
                size: 6,
            },
        ])
    );
    assert_eq!(std::fs::read(dest.join("a.txt")).unwrap(), b"hello\n");
    assert_eq!(std::fs::read(dest.join("sub/b.txt")).unwrap(), b"world\n");
}

#[test]
fn wasm_unpacker_participates_in_magic_byte_format_routing() {
    let dir = temp_dir("routing");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let plugin = WasmUnpacker::load(&fixture_dir("zip-unpacker"), store).unwrap();

    let mut registry = UnpackerRegistry::new();
    registry.register(Box::new(plugin));

    let zip_bytes = build_zip(&[("a.txt", b"hi\n")]);
    let format = detect_format(&zip_bytes).unwrap();
    assert_eq!(format, ArchiveFormat::Zip);
    assert_eq!(registry.select(format, None).unwrap().id(), "zip-unpacker");
}

#[test]
fn wasm_unpacker_probe_is_honoured() {
    let dir = temp_dir("probe");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let plugin = WasmUnpacker::load(&fixture_dir("unavailable-unpacker"), store).unwrap();
    assert_eq!(
        plugin.probe(),
        Availability::Unavailable {
            reason: "deliberately unavailable, for testing".to_string()
        }
    );

    let mut registry = UnpackerRegistry::new();
    registry.register(Box::new(plugin));

    match registry.select(ArchiveFormat::Zip, None) {
        Err(UnpackError::NoAvailableUnpacker { .. }) => {}
        other => panic!("expected NoAvailableUnpacker, got {}", other.is_ok()),
    }
}

#[test]
fn path_traversal_entry_is_rejected_without_partial_extraction() {
    let dir = temp_dir("traversal");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let bytes = build_zip(&[("safe.txt", b"ok\n"), ("../evil.txt", b"pwned\n")]);
    let hash = store.put(Cursor::new(bytes)).unwrap();
    let plugin = WasmUnpacker::load(&fixture_dir("zip-unpacker"), store).unwrap();

    let dest = dir.join("dest");
    let err = plugin.unpack(&hash, &dest).unwrap_err();
    assert!(
        matches!(err, UnpackError::PathTraversal { .. }),
        "expected PathTraversal, got {err:?}"
    );
    assert!(!dest.exists() || std::fs::read_dir(&dest).unwrap().count() == 0);
}
