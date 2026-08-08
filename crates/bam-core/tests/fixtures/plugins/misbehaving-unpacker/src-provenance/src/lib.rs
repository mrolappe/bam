//! P8.5 test fixture: an `unpacker` plugin that misbehaves on request, to
//! exercise the host's failure isolation (panics, timeouts, memory limits)
//! without needing three separate fixture binaries. The "archive" bytes
//! double as a behavior tag (`b"panic"`, `b"loop"`, `b"memory"`) — decoded
//! from `bytes_b64` the same way a real archive would be, so the test goes
//! through the ordinary `Unpacker::unpack` path via ordinary blob content.
//!
//! Rebuild with: cargo build --target wasm32-unknown-unknown --release
//! then copy target/wasm32-unknown-unknown/release/misbehaving_unpacker.wasm
//! to crates/bam-core/tests/fixtures/plugins/misbehaving-unpacker/plugin.wasm

use base64::Engine;
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
    let raw = base64::engine::general_purpose::STANDARD
        .decode(&req.bytes_b64)
        .unwrap_or_default();
    match raw.as_slice() {
        b"panic" => panic!("misbehaving-unpacker: deliberate panic"),
        b"loop" => loop {},
        b"memory" => {
            let mut hog: Vec<u8> = Vec::new();
            loop {
                hog.extend(std::iter::repeat(0u8).take(1 << 20));
                std::hint::black_box(&hog);
            }
        }
        _ => Ok(Json(UnpackResponse { files: Vec::new() })),
    }
}
