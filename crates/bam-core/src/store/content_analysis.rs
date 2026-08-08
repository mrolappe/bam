//! Content-analyzer enrichment (P8.3): the DB half of the `content_analyzer`
//! extension point (`bam_core::plugin`). Feeds a plugin's `analyze` export
//! one extracted file at a time, `claims`-prefiltered (P8.1) so a plugin is
//! never invoked for a file it can't handle, and stores each result under
//! its own per-file, per-plugin `enrichment` row — so bumping one plugin's
//! version reprocesses only that plugin's rows for the files it claims,
//! never another plugin's rows or the `llm_summary` kind (P7.5).

use std::hash::{Hash, Hasher};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use rusqlite::Connection;
use thiserror::Error;

use crate::plugin::{ContentAnalyzerInput, WasmContentAnalyzer};

use super::tables::{Enrichment, get_enrichment, upsert_enrichment};

/// `enrichment.kind` prefix every content-analyzer row is stored under —
/// `{PREFIX}{plugin_id}:{file_path}`, so each plugin/file pair gets its own
/// row under the table's `(package_id, kind)` primary key.
pub const CONTENT_ANALYZER_KIND_PREFIX: &str = "content_analyzer:";

pub fn enrichment_kind(plugin_id: &str, path: &str) -> String {
    format!("{CONTENT_ANALYZER_KIND_PREFIX}{plugin_id}:{path}")
}

/// Turns a plugin's free-form `version` string into the `enrichment.
/// producer_version` i64 the table needs. `DefaultHasher::new()` uses fixed
/// keys, so this is deterministic across runs and processes: the same
/// version string always lands the same producer_version, so a changed
/// version string is what "reprocess this plugin's rows" keys off.
fn producer_version(version: &str) -> i64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    version.hash(&mut hasher);
    hasher.finish() as i64
}

#[derive(Debug, Error)]
pub enum ContentAnalysisError {
    #[error("db error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("payload serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyzeOutcome {
    pub analyzed: usize,
    pub up_to_date: usize,
    /// Files whose plugin call failed or returned malformed JSON —
    /// recorded, not fatal, so the rest of the batch still runs.
    pub failed: Vec<(String, String)>,
}

/// Runs `analyzer` over every `(path, bytes)` in `files` that its manifest
/// `claims`, skipping files already analyzed at the plugin's current
/// `version` and files whose analysis failed (recorded in
/// [`AnalyzeOutcome::failed`], never fatal to the rest of the batch).
pub fn analyze_files(
    conn: &Connection,
    package_id: i64,
    analyzer: &WasmContentAnalyzer,
    files: &[(String, Vec<u8>)],
    hint: &str,
) -> Result<AnalyzeOutcome, ContentAnalysisError> {
    let pv = producer_version(analyzer.version());
    let mut analyzed = 0;
    let mut up_to_date = 0;
    let mut failed = Vec::new();

    for (path, bytes) in files {
        if !analyzer.claims_file(path) {
            continue;
        }
        let kind = enrichment_kind(analyzer.id(), path);
        if is_up_to_date(conn, package_id, &kind, pv)? {
            up_to_date += 1;
            continue;
        }

        let input = ContentAnalyzerInput {
            path: path.clone(),
            size: bytes.len() as u64,
            bytes_b64: BASE64.encode(bytes),
            hint: hint.to_string(),
        };
        match analyzer.analyze(&input) {
            Ok(output) => {
                let payload = serde_json::to_string(&output)?;
                upsert_enrichment(
                    conn,
                    &Enrichment {
                        package_id,
                        kind,
                        producer_version: pv,
                        produced_at: crate::now_rfc3339(),
                        payload,
                    },
                )?;
                analyzed += 1;
            }
            Err(e) => failed.push((path.clone(), e.to_string())),
        }
    }

    Ok(AnalyzeOutcome {
        analyzed,
        up_to_date,
        failed,
    })
}

fn is_up_to_date(
    conn: &Connection,
    package_id: i64,
    kind: &str,
    pv: i64,
) -> Result<bool, ContentAnalysisError> {
    match get_enrichment(conn, package_id, kind) {
        Ok(row) => Ok(row.producer_version == pv),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
        Err(e) => Err(e.into()),
    }
}
