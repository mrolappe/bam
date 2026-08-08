//! P8.4 test fixture: an `unpacker` plugin that always reports itself
//! unavailable, so a test can prove `probe()` is actually honoured by
//! `UnpackerRegistry::select` (not just called and ignored) — distinct
//! from P8.2's coverage, which only ever exercised the available path.
//!
//! Rebuild with: cargo build --target wasm32-unknown-unknown --release
//! then copy target/wasm32-unknown-unknown/release/unavailable_unpacker.wasm
//! to crates/bam-core/tests/fixtures/plugins/unavailable-unpacker/plugin.wasm

use extism_pdk::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ProbeResponse {
    available: bool,
    reason: Option<String>,
}

#[plugin_fn]
pub fn probe(_: ()) -> FnResult<Json<ProbeResponse>> {
    Ok(Json(ProbeResponse {
        available: false,
        reason: Some("deliberately unavailable, for testing".to_string()),
    }))
}

#[plugin_fn]
pub fn unpack(_: String) -> FnResult<String> {
    Err(Error::msg("unavailable-unpacker should never be called").into())
}
