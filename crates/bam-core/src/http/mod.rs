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

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HttpError {
    #[error("http request failed: {0}")]
    Request(String),
}

// Used only via generics (`impl HttpClient`, never `dyn`), so the missing
// auto-trait bounds this lint warns about don't apply here.
#[allow(async_fn_in_trait)]
pub trait HttpClient {
    async fn get(&self, req: HttpRequest) -> Result<HttpResponse, HttpError>;
}

#[cfg(feature = "native")]
mod reqwest_client;
#[cfg(feature = "native")]
pub use reqwest_client::{ReqwestClient, USER_AGENT};
