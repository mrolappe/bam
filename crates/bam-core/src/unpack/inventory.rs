//! Archive inventory extraction (P5.8): extract to a scratch directory,
//! collect the file list, discard the extraction. Storing the result as an
//! `enrichment` row is [`crate::store::inventory`]'s job — invariant I1
//! confines the SQL layer to `store::`, so this module never touches the DB.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::blob::BlobHash;

use super::{ArchiveFormat, ExtractedFile, UnpackError, UnpackerRegistry};

/// `enrichment.kind` this module's output is stored under.
pub const INVENTORY_KIND: &str = "inventory";
/// `enrichment.producer_version` for this module's output shape.
pub const INVENTORY_PRODUCER_VERSION: i64 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventoryEntry {
    pub path: String,
    pub size: u64,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inventory {
    pub files: Vec<InventoryEntry>,
}

/// Extracts `blob` via `registry` into a private scratch directory, and
/// always removes it afterwards, success or error — nothing under it
/// survives this call, only the returned [`Inventory`] does.
pub fn extract_inventory(
    registry: &UnpackerRegistry,
    blob: &BlobHash,
    format: ArchiveFormat,
    override_id: Option<&str>,
) -> Result<Inventory, UnpackError> {
    let unpacker = registry.select(format, override_id)?;
    let scratch = scratch_dir();
    let result = unpacker.unpack(blob, &scratch);
    let _ = fs::remove_dir_all(&scratch);
    let files = result?;
    Ok(Inventory {
        files: files.into_iter().map(to_entry).collect(),
    })
}

fn to_entry(f: ExtractedFile) -> InventoryEntry {
    InventoryEntry {
        kind: detect_kind(&f.path).to_string(),
        path: f.path.to_string_lossy().into_owned(),
        size: f.size,
    }
}

/// A coarse type guess from the extracted file's own extension. Unlike the
/// outer archive (§P5.3: Aminet `.lha` files routinely lie), a file's
/// extension inside the archive isn't adversarial input from Aminet's own
/// naming conventions, so this stays a plain extension map rather than a
/// magic-byte sniffer.
fn detect_kind(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("txt" | "doc" | "guide" | "readme") => "text",
        Some("iff" | "ilbm" | "lbm" | "png" | "jpg" | "jpeg" | "gif") => "image",
        Some("mod" | "8svx" | "wav" | "aiff") => "audio",
        Some("info") => "icon",
        Some("library" | "device") => "system",
        Some(_) => "other",
        None => "unknown",
    }
}

fn scratch_dir() -> std::path::PathBuf {
    static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("bam-inventory-scratch-{}-{n}", std::process::id()))
}
