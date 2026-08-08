//! P8.2 test fixture: an `unpacker` plugin (bam's WASM plugin contract,
//! see crates/bam-core/src/plugin/mod.rs) that always reports available
//! and, on `unpack`, echoes the input bytes back as a single file
//! `echo.txt` — proof that the host<->plugin JSON round-trip and the
//! registry integration work, without needing real archive parsing inside
//! WASM (that's P8.4).
//!
//! Rebuild with: cargo build --target wasm32-unknown-unknown --release
//! then copy target/wasm32-unknown-unknown/release/echo_unpacker.wasm to
//! crates/bam-core/tests/fixtures/plugins/echo-unpacker/plugin.wasm

use extism_pdk::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ProbeResponse {
    available: bool,
    reason: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct UnpackRequest {
    bytes_b64: String,
}

#[derive(Serialize, Deserialize)]
struct UnpackedFile {
    path: String,
    bytes_b64: String,
}

#[derive(Serialize, Deserialize)]
struct UnpackResponse {
    files: Vec<UnpackedFile>,
}

#[plugin_fn]
pub fn probe(_: ()) -> FnResult<Json<ProbeResponse>> {
    Ok(Json(ProbeResponse {
        available: true,
        reason: None,
    }))
}

#[plugin_fn]
pub fn unpack(Json(req): Json<UnpackRequest>) -> FnResult<Json<UnpackResponse>> {
    Ok(Json(UnpackResponse {
        files: vec![UnpackedFile {
            path: "echo.txt".to_string(),
            bytes_b64: req.bytes_b64,
        }],
    }))
}
