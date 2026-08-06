//! Gunzip for fetched `INDEX.gz`/`RECENT.gz` bodies. `flate2`'s default
//! backend (`miniz_oxide`) is pure Rust with no OS dependency, so this stays
//! ungated like the rest of `ingest` — confirmed by the wasm32
//! `--no-default-features` check.

use std::io::Read;

use flate2::read::GzDecoder;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("gzip decode failed: {0}")]
pub struct GunzipError(String);

pub fn gunzip(bytes: &[u8]) -> Result<Vec<u8>, GunzipError> {
    let mut out = Vec::new();
    GzDecoder::new(bytes)
        .read_to_end(&mut out)
        .map_err(|e| GunzipError(e.to_string()))?;
    Ok(out)
}
