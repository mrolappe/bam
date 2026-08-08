//! P8.3 test fixture: a `content_analyzer` plugin (bam's WASM plugin
//! contract, see crates/bam-core/src/plugin/mod.rs) that always reports
//! available and, on `analyze`, classifies the input as `kind: "echo"` with
//! `searchable_text` derived from its decoded bytes — proof that the
//! content-analyzer host<->plugin JSON round-trip, FTS5 wiring, and
//! producer-version reprocessing all work, without needing real module
//! parsing inside WASM. A file whose path ends in `broken.mod` returns
//! intentionally malformed JSON, exercising the host's malformed-output
//! handling.
//!
//! Rebuild with: cargo build --target wasm32-unknown-unknown --release
//! then copy target/wasm32-unknown-unknown/release/echo_analyzer.wasm to
//! crates/bam-core/tests/fixtures/plugins/echo-analyzer/plugin.wasm

use base64::Engine;
use extism_pdk::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ProbeResponse {
    available: bool,
    reason: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ContentAnalyzerInput {
    path: String,
    size: u64,
    bytes_b64: String,
    hint: String,
}

#[derive(Serialize, Deserialize)]
struct ContentAnalyzerOutput {
    kind: String,
    confidence: f64,
    attributes: serde_json::Value,
    searchable_text: String,
}

#[plugin_fn]
pub fn probe(_: ()) -> FnResult<Json<ProbeResponse>> {
    Ok(Json(ProbeResponse {
        available: true,
        reason: None,
    }))
}

#[plugin_fn]
pub fn analyze(Json(req): Json<ContentAnalyzerInput>) -> FnResult<String> {
    if req.path.ends_with("broken.mod") {
        return Ok("not valid json{".to_string());
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&req.bytes_b64)
        .unwrap_or_default();
    let text = String::from_utf8_lossy(&bytes);

    let out = ContentAnalyzerOutput {
        kind: "echo".to_string(),
        confidence: 1.0,
        attributes: serde_json::json!({ "hint": req.hint }),
        searchable_text: format!("echo-analyzed {}: {text}", req.path),
    };
    Ok(serde_json::to_string(&out)?)
}
