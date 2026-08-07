//! `zip` backend — same trait as `unar`'s, but in-process via the `zip`
//! crate rather than shelling out (§4/P5.5: Aminet does host `.zip`
//! uploads). `ZipFile::enclosed_name` already rejects `../`/absolute member
//! names, which is what `unar`'s hand-rolled `reject_path_traversal` checks
//! for by hand against `lsar -json` output — reused here instead of
//! reimplemented. Checked in a first pass over every entry before any file
//! is written, so a traversal entry anywhere in the archive leaves nothing
//! under `dest`, matching `unar`'s all-or-nothing extraction.

use std::fs;
use std::io::Cursor;
use std::path::Path;

use crate::blob::{BlobHash, BlobStore};

use super::{ArchiveFormat, Availability, ExtractedFile, UnpackError, Unpacker};

pub struct ZipUnpacker<S: BlobStore> {
    store: S,
}

impl<S: BlobStore> ZipUnpacker<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
}

fn zip_err(e: zip::result::ZipError) -> UnpackError {
    UnpackError::ExtractionFailed {
        message: e.to_string(),
    }
}

impl<S: BlobStore> Unpacker for ZipUnpacker<S> {
    fn id(&self) -> &str {
        "zip"
    }

    fn handles(&self, format: ArchiveFormat) -> bool {
        format == ArchiveFormat::Zip
    }

    fn probe(&self) -> Availability {
        Availability::Available
    }

    fn unpack(&self, blob: &BlobHash, dest: &Path) -> Result<Vec<ExtractedFile>, UnpackError> {
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut self.store.get(blob)?, &mut bytes)?;
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(zip_err)?;

        for i in 0..archive.len() {
            let entry = archive.by_index(i).map_err(zip_err)?;
            if entry.enclosed_name().is_none() {
                return Err(UnpackError::PathTraversal {
                    entry: entry.name().to_string(),
                });
            }
        }

        fs::create_dir_all(dest)?;
        let mut files = Vec::new();
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(zip_err)?;
            let rel = entry.enclosed_name().expect("checked above").to_path_buf();
            if entry.is_dir() {
                continue;
            }
            let target = dest.join(&rel);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out = fs::File::create(&target)?;
            std::io::copy(&mut entry, &mut out)?;
            files.push(ExtractedFile {
                path: rel,
                size: entry.size(),
            });
        }
        Ok(files)
    }
}
