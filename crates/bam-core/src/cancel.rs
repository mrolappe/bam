//! Cooperative cancellation (invariant I5): every long-running operation
//! accepts a `CancellationToken` and polls it between steps. Plain
//! `Arc<AtomicBool>` rather than `tokio_util::sync::CancellationToken` — the
//! only behavior any caller needs is "cancel" and "is it cancelled", and
//! this stays usable from a wasm build with no `native` feature and no
//! async runtime.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
