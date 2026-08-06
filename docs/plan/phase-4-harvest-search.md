# Phase 4 — Readme harvesting and full-text search

← [Implementation plan index](../../IMPLEMENTATION_PLAN.md)

`bam-handoff.md` §7 is effectively this phase's specification — it enumerates
every politeness requirement, so most tasks here are assembly against a
checklist rather than design.

> **Before the bulk run:** ask a mirror operator (e.g. ftp.fau.de) about rsync
> access. A single `rsync --include='*.readme'` pass is dramatically more
> mirror-friendly than 84,000 individual HTTP requests, and the answer decides
> whether P4.3 is the bulk path or only the incremental one. Worth an email
> well before it is worth twelve hours of someone else's bandwidth.

---

### P4.1 — `fetch_queue` schema and atomic claim · **S**

```sql
CREATE TABLE fetch_queue (
  url             TEXT PRIMARY KEY,
  kind            TEXT NOT NULL,        -- 'readme' | 'archive' | ...
  priority        INTEGER NOT NULL DEFAULT 0,
  attempts        INTEGER NOT NULL DEFAULT 0,
  next_attempt_at TEXT,
  etag            TEXT,
  last_status     INTEGER,
  claimed_at      TEXT                  -- NULL when free
);
```

Plus the operations a worker needs: atomically claim the next due item, mark
success or failure, schedule a retry. The claim must be a single statement
(`UPDATE ... RETURNING` with a status transition) so concurrent workers cannot
double-fetch.

**Tests first:**
- Two threads claiming simultaneously receive different rows.
- Items with `next_attempt_at` in the future are not claimed.
- Higher priority is claimed first.
- Marking failure increments `attempts` and sets a future `next_attempt_at`.
- A claim abandoned by a crashed worker becomes reclaimable after a timeout.

**Why S:** the schema is given and the atomic-claim pattern is standard SQLite,
but the concurrency test is the point of the task and easy to skip.

**Hand over:** the table above, "claim must be atomic under concurrent
workers", the five tests.

**Done when:** all five pass.

---

### P4.2 — Configurable token-bucket rate limiter · **H**

Token bucket over an **injected clock**, so it is testable without sleeping.

Rate and burst come from config with documented defaults — **2.0 requests per
second, burst 4** — falling back to those defaults when unset. The default is
a starting point for a polite crawl, not a physical constant: real mirrors
differ, and the knob exists so it can be tuned down without a rebuild.

**Tests first:**
- 100 requests through a fake clock observe the configured rate, measured in
  fake time, with the test completing in milliseconds of wall time.
- Burst allows N immediate requests, then throttles.
- Absent config yields exactly the documented defaults.
- A configured rate of 0 is rejected at load with a clear error rather than
  hanging forever at runtime.

**Why H:** a well-known forty-line algorithm; the clock injection and the
config fallback are both stated.

**Hand over:** "token bucket, rate and burst from config, defaults 2.0/4, take
a clock as a parameter rather than calling `Instant::now` internally", the four
tests.

**Done when:** all four pass.

---

### P4.3 — Background fetch worker · **S**

Pulls from the queue and honours §7 in full: the P4.2 rate limiter over a
single keep-alive connection, conditional GET via the stored ETag, exponential
backoff on 429 and 5xx, `robots.txt` respected, a descriptive `User-Agent` with
contact information, and a priority boost for whatever the user is currently
viewing so browsing stays responsive under a bulk run.

Uses P1.9's `HttpClient` trait, so the whole worker is testable without a
network.

**Tests first** (all against a fake `HttpClient`):
- A 429 triggers backoff with increasing delays, not a tight retry loop.
- A stored ETag is sent, and a 304 marks success without rewriting the body.
- `robots.txt` disallowing a path prevents the fetch.
- Interrupting mid-run and restarting does not re-fetch completed items.
- A high-priority item enqueued during a bulk run is served before the backlog.
- One `#[ignore]`d test runs 1,000 real readmes against a mirror and asserts
  the observed rate and a zero 429 count.

