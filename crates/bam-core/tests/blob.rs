use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use bam_core::blob::{BlobError, BlobHash, BlobStore, FsBlobStore};

fn temp_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("bam-blob-test-{label}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Yields `good` bytes, then errors — simulates a connection drop or a
/// truncated archive mid-fetch.
struct FailingReader<'a> {
    good: &'a [u8],
    pos: usize,
}

impl Read for FailingReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.good.len() {
            return Err(std::io::Error::other("simulated read failure"));
        }
        let n = buf.len().min(self.good.len() - self.pos);
        buf[..n].copy_from_slice(&self.good[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

fn count_blob_files(root: &std::path::Path) -> usize {
    fn walk(dir: &std::path::Path, count: &mut usize) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                walk(&path, count);
            } else if !path.file_name().unwrap().to_string_lossy().starts_with('.') {
                *count += 1;
            }
        }
    }
    let mut count = 0;
    walk(root, &mut count);
    count
}

#[test]
fn interrupted_write_leaves_no_file_under_a_real_hash_name() {
    let dir = temp_dir("interrupted");
    let store = FsBlobStore::new(&dir).unwrap();

    let reader = FailingReader {
        good: b"partial archive bytes",
        pos: 0,
    };
    let err = store.put(reader).unwrap_err();
    assert!(matches!(err, BlobError::Io(_)));

    assert_eq!(count_blob_files(&dir), 0);
}

#[test]
fn storing_identical_bytes_twice_yields_one_blob() {
    let dir = temp_dir("dedup");
    let store = FsBlobStore::new(&dir).unwrap();

    let hash1 = store.put(Cursor::new(b"same content".to_vec())).unwrap();
    let hash2 = store.put(Cursor::new(b"same content".to_vec())).unwrap();

    assert_eq!(hash1, hash2);
    assert_eq!(count_blob_files(&dir), 1);

    let mut got = Vec::new();
    store.get(&hash1).unwrap().read_to_end(&mut got).unwrap();
    assert_eq!(got, b"same content");
}

#[test]
fn corrupted_blob_is_detected_by_recomputing_its_hash() {
    let dir = temp_dir("corrupt");
    let store = FsBlobStore::new(&dir).unwrap();

    let hash = store.put(Cursor::new(b"original".to_vec())).unwrap();
    let hex = hash.as_str();
    let path = dir.join(&hex[0..2]).join(&hex[2..4]).join(hex);
    std::fs::write(&path, b"tampered").unwrap();

    match store.get(&hash) {
        Err(BlobError::Corrupted { .. }) => {}
        other => panic!("expected Corrupted, got {}", other.is_ok()),
    }
}

#[test]
fn get_on_missing_hash_errors_rather_than_panicking() {
    let dir = temp_dir("missing");
    let store = FsBlobStore::new(&dir).unwrap();

    let hash = BlobHash::from_hex("0".repeat(64));
    match store.get(&hash) {
        Err(BlobError::NotFound(_)) => {}
        other => panic!("expected NotFound, got {}", other.is_ok()),
    }
}
