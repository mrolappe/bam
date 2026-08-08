//! Archive inventory enrichment (P5.8): the DB half of
//! [`crate::unpack::extract_inventory`] — checks whether an up-to-date
//! inventory already exists, and if not, stores the freshly extracted one
//! as an `enrichment` row. Kept in `store::` per invariant I1, which
//! confines `rusqlite` to this module tree.

use rusqlite::Connection;
use thiserror::Error;

use crate::blob::BlobHash;
use crate::unpack::{
    ArchiveFormat, INVENTORY_KIND, INVENTORY_PRODUCER_VERSION, Inventory, UnpackError,
    UnpackerRegistry, extract_inventory,
};

use super::tables::{Enrichment, get_enrichment, upsert_enrichment};

#[derive(Debug, Error)]
pub enum InventoryError {
    #[error("unpack error: {0}")]
    Unpack(#[from] UnpackError),
    #[error("db error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("payload serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryOutcome {
    /// Extracted and (re)wrote the enrichment row.
    Written,
    /// An up-to-date inventory already existed; nothing was extracted.
    UpToDate,
}

/// Extracts `blob` via `registry`, records its file list into `enrichment`.
/// A no-op if an inventory at the current [`INVENTORY_PRODUCER_VERSION`]
/// already exists for `package_id`.
pub fn enrich_inventory(
    conn: &Connection,
    registry: &UnpackerRegistry,
    package_id: i64,
    blob: &BlobHash,
    format: ArchiveFormat,
    override_id: Option<&str>,
) -> Result<InventoryOutcome, InventoryError> {
    if is_up_to_date(conn, package_id)? {
        return Ok(InventoryOutcome::UpToDate);
    }

    let inventory: Inventory = extract_inventory(registry, blob, format, override_id)?;
    let payload = serde_json::to_string(&inventory)?;
    upsert_enrichment(
        conn,
        &Enrichment {
            package_id,
            kind: INVENTORY_KIND.to_string(),
            producer_version: INVENTORY_PRODUCER_VERSION,
            produced_at: crate::now_rfc3339(),
            payload,
        },
    )?;
    Ok(InventoryOutcome::Written)
}

fn is_up_to_date(conn: &Connection, package_id: i64) -> Result<bool, InventoryError> {
    match get_enrichment(conn, package_id, INVENTORY_KIND) {
        Ok(row) => Ok(row.producer_version == INVENTORY_PRODUCER_VERSION),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
        Err(e) => Err(e.into()),
    }
}
