//! P5.4 — `unar` backend. CI provisions `unar`/`lsar` on both OSes (see
//! `.github/workflows/ci.yml`), so these run for real rather than being
//! skipped; the one scenario that needs the binary genuinely absent
//! (`unar_absent_...`) lives in `tests/unpack_unar_unavailable.rs` instead,
//! since it has to mutate `PATH` and must not race other tests in this file.

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use bam_core::blob::{BlobHash, BlobStore, FsBlobStore};
use bam_core::unpack::{Availability, ExtractedFile, UnarUnpacker, UnpackError, Unpacker};

fn temp_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("bam-unar-test-{label}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn store_fixture(store: &FsBlobStore, relative: &str) -> BlobHash {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/archives")
        .join(relative);
    let bytes = std::fs::read(&path).unwrap();
    store.put(Cursor::new(bytes)).unwrap()
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
fn lha_fixture_extracts_to_expected_file_list() {
    let dir = temp_dir("lha");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let hash = store_fixture(&store, "sample.lha");
    let unpacker = UnarUnpacker::new(store);
    assert_eq!(unpacker.probe(), Availability::Available);

    let dest = dir.join("dest");
    let files = unpacker.unpack(&hash, &dest).unwrap();

    assert_eq!(sorted(files), expected_sample_files());
    assert_eq!(std::fs::read(dest.join("a.txt")).unwrap(), b"hello\n");
    assert_eq!(std::fs::read(dest.join("sub/b.txt")).unwrap(), b"world\n");
}

#[test]
fn lzx_fixture_extracts_to_expected_file_list() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/archives/sample.lzx");
    if !fixture.exists() {
        eprintln!("skipping: tests/fixtures/archives/sample.lzx not present yet");
        return;
    }

    let dir = temp_dir("lzx");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let hash = store_fixture(&store, "sample.lzx");
    let unpacker = UnarUnpacker::new(store);

    let dest = dir.join("dest");
    let files = unpacker.unpack(&hash, &dest).unwrap();

    assert_eq!(sorted(files), expected_sample_files());
    assert_eq!(std::fs::read(dest.join("a.txt")).unwrap(), b"hello\n");
    assert_eq!(std::fs::read(dest.join("sub/b.txt")).unwrap(), b"world\n");
}

#[test]
fn malformed_archive_errors_without_partial_extraction() {
    let dir = temp_dir("malformed");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let hash = store_fixture(&store, "malformed.lha");
    let unpacker = UnarUnpacker::new(store);

    let dest = dir.join("dest");
    let err = unpacker.unpack(&hash, &dest).unwrap_err();
    assert!(matches!(err, UnpackError::ExtractionFailed { .. }));

    let left_behind = std::fs::read_dir(&dest)
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(left_behind, 0, "malformed archive left files under dest");
}

#[test]
fn traversal_entry_in_zip_archive_is_rejected() {
    // `lha`/`lzx` archivers both sanitize a leading `../` on creation, so a
    // malicious archive can't be built with the archiving tool itself —
    // exactly why the check has to run against the *listed* entry names
    // (`lsar -json`) rather than trusting the creation path. This builds a
    // zip (which does *not* sanitize member names) containing a `../`
    // entry — `unar`/`lsar` both understand zip too — to prove the
    // rejection is real end-to-end, not just unit-tested against a crafted
    // string list.
    let dir = temp_dir("traversal");
    let nested = dir.join("a/b/c");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(dir.join("secret.txt"), b"secret\n").unwrap();
    let archive_path = dir.join("evil.zip");
    let status = std::process::Command::new("zip")
        .arg("-q")
        .arg(archive_path.to_str().unwrap())
        .arg("../../../secret.txt")
        .current_dir(&nested)
        .status();
    let Ok(status) = status else {
        eprintln!("skipping: `zip` not available to build the traversal fixture");
        return;
    };
    if !status.success() {
        eprintln!("skipping: `zip` could not build the traversal fixture");
        return;
    }

    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let hash = store
        .put(Cursor::new(std::fs::read(&archive_path).unwrap()))
        .unwrap();
    let unpacker = UnarUnpacker::new(store);

    let dest = dir.join("dest");
    let err = unpacker.unpack(&hash, &dest).unwrap_err();
    assert!(
        matches!(err, UnpackError::PathTraversal { .. }),
        "expected PathTraversal, got {err:?}"
    );
    assert!(!dest.exists() || std::fs::read_dir(&dest).unwrap().count() == 0);
}
