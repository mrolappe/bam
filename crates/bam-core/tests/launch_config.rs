//! P6.3: launcher configuration (`bam.toml`'s `[launch]` section) — binary
//! path override, extra args, and preference order across launchers.

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bam_core::blob::{BlobError, BlobHash, BlobStore};
use bam_core::launch::{
    Availability, FsUaeLauncher, LaunchArchive, LaunchRequest, Launcher, LauncherCaps,
    LauncherError, LauncherRegistry, resolve_candidates,
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

struct StubLauncher {
    launcher_id: &'static str,
}

impl Launcher for StubLauncher {
    fn id(&self) -> &str {
        self.launcher_id
    }

    fn probe(&self) -> Availability {
        Availability::Available
    }

    fn capabilities(&self) -> LauncherCaps {
        LauncherCaps::default()
    }

    fn launch(
        &self,
        _req: &LaunchRequest,
    ) -> Result<bam_core::launch::LaunchHandle, LauncherError> {
        unimplemented!("not exercised by these tests")
    }
}

#[test]
fn with_no_configured_path_defaults_are_probed_in_order() {
    assert_eq!(
        resolve_candidates(vec![PathBuf::from("/a"), PathBuf::from("/b")], None),
        vec![PathBuf::from("/a"), PathBuf::from("/b")]
    );

    // First-in-order candidate wins even when a later one also exists.
    let dir = std::env::temp_dir().join(format!("bam-launch-config-order-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let first = dir.join("first");
    let second = dir.join("second");
    fs::write(&first, b"").unwrap();
    fs::write(&second, b"").unwrap();

    let launcher =
        FsUaeLauncher::with_candidates(EmptyStore, registry(), vec![first.clone(), second.clone()]);
    assert_eq!(launcher.probe(), Availability::Available);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_explicit_path_overrides_the_candidates() {
    let configured = PathBuf::from("/custom/fs-uae");
    assert_eq!(
        resolve_candidates(
            vec![PathBuf::from("/a"), PathBuf::from("/b")],
            Some(configured.clone())
        ),
        vec![configured]
    );
}

#[test]
fn extra_arguments_reach_the_spawned_command_line() {
    let dir = std::env::temp_dir().join(format!("bam-launch-config-args-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let recorder = dir.join("recorder.sh");
    fs::write(&recorder, "#!/bin/sh\necho \"$@\" > \"$0.out\"\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&recorder, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let launcher = FsUaeLauncher::with_candidates_and_args(
        EmptyStore,
        registry(),
        vec![recorder.clone()],
        vec!["--fullscreen".to_string(), "--foo".to_string()],
    );
    let req = LaunchRequest {
        required: LauncherCaps::default(),
        archive: Some(LaunchArchive {
            blob: BlobHash::from_hex("deadbeef"),
            format: ArchiveFormat::Zip,
        }),
    };
    let handle = launcher.launch(&req).unwrap();

    let out = recorder.with_extension("sh.out");
    let deadline = Instant::now() + Duration::from_secs(2);
    while !out.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let recorded = fs::read_to_string(&out).unwrap();
    assert!(recorded.contains("--fullscreen"));
    assert!(recorded.contains("--foo"));

    drop(handle);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_launcher_id_in_preference_errors_naming_it() {
    let mut reg = LauncherRegistry::new();
    reg.register(Box::new(StubLauncher {
        launcher_id: "fs-uae",
    }));

    let err = reg.apply_preference(&["amiberry".to_string()]).unwrap_err();
    assert!(matches!(err, LauncherError::UnknownLauncher(id) if id == "amiberry"));
}

#[test]
fn preference_order_reorders_registered_launchers() {
    let mut reg = LauncherRegistry::new();
    reg.register(Box::new(StubLauncher {
        launcher_id: "fs-uae",
    }));
    reg.register(Box::new(StubLauncher {
        launcher_id: "amiberry",
    }));

    reg.apply_preference(&["amiberry".to_string(), "fs-uae".to_string()])
        .unwrap();

    let selected = reg.select(&LaunchRequest::default(), None).unwrap();
    assert_eq!(selected.id(), "amiberry");
}