**Why S:** every requirement is enumerated in §7; this is assembly plus the
test harness.

**Hand over:** §7 in full, P4.1's queue operations, P4.2's limiter, P1.9's
`HttpClient` trait, the six test groups.

**Done when:** the five offline tests pass; the ignored one is run manually
once.

---

### P4.4 — Readme landing storage · **S**

`landing_readme(package_id, url, fetched_at, raw BLOB, detected_encoding)`.
Bytes, not text — the same reasoning as P1.2.

**Tests first:**
- A fetched readme round-trips to its original bytes exactly.
- The detected encoding label from P1.5 is stored and readable.
- Re-fetching the same URL updates rather than duplicating.

**Why S:** mirrors the landing pattern already established in P1.2.

**Hand over:** `landing_index_line` as the pattern to follow, P1.5's decode
signature, the three tests.

**Done when:** all three pass.

---

### P4.5 — Readme header parser · **S**

Aminet readmes carry a semi-standardised header block: `Short:`, `Author:`,
`Uploader:`, `Type:`, `Version:`, `Requires:`, `Distribution:`. Parse into
`enrichment` with `kind = 'readme_header'`, `producer_version = 1`.

"Semi-standardised" is doing real work in that sentence: field order varies,
some files omit the block, capitalisation differs, values wrap across lines.
**Parse leniently** — extract what is recognisable, record nothing for the
rest, and never fail a whole file over one bad field.

**Tests first:**
- Twenty real readmes, sampled across categories, committed as fixtures and
  all parsed without error.
- A test records **how many fields each fixture yielded**, pinned as expected
  values — so a later parser improvement shows up as a visible diff in those
  counts rather than passing silently.
- A readme with no header block yields an empty result, not an error.
- A wrapped multi-line value is captured whole.

**Why S:** messy but bounded, and the leniency rule is stated rather than left
to judgement.

**Hand over:** the field list, the leniency rule, the `enrichment` row shape,
and the instruction to sample twenty readmes across different categories.

**Done when:** the four test groups pass.

---

### P4.6 — FTS5 index over description and readme · **S**

An external-content FTS5 table fed from `package.description` and readme text.
Wire P2.1's `FullText` IR node to it, replacing P2.5's `LIKE` fallback — in the
one place the compiler puts it.

Provide an **explicit rebuild path**. Triggers alone are not sufficient because
the normalized layer gets bulk-rebuilt (P1.6), and a trigger-only design
silently desynchronises the moment that happens.

**Tests first:**
- A distinctive word from a known readme finds exactly that package.
- `DROP`ping the FTS table and rebuilding restores identical results.
- A bulk re-normalize followed by rebuild leaves the index consistent.
- `FullText` now compiles to an FTS5 match; the `LIKE` fallback is gone from
  the compiler, changed in exactly one place.
- Searching a term present only in a readme (not the description) finds it.

**Why S:** FTS5 is well documented and the integration point was fixed by P2.5.

**Hand over:** the external-content FTS5 pattern, which columns feed it, P2.5's
`FullText` hook, "explicit rebuild, do not rely on triggers alone", the five
tests.

**Done when:** all five pass.

---

### P4.7 — Prioritise readmes for filtered and visible entries · **S**

§7's requirement: every entry surviving the active filters gets its readme
queued. Enqueue on query execution; boost priority for the visible window.

**Tests first:**
- Running a query enqueues exactly its result set.
- Rows in the visible window carry higher priority than the rest.
- Re-running the same query does not duplicate queue entries.
- Already-fetched readmes are not re-enqueued.

**Why S:** connects three pieces that already exist and are already tested.

**Hand over:** the queue's priority semantics, P2.6's search API, the four
tests.

**Done when:** all four pass.

---

**Phase 4 exit:** full-text search over real readme content, harvested politely
and resumably.
