//! FS-UAE launcher (P6.2): extracts an archive to a scratch directory,
//! writes `.uaem` sidecars for entries whose LHA header carried Amiga
//! protection/comment data (P5.6/P5.7), generates an FS-UAE configuration
//! pointing a directory volume at the scratch dir, and spawns the process.
//!
//! FS-UAE reads a directory as a hard drive when `hard_drive_N` names a
//! directory rather than a `.hdf` image (FS-UAE's own documented "hard
//! drive folder" support) — that's why `directory_volume` maps to
//! `hard_drive_0` here rather than a floppy slot.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::blob::BlobStore;
use crate::unpack::{ArchiveFormat, LhaFileHeader, UnpackerRegistry, list_headers, write_sidecar};

use super::{Availability, LaunchHandle, LaunchRequest, Launcher, LauncherCaps, LauncherError};

#[cfg(target_os = "macos")]
fn default_candidates() -> Vec<PathBuf> {
    [
        "/Applications/FS-UAE.app/Contents/MacOS/fs-uae",
        "/opt/homebrew/bin/fs-uae",
        "/usr/local/bin/fs-uae",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

#[cfg(target_os = "linux")]
fn default_candidates() -> Vec<PathBuf> {
    [
        "/usr/bin/fs-uae",
        "/usr/local/bin/fs-uae",
        "/var/lib/flatpak/exports/bin/net.fs_uae.fs-uae",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn default_candidates() -> Vec<PathBuf> {
    Vec::new()
}

/// FS-UAE's directory-volume config: a `hard_drive_0` key pointing at a
/// host directory boots and runs it like an Amiga hard drive.
pub fn fs_uae_config(volume: &Path) -> String {
    format!(
        "[fs-uae]\namiga_model = A500\nhard_drive_0 = {}\n",
        volume.display()
    )
}

static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

fn scratch_dir() -> PathBuf {
    let n = SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("bam-fs-uae-scratch-{}-{n}", std::process::id()))
}

/// Writes a `.uaem` sidecar for every extracted file whose original LHA
/// header carried Amiga protection/comment data, matched by filename.
/// A header with neither, or no matching extracted file, is skipped
/// silently — best-effort, same stance as `unpack::lha_header` itself.
fn write_sidecars(dir: &Path, headers: &[LhaFileHeader]) {
    let now = std::time::SystemTime::now();
    for header in headers {
        if header.protection.is_none() && header.comment.is_none() {
            continue;
        }
        let target = dir.join(&header.filename);
        if !target.is_file() {
            continue;
        }
        let _ = write_sidecar(&target, header.protection, header.comment.as_deref(), now);
    }
}

pub struct FsUaeLauncher<S: BlobStore> {
    store: S,
    unpackers: UnpackerRegistry,
    candidates: Vec<PathBuf>,
}

impl<S: BlobStore> FsUaeLauncher<S> {
    pub fn new(store: S, unpackers: UnpackerRegistry) -> Self {
        Self::with_candidates(store, unpackers, default_candidates())
    }

    /// Bypasses the platform-default candidate paths — the launch config
    /// override (P6.3) and tests both need to probe/launch against a path
    /// that isn't a real system install.
    pub fn with_candidates(
        store: S,
        unpackers: UnpackerRegistry,
        candidates: Vec<PathBuf>,
    ) -> Self {
        Self {
            store,
            unpackers,
            candidates,
        }
    }

    fn find_binary(&self) -> Option<PathBuf> {
        self.candidates.iter().find(|p| p.is_file()).cloned()
    }
}

impl<S: BlobStore> Launcher for FsUaeLauncher<S> {
    fn id(&self) -> &str {
        "fs-uae"
    }

    fn probe(&self) -> Availability {
        match self.find_binary() {
            Some(_) => Availability::Available,
            None => Availability::Unavailable {
                reason: format!(
                    "fs-uae not found at any candidate path ({:?}); install FS-UAE",
                    self.candidates
                ),
            },
        }
    }

    fn capabilities(&self) -> LauncherCaps {
        LauncherCaps {
            directory_volume: true,
            uaem_sidecars: true,
            ..Default::default()
        }
    }

    fn launch(&self, req: &LaunchRequest) -> Result<LaunchHandle, LauncherError> {
        let archive = req.archive.as_ref().ok_or(LauncherError::MissingArchive)?;
        let binary = self
            .find_binary()
            .ok_or_else(|| LauncherError::Unavailable(self.id().to_string()))?;

        let scratch = scratch_dir();
        let volume = scratch.join("volume");
        fs::create_dir_all(&volume).map_err(|e| LauncherError::Launch(e.to_string()))?;

        self.unpackers
            .select(archive.format, None)
            .and_then(|u| u.unpack(&archive.blob, &volume))
            .map_err(|e| LauncherError::Launch(e.to_string()))?;

        if archive.format == ArchiveFormat::Lha {
            if let Ok(mut reader) = self.store.get(&archive.blob) {
                let mut bytes = Vec::new();
                if reader.read_to_end(&mut bytes).is_ok() {
                    write_sidecars(&volume, &list_headers(&bytes));
                }
            }
        }

        let config_path = scratch.join("bam.fs-uae");
        fs::write(&config_path, fs_uae_config(&volume))
            .map_err(|e| LauncherError::Launch(e.to_string()))?;

        Command::new(&binary)
            .arg(&config_path)
            .spawn()
            .map_err(|e| LauncherError::Launch(e.to_string()))?;

        Ok(LaunchHandle {
            launcher_id: self.id().to_string(),
            scratch_dir: Some(scratch),
        })
    }
}
