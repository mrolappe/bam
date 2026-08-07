//! `unar` backend (§4: archive extraction is out-of-process, via
//! `unar`/XADMaster, to avoid the licensing status of the classic
//! `unlzx.c`). Extracts into a private scratch directory first and only
//! moves files into `dest` on full success, so a failing or malicious
//! archive can never leave partial or escaped output behind.

use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{fs, io};

use crate::blob::{BlobHash, BlobStore};

use super::{ArchiveFormat, Availability, ExtractedFile, UnpackError, Unpacker};

static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct UnarUnpacker<S: BlobStore> {
    store: S,
}

impl<S: BlobStore> UnarUnpacker<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S: BlobStore> Unpacker for UnarUnpacker<S> {
    fn id(&self) -> &str {
        "unar"
    }

    fn handles(&self, format: ArchiveFormat) -> bool {
        matches!(format, ArchiveFormat::Lha | ArchiveFormat::Lzx)
    }

    fn probe(&self) -> Availability {
        for bin in ["unar", "lsar"] {
            if let Err(e) = Command::new(bin).arg("-v").output() {
                return Availability::Unavailable {
                    reason: format!(
                        "the `{bin}` binary was not found on PATH ({e}); install it, e.g. \
                         `brew install unar` on macOS or `apt install unar` on Debian/Ubuntu"
                    ),
                };
            }
        }
        Availability::Available
    }

    fn unpack(&self, blob: &BlobHash, dest: &Path) -> Result<Vec<ExtractedFile>, UnpackError> {
        let scratch = scratch_dir();
        fs::create_dir_all(&scratch)?;
        let result = unpack_into_scratch(&self.store, blob, dest, &scratch);
        let _ = fs::remove_dir_all(&scratch);
        result
    }
}

fn unpack_into_scratch<S: BlobStore>(
    store: &S,
    blob: &BlobHash,
    dest: &Path,
    scratch: &Path,
) -> Result<Vec<ExtractedFile>, UnpackError> {
    let mut reader = store.get(blob)?;
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    let archive_path = scratch.join("archive");
    fs::write(&archive_path, &bytes)?;

    reject_path_traversal(&list_entries(&archive_path)?)?;

    let extract_dir = scratch.join("out");
    fs::create_dir_all(&extract_dir)?;
    let output = Command::new("unar")
        .args(["-f", "-D", "-q", "-o"])
        .arg(&extract_dir)
        .arg(&archive_path)
        .output()?;
    if !output.status.success() {
        return Err(UnpackError::ExtractionFailed {
            message: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    fs::create_dir_all(dest)?;
    let mut files = Vec::new();
    move_all(&extract_dir, &extract_dir, dest, &mut files)?;
    Ok(files)
}

/// Lists archive member names via `lsar -json`, without extracting.
fn list_entries(archive_path: &Path) -> Result<Vec<String>, UnpackError> {
    let output = Command::new("lsar")
        .arg("-json")
        .arg(archive_path)
        .output()?;
    if !output.status.success() {
        return Err(UnpackError::ExtractionFailed {
            message: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| UnpackError::ExtractionFailed {
            message: format!("could not parse `lsar -json` output: {e}"),
        })?;
    let entries = json["lsarContents"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("XADFileName")?.as_str().map(str::to_string))
        .collect();
    Ok(entries)
}

fn reject_path_traversal(entries: &[String]) -> Result<(), UnpackError> {
    for entry in entries {
        if Path::new(entry)
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::RootDir))
        {
            return Err(UnpackError::PathTraversal {
                entry: entry.clone(),
            });
        }
    }
    Ok(())
}

/// Recursively moves every file under `dir` into `dest`, preserving its
/// path relative to `root`, and records it as an [`ExtractedFile`].
fn move_all(
    root: &Path,
    dir: &Path,
    dest: &Path,
    files: &mut Vec<ExtractedFile>,
) -> Result<(), UnpackError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            move_all(root, &path, dest, files)?;
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .expect("walked entry is under root")
            .to_path_buf();
        let size = entry.metadata()?.len();
        let target = dest.join(&rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        rename_or_copy(&path, &target)?;
        files.push(ExtractedFile { path: rel, size });
    }
    Ok(())
}

fn rename_or_copy(from: &Path, to: &Path) -> io::Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(from, to)?;
            Ok(())
        }
    }
}

fn scratch_dir() -> PathBuf {
    let n = SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("bam-unar-scratch-{}-{n}", std::process::id()))
}
