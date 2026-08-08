//! P8.2: the `extism` host, wired into `UnpackerRegistry` through the same
//! [`Unpacker`] trait every native backend implements — no parallel
//! dispatch path, per I4. A `WasmUnpacker` is a directory containing
//! `manifest.toml` (P8.1) and `plugin.wasm`, exporting `probe` and `unpack`
//! functions that exchange this module's `Unpack*` contract types as JSON.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use extism::convert::Json;
use thiserror::Error;

use crate::blob::{BlobHash, BlobStore};
use crate::unpack::{ArchiveFormat, Availability, ExtractedFile, UnpackError, Unpacker};

use super::{
    ContentAnalyzerInput, ContentAnalyzerOutput, ManifestError, PluginConfig, PluginManifest,
    UnpackProbeResponse, UnpackRequest, UnpackResponse,
};

#[derive(Debug, Error)]
pub enum PluginLoadError {
    #[error("plugin manifest: {0}")]
    Manifest(#[from] ManifestError),
    #[error("plugin extension_point '{found}' is not '{expected}'")]
    WrongExtensionPoint { found: String, expected: String },
    #[error("io error loading plugin: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to load wasm module: {0}")]
    Wasm(String),
    #[error("plugin '{0}' is disabled in config")]
    Disabled(String),
}

/// Builds an `extism::Plugin` with P8.5's resource limits applied: a call
/// exceeding `timeout_ms` is interrupted, one exceeding `max_memory_pages`
/// is trapped — both surface as an ordinary `Err` from `plugin.call`, the
/// same path a guest panic/trap already takes, so no extra catch-unwind
/// wrapper is needed at the call sites.
fn build_plugin(
    wasm_bytes: &[u8],
    config: &PluginConfig,
) -> Result<extism::Plugin, PluginLoadError> {
    let mut manifest = extism::Manifest::new([extism::Wasm::data(wasm_bytes.to_vec())]);
    manifest.timeout_ms = config.timeout_ms;
    if let Some(pages) = config.max_memory_pages {
        manifest = manifest.with_memory_max(pages);
    }
    extism::Plugin::new(manifest, [], true).map_err(|e| PluginLoadError::Wasm(e.to_string()))
}

/// One plugin directory under a discovery root that failed to load —
/// reported at startup, naming the directory, without stopping discovery of
/// the rest (P8.5).
#[derive(Debug, Clone)]
pub struct PluginLoadReport {
    pub dir: std::path::PathBuf,
    pub error: String,
}

#[derive(Debug, Error)]
pub enum AnalyzeError {
    #[error("plugin call failed: {0}")]
    Wasm(String),
    #[error("plugin returned malformed JSON: {0}")]
    MalformedOutput(#[from] serde_json::Error),
}

/// A WASM-backed [`Unpacker`], loaded from `dir/manifest.toml` +
/// `dir/plugin.wasm`. `S` is the same [`BlobStore`] every native backend
/// takes — the plugin never touches the filesystem or blob store directly,
/// it only receives bytes and returns bytes.
pub struct WasmUnpacker<S: BlobStore> {
    manifest: PluginManifest,
    plugin: Mutex<extism::Plugin>,
    store: S,
}

fn claim_glob(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::Lha => "*.lha",
        ArchiveFormat::Lzx => "*.lzx",
        ArchiveFormat::Zip => "*.zip",
    }
}

impl<S: BlobStore> WasmUnpacker<S> {
    pub fn load(dir: &Path, store: S) -> Result<Self, PluginLoadError> {
        Self::load_with_config(dir, store, &PluginConfig::default())
    }

