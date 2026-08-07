//! Explicit rebuild of `package_fts` (P4.6, migration 0005). No triggers:
//! `normalize` (P1.6) bulk-rebuilds `package` from landing data, which would
//! silently desync a trigger-fed index — a full drop-and-repopulate rebuild
//! sidesteps that class of bug entirely rather than chasing it with more
//! triggers.

use rusqlite::{Connection, Result, params};

use crate::ingest::charset::decode;

const FTS_DDL: &str =
    "CREATE VIRTUAL TABLE package_fts USING fts5(description, readme_text, content='')";

/// Drops and repopulates `package_fts` from the current `package` and
/// `landing_readme` tables. Returns the number of packages indexed.
pub fn rebuild_fts(conn: &Connection) -> Result<usize> {
    conn.execute("DROP TABLE IF EXISTS package_fts", [])?;
    conn.execute_batch(FTS_DDL)?;

    let mut pkg_stmt = conn.prepare("SELECT id, description FROM package")?;
    let packages: Vec<(i64, Option<String>)> = pkg_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_>>()?;
    drop(pkg_stmt);

    let mut readme_stmt = conn.prepare("SELECT raw FROM landing_readme WHERE package_id = ?1")?;
    let mut insert_stmt = conn
        .prepare("INSERT INTO package_fts (rowid, description, readme_text) VALUES (?1, ?2, ?3)")?;

    for (id, description) in &packages {
        let raws: Vec<Vec<u8>> = readme_stmt
            .query_map(params![id], |row| row.get(0))?
            .collect::<Result<_>>()?;
        let readme_text = raws
            .iter()
            .map(|raw| decode(raw).0)
            .collect::<Vec<_>>()
            .join("\n");

        insert_stmt.execute(params![id, description, readme_text])?;
    }

    Ok(packages.len())
}
