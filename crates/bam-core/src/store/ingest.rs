//! `bam ingest` orchestration (P1.10): wires the HTTP fetch (P1.9), landing
//! (P1.2), and normalization (P1.6) behind one entry point, reporting
//! progress through invariant I5's typed [`ProgressSink`] rather than
//! formatting anything itself — the CLI renders a progress bar from the
//! same events a future web client would consume as JSON.

use rusqlite::Connection;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::fetch::{FetchError, fetch_and_land};
use super::land::land_lines;
use super::normalize::normalize;
use crate::http::HttpClient;
use crate::progress::{OperationId, Outcome, ProgressEvent, ProgressSink};

/// The real Aminet mirror's INDEX, fetched in [`IngestMode::Fetch`].
pub const INDEX_URL: &str = "https://ftp.fau.de/aminet/INDEX.gz";

/// A trimmed real INDEX, embedded so `--offline` needs neither a network
/// call nor a fixture path resolved at runtime.
const OFFLINE_INDEX_FIXTURE: &[u8] = include_bytes!("../../tests/fixtures/index_sample.txt");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum IngestMode {
    /// Fetch [`INDEX_URL`] over HTTP, land it, then normalize.
    Fetch,
    /// Land the bundled fixture, then normalize. No network.
    Offline,
    /// Skip fetch and land entirely; re-derive `package` from whatever is
    /// already in `landing_index_line`. `client` is never called.
    RebuildNormalized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngestOutcome {
    pub package_count: usize,
}

#[derive(Debug, Error)]
pub enum IngestError {
    #[error(transparent)]
    Fetch(#[from] FetchError),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
}

async fn land_and_normalize(
    conn: &Connection,
    client: &impl HttpClient,
    sink: &mut impl ProgressSink,
    mode: IngestMode,
    fetched_at: &str,
    operation: OperationId,
) -> Result<usize, IngestError> {
    match mode {
        IngestMode::Fetch => {
            fetch_and_land(conn, client, INDEX_URL, fetched_at).await?;
            sink.emit(ProgressEvent::Advanced { operation, done: 1 });
        }
        IngestMode::Offline => {
            land_lines(
                conn,
                "offline:index_sample.txt",
                fetched_at,
                OFFLINE_INDEX_FIXTURE,
            )?;
            sink.emit(ProgressEvent::Advanced { operation, done: 1 });
        }
        IngestMode::RebuildNormalized => {}
    }

    let package_count = normalize(conn)?;
    let done: u64 = if mode == IngestMode::RebuildNormalized {
        1
    } else {
        2
    };
    sink.emit(ProgressEvent::Advanced { operation, done });
    Ok(package_count)
}

/// Runs one ingest per `mode`, reporting `Started`/`Advanced`/`Finished`
/// through `sink` under `operation` — caller-assigned (P2.6's `Session`
/// hands out a fresh, session-unique id; a one-off CLI run can just pass
/// `OperationId(0)`).
pub async fn run_ingest(
    conn: &Connection,
    client: &impl HttpClient,
    sink: &mut impl ProgressSink,
    mode: IngestMode,
    fetched_at: &str,
    operation: OperationId,
) -> Result<IngestOutcome, IngestError> {
    let total: u64 = if mode == IngestMode::RebuildNormalized {
        1
    } else {
        2
    };
    sink.emit(ProgressEvent::Started {
        operation,
        total: Some(total),
    });

    match land_and_normalize(conn, client, sink, mode, fetched_at, operation).await {
        Ok(package_count) => {
            sink.emit(ProgressEvent::Finished {
                operation,
                outcome: Outcome::Success,
            });
            Ok(IngestOutcome { package_count })
        }
        Err(e) => {
            sink.emit(ProgressEvent::Finished {
                operation,
                outcome: Outcome::Failed {
                    message: e.to_string(),
                },
            });
            Err(e)
        }
    }
}
