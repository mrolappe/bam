//! `bam-core::api` (P2.6, invariant I5): the use-case layer every adapter —
//! `bam-tui`, `bam-gui`, and eventually `bam-mcp`/`bam-server` — calls
//! through instead of driving [`Session`] or the store directly. Kept thin
//! by design: it adapts typed, serializable request/response pairs
//! ([`types`]) onto [`Session`]'s plain-Rust methods, and never touches a
//! database driver itself (P0.4's purity check covers this module too — it
//! must not name the confined driver, only `crate::store::*` may).
//!
//! Rules this module establishes, extending `bam-handoff.md` §8 for the web
//! variant (invariant I5):
//! 1. No stdout/stderr writes (checked by P0.4, already crate-wide).
//! 2. A long-running operation takes a [`CancellationToken`] — see
//!    [`ingest::start_ingest`], the only long operation that exists yet.
//! 3. Request/response types are `Serialize` + `Deserialize` + `JsonSchema`
//!    ([`types`]).
//! 4. No global mutable state — every call takes an explicit `&Session`.
//! 5. Progress is the typed [`crate::progress::ProgressEvent`] sequence,
//!    never a formatted string.
//! 6. A long operation returns an [`crate::progress::OperationId`]; its
//!    status stays queryable afterward via [`ingest::operation_status`], so
//!    a reconnecting client re-attaches instead of orphaning the run.

pub mod ingest;
pub mod query;
pub mod selection;
pub mod types;

pub use crate::cancel::CancellationToken;
pub use crate::store::session::{OperationStatus, SelectionMode, Session};
pub use ingest::{operation_status, start_ingest};
pub use query::{get_package, list_categories, parse_query, search_packages, search_window};
pub use selection::{
    clear, delete, is_marked, list, load, mark, save_as, select_by_query, toggle, unmark,
};
pub use types::*;

pub use crate::store::session::SessionError as Error;
