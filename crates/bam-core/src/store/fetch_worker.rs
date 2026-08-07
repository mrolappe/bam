//! Background fetch worker (P4.3): pulls from P4.1's `fetch_queue`, honouring
//! `bam-handoff.md` §7's politeness requirements — the P4.2 rate limiter over
//! a single reused `HttpClient`, conditional GET via the queue's own stored
//! ETag, exponential backoff on 429/5xx, `robots.txt`, a descriptive
//! `User-Agent` (already `HttpClient::get`'s job, P1.9), and the priority
//! boost. That last one is free: `fetch_queue::enqueue`'s
//! `MAX(priority, ...)` upsert means re-enqueuing a currently-visible item
//! (P4.7's job) just raises its priority, and `claim_next`'s own
//! `ORDER BY priority DESC` is what serves it before the backlog — nothing
//! here needs to special-case "the visible window."

use std::collections::HashMap;
use std::time::Duration;

use rusqlite::Connection;

use super::fetch_queue;
use crate::http::{HttpClient, HttpError, HttpRequest};
use crate::ratelimit::{Clock, TokenBucket};
use crate::rfc3339_from_unix;
use crate::robots::{self, RobotsRules};

/// Stale-claim reclaim timeout: an abandoned claim (crashed worker, never
/// `mark_success`/`mark_failure`d) becomes reclaimable after this long.
pub const CLAIM_TIMEOUT_SECS: u64 = 300;
/// Backoff base: retry attempt N (0-indexed, before increment) waits
/// `BACKOFF_BASE_SECS * 2^N` seconds — increasing delays, never a tight loop.
pub const BACKOFF_BASE_SECS: u64 = 1;
/// Sentinel for "never retry automatically" (a permanent failure), matching
/// the far-future-timestamp convention `fetch_queue`'s own tests already use.
const FAR_FUTURE: &str = "2999-01-01T00:00:00Z";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchResult {
    Success {
        status: u16,
    },
    NotModified,
    RobotsDisallowed,
    /// A retryable failure (429/5xx); `next_attempt_at` is the backoff
    /// deadline just recorded.
    Retrying {
        status: Option<u16>,
        next_attempt_at: String,
    },
    /// A non-retryable failure; `next_attempt_at` is set to [`FAR_FUTURE`].
    PermanentFailure {
        status: Option<u16>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    /// The queue had nothing due to claim.
    Empty,
    /// No token was available; the caller should wait this long (real:
    /// `tokio::time::sleep`; test: advance a fake clock) and call `step`
    /// again — no claim was attempted, so nothing here needs undoing.
    RateLimited(Duration),
    /// An item was claimed and this is what happened to it.
    Fetched { url: String, result: FetchResult },
}

/// Per-origin `robots.txt` cache, reused across [`step`] calls so a bulk run
/// fetches each origin's `robots.txt` once, not once per queued item.
pub type RobotsCache = HashMap<String, RobotsRules>;

/// Runs one step of the worker loop: rate-limit, claim, robots-check, fetch,
/// record the outcome — one queue item per call, never a loop, so the caller
/// controls pacing and can interleave a `CancellationToken` check between
/// calls (P2.6's I5 convention).
pub async fn step<C: Clock>(
    conn: &Connection,
    client: &impl HttpClient,
    bucket: &TokenBucket<C>,
    robots_cache: &mut RobotsCache,
    now_unix: u64,
) -> Result<StepOutcome, rusqlite::Error> {
    if let Err(wait) = bucket.try_acquire() {
        return Ok(StepOutcome::RateLimited(wait));
    }

    let now = rfc3339_from_unix(now_unix);
    let stale_before = rfc3339_from_unix(now_unix.saturating_sub(CLAIM_TIMEOUT_SECS));
    let Some(item) = fetch_queue::claim_next(conn, &now, &stale_before)? else {
        return Ok(StepOutcome::Empty);
    };

    let (origin, path) = robots::origin_and_path(&item.url);
    if !robots_cache.contains_key(origin) {
        let rules = robots::fetch_rules(client, origin).await;
        robots_cache.insert(origin.to_string(), rules);
    }
    if !robots_cache[origin].is_allowed(path) {
        fetch_queue::mark_failure(conn, &item.url, None, FAR_FUTURE)?;
        return Ok(StepOutcome::Fetched {
            url: item.url,
            result: FetchResult::RobotsDisallowed,
        });
    }

    let req = HttpRequest {
        url: item.url.clone(),
        if_none_match: item.etag.clone(),
    };
    let result = match client.get(req).await {
        Ok(resp) if resp.status == 304 => {
            // Confirmed-unchanged is as complete as a fresh 200: both mean
            // "this URL's state is now known", so both permanently retire
            // the item from automatic reclaiming.
            fetch_queue::mark_success(conn, &item.url, 304, None, Some(FAR_FUTURE))?;
            FetchResult::NotModified
        }
        Ok(resp) => {
            fetch_queue::mark_success(
                conn,
                &item.url,
                resp.status as i64,
                resp.etag.as_deref(),
                Some(FAR_FUTURE),
            )?;
            FetchResult::Success {
                status: resp.status,
            }
        }
        Err(HttpError::Status(code)) if code == 429 || code >= 500 => {
            let exp = (item.attempts.max(0) as u32).min(20);
            let delay = BACKOFF_BASE_SECS * 2u64.saturating_pow(exp);
            let next_attempt_at = rfc3339_from_unix(now_unix + delay);
            fetch_queue::mark_failure(conn, &item.url, Some(code as i64), &next_attempt_at)?;
            FetchResult::Retrying {
                status: Some(code),
                next_attempt_at,
            }
        }
        Err(err) => {
            let status = match err {
                HttpError::Status(code) => Some(code as i64),
                HttpError::Request(_) => None,
            };
            fetch_queue::mark_failure(conn, &item.url, status, FAR_FUTURE)?;
            FetchResult::PermanentFailure {
                status: status.map(|s| s as u16),
            }
        }
    };

    Ok(StepOutcome::Fetched {
        url: item.url,
        result,
    })
}
