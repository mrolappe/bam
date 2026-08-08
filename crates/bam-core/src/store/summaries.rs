//! Batched, resumable `llm_summary` enrichment (P7.5, §16). Mirrors P7.4's
//! [`super::embeddings::run_batch`] shape: one call processes up to
//! `batch_size` pending packages, so an interrupted run resumes for free —
//! the next call's `pending()` just excludes whatever the previous call
//! already wrote. Unlike embeddings' single batched `embed` call, each
//! package gets its own [`LlmProvider::complete`] call (summaries are
//! per-package prose, not a batchable vector op), so a provider error on one
//! package is caught and recorded rather than aborting the rest of the
//! batch.

use rusqlite::{Connection, params, params_from_iter};
use thiserror::Error;

use super::tables::{self, Enrichment, get_enrichment};
use crate::ingest::charset::decode;
use crate::llm::{CompletionRequest, LlmError, LlmProvider};
use crate::unpack::{INVENTORY_KIND, Inventory};

pub const SUMMARY_KIND: &str = "llm_summary";
pub const SUMMARY_PRODUCER_VERSION: i64 = 1;

#[derive(Debug, Error)]
pub enum SummaryError {
    #[error("sqlite error: {0}")]
    Sqlite(String),
    #[error("payload serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error(
        "cost/token estimate not confirmed — call estimate_run and get \
         user confirmation before starting a bulk run"
    )]
    ConfirmationRequired,
}

impl From<rusqlite::Error> for SummaryError {
    fn from(e: rusqlite::Error) -> Self {
        SummaryError::Sqlite(e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SummaryOutcome {
    pub summarized: usize,
    /// Packages whose `complete` call failed — recorded, not fatal, so the
    /// rest of the batch still runs.
    pub failed: Vec<(i64, LlmError)>,
}

/// Rough token-count and (for paid providers) cost estimate over every
/// package still pending a summary, not just one batch — §16's "before a
/// bulk run starts" needs the whole backlog's size up front.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CostEstimate {
    pub packages: usize,
    pub estimated_tokens: u64,
    /// `None` for providers with no per-token price (local models).
    pub estimated_cost: Option<f64>,
}

/// Packages with landed readme text but no up-to-date `llm_summary`
/// enrichment, oldest package id first. `package_ids`, when given, scopes
/// the candidate set to a selection (I7) — resolving a selection name to
/// ids is the caller's job (`Session::search_packages`).
fn pending(
    conn: &Connection,
    limit: usize,
    package_ids: Option<&[i64]>,
) -> Result<Vec<(i64, String)>, rusqlite::Error> {
    let scope_sql = package_ids
        .map(|ids| format!(" AND p.id IN ({})", vec!["?"; ids.len()].join(",")))
        .unwrap_or_default();
    let sql = format!(
        "SELECT DISTINCT p.id FROM package p
         JOIN landing_readme lr ON lr.package_id = p.id
         WHERE NOT EXISTS (
             SELECT 1 FROM enrichment e
             WHERE e.package_id = p.id AND e.kind = ? AND e.producer_version = ?
         ){scope_sql}
         ORDER BY p.id
         LIMIT ?"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut bind: Vec<rusqlite::types::Value> = vec![
        SUMMARY_KIND.to_string().into(),
        SUMMARY_PRODUCER_VERSION.into(),
    ];
    if let Some(ids) = package_ids {
        bind.extend(ids.iter().map(|id| rusqlite::types::Value::Integer(*id)));
    }
    bind.push((limit as i64).into());
    let ids: Vec<i64> = stmt
        .query_map(params_from_iter(bind.iter()), |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    drop(stmt);

    let mut readme_stmt = conn.prepare("SELECT raw FROM landing_readme WHERE package_id = ?1")?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let raws: Vec<Vec<u8>> = readme_stmt
            .query_map(params![id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let readme = raws
            .iter()
            .map(|raw| decode(raw).0)
            .collect::<Vec<_>>()
            .join("\n");
        out.push((id, summary_input(conn, id, &readme)?));
    }
    Ok(out)
}

/// Readme text plus, when an inventory enrichment (P5.8) exists, its file
/// listing — both go into the summary prompt.
fn summary_input(
    conn: &Connection,
    package_id: i64,
    readme: &str,
) -> Result<String, rusqlite::Error> {
    let files = match get_enrichment(conn, package_id, INVENTORY_KIND) {
        Ok(row) => serde_json::from_str::<Inventory>(&row.payload)
            .map(|inv| {
                inv.files
                    .iter()
                    .map(|f| f.path.clone())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default(),
        Err(rusqlite::Error::QueryReturnedNoRows) => String::new(),
        Err(e) => return Err(e),
    };
    Ok(if files.is_empty() {
        readme.to_string()
    } else {
        format!("{readme}\n\nFiles:\n{files}")
    })
}

fn build_prompt(input: &str) -> String {
    format!(
        "Summarize this Aminet package in 2-3 sentences, based on its \
         readme and file listing:\n\n{input}"
    )
}

/// Chars-per-token is a rough stand-in for a real tokenizer, which no
/// [`LlmProvider`] exposes (`Capabilities` has no `count_tokens`).
/// ponytail: heuristic estimate, swap for a real tokenizer if estimates
/// drift too far from actual usage.
fn estimate_tokens(text: &str) -> u64 {
    (text.len() as u64).div_ceil(4)
}

/// Estimates the whole pending backlog's size — call before a bulk
/// [`run_batch`] run and show the user `packages`/`estimated_tokens`/
/// `estimated_cost` so they can confirm. `cost_per_1k_tokens` is `None` for
/// providers with no per-token price (local models).
pub fn estimate_run(
    conn: &Connection,
    package_ids: Option<&[i64]>,
    cost_per_1k_tokens: Option<f64>,
) -> Result<CostEstimate, SummaryError> {
    let items = pending(conn, usize::MAX, package_ids)?;
    let estimated_tokens: u64 = items.iter().map(|(_, text)| estimate_tokens(text)).sum();
    let estimated_cost = cost_per_1k_tokens.map(|price| (estimated_tokens as f64 / 1000.0) * price);
    Ok(CostEstimate {
        packages: items.len(),
        estimated_tokens,
        estimated_cost,
    })
}

/// Summarizes up to `batch_size` pending packages. `confirmed` must be
/// `true` — the caller is expected to have shown the user [`estimate_run`]'s
/// numbers and gotten confirmation first; this is the gate that makes that
/// structural, not just documented (§16).
pub async fn run_batch(
    conn: &Connection,
    provider: &impl LlmProvider,
    package_ids: Option<&[i64]>,
    batch_size: usize,
    confirmed: bool,
) -> Result<SummaryOutcome, SummaryError> {
    if !confirmed {
        return Err(SummaryError::ConfirmationRequired);
    }

    let items = pending(conn, batch_size, package_ids)?;
    let mut summarized = 0;
    let mut failed = Vec::new();

    for (package_id, input) in items {
        let req = CompletionRequest {
            prompt: build_prompt(&input),
            grammar: None,
            json_schema: None,
            max_tokens: Some(300),
        };
        match provider.complete(req).await {
            Ok(summary) => {
                tables::upsert_enrichment(
                    conn,
                    &Enrichment {
                        package_id,
                        kind: SUMMARY_KIND.to_string(),
                        producer_version: SUMMARY_PRODUCER_VERSION,
                        produced_at: crate::now_rfc3339(),
                        payload: summary,
                    },
                )?;
                summarized += 1;
            }
            Err(e) => failed.push((package_id, e)),
        }
    }

    Ok(SummaryOutcome { summarized, failed })
}
