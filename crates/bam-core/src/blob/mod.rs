//! `BlobStore` trait (invariant I1: `bam-core` must not call `std::fs`
//! directly). The trait and [`BlobHash`] are plain, ungated code so a fake
//! implementation can be tested with no `native` feature; [`FsBlobStore`]
//! is the real, `native`-gated, filesystem-backed implementation.

use std::fmt;
use std::io::Read;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A BLAKE3 content hash, hex-encoded (§6: cache is content-addressed).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlobHash(String);

impl BlobHash {
    /// Not validated as a real BLAKE3 digest — used to name a hash whose
    /// bytes are not (or not yet) known to be stored, e.g. a lookup key.
    pub fn from_hex(hex: impl Into<String>) -> Self {
        Self(hex.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BlobHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Error)]
pub enum BlobError {
    #[error("blob io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("blob not found: {0}")]
    NotFound(BlobHash),
    #[error("blob corrupted: expected hash {expected}, found {actual}")]
    Corrupted {
        expected: BlobHash,
        actual: BlobHash,
    },
}

pub trait BlobStore {
    /// Hashes while writing; the returned hash is the content's real
    /// address, only known once every byte has been read.
    fn put(&self, bytes: impl Read) -> Result<BlobHash, BlobError>;
    fn get(&self, hash: &BlobHash) -> Result<impl Read, BlobError>;
    fn remove(&self, hash: &BlobHash) -> Result<(), BlobError>;
}

#[cfg(feature = "native")]
mod fs_store;
#[cfg(feature = "native")]
pub use fs_store::FsBlobStore;
