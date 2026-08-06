use reqwest::Client;
use reqwest::header::ETAG;

use super::{HttpClient, HttpError, HttpRequest, HttpResponse};

/// Descriptive per invariant I1's hand-over: names the tool and gives a
/// contact point, as Aminet mirror operators ask of automated clients.
pub const USER_AGENT: &str = concat!(
    "bam/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/mrolappe/bam)"
);

pub struct ReqwestClient {
    inner: Client,
}

impl ReqwestClient {
    pub fn new() -> Self {
        Self {
            inner: Client::new(),
        }
    }
}

impl Default for ReqwestClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient for ReqwestClient {
    async fn get(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        let mut builder = self.inner.get(&req.url).header("User-Agent", USER_AGENT);
        if let Some(etag) = &req.if_none_match {
            builder = builder.header("If-None-Match", etag.as_str());
        }

        let resp = builder
            .send()
            .await
            .map_err(|e| HttpError::Request(e.to_string()))?;
        let status = resp.status().as_u16();
        let etag = resp
            .headers()
            .get(ETAG)
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        if status == 304 {
            return Ok(HttpResponse {
                status,
                body: Vec::new(),
                etag,
            });
        }
        if !resp.status().is_success() {
            return Err(HttpError::Request(format!("unexpected status {status}")));
        }

        let body = resp
            .bytes()
            .await
            .map_err(|e| HttpError::Request(e.to_string()))?
            .to_vec();
        Ok(HttpResponse { status, body, etag })
    }
}
