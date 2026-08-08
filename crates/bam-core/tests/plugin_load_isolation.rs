//! P8.5 — plugin loading configuration and failure isolation: a broken
//! plugin degrades one feature, never the application. Uses
//! `tests/fixtures/plugins/misbehaving-unpacker/`, whose `unpack` panics,
//! loops, or hogs memory depending on the archive bytes it's given (see
//! that fixture's `src-provenance` for why one plugin covers all three).

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use bam_core::blob::{BlobStore, FsBlobStore};
use bam_core::plugin::{PluginConfig, PluginLoadError, WasmUnpacker, discover_unpackers};
use bam_core::unpack::Unpacker;

fn temp_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "bam-plugin-isolation-test-{label}-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugins")
}

fn fixture_dir(name: &str) -> PathBuf {
    fixtures_root().join(name)
}

#[test]
fn a_panicking_plugin_call_is_caught_and_reported() {
    let dir = temp_dir("panic");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let hash = store.put(Cursor::new(b"panic".to_vec())).unwrap();
    let plugin = WasmUnpacker::load(&fixture_dir("misbehaving-unpacker"), store).unwrap();

    let err = plugin.unpack(&hash, &dir.join("dest")).unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("misbehaving-unpacker") || message.to_lowercase().contains("panic"),
        "expected the error to name the failure, got: {message}"
    );

    // The host itself is unharmed: the same plugin instance still answers.
    assert!(matches!(
        plugin.probe(),
        bam_core::unpack::Availability::Available
    ));
}

#[test]
fn a_plugin_exceeding_its_time_limit_is_terminated() {
    let dir = temp_dir("timeout");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let hash = store.put(Cursor::new(b"loop".to_vec())).unwrap();
    let config = PluginConfig {
        timeout_ms: Some(200),
        ..Default::default()
    };
    let plugin =
        WasmUnpacker::load_with_config(&fixture_dir("misbehaving-unpacker"), store, &config)
            .unwrap();

    let start = std::time::Instant::now();
    let result = plugin.unpack(&hash, &dir.join("dest"));
    assert!(result.is_err(), "expected the timeout to fail the call");
    assert!(
        start.elapsed() < std::time::Duration::from_secs(5),
        "the timeout should have terminated the call quickly, took {:?}",
        start.elapsed()
    );
}

#[test]
fn a_plugin_exceeding_its_memory_limit_is_terminated() {
    let dir = temp_dir("memory");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let hash = store.put(Cursor::new(b"memory".to_vec())).unwrap();
    let config = PluginConfig {
        max_memory_pages: Some(16), // 16 * 64KiB = 1MiB, far below the hog's appetite
        timeout_ms: Some(5_000),    // belt and braces: don't hang the suite if unbounded
        ..Default::default()
    };
    let plugin =
        WasmUnpacker::load_with_config(&fixture_dir("misbehaving-unpacker"), store, &config)
            .unwrap();

    let result = plugin.unpack(&hash, &dir.join("dest"));
    assert!(
        result.is_err(),
        "expected the memory limit to fail the call"
    );
}

#[test]
fn a_plugin_failing_to_load_is_reported_without_blocking_startup() {
    let harness = temp_dir("discover-broken");
    let root = harness.join("plugins");
    std::fs::create_dir_all(&root).unwrap();
    // A malformed manifest: missing `extension_point`.
    let broken = root.join("broken-plugin");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(
        broken.join("manifest.toml"),
        "name = \"broken\"\nversion = \"0.1.0\"\napi_version = 1\n",
    )
    .unwrap();

    // A good plugin alongside it.
    let good = root.join("good-plugin");
    std::fs::create_dir_all(&good).unwrap();
    std::fs::copy(
        fixture_dir("misbehaving-unpacker").join("manifest.toml"),
        good.join("manifest.toml"),
    )
    .unwrap();
    std::fs::copy(
        fixture_dir("misbehaving-unpacker").join("plugin.wasm"),
        good.join("plugin.wasm"),
    )
    .unwrap();

    let store = FsBlobStore::new(harness.join("blobs")).unwrap();
    let (loaded, failures) = discover_unpackers(&root, &store, &PluginConfig::default());

    assert_eq!(loaded.len(), 1, "the good plugin should still load");
    assert_eq!(failures.len(), 1, "the broken plugin should be reported");
    assert_eq!(failures[0].dir, broken);
    assert!(!failures[0].error.is_empty());
}

#[test]
fn disabling_a_plugin_in_config_prevents_loading_it_entirely() {
    let dir = temp_dir("disabled");
    let store = FsBlobStore::new(dir.join("blobs")).unwrap();
    let config = PluginConfig {
        disabled: vec!["misbehaving-unpacker".to_string()],
        ..Default::default()
    };

    let result =
        WasmUnpacker::load_with_config(&fixture_dir("misbehaving-unpacker"), store, &config);
    match result {
        Err(PluginLoadError::Disabled(name)) => assert_eq!(name, "misbehaving-unpacker"),
        Ok(_) => panic!("expected the disabled plugin to be rejected"),
        Err(_) => panic!("expected PluginLoadError::Disabled"),
    }
}