    /// Same as [`Self::load`], but honouring P8.5's `[plugins]` config: a
    /// disabled plugin is rejected before its `.wasm` is even read, and
    /// `config`'s resource limits are applied to every subsequent call.
    pub fn load_with_config(
        dir: &Path,
        store: S,
        config: &PluginConfig,
    ) -> Result<Self, PluginLoadError> {
        let manifest_src = fs::read_to_string(dir.join("manifest.toml"))?;
        let manifest = PluginManifest::parse(&manifest_src)?;
        if manifest.extension_point != "unpacker" {
            return Err(PluginLoadError::WrongExtensionPoint {
                found: manifest.extension_point,
                expected: "unpacker".to_string(),
            });
        }
        if config.disabled.contains(&manifest.name) {
            return Err(PluginLoadError::Disabled(manifest.name));
        }
        let wasm_bytes = fs::read(dir.join("plugin.wasm"))?;
        let plugin = build_plugin(&wasm_bytes, config)?;
        Ok(Self {
            manifest,
            plugin: Mutex::new(plugin),
            store,
        })
    }
}

/// Loads every plugin subdirectory of `root` as a `WasmUnpacker` (P8.5): a
/// directory that fails to load — malformed manifest, missing `.wasm`,
/// disabled in `config` — is collected into a report instead of stopping
/// discovery, so one broken plugin never blocks startup or the rest of the
/// registry from loading.
pub fn discover_unpackers<S: BlobStore + Clone>(
    root: &Path,
    store: &S,
    config: &PluginConfig,
) -> (Vec<WasmUnpacker<S>>, Vec<PluginLoadReport>) {
    let mut loaded = Vec::new();
    let mut failures = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return (loaded, failures);
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        match WasmUnpacker::load_with_config(&dir, store.clone(), config) {
            Ok(plugin) => loaded.push(plugin),
            Err(e) => failures.push(PluginLoadReport {
                dir,
                error: e.to_string(),
            }),
        }
    }
    (loaded, failures)
}

impl<S: BlobStore> Unpacker for WasmUnpacker<S> {
    fn id(&self) -> &str {
        &self.manifest.name
    }

    fn handles(&self, format: ArchiveFormat) -> bool {
        self.manifest.claims_file(claim_glob(format))
    }

    fn probe(&self) -> Availability {
        let mut plugin = self.plugin.lock().expect("plugin mutex poisoned");
        match plugin.call::<&str, Json<UnpackProbeResponse>>("probe", "") {
            Ok(Json(out)) if out.available => Availability::Available,
            Ok(Json(out)) => Availability::Unavailable {
                reason: out
                    .reason
                    .unwrap_or_else(|| "plugin reported unavailable".into()),
            },
            Err(e) => Availability::Unavailable {
                reason: e.to_string(),
            },
        }
    }

    fn unpack(&self, blob: &BlobHash, dest: &Path) -> Result<Vec<ExtractedFile>, UnpackError> {
        let mut bytes = Vec::new();
        self.store.get(blob)?.read_to_end(&mut bytes)?;
        let req = UnpackRequest {
            bytes_b64: BASE64.encode(&bytes),
        };

        let mut plugin = self.plugin.lock().expect("plugin mutex poisoned");
        let Json(resp): Json<UnpackResponse> =
            plugin
                .call("unpack", Json(req))
                .map_err(|e| UnpackError::ExtractionFailed {
                    message: e.to_string(),
                })?;
        drop(plugin);

        // Validate every entry (path traversal, base64) before writing any
        // of them, so a malicious entry found after a safe one leaves no
        // partial output behind — the same guarantee `unar`/`zip` hold via
        // their scratch-then-move pattern (P5.4/P5.5).
        let mut decoded = Vec::with_capacity(resp.files.len());
        for f in &resp.files {
            let rel = Path::new(&f.path);
            if rel.is_absolute()
                || rel
                    .components()
                    .any(|c| c == std::path::Component::ParentDir)
            {
                return Err(UnpackError::PathTraversal {
                    entry: f.path.clone(),
                });
            }
            let content =
                BASE64
                    .decode(&f.bytes_b64)
                    .map_err(|e| UnpackError::ExtractionFailed {
                        message: format!("plugin returned invalid base64 for '{}': {e}", f.path),
                    })?;
            decoded.push((rel.to_path_buf(), content));
        }

        fs::create_dir_all(dest)?;
        let mut files = Vec::with_capacity(decoded.len());
        for (rel, content) in decoded {
            let target = dest.join(&rel);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&target, &content)?;
            files.push(ExtractedFile {
                path: rel,
                size: content.len() as u64,
            });
        }
        Ok(files)
    }
}

