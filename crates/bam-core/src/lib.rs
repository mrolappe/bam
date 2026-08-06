pub mod http;
pub mod ingest;
pub mod progress;
pub mod query;
#[cfg(feature = "native")]
pub mod store;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Current UTC time as RFC3339, for stamping a landing fetch's `fetched_at`.
pub fn now_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (y, m, d) = ingest::normalize::civil_from_days((secs / 86_400) as i64);
    let rem = secs % 86_400;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}
