//! P8.2 — extism host and registry integration. Uses the fixture plugin
//! under `tests/fixtures/plugins/echo-unpacker/`: it always reports
//! available and echoes its input bytes back as a single `echo.txt` file,
//! which is enough to prove the registry/JSON-contract mechanism without
//! needing real archive parsing inside WASM (that's P8.4).

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use bam_core::blob::{BlobStore, FsBlobStore};
use bam_core::plugin::WasmUnpacker;
use bam_core::unpack::{ArchiveFormat, Availability, Unpacker, UnpackerRegistry, ZipUnpacker};

fn temp_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "bam-wasm-unpacker-test-{label}-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugins/echo-unpacker")
}

#[test]
fn wasm_plugin_registers_and_is_selected_by_normal_selection_logic() {
    let dir = temp_dir("select");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let plugin = WasmUnpacker::load(&fixture_dir(), store).unwrap();

    let mut registry = UnpackerRegistry::new();
    registry.register(Box::new(plugin));

    let selected = registry.select(ArchiveFormat::Zip, None).unwrap();
    assert_eq!(selected.id(), "echo-unpacker");
}

#[test]
fn host_and_plugin_exchange_json_and_bytes_round_trip() {
    let dir = temp_dir("roundtrip");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let hash = store.put(Cursor::new(b"hello wasm".to_vec())).unwrap();

    let plugin = WasmUnpacker::load(&fixture_dir(), store).unwrap();
    assert_eq!(plugin.probe(), Availability::Available);

    let dest = dir.join("dest");
    let files = plugin.unpack(&hash, &dest).unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, PathBuf::from("echo.txt"));
    assert_eq!(std::fs::read(dest.join("echo.txt")).unwrap(), b"hello wasm");
}

#[test]
fn native_and_wasm_backends_resolve_by_registration_order() {
    let dir = temp_dir("preference");

    // Native registered first: it wins.
    let mut registry = UnpackerRegistry::new();
    registry.register(Box::new(ZipUnpacker::new(
        FsBlobStore::new(dir.join("blobs-a")).unwrap(),
    )));
    registry.register(Box::new(
        WasmUnpacker::load(
            &fixture_dir(),
            FsBlobStore::new(dir.join("blobs-a")).unwrap(),
        )
        .unwrap(),
    ));
    assert_eq!(
        registry.select(ArchiveFormat::Zip, None).unwrap().id(),
        "zip"
    );

    // WASM registered first: it wins instead — same rule, no special-casing.
    let mut registry = UnpackerRegistry::new();
    registry.register(Box::new(
        WasmUnpacker::load(
            &fixture_dir(),
            FsBlobStore::new(dir.join("blobs-b")).unwrap(),
        )
        .unwrap(),
    ));
    registry.register(Box::new(ZipUnpacker::new(
        FsBlobStore::new(dir.join("blobs-b")).unwrap(),
    )));
    assert_eq!(
        registry.select(ArchiveFormat::Zip, None).unwrap().id(),
        "echo-unpacker"
    );
}

#[test]
fn loading_the_same_plugin_twice_is_idempotent() {
    let dir = temp_dir("idempotent");

    let mut registry = UnpackerRegistry::new();
    registry.register(Box::new(
        WasmUnpacker::load(&fixture_dir(), FsBlobStore::new(dir.join("blobs")).unwrap()).unwrap(),
    ));
    registry.register(Box::new(
        WasmUnpacker::load(&fixture_dir(), FsBlobStore::new(dir.join("blobs")).unwrap()).unwrap(),
    ));

    let selected = registry.select(ArchiveFormat::Zip, None).unwrap();
    assert_eq!(selected.id(), "echo-unpacker");
    assert_eq!(selected.probe(), Availability::Available);
}