/// A WASM-backed `content_analyzer` plugin (P8.3), loaded the same way as
/// [`WasmUnpacker`]. It has no [`BlobStore`] — the host, not the plugin,
/// decides which extracted files to feed it and passes their bytes
/// directly, one call per file.
pub struct WasmContentAnalyzer {
    manifest: PluginManifest,
    plugin: Mutex<extism::Plugin>,
}

impl WasmContentAnalyzer {
    pub fn load(dir: &Path) -> Result<Self, PluginLoadError> {
        Self::load_with_config(dir, &PluginConfig::default())
    }

    /// Same as [`Self::load`], but honouring P8.5's `[plugins]` config —
    /// see [`WasmUnpacker::load_with_config`].
    pub fn load_with_config(dir: &Path, config: &PluginConfig) -> Result<Self, PluginLoadError> {
        let manifest_src = fs::read_to_string(dir.join("manifest.toml"))?;
        let manifest = PluginManifest::parse(&manifest_src)?;
        if manifest.extension_point != "content_analyzer" {
            return Err(PluginLoadError::WrongExtensionPoint {
                found: manifest.extension_point,
                expected: "content_analyzer".to_string(),
            });
        }
        if config.disabled.contains(&manifest.name) {
            return Err(PluginLoadError::Disabled(manifest.name));
        }
        let wasm_bytes = fs::read(dir.join("plugin.wasm"))?;
        let plugin = build_plugin(&wasm_bytes, config)?;
        Ok(Self {
            manifest,
            plugin: Mutex::new(plugin),
        })
    }

    pub fn id(&self) -> &str {
        &self.manifest.name
    }

    pub fn version(&self) -> &str {
        &self.manifest.version
    }

    /// `claims` pre-filtering (P8.1): whether this plugin should even be
    /// offered `filename` — the host's job to check before calling
    /// [`Self::analyze`], not the plugin's.
    pub fn claims_file(&self, filename: &str) -> bool {
        self.manifest.claims_file(filename)
    }

    /// Calls the plugin's `analyze` export. The output is read as a raw
    /// string and parsed here, rather than through extism's typed `Json<T>`
    /// convert on the guest side, so a plugin returning malformed JSON
    /// surfaces as [`AnalyzeError::MalformedOutput`] instead of a host
    /// panic (I4: a plugin is less trusted than in-tree code).
    pub fn analyze(
        &self,
        input: &ContentAnalyzerInput,
    ) -> Result<ContentAnalyzerOutput, AnalyzeError> {
        let mut plugin = self.plugin.lock().expect("plugin mutex poisoned");
        let raw: String = plugin
            .call("analyze", Json(input))
            .map_err(|e| AnalyzeError::Wasm(e.to_string()))?;
        Ok(serde_json::from_str(&raw)?)
    }
}

/// Loads every plugin subdirectory of `root` as a `WasmContentAnalyzer` —
/// see [`discover_unpackers`].
pub fn discover_content_analyzers(
    root: &Path,
    config: &PluginConfig,
) -> (Vec<WasmContentAnalyzer>, Vec<PluginLoadReport>) {
    let mut loaded = Vec::new();
    let mut failures = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return (loaded, failures);
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        match WasmContentAnalyzer::load_with_config(&dir, config) {
            Ok(plugin) => loaded.push(plugin),
            Err(e) => failures.push(PluginLoadReport {
                dir,
                error: e.to_string(),
            }),
        }
    }
    (loaded, failures)
}
