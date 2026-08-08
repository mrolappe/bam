//! `bam-server` (P9.2): a thin HTTP/SSE adapter over `bam_core::api`. No
//! SQL, no query logic — every handler in [`routes`] just deserializes a
//! request, calls into `bam_core::api`, and serializes the response.

mod routes;
mod session_layer;
pub mod state;

pub use state::AppState;

pub fn app(state: std::sync::Arc<AppState>) -> axum::Router {
    routes::router(state)
}
