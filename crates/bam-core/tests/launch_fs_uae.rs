//! P6.2: FS-UAE launcher.

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use bam_core::blob::{BlobError, BlobHash, BlobStore};
use bam_core::launch::{
    Availability, FsUaeLauncher, LaunchArchive, LaunchRequest, Launcher, LauncherCaps,
    fs_uae_config,
};
use bam_core::unpack::{ArchiveFormat, ExtractedFile, UnpackError, Unpacker, UnpackerRegistry};

struct EmptyStore;

impl BlobStore for EmptyStore {
    fn put(&self, _bytes: impl Read) -> Result<BlobHash, BlobError> {
        unimplemented!("not exercised by these tests")
    }

    fn get(&self, _hash: &BlobHash) -> Result<impl Read, BlobError> {
        Ok(Cursor::new(Vec::<u8>::new()))
    }

    fn remove(&self, _hash: &BlobHash) -> Result<(), BlobError> {
        Ok(())
    }
}

struct NoopUnpacker;

impl Unpacker for NoopUnpacker {
    fn id(&self) -> &str {
        "noop"
    }

    fn handles(&self, _format: ArchiveFormat) -> bool {
        true
    }

    fn probe(&self) -> bam_core::unpack::Availability {
        bam_core::unpack::Availability::Available
    }

    fn unpack(&self, _blob: &BlobHash, _dest: &Path) -> Result<Vec<ExtractedFile>, UnpackError> {
        Ok(Vec::new())
    }
}

fn registry() -> UnpackerRegistry {
    let mut reg = UnpackerRegistry::new();
    reg.register(Box::new(NoopUnpacker));
    reg
}

#[test]
fn generated_config_matches_fixture_field_for_field() {
    let config = fs_uae_config(Path::new("/tmp/bam-volume"));
    assert_eq!(
        config,
        "[fs-uae]\namiga_model = A500\nhard_drive_0 = /tmp/bam-volume\n"
    );
}

#[test]
fn probe_finds_fs_uae_at_candidate_path_and_reports_unavailable_when_absent() {
    let dir = std::env::temp_dir().join(format!("bam-fs-uae-probe-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("fs-uae");
    fs::write(&bin, b"").unwrap();

    let found = FsUaeLauncher::with_candidates(EmptyStore, registry(), vec![bin.clone()]);
    assert_eq!(found.probe(), Availability::Available);

    let missing =
        FsUaeLauncher::with_candidates(EmptyStore, registry(), vec![dir.join("does-not-exist")]);
    assert!(matches!(missing.probe(), Availability::Unavailable { .. }));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn capabilities_report_directory_volume_and_uaem_sidecars() {
    let launcher = FsUaeLauncher::with_candidates(EmptyStore, registry(), vec![]);
    let caps = launcher.capabilities();
    assert_eq!(
        caps,
        LauncherCaps {
            directory_volume: true,
            uaem_sidecars: true,
            ..Default::default()
        }
    );
}

#[test]
fn scratch_directory_is_cleaned_up_when_handle_drops() {
    // Stands in for the real fs-uae binary: always present, exits
    // immediately, harmless to spawn as part of a test.
    let launcher = FsUaeLauncher::with_candidates(
        EmptyStore,
        registry(),
        vec![PathBuf::from("/usr/bin/true")],
    );
    let req = LaunchRequest {
        required: LauncherCaps::default(),
        archive: Some(LaunchArchive {
            blob: BlobHash::from_hex("deadbeef"),
            format: ArchiveFormat::Zip,
        }),
    };

    let handle = launcher.launch(&req).unwrap();
    let scratch = handle.scratch_dir.clone().unwrap();
    assert!(scratch.exists());

    drop(handle);
    assert!(!scratch.exists());
}

/// The real end-to-end check of whether P5.6 (LHA header protection bits)
/// and P5.7 (`.uaem` sidecars) actually work: launches a genuine
/// Amiga-built `.lha` containing `s/startup-sequence` through a real,
/// installed FS-UAE and requires a human to watch the emulator window and
/// confirm the script actually *runs* — not just that a window opens.
///
/// Needs two things this environment doesn't have:
/// - FS-UAE actually installed (`fs-uae` on `PATH` or a default candidate
///   path — see `fs_uae::default_candidates`).
/// - `tests/fixtures/archives/startup_sequence.lha`, a *genuine*
///   Amiga-built LHA archive (the system `lha`/`unar` tools have no Amiga
///   awareness and can't produce one — same gap `unpack::lha_header`'s
///   module doc records since Round 32).
///
/// Run manually with `cargo test --features native -- --ignored
/// manual_launch_runs_the_startup_sequence_script` once both are in place.
#[test]
#[ignore = "manual: needs a real FS-UAE install and a genuine Amiga-built .lha fixture; watch the emulator window"]
fn manual_launch_runs_the_startup_sequence_script() {
    use bam_core::blob::FsBlobStore;
    use bam_core::launch::FsUaeLauncher;
    use bam_core::unpack::UnarUnpacker;

    let blob_root = std::env::temp_dir().join("bam-manual-launch-blobs");
    let store = FsBlobStore::new(&blob_root).unwrap();
    let bytes = fs::read("tests/fixtures/archives/startup_sequence.lha").expect(
        "place a genuine Amiga-built .lha with a runnable s/startup-sequence at \
         tests/fixtures/archives/startup_sequence.lha before running this manually",
    );
    let blob = store.put(Cursor::new(bytes)).unwrap();

    let mut unpackers = UnpackerRegistry::new();
    unpackers.register(Box::new(UnarUnpacker::new(
        FsBlobStore::new(&blob_root).unwrap(),
    )));

    let launcher = FsUaeLauncher::new(store, unpackers);
    let req = LaunchRequest {
        required: LauncherCaps {
            directory_volume: true,
            uaem_sidecars: true,
            ..Default::default()
        },
        archive: Some(LaunchArchive {
            blob,
            format: ArchiveFormat::Lha,
        }),
    };

    let handle = launcher.launch(&req).expect("fs-uae should launch");
    std::thread::sleep(std::time::Duration::from_secs(20));
    drop(handle);
}
