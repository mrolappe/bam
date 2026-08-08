//! Filesystem-backed [`BlobStore`]: BLAKE3-addressed files under
//! `<root>/aa/bb/<full-hash>` (two-level fanout). A `put` streams into a
//! temp file while hashing; the real hash name is only known once the read
//! completes, so an interrupted write can never leave anything under it —
//! the temp file is renamed into place only after a full, successful read.

use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{BlobError, BlobHash, BlobStore};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct FsBlobStore {
    root: PathBuf,
}

impl FsBlobStore {
    pub fn new(root: impl Into<PathBuf>) -> std::io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn path_for(&self, hash: &BlobHash) -> PathBuf {
        let hex = hash.as_str();
        self.root.join(&hex[0..2]).join(&hex[2..4]).join(hex)
    }

    fn temp_path(&self) -> PathBuf {
        let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        self.root.join(format!(".tmp-{}-{n}", std::process::id()))
    }

    fn write_temp(tmp: &Path, mut bytes: impl Read) -> Result<BlobHash, BlobError> {
        let mut hasher = blake3::Hasher::new();
        let mut file = fs::File::create(tmp)?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = bytes.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n])?;
        }
        file.flush()?;
        Ok(BlobHash::from_hex(hasher.finalize().to_hex().to_string()))
    }
}

impl BlobStore for FsBlobStore {
    fn put(&self, bytes: impl Read) -> Result<BlobHash, BlobError> {
        let tmp = self.temp_path();
        let hash = match Self::write_temp(&tmp, bytes) {
            Ok(hash) => hash,
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                return Err(e);
            }
        };

        let dest = self.path_for(&hash);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        if dest.exists() {
            // Identical content already cached under this hash — dedup by
            // construction, drop the redundant temp copy.
            fs::remove_file(&tmp)?;
        } else {
            fs::rename(&tmp, &dest)?;
        }
        Ok(hash)
    }

    fn get(&self, hash: &BlobHash) -> Result<impl Read, BlobError> {
        let path = self.path_for(hash);
        let bytes = fs::read(&path).map_err(|e| map_missing(e, hash))?;

        // ponytail: rehashes the whole blob on every get; fine at Aminet
        // archive sizes, revisit with streaming verify if that changes.
        let actual = BlobHash::from_hex(blake3::hash(&bytes).to_hex().to_string());
        if &actual != hash {
            return Err(BlobError::Corrupted {
                expected: hash.clone(),
                actual,
            });
        }
        Ok(Cursor::new(bytes))
    }

    fn remove(&self, hash: &BlobHash) -> Result<(), BlobError> {
        let path = self.path_for(hash);
        fs::remove_file(&path).map_err(|e| map_missing(e, hash))
    }
}

fn map_missing(e: std::io::Error, hash: &BlobHash) -> BlobError {
    if e.kind() == std::io::ErrorKind::NotFound {
        BlobError::NotFound(hash.clone())
    } else {
        BlobError::Io(e)
    }
}
