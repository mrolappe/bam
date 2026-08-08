use rusqlite::{Connection, OptionalExtension, Result, params};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub struct LandingIndexLine {
    pub id: i64,
    pub fetched_at: String,
    pub source_url: String,
    pub line_no: i64,
    pub raw: Vec<u8>,
}

pub fn insert_landing_index_line(conn: &Connection, row: &LandingIndexLine) -> Result<i64> {
    conn.execute(
        "INSERT INTO landing_index_line (fetched_at, source_url, line_no, raw)
         VALUES (?1, ?2, ?3, ?4)",
        params![row.fetched_at, row.source_url, row.line_no, row.raw],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_landing_index_line(conn: &Connection, id: i64) -> Result<LandingIndexLine> {
    conn.query_row(
        "SELECT id, fetched_at, source_url, line_no, raw FROM landing_index_line WHERE id = ?1",
        params![id],
        |row| {
            Ok(LandingIndexLine {
                id: row.get(0)?,
                fetched_at: row.get(1)?,
                source_url: row.get(2)?,
                line_no: row.get(3)?,
                raw: row.get(4)?,
            })
        },
    )
}

/// Also the shape of `bam_core::api`'s package response (P2.6) — reused
/// rather than duplicated into a separate DTO.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Package {
    pub id: i64,
    pub dir: String,
    pub file: String,
    pub name: String,
    pub version: Option<String>,
    pub size_bytes: Option<i64>,
    pub uploaded_on: Option<String>,
    pub date_precision: String,
    pub description: Option<String>,
    pub landing_id: i64,
}

pub fn insert_package(conn: &Connection, row: &Package) -> Result<i64> {
    conn.execute(
        "INSERT INTO package
           (dir, file, name, version, size_bytes, uploaded_on, date_precision, description, landing_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            row.dir,
            row.file,
            row.name,
            row.version,
            row.size_bytes,
            row.uploaded_on,
            row.date_precision,
            row.description,
            row.landing_id,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_package(conn: &Connection, id: i64) -> Result<Package> {
    conn.query_row(
        "SELECT id, dir, file, name, version, size_bytes, uploaded_on, date_precision, description, landing_id
         FROM package WHERE id = ?1",
        params![id],
        |row| {
            Ok(Package {
                id: row.get(0)?,
                dir: row.get(1)?,
                file: row.get(2)?,
                name: row.get(3)?,
                version: row.get(4)?,
                size_bytes: row.get(5)?,
                uploaded_on: row.get(6)?,
                date_precision: row.get(7)?,
                description: row.get(8)?,
                landing_id: row.get(9)?,
            })
        },
    )
}

/// Sets or clears (`hash: None`) a package's cached-archive pointer
/// (`package.archive_hash`, migration 6). Not part of the [`Package`] struct
/// itself — that struct is shared by every pre-P5.2 caller across the
/// codebase, and this column is read/written only by the blob cache
/// (P5.2, §6) so far.
pub fn set_archive_hash(conn: &Connection, package_id: i64, hash: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE package SET archive_hash = ?2 WHERE id = ?1",
        params![package_id, hash],
    )?;
    Ok(())
}

pub fn get_archive_hash(conn: &Connection, package_id: i64) -> Result<Option<String>> {
    conn.query_row(
        "SELECT archive_hash FROM package WHERE id = ?1",
        params![package_id],
        |row| row.get(0),
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct Enrichment {
    pub package_id: i64,
    pub kind: String,
    pub producer_version: i64,
    pub produced_at: String,
    pub payload: String,
}

pub fn insert_enrichment(conn: &Connection, row: &Enrichment) -> Result<()> {
    conn.execute(
        "INSERT INTO enrichment (package_id, kind, producer_version, produced_at, payload)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            row.package_id,
            row.kind,
            row.producer_version,
            row.produced_at,
            row.payload,
        ],
    )?;
    Ok(())
}

/// Like [`insert_enrichment`], but replaces an existing `(package_id, kind)`
/// row instead of erroring — P5.8's "bumping `producer_version` reprocesses"
/// needs this; a plain `INSERT` would collide with the table's primary key.
pub fn upsert_enrichment(conn: &Connection, row: &Enrichment) -> Result<()> {
    conn.execute(
        "INSERT INTO enrichment (package_id, kind, producer_version, produced_at, payload)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(package_id, kind) DO UPDATE SET
           producer_version = excluded.producer_version,
           produced_at = excluded.produced_at,
           payload = excluded.payload",
        params![
            row.package_id,
            row.kind,
            row.producer_version,
            row.produced_at,
            row.payload,
        ],
    )?;
    Ok(())
}

pub fn get_enrichment(conn: &Connection, package_id: i64, kind: &str) -> Result<Enrichment> {
    conn.query_row(
        "SELECT package_id, kind, producer_version, produced_at, payload
         FROM enrichment WHERE package_id = ?1 AND kind = ?2",
        params![package_id, kind],
        |row| {
            Ok(Enrichment {
                package_id: row.get(0)?,
                kind: row.get(1)?,
                producer_version: row.get(2)?,
                produced_at: row.get(3)?,
                payload: row.get(4)?,
            })
        },
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub id: i64,
    pub name: Option<String>,
    pub created_at: String,
    pub ephemeral: bool,
}

pub fn insert_selection(conn: &Connection, row: &Selection) -> Result<i64> {
    conn.execute(
        "INSERT INTO selection (name, created_at, ephemeral) VALUES (?1, ?2, ?3)",
        params![row.name, row.created_at, row.ephemeral],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_selection(conn: &Connection, id: i64) -> Result<Selection> {
    conn.query_row(
        "SELECT id, name, created_at, ephemeral FROM selection WHERE id = ?1",
        params![id],
        |row| {
            Ok(Selection {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                ephemeral: row.get(3)?,
            })
        },
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectionMember {
    pub selection_id: i64,
    pub package_id: i64,
}

pub fn insert_selection_member(conn: &Connection, row: &SelectionMember) -> Result<()> {
    conn.execute(
        "INSERT INTO selection_member (selection_id, package_id) VALUES (?1, ?2)",
        params![row.selection_id, row.package_id],
    )?;
    Ok(())
}

/// Per-URL ETag for conditional GET (P1.9). `None` when nothing has been
/// fetched from `url` yet.
pub fn get_etag(conn: &Connection, url: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT etag FROM http_cache WHERE url = ?1",
        params![url],
        |row| row.get(0),
    )
    .optional()
}

pub fn set_etag(conn: &Connection, url: &str, etag: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO http_cache (url, etag) VALUES (?1, ?2)
         ON CONFLICT(url) DO UPDATE SET etag = excluded.etag",
        params![url, etag],
    )?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct LandingReadme {
    pub id: i64,
    pub package_id: i64,
    pub url: String,
    pub fetched_at: String,
    pub raw: Vec<u8>,
    pub detected_encoding: String,
}

/// Upserts by `url`: a re-fetch of an already-landed readme updates the
/// existing row (id preserved) instead of duplicating it.
pub fn insert_landing_readme(conn: &Connection, row: &LandingReadme) -> Result<i64> {
    conn.query_row(
        "INSERT INTO landing_readme (package_id, url, fetched_at, raw, detected_encoding)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(url) DO UPDATE SET
           package_id = excluded.package_id,
           fetched_at = excluded.fetched_at,
           raw = excluded.raw,
           detected_encoding = excluded.detected_encoding
         RETURNING id",
        params![
            row.package_id,
            row.url,
            row.fetched_at,
            row.raw,
            row.detected_encoding,
        ],
        |r| r.get(0),
    )
}

pub fn landing_readme_exists(conn: &Connection, url: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM landing_readme WHERE url = ?1",
            params![url],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

pub fn get_landing_readme(conn: &Connection, url: &str) -> Result<LandingReadme> {
    conn.query_row(
        "SELECT id, package_id, url, fetched_at, raw, detected_encoding
         FROM landing_readme WHERE url = ?1",
        params![url],
        |row| {
            Ok(LandingReadme {
                id: row.get(0)?,
                package_id: row.get(1)?,
                url: row.get(2)?,
                fetched_at: row.get(3)?,
                raw: row.get(4)?,
                detected_encoding: row.get(5)?,
            })
        },
    )
}

pub fn get_selection_member(
    conn: &Connection,
    selection_id: i64,
    package_id: i64,
) -> Result<SelectionMember> {
    conn.query_row(
        "SELECT selection_id, package_id FROM selection_member
         WHERE selection_id = ?1 AND package_id = ?2",
        params![selection_id, package_id],
        |row| {
            Ok(SelectionMember {
                selection_id: row.get(0)?,
                package_id: row.get(1)?,
            })
        },
    )
}

/// One package's embedding (P7.4). `vector` is packed/unpacked as raw
/// little-endian float32 bytes on the way in/out — the layout
/// `vec_distance_cosine` (sqlite-vec) reads directly, so a stored vector
/// never needs re-encoding to be compared at query time.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageEmbedding {
    pub package_id: i64,
    pub model: String,
    pub dim: i64,
    pub vector: Vec<f32>,
}

pub fn pack_vector(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

pub fn unpack_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Upserts by `package_id`: re-embedding (e.g. after a readme update) just
/// replaces the row, matching `upsert_enrichment`'s reprocessing convention.
pub fn upsert_package_embedding(conn: &Connection, row: &PackageEmbedding) -> Result<()> {
    conn.execute(
        "INSERT INTO package_embedding (package_id, model, dim, vector)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(package_id) DO UPDATE SET
           model = excluded.model,
           dim = excluded.dim,
           vector = excluded.vector",
        params![row.package_id, row.model, row.dim, pack_vector(&row.vector)],
    )?;
    Ok(())
}

pub fn get_package_embedding(
    conn: &Connection,
    package_id: i64,
) -> Result<Option<PackageEmbedding>> {
    conn.query_row(
        "SELECT package_id, model, dim, vector FROM package_embedding WHERE package_id = ?1",
        params![package_id],
        |row| {
            let vector: Vec<u8> = row.get(3)?;
            Ok(PackageEmbedding {
                package_id: row.get(0)?,
                model: row.get(1)?,
                dim: row.get(2)?,
                vector: unpack_vector(&vector),
            })
        },
    )
    .optional()
}

/// Any existing embedding's `dim`, used to detect a model switch (P7.4)
/// before writing a vector of a different dimension into the same table.
pub fn any_package_embedding_dim(conn: &Connection) -> Result<Option<i64>> {
    conn.query_row("SELECT dim FROM package_embedding LIMIT 1", [], |row| {
        row.get(0)
    })
    .optional()
}
