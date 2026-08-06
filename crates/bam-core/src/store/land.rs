//! Splits a raw INDEX/RECENT body into lines and appends each to
//! `landing_index_line`. Shared by the RECENT upsert path (P1.8) and the
//! HTTP fetch path (P1.9) — both land raw bytes the same way the initial
//! INDEX ingest does.

use rusqlite::{Connection, Result};

use super::tables::{LandingIndexLine, insert_landing_index_line};

/// Returns the inserted landing ids, in line order.
pub fn land_lines(
    conn: &Connection,
    source_url: &str,
    fetched_at: &str,
    body: &[u8],
) -> Result<Vec<i64>> {
    let mut lines: Vec<&[u8]> = body.split(|&b| b == b'\n').collect();
    if lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }

    lines
        .into_iter()
        .enumerate()
        .map(|(i, raw)| {
            insert_landing_index_line(
                conn,
                &LandingIndexLine {
                    id: 0,
                    fetched_at: fetched_at.to_string(),
                    source_url: source_url.to_string(),
                    line_no: i as i64 + 1,
                    raw: raw.to_vec(),
                },
            )
        })
        .collect()
}
