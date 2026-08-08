//! `HttpClient` trait (invariant I1: `bam-core` must not call `reqwest`
//! directly). The trait and its data types are plain, ungated code so a
//! fake implementation can drive ingest with no network and no `native`
//! feature; [`ReqwestClient`] is the real, `native`-gated implementation.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub url: String,
    /// Sent as `If-None-Match` when set, for conditional GET.
    pub if_none_match: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub etag: Option<String>,
}

/// A JSON POST, for P7.1's LLM provider — separate from [`HttpRequest`]
/// rather than adding optional fields to it, so the many existing GET-only
/// fakes are untouched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpPostRequest {
    pub url: String,
    pub body: Vec<u8>,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HttpError {
    #[error("http request failed: {0}")]
    Request(String),
    /// A response outside 2xx/304, with the status code broken out so a
    /// caller (P4.3's worker) can tell a retryable 429/5xx from a permanent
    /// 4xx without parsing [`HttpError::Request`]'s message string.
    #[error("http request failed: unexpected status {0}")]
    Status(u16),
}

// Used only via generics (`impl HttpClient`, never `dyn`), so the missing
// auto-trait bounds this lint warns about don't apply here.
#[allow(async_fn_in_trait)]
pub trait HttpClient {
    async fn get(&self, req: HttpRequest) -> Result<HttpResponse, HttpError>;

    /// Default errors out; only implementations that need it (currently
    /// [`ReqwestClient`] and P7.1's LLM test fakes) override it.
    async fn post(&self, req: HttpPostRequest) -> Result<HttpResponse, HttpError> {
        let _ = req;
        Err(HttpError::Request(
            "this client does not support POST".into(),
        ))
    }
}

#[cfg(feature = "native")]
mod reqwest_client;
#[cfg(feature = "native")]
pub use reqwest_client::{ReqwestClient, USER_AGENT};
