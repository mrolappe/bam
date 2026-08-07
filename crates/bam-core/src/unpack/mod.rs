//! The `Unpacker` trait (invariant I4's registry pattern, mirroring
//! `query::lang::LanguageRegistry`): an archive backend is anything that
//! claims an [`ArchiveFormat`] and can extract it. Format is detected from
//! magic bytes, never from filename — Aminet's `.lha` files are routinely
//! actually LZX, so an extension-keyed registry breaks on real data.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::blob::{BlobError, BlobHash};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchiveFormat {
    Lha,
    Lzx,
    Zip,
}

impl ArchiveFormat {
    fn name(self) -> &'static str {
        match self {
            ArchiveFormat::Lha => "LHA",
            ArchiveFormat::Lzx => "LZX",
            ArchiveFormat::Zip => "ZIP",
        }
    }
}

/// LZX archives start with the literal signature `LZX\0`; ZIP archives with
/// the local-file-header signature `PK\x03\x04`. LHA/LZH archives carry no
/// fixed leading signature (the first two bytes are header size and
/// checksum) but always spell their method id as `-lh?-` or `-lz?-` at
/// offset 2.
pub fn detect_format(bytes: &[u8]) -> Result<ArchiveFormat, UnpackError> {
    if bytes.starts_with(b"LZX\0") {
        return Ok(ArchiveFormat::Lzx);
    }
    if bytes.starts_with(b"PK\x03\x04") {
        return Ok(ArchiveFormat::Zip);
    }
    if let Some(method) = bytes.get(2..7) {
        if (method.starts_with(b"-lh") || method.starts_with(b"-lz")) && method.ends_with(b"-") {
            return Ok(ArchiveFormat::Lha);
        }
    }
    Err(UnpackError::UnknownFormat {
        leading: bytes.iter().take(8).copied().collect(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Available,
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedFile {
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Debug, Error)]
pub enum UnpackError {
    #[error("unrecognized archive format; leading bytes {leading:02x?}")]
    UnknownFormat { leading: Vec<u8> },
    #[error("unpacker '{0}' not registered")]
    UnknownUnpacker(String),
    #[error(
        "no available unpacker for format {}; install a backend and check its `probe()`",
        .format.name()
    )]
    NoAvailableUnpacker { format: ArchiveFormat },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("blob error: {0}")]
    Blob(#[from] BlobError),
    #[error("archive entry '{entry}' contains a path-traversal component; rejected")]
    PathTraversal { entry: String },
    #[error("archive extraction failed: {message}")]
    ExtractionFailed { message: String },
}

pub trait Unpacker {
    fn id(&self) -> &str;
    fn handles(&self, format: ArchiveFormat) -> bool;
    /// Is the backend usable here (e.g. an external binary found on `PATH`)?
    fn probe(&self) -> Availability;
    fn unpack(&self, blob: &BlobHash, dest: &Path) -> Result<Vec<ExtractedFile>, UnpackError>;
}

pub struct UnpackerRegistry {
    unpackers: Vec<Box<dyn Unpacker>>,
}

impl UnpackerRegistry {
    pub fn new() -> Self {
        Self {
            unpackers: Vec::new(),
        }
    }

    pub fn register(&mut self, unpacker: Box<dyn Unpacker>) {
        self.unpackers.push(unpacker);
    }

    /// Config override first (must be registered, handle `format`, and
    /// probe available), else the first registered unpacker that both
    /// claims `format` and probes available.
    pub fn select(
        &self,
        format: ArchiveFormat,
        override_id: Option<&str>,
    ) -> Result<&dyn Unpacker, UnpackError> {
        if let Some(id) = override_id {
            let u = self
                .unpackers
                .iter()
                .find(|u| u.id() == id)
                .ok_or_else(|| UnpackError::UnknownUnpacker(id.to_string()))?;
            return match u.probe() {
                Availability::Available if u.handles(format) => Ok(u.as_ref()),
                _ => Err(UnpackError::NoAvailableUnpacker { format }),
            };
        }

        self.unpackers
            .iter()
            .find(|u| u.handles(format) && u.probe() == Availability::Available)
            .map(|u| u.as_ref())
            .ok_or(UnpackError::NoAvailableUnpacker { format })
    }
}

impl Default for UnpackerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

mod lha_header;
pub use lha_header::{
    HeaderLevel, LhaFileHeader, LhaHeaderError, ProtectionBits, parse_lha_header,
};

mod uaem;
#[cfg(feature = "native")]
pub use uaem::write_sidecar;
pub use uaem::{UaemError, format_uaem_line};

#[cfg(feature = "native")]
mod unar;
#[cfg(feature = "native")]
pub use unar::UnarUnpacker;

#[cfg(feature = "native")]
mod zip_backend;
#[cfg(feature = "native")]
pub use zip_backend::ZipUnpacker;
