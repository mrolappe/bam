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

/// Returns the JSON Schema for `extension_point`'s input contract, `None`
/// for a point the host doesn't recognise. Generated straight from the
/// Rust type via `schemars`, so the published schema cannot drift from what
/// the host actually deserializes.
pub fn contract_schema(extension_point: &str) -> Option<Value> {
    match extension_point {
        "content_analyzer" => Some(
            serde_json::to_value(schema_for!(ContentAnalyzerInput)).expect("schema serializes"),
        ),
        _ => None,
    }
}
