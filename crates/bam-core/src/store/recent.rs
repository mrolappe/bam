//! RECENT-based incremental update (P1.8). `RECENT` shares INDEX's line
//! format, so parsing and derivation are reused wholesale from
//! `ingest::normalize`; the new part is upserting by `(dir, file)` instead
//! of `normalize`'s full rebuild — preserving an existing package's `id` (and
//! hence any FK-linked `enrichment`/`selection_member`) when it is updated
//! rather than freshly inserted.

use rusqlite::{Connection, OptionalExtension, Result, params};

use super::land::land_lines;
use crate::ingest::normalize::normalize_line;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedPackage {
    pub id: i64,
    pub dir: String,
    pub file: String,
}

type PackageFields = (
    String,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<String>,
);
type ExistingRow = (i64, PackageFields);

/// Lands `body`'s lines, then upserts each into `package` by `(dir, file)`.
/// A row whose derived fields are identical to what's already stored is left
/// completely untouched (not even `landing_id` is rewritten), so only
/// genuine additions and changes appear in the returned list. Lines that
/// fail to parse are skipped, same as `normalize`.
pub fn upsert_recent(
    conn: &Connection,
    source_url: &str,
    fetched_at: &str,
    body: &[u8],
) -> Result<Vec<ChangedPackage>> {
    let landing_ids = land_lines(conn, source_url, fetched_at, body)?;
    let mut changed = Vec::new();

    for landing_id in landing_ids {
        let raw: Vec<u8> = conn.query_row(
            "SELECT raw FROM landing_index_line WHERE id = ?1",
            params![landing_id],
            |row| row.get(0),
        )?;
        let Ok(pkg) = normalize_line(&raw, fetched_at) else {
            continue;
        };

        let existing: Option<ExistingRow> = conn
            .query_row(
                "SELECT id, name, version, size_bytes, uploaded_on, description
                 FROM package WHERE dir = ?1 AND file = ?2",
                params![pkg.dir, pkg.file],
                |row| {
                    Ok((
                        row.get(0)?,
                        (
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                        ),
                    ))
                },
            )
            .optional()?;

        let current = (
            pkg.name.clone(),
            pkg.version.clone(),
            pkg.size_bytes,
            pkg.uploaded_on.clone(),
            pkg.description.clone(),
        );

        match existing {
            None => {
                conn.execute(
                    "INSERT INTO package
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
                changed.push(ChangedPackage {
                    id: conn.last_insert_rowid(),
                    dir: pkg.dir,
                    file: pkg.file,
                });
            }
            Some((id, ref existing_fields)) if *existing_fields != current => {
                conn.execute(
                    "UPDATE package
                       SET name = ?1, version = ?2, size_bytes = ?3, uploaded_on = ?4,
                           description = ?5, landing_id = ?6
                     WHERE id = ?7",
                    params![
                        pkg.name,
                        pkg.version,
                        pkg.size_bytes,
                        pkg.uploaded_on,
                        pkg.description,
                        landing_id,
                        id,
                    ],
                )?;
                changed.push(ChangedPackage {
                    id,
                    dir: pkg.dir,
                    file: pkg.file,
                });
            }
            Some(_) => {} // unchanged: leave the row untouched entirely
        }
    }

    Ok(changed)
}
