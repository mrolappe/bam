//! P8.4 test fixture: an `unpacker` plugin that does real archive
//! extraction inside WASM (vs. P8.2's echo-unpacker, which just bounces
//! bytes back) — a genuine ZIP reader via the `zip` crate, proving I4's
//! registry claim against a second, non-trivial unpacker implementation.
//! Directory entries are skipped; every other entry's raw (untranslated)
//! name is returned as-is, including any `../` traversal a malicious
//! archive might contain — rejecting that is the host's job
//! (`WasmUnpacker::unpack`), not the plugin's, per I4 (a plugin is less
//! trusted than in-tree code).
//!
//! Rebuild with: cargo build --target wasm32-unknown-unknown --release
//! then copy target/wasm32-unknown-unknown/release/zip_unpacker.wasm to
//! crates/bam-core/tests/fixtures/plugins/zip-unpacker/plugin.wasm

use std::io::{Cursor, Read};

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
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&req.bytes_b64)
        .map_err(|e| Error::msg(format!("invalid base64 input: {e}")))?;

    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| Error::msg(format!("not a valid zip archive: {e}")))?;

    let mut files = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| Error::msg(format!("zip entry {i}: {e}")))?;
        if entry.is_dir() {
            continue;
        }
        let path = entry.name().to_string();
        let mut content = Vec::new();
        entry
            .read_to_end(&mut content)
            .map_err(|e| Error::msg(format!("reading '{path}': {e}")))?;
        files.push(UnpackedFile {
            path,
            bytes_b64: base64::engine::general_purpose::STANDARD.encode(&content),
        });
    }

    Ok(Json(UnpackResponse { files }))
}
