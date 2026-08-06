pub mod ingest;
#[cfg(feature = "native")]
pub mod store;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
