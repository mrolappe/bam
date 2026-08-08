//! WASM plugin host (Phase 8, §9): third-party extensions loaded through
//! `extism`. This module holds the versioned, host-independent contract —
//! manifest parsing and per-extension-point JSON schemas — so it compiles
//! everywhere `bam-core` does; the `extism` runtime itself is native-only
//! and lands in P8.2.

use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// The contract major version this host implements. A manifest naming a
/// higher `api_version` is rejected outright — running it against a
/// contract shape the host doesn't know would let it start half-broken.
pub const HOST_API_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub api_version: u32,
    pub extension_point: String,
    #[serde(default)]
    pub claims: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error(
        "plugin manifest requests api_version {found}, this host supports up to {supported}; upgrade bam or the plugin"
    )]
    UnsupportedApiVersion { found: u32, supported: u32 },
    #[error("malformed plugin manifest: {0}")]
    Malformed(#[from] toml::de::Error),
}

impl PluginManifest {
    pub fn parse(src: &str) -> Result<Self, ManifestError> {
        let manifest: PluginManifest = toml::from_str(src)?;
        if manifest.api_version > HOST_API_VERSION {
            return Err(ManifestError::UnsupportedApiVersion {
                found: manifest.api_version,
                supported: HOST_API_VERSION,
            });
        }
        Ok(manifest)
    }

    /// Whether `filename` matches one of `claims`'s patterns. Patterns carry
    /// at most one `*` wildcard (`*.mod`, `mod.*`) — the only shape §9's
    /// examples use — so the host can pre-filter without a glob dependency.
    pub fn claims_file(&self, filename: &str) -> bool {
        self.claims.iter().any(|pat| glob_match(pat, filename))
    }
}

fn glob_match(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        Some((prefix, suffix)) => name.starts_with(prefix) && name.ends_with(suffix),
        None => pattern == name,
    }
}

/// P8.3's concrete extension point: input passed to a `content_analyzer`
/// plugin for one extracted file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentAnalyzerInput {
    pub path: String,
    pub size: u64,
    pub bytes_b64: String,
    pub hint: String,
}

/// P8.2's extension point: an archive's bytes, passed to an `unpacker`
/// plugin's `unpack` export.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UnpackRequest {
    pub bytes_b64: String,
}

/// One extracted file, as returned by an `unpacker` plugin. The host writes
/// `bytes_b64` to `dest` itself — a plugin proposes paths, it never touches
/// the filesystem — so the same path-traversal check P5.4/P5.5 apply here
/// too (I4: a plugin is less trusted than in-tree code, not more).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UnpackedFile {
    pub path: String,
    pub bytes_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UnpackResponse {
    pub files: Vec<UnpackedFile>,
}

/// Response from an `unpacker` plugin's `probe` export.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UnpackProbeResponse {
    pub available: bool,
    #[serde(default)]
    pub reason: Option<String>,
}

/// Output of a `content_analyzer` plugin's `analyze` export for one file.
/// The host, not the plugin, deserializes this from a raw string rather
/// than through extism's typed `Json<T>` convert — a plugin's output is
/// untrusted the same way its unpacked bytes are (I4), so malformed JSON
/// must become a reportable, skippable error rather than a host panic.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentAnalyzerOutput {
    pub kind: String,
    pub confidence: f64,
    #[serde(default)]
    pub attributes: Value,
    pub searchable_text: String,
}

/// Returns the JSON Schema for `extension_point`'s input contract, `None`
/// for a point the host doesn't recognise. Generated straight from the
/// Rust type via `schemars`, so the published schema cannot drift from what
/// the host actually deserializes.
pub fn contract_schema(extension_point: &str) -> Option<Value> {
    match extension_point {
        "content_analyzer" => Some(
            serde_json::to_value(schema_for!(ContentAnalyzerInput)).expect("schema serializes"),
        ),
        "unpacker" => {
            Some(serde_json::to_value(schema_for!(UnpackRequest)).expect("schema serializes"))
        }
        _ => None,
    }
}

/// The `[plugins]` section of `bam.toml` (P8.5): which plugins are disabled
/// outright, plus the resource limits `extism` enforces per plugin call —
/// `bam-core` doesn't depend on `toml` itself, only on `serde`, same pattern
/// as `launch::LaunchConfig`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PluginConfig {
    #[serde(default)]
    pub disabled: Vec<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub max_memory_pages: Option<u32>,
}

#[cfg(feature = "native")]
mod wasm;
#[cfg(feature = "native")]
pub use wasm::{
    AnalyzeError, PluginLoadError, PluginLoadReport, WasmContentAnalyzer, WasmUnpacker,
};
#[cfg(feature = "native")]
pub use wasm::{discover_content_analyzers, discover_unpackers};
