//! Batched, resumable readme embedding (P7.4, §10). One [`run_batch`] call
//! embeds up to `batch_size` packages that don't have a [`PackageEmbedding`]
//! row yet — never a full loop — so the caller controls pacing and can stop
//! between calls, the same shape `fetch_worker::step` uses for the same
//! reason (I5). Resumability falls out of that for free: the next call's
//! `SELECT` just excludes whatever the previous call already wrote.

use rusqlite::{Connection, params};
use thiserror::Error;

use super::tables::{self, PackageEmbedding};
use crate::ingest::charset::decode;
use crate::llm::{LlmError, LlmProvider};

#[derive(Debug, Clone, PartialEq, Error)]
pub enum EmbedError {
    #[error(transparent)]
    Llm(#[from] LlmError),
    #[error("sqlite error: {0}")]
    Sqlite(String),
    #[error(
        "embedding dimension changed from {expected} to {actual} — \
         looks like the embedding model changed; re-embed from scratch"
    )]
    DimensionMismatch { expected: i64, actual: i64 },
}

impl From<rusqlite::Error> for EmbedError {
    fn from(e: rusqlite::Error) -> Self {
        EmbedError::Sqlite(e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchOutcome {
    /// Packages embedded by this call (0 when nothing was pending).
    pub embedded: usize,
}

/// Up to `limit` packages with landed readme text but no embedding yet,
/// oldest package id first — a stable order so repeated calls make steady
/// progress through the backlog rather than re-picking randomly.
fn pending(conn: &Connection, limit: usize) -> Result<Vec<(i64, String)>, rusqlite::Error> {
    let mut id_stmt = conn.prepare(
        "SELECT DISTINCT p.id FROM package p
         JOIN landing_readme lr ON lr.package_id = p.id
         WHERE NOT EXISTS (
             SELECT 1 FROM package_embedding pe WHERE pe.package_id = p.id
         )
         ORDER BY p.id
         LIMIT ?1",
    )?;
    let ids: Vec<i64> = id_stmt
        .query_map(params![limit as i64], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    drop(id_stmt);

    let mut readme_stmt = conn.prepare("SELECT raw FROM landing_readme WHERE package_id = ?1")?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let raws: Vec<Vec<u8>> = readme_stmt
            .query_map(params![id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let text = raws
            .iter()
            .map(|raw| decode(raw).0)
            .collect::<Vec<_>>()
            .join("\n");
        out.push((id, text));
    }
    Ok(out)
}

/// Embeds up to `batch_size` pending packages in exactly one
/// [`LlmProvider::embed`] call (already a batch API by construction —
/// P7.1), so `ceil(pending / batch_size)` calls cover the whole backlog
/// rather than one call per package. `model` is recorded alongside each
/// vector so a later dimension mismatch names what changed.
pub async fn run_batch(
    conn: &Connection,
    provider: &impl LlmProvider,
    model: &str,
    batch_size: usize,
) -> Result<BatchOutcome, EmbedError> {
    let items = pending(conn, batch_size)?;
    if items.is_empty() {
        return Ok(BatchOutcome { embedded: 0 });
    }

    let texts: Vec<String> = items.iter().map(|(_, text)| text.clone()).collect();
    let vectors = provider.embed(&texts).await?;
    let expected_dim = tables::any_package_embedding_dim(conn)?;

    for ((package_id, _), vector) in items.iter().zip(vectors) {
        let dim = vector.len() as i64;
        if let Some(expected) = expected_dim {
            if expected != dim {
                return Err(EmbedError::DimensionMismatch {
                    expected,
                    actual: dim,
                });
            }
        }
        tables::upsert_package_embedding(
            conn,
            &PackageEmbedding {
                package_id: *package_id,
                model: model.to_string(),
                dim,
                vector,
            },
        )?;
    }

    Ok(BatchOutcome {
        embedded: items.len(),
    })
}
