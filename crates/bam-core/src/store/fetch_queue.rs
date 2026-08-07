use rusqlite::{Connection, OptionalExtension, Result, params};

#[derive(Debug, Clone, PartialEq)]
pub struct FetchQueueItem {
    pub url: String,
    pub kind: String,
    pub priority: i64,
    pub attempts: i64,
    pub next_attempt_at: Option<String>,
    pub etag: Option<String>,
    pub last_status: Option<i64>,
    pub claimed_at: Option<String>,
}

fn row_to_item(row: &rusqlite::Row) -> Result<FetchQueueItem> {
    Ok(FetchQueueItem {
        url: row.get(0)?,
        kind: row.get(1)?,
        priority: row.get(2)?,
        attempts: row.get(3)?,
        next_attempt_at: row.get(4)?,
        etag: row.get(5)?,
        last_status: row.get(6)?,
        claimed_at: row.get(7)?,
    })
}

pub fn enqueue(conn: &Connection, url: &str, kind: &str, priority: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO fetch_queue (url, kind, priority) VALUES (?1, ?2, ?3)
         ON CONFLICT(url) DO UPDATE SET priority = MAX(priority, excluded.priority)",
        params![url, kind, priority],
    )?;
    Ok(())
}

/// Atomically claims the highest-priority due, unclaimed-or-stale item: one
/// `UPDATE ... RETURNING` keyed by a correlated subquery, so SQLite's own
/// write lock on the row selection — not a separate check-then-act pair of
/// statements — is what stops two callers from claiming the same url.
/// `now` gates `next_attempt_at`; `stale_before` gates `claimed_at`, letting
/// a claim abandoned by a crashed worker (never marked success or failure)
/// become reclaimable once it's older than the caller's own timeout.
pub fn claim_next(
    conn: &Connection,
    now: &str,
    stale_before: &str,
) -> Result<Option<FetchQueueItem>> {
    conn.query_row(
        "UPDATE fetch_queue
         SET claimed_at = ?1
         WHERE url = (
             SELECT url FROM fetch_queue
             WHERE (next_attempt_at IS NULL OR next_attempt_at <= ?1)
               AND (claimed_at IS NULL OR claimed_at <= ?2)
             ORDER BY priority DESC, url ASC
             LIMIT 1
         )
         RETURNING url, kind, priority, attempts, next_attempt_at, etag, last_status, claimed_at",
        params![now, stale_before],
        row_to_item,
    )
    .optional()
}

/// Marks a claimed item as successfully fetched: clears the claim, records
/// the response status, and updates the stored ETag when one was returned
/// (a 304 passes `None` to leave the existing ETag untouched).
pub fn mark_success(conn: &Connection, url: &str, status: i64, etag: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE fetch_queue
         SET claimed_at = NULL, last_status = ?2, etag = COALESCE(?3, etag)
         WHERE url = ?1",
        params![url, status, etag],
    )?;
    Ok(())
}

/// Marks a claimed item as failed: clears the claim, increments `attempts`,
/// records the status (`None` for a transport-level failure with no HTTP
/// response), and schedules the next attempt no earlier than `next_attempt_at`
/// — the caller (P4.3's backoff policy) computes that delay.
pub fn mark_failure(
    conn: &Connection,
    url: &str,
    status: Option<i64>,
    next_attempt_at: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE fetch_queue
         SET claimed_at = NULL, attempts = attempts + 1, last_status = ?2, next_attempt_at = ?3
         WHERE url = ?1",
        params![url, status, next_attempt_at],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, url: &str) -> Result<Option<FetchQueueItem>> {
    conn.query_row(
        "SELECT url, kind, priority, attempts, next_attempt_at, etag, last_status, claimed_at
         FROM fetch_queue WHERE url = ?1",
        params![url],
        row_to_item,
    )
    .optional()
}
