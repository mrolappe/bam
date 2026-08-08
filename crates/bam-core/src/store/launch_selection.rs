//! Launching a selection (P6.4, invariant I7's first bulk consumer):
//! sequential, continue-on-failure launches through the `Launcher` registry
//! (P6.1), gated by a size-threshold confirmation. Resolving a selection to
//! package ids is the caller's job (`Session::search_packages`) — same
//! division of labor as `summaries::run_batch`'s `package_ids`.

use std::io::Read;

use rusqlite::Connection;
use thiserror::Error;

use crate::blob::{BlobHash, BlobStore};
use crate::cancel::CancellationToken;
use crate::launch::{LaunchArchive, LaunchHandle, LaunchRequest, LauncherCaps, LauncherRegistry};
use crate::unpack::detect_format;

use super::tables::get_archive_hash;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LaunchSelectionError {
    #[error(
        "selection has {count} members, above the {threshold}-member confirmation \
         threshold — confirm before launching"
    )]
    ConfirmationRequired { count: usize, threshold: usize },
}

#[derive(Debug, Default)]
pub struct LaunchSelectionOutcome {
    /// Package ids launched, in order, paired with the handle that keeps
    /// their scratch directory alive until the caller drops it.
    pub launched: Vec<(i64, LaunchHandle)>,
    /// Package ids that failed to launch, with a human-readable reason —
    /// recorded, not fatal, so the rest of the batch still runs.
    pub failed: Vec<(i64, String)>,
    /// `true` if `cancel` fired before every member ran.
    pub cancelled: bool,
}

/// Reads only the leading bytes `unpack::detect_format` needs, not the whole
/// archive — a full-content read only happens once, inside whichever
/// `Launcher` actually extracts it.
fn resolve_archive(
    conn: &Connection,
    store: &impl BlobStore,
    package_id: i64,
) -> Result<LaunchArchive, String> {
    let hash = get_archive_hash(conn, package_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no cached archive for this package".to_string())?;
    let blob = BlobHash::from_hex(hash);
    let mut head = Vec::new();
    store
        .get(&blob)
        .map_err(|e| e.to_string())?
        .take(16)
        .read_to_end(&mut head)
        .map_err(|e| e.to_string())?;
    let format = detect_format(&head).map_err(|e| e.to_string())?;
    Ok(LaunchArchive { blob, format })
}

/// Launches every member of `package_ids` sequentially, continuing past a
/// per-member failure rather than aborting the batch. Once
/// `package_ids.len()` exceeds `threshold`, requires `confirmed: true` — the
/// caller is expected to have shown the count and gotten user confirmation
/// first, the same structural gate as `summaries::run_batch`'s cost
/// estimate. Checks `cancel` before each member, stopping cleanly and
/// reporting how many ran.
#[allow(clippy::too_many_arguments)]
pub fn launch_selection(
    conn: &Connection,
    store: &impl BlobStore,
    registry: &LauncherRegistry,
    override_id: Option<&str>,
    package_ids: &[i64],
    threshold: usize,
    confirmed: bool,
    cancel: &CancellationToken,
) -> Result<LaunchSelectionOutcome, LaunchSelectionError> {
    if package_ids.len() > threshold && !confirmed {
        return Err(LaunchSelectionError::ConfirmationRequired {
            count: package_ids.len(),
            threshold,
        });
    }

    let mut outcome = LaunchSelectionOutcome::default();
    for &package_id in package_ids {
        if cancel.is_cancelled() {
            outcome.cancelled = true;
            break;
        }
        let result = resolve_archive(conn, store, package_id).and_then(|archive| {
            let req = LaunchRequest {
                required: LauncherCaps::default(),
                archive: Some(archive),
            };
            let launcher = registry
                .select(&req, override_id)
                .map_err(|e| e.to_string())?;
            launcher.launch(&req).map_err(|e| e.to_string())
        });
        match result {
            Ok(handle) => outcome.launched.push((package_id, handle)),
            Err(e) => outcome.failed.push((package_id, e)),
        }
    }
    Ok(outcome)
}
