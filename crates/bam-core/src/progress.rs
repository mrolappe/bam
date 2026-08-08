//! Typed, serializable progress events (invariant I5): the core reports
//! progress as data, never as a formatted string — a CLI progress bar, a
//! web progress bar, and a future JSON-RPC client all consume the same
//! typed sequence, each rendering it however fits.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct OperationId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Outcome {
    Success,
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ProgressEvent {
    Started {
        operation: OperationId,
        total: Option<u64>,
    },
    Advanced {
        operation: OperationId,
        done: u64,
    },
    Finished {
        operation: OperationId,
        outcome: Outcome,
    },
}

/// Implemented by whatever a caller wants progress rendered as — a CLI
/// progress bar, a recording sink for tests, a web client's event stream.
pub trait ProgressSink {
    fn emit(&mut self, event: ProgressEvent);
}
