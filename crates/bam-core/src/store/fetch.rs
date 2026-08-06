//! HTTP fetch orchestration (P1.9): conditional GET against a per-URL stored
//! ETag, gunzip, land the raw lines. The only file that mixes `HttpClient`
//! with `rusqlite` — everything else in `http::` stays free of it.

use rusqlite::Connection;
use thiserror::Error;

use super::land::land_lines;
use super::tables::{get_etag, set_etag};
use crate::http::{HttpClient, HttpError, HttpRequest};
use crate::ingest::gzip::{GunzipError, gunzip};

#[derive(Debug, Error)]
pub enum FetchError {
    #[error(transparent)]
    Http(#[from] HttpError),
    #[error(transparent)]
    Gunzip(#[from] GunzipError),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchOutcome {
    Fetched { landing_ids: Vec<i64> },
    NotModified,
}

/// Fetches a `.gz` INDEX/RECENT URL with conditional GET, gunzips a 200
/// response, and lands its raw lines. A 304 lands nothing and is not an
/// error; an error from `client` (e.g. a 500) propagates before anything is
/// written, so a failed fetch never lands a partial file.
pub async fn fetch_and_land(
    conn: &Connection,
    client: &impl HttpClient,
    url: &str,
    fetched_at: &str,
) -> Result<FetchOutcome, FetchError> {
    let if_none_match = get_etag(conn, url)?;
    let resp = client
        .get(HttpRequest {
            url: url.to_string(),
            if_none_match,
        })
        .await?;

    if resp.status == 304 {
        return Ok(FetchOutcome::NotModified);
    }

    if let Some(etag) = &resp.etag {
        set_etag(conn, url, etag)?;
    }

    let body = gunzip(&resp.body)?;
    let landing_ids = land_lines(conn, url, fetched_at, &body)?;
    Ok(FetchOutcome::Fetched { landing_ids })
}
