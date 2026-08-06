//! Rebuilds `package` from `landing_index_line` (invariant I1: the only
//! `rusqlite`-touching half of normalization; the derivation rules live in
//! `ingest::normalize`).

use rusqlite::{Connection, Result, params};

use crate::ingest::normalize::normalize_line;

/// Full rebuild of `package` from `landing_index_line`, in landing order.
/// Not an incremental upsert — P1.8 adds upsert-by-`(dir, file)` on top of
/// this for RECENT-based updates, where preserving existing package ids (and
/// the enrichment rows that reference them) matters. Lines that fail to
/// parse (preamble, truncated) are skipped rather than failing the rebuild.
/// A `(dir, file)` collision keeps whichever landing row is processed first.
pub fn normalize(conn: &Connection) -> Result<usize> {
    let mut stmt =
        conn.prepare("SELECT id, fetched_at, raw FROM landing_index_line ORDER BY id")?;
    let rows: Vec<(i64, String, Vec<u8>)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_>>()?;
    drop(stmt);

    conn.execute("DELETE FROM package", [])?;

    for (landing_id, fetched_at, raw) in &rows {
        let Ok(pkg) = normalize_line(raw, fetched_at) else {
            continue;
        };
        conn.execute(
            "INSERT OR IGNORE INTO package
               (dir, file, name, version, size_bytes, uploaded_on, date_precision, description, landing_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                pkg.dir,
                pkg.file,
                pkg.name,
                pkg.version,
                pkg.size_bytes,
                pkg.uploaded_on,
                pkg.date_precision,
                pkg.description,
                landing_id,
            ],
        )?;
    }

    conn.query_row("SELECT COUNT(*) FROM package", [], |row| row.get(0))
        .map(|n: i64| n as usize)
}
