//! LRU eviction over the `blobs` table (P5.2, §6). Hard invariant: eviction
//! only ever removes blob bytes and clears `package.archive_hash` — it must
//! never touch `enrichment` or delete a `package` row, otherwise a re-fetch
//! would mean paying for LLM summarisation twice.

use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;

use crate::blob::{BlobError, BlobHash, BlobStore};

/// Registers a freshly-stored blob (or bumps an existing one's recency).
pub fn record_blob(
    conn: &Connection,
    hash: &BlobHash,
    size: i64,
    now: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO blobs (hash, size, last_used, pinned) VALUES (?1, ?2, ?3, 0)
         ON CONFLICT(hash) DO UPDATE SET last_used = excluded.last_used",
        params![hash.as_str(), size, now],
    )?;
    Ok(())
}

pub fn touch(conn: &Connection, hash: &BlobHash, now: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE blobs SET last_used = ?2 WHERE hash = ?1",
        params![hash.as_str(), now],
    )?;
    Ok(())
}

pub fn set_pinned(conn: &Connection, hash: &BlobHash, pinned: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE blobs SET pinned = ?2 WHERE hash = ?1",
        params![hash.as_str(), pinned],
    )?;
    Ok(())
}

#[derive(Debug, Default, PartialEq)]
pub struct EvictionReport {
    /// Oldest-evicted-first.
    pub evicted: Vec<BlobHash>,
    pub freed_bytes: i64,
}

#[derive(Debug, Error)]
pub enum EvictionError {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Blob(#[from] BlobError),
    #[error(
        "cannot evict down to {budget_bytes} bytes: {remaining_bytes} bytes remain, all in pinned blobs"
    )]
    BudgetNotMet {
        budget_bytes: i64,
        remaining_bytes: i64,
    },
}

/// Evicts least-recently-used, unpinned blobs until total size is within
/// `budget_bytes`. Each eviction removes the blob's bytes via `store`,
/// deletes its `blobs` row, and clears `archive_hash` on every `package` row
/// that referenced it — `enrichment` and `package` rows themselves are never
/// touched. Errors with [`EvictionError::BudgetNotMet`], evicting nothing
/// further, if unpinned blobs run out before the budget is met.
pub fn evict_to_budget<B: BlobStore>(
    conn: &Connection,
    store: &B,
    budget_bytes: i64,
) -> Result<EvictionReport, EvictionError> {
    let mut report = EvictionReport::default();
    loop {
        let total: i64 =
            conn.query_row("SELECT COALESCE(SUM(size), 0) FROM blobs", [], |r| r.get(0))?;
        if total <= budget_bytes {
            return Ok(report);
        }

        let next: Option<(String, i64)> = conn
            .query_row(
                "SELECT hash, size FROM blobs WHERE pinned = 0 ORDER BY last_used ASC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((hash_str, size)) = next else {
            return Err(EvictionError::BudgetNotMet {
                budget_bytes,
                remaining_bytes: total - budget_bytes,
            });
        };

        let hash = BlobHash::from_hex(hash_str);
        store.remove(&hash)?;
        conn.execute(
            "UPDATE package SET archive_hash = NULL WHERE archive_hash = ?1",
            params![hash.as_str()],
        )?;
        conn.execute("DELETE FROM blobs WHERE hash = ?1", params![hash.as_str()])?;

        report.freed_bytes += size;
        report.evicted.push(hash);
    }
}
