# Phase 4 progress — Fetch queue, rate limiting, background harvest, FTS5

← [PROGRESS.md](../../PROGRESS.md)

Round-by-round log for Phase 4 (Rounds 20–26), extracted from the top-level
progress file to keep that file scannable. Task ids refer to
[`IMPLEMENTATION_PLAN.md`](../../IMPLEMENTATION_PLAN.md) and
[phase-4-harvest-search.md](../plan/phase-4-harvest-search.md).

---

## Round 20 — 2026-08-07 · `fetch_queue` schema and atomic claim (P4.1) — Phase 4 start

**Done:**

- **P4.1** — `crates/bam-core/migrations/0003_fetch_queue.sql` (the phase
  doc's `fetch_queue` table verbatim), migration 3 registered in
  `store/migrations.rs`. New `store::fetch_queue` module (native-gated,
  inside `store::` per I1): `enqueue` (upsert, `priority = MAX(existing,
  new)` on conflict — a re-enqueue never lowers an already-boosted
  priority), `claim_next(now, stale_before)`, `mark_success`, `mark_failure`,
  `get`. The atomic claim is one `UPDATE fetch_queue SET claimed_at = ?1
  WHERE url = (SELECT ... ORDER BY priority DESC, url ASC LIMIT 1) RETURNING
  ...` — a single statement, so SQLite's own write lock on the row selection
  (not a separate check-then-act pair) is what stops two callers from
  claiming the same row; `url ASC` as the tiebreak makes the claimed row
  deterministic when priorities match, needed for the priority test to
  assert a specific url rather than "one of the equal-priority set."
  `claimed_at IS NULL OR claimed_at <= stale_before` is what makes an
  abandoned claim reclaimable — a crashed worker's claim was never cleared
  by `mark_success`/`mark_failure`, so a caller-chosen staleness cutoff
  reclaims it exactly like an unclaimed row. Also added
  `conn.busy_timeout(Duration::from_secs(5))` to `store::open` (previously
  unset) — needed for the concurrency test itself: without it, two real
  connections to the same file hammering the same `UPDATE` would surface
  `SQLITE_BUSY` as an error instead of blocking and retrying, which is the
  actual behavior a multi-worker deployment needs too, not just a test
  convenience. Six tests in the new `tests/store_fetch_queue.rs` (five from
  the phase doc's list plus a `mark_success`/ETag-preserved-on-304 test,
  since `mark_success`'s `COALESCE(?3, etag)` behavior has no other test
  coverage): the concurrency test spawns 4 real OS threads, each with its
  own `Connection` opened via `store::open` against one temp-file DB and 40
  queued urls, and asserts all 40 are claimed exactly once with no
  duplicates across threads — a real multi-connection race, not a
  single-connection stand-in for one.

  Round 5's own "false-vacuous-pass" caution paid off again: adding
  migration 3 broke `migrations.rs`'s
  `db_at_version_n_only_runs_migrations_above_n` test the same way adding
  migration 2 did in Round 5 — it asserted *exactly* `["http_cache"]` after
  skipping migration 1, which migration 3's new `fetch_queue` table now
  falsifies. Updated to assert the sorted pair `["fetch_queue",
  "http_cache"]`, watched for and fixed before considering the round done,
  not discovered after the fact.

116 tests total (6 new + 110 pre-existing; summed via `cargo test --workspace
2>&1 | grep -oE '[0-9]+ passed' | awk '{s+=$1} END{print s}'`, continuing
Round 10/14's caution about hand-counting). `cargo fmt --check`, `cargo
clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean.

**No deviations beyond the migrations-test fix described above.**

---

## Round 21 — 2026-08-07 · Token-bucket rate limiter (P4.2)

**Done:**

- **P4.2** — `crates/bam-core/src/ratelimit.rs`, a new top-level module,
  ungated like `cancel.rs`/`highlight.rs`: the algorithm itself is pure
  computation over an injected `Clock` trait, and `SystemClock`'s
  `Instant::now()` body only needs to *compile* under the wasm32 check, not
  run correctly there (same reasoning `progress.rs` and the `http` trait
  module already established for a real-vs-fake split). `TokenBucket<C:
  Clock>` holds `Cell<f64>` tokens and a `Cell<Instant>` last-refill mark,
  refilled lazily on every `try_acquire` call rather than by a background
  timer — no ticking task to schedule, and identical results either way
  since refill amount depends only on elapsed time since the last call.
  `try_acquire` is deliberately the *only* operation and is non-blocking: it
  returns `Ok(())` or `Err(Duration)` (how long until a token would be
  available) and never sleeps itself, so a caller — real (P4.3, via
  `tokio::time::sleep`) or test (advancing a fake clock) — owns the actual
  waiting. `RateLimitConfig { rate: f64, burst: u32 }` derives `Deserialize`
  with per-field `#[serde(default = ...)]` pointing at the documented
  2.0/4 constants, so a `bam.toml` `[rate_limit]` section — and each field
  within it — independently falls back rather than requiring an
  all-or-nothing section; not wired into `bam-tui`'s config loader yet, since
  no caller of the rate limiter exists before P4.3. `TokenBucket::new`
  rejects `rate <= 0.0` with `NonPositiveRate`, checked once at construction
  rather than on every `try_acquire`. `impl<C: Clock> Clock for &C` (a small,
  necessary addition beyond the trait's minimal shape) lets a test share one
  `FakeClock` between the clock-advancing driver and the bucket itself
  without an `Rc`/`Arc` wrapper — `TokenBucket` owns its clock by value, and
  a shared reference is a `Clock` too.

  Four tests, inline `#[cfg(test)]` per this round's own algorithm-as-the-
  artifact framing (matching P3.1/P3.2's precedent for a self-contained
  module): a 100-request drain against `rate=2.0, burst=4` advances the fake
  clock by the expected 48s (`(100-4)/2`) within a tight tolerance while wall
  time stays under 200ms, proving no real sleep occurs; four immediate
  `try_acquire`s succeed and a fifth returns a positive wait; an empty
  `serde_json` object deserializes to exactly `RateLimitConfig::default()`
  (`toml` isn't a `bam-core` dependency — every prior config type needing it,
  `KeymapConfig`/`RuleConfig`, lives in `bam-tui`; deserializing from
  `serde_json`'s `"{}"` proves the same per-field-default behavior a
  `bam.toml` `[rate_limit]`-absent case would, without adding the dependency
  a caller-less module doesn't yet need); `rate: 0.0` is rejected by
  `TokenBucket::new` rather than accepted and left to hang at first use.

124 tests total (4 new + 120 pre-existing — Round 20's own count was
double-checked directly via `cargo test --workspace 2>&1 | grep -oE '[0-9]+
passed' | awk '{s+=$1} END{print s}'` before adding to it, not assumed).
`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
warnings`, and the wasm32 `--no-default-features` check all clean.

**Deviations for the next session to know about:**
- `RateLimitConfig` is not yet read from a real `bam.toml` — no caller
  (P4.3's fetch worker) exists yet to need it wired into `bam-tui`'s
  `resolve_config_path`/`load_keymap`-style loader. Revisit together when
  P4.3 lands.
- `try_acquire` is non-blocking by design, returning a wait `Duration`
  rather than blocking or sleeping — P4.3's own task text ("the P4.2 rate
  limiter over a single keep-alive connection") will need to decide how it
  turns that into an actual `tokio::time::sleep`; not decided here since no
  async caller exists yet.

---

## Round 22 — 2026-08-07 · Background fetch worker (P4.3)

**Done:**

- **P4.3** — `crates/bam-core/src/store/fetch_worker.rs` (native-gated, inside
  `store::` per I1 — it mixes P4.1's `fetch_queue` with `HttpClient`, same
  reasoning as P1.9's `store::fetch`). One entry point, `step()`: rate-limit
  (P4.2's `TokenBucket::try_acquire`, checked *before* claiming, so a
  rate-limited call touches the DB not at all), atomically claim (P4.1's
  `claim_next`), check `robots.txt` (new), fetch (P1.9's `HttpClient`), record
  the outcome. One queue item per call, never an internal loop — the caller
  drives pacing and can check a `CancellationToken` between calls, matching
  I5's convention. `RobotsCache` (`HashMap<origin, RobotsRules>`) is threaded
  in by the caller so a bulk run fetches each origin's `robots.txt` once.

  New `bam_core::robots` module (top-level, ungated like `http`/`ratelimit` —
  parsing is pure, `fetch_rules` is generic over `HttpClient`): parses only
  the `User-agent: *` group's `Disallow` lines (bam is a generic polite
  crawler, not a named one a site would single out — a full RFC 9309
  implementation with `Allow` overrides and crawl-delay is more than anything
  here needs), permissive on any fetch failure or non-200 (a broken
  `robots.txt` must never itself block otherwise-permitted fetches). No `url`
  crate dependency: `origin_and_path` is a dozen-line hand-rolled split, the
  only thing either caller (this module, the worker) needs from a URL.

  `HttpError` gained a `Status(u16)` variant (`http/mod.rs`); `reqwest_client.rs`
  now returns it instead of folding the code into `Request`'s message string —
  needed so the worker can pattern-match 429/5xx (retryable) against
  everything else (permanent) without parsing text. `lib.rs`'s `now_rfc3339`
  was split to expose `rfc3339_from_unix(secs: u64)`, so the worker can stamp
  a computed `now + backoff` moment as RFC3339 without a date-arithmetic
  dependency — it already had `civil_from_days` for exactly this from P1.6.

  **Design decision beyond the phase doc's text, needed to make "restarting
  does not re-fetch completed items" true rather than aspirational:**
  `fetch_queue::mark_success` gained a fifth parameter, `next_attempt_at:
  Option<&str>`, COALESCE'd exactly like the existing `etag` parameter (`None`
  leaves it unchanged — the two P4.1 tests calling it needed one added `None`
  argument each, no assertion changed). Without this, a successfully-fetched
  item's `next_attempt_at` stays `NULL` forever (nothing in the original P4.1
  schema/functions ever sets it on success), so `claim_next`'s own "always
  due" gate would let it be reclaimed and re-fetched indefinitely — priority
  order, not completion, would be the only thing standing between it and a
  future `claim_next` call. The worker passes `Some(FAR_FUTURE)` (a
  `"2999-01-01T00:00:00Z"` sentinel, the same convention P4.1's own tests
  already use for "never") on *both* a fresh 200 and a confirmed-unchanged
  304 — a confirmed-unchanged fetch is exactly as complete as a fresh one, so
  both permanently retire the item from automatic reclaiming. A robots-txt
  disallow or a permanent (non-429/5xx) failure also gets `FAR_FUTURE` via
  `mark_failure`, for the same reason: none of these are conditions a bare
  retry would fix.

  Six tests: five in the new `tests/store_fetch_worker.rs` (the phase doc's
  five offline groups, table for table), plus one `#[ignore]`d real-mirror
  test. All five use a `ByUrlClient` (a response queue keyed by URL, request
  order preserved for assertions) that **panics on any unscripted URL** — the
  same "prove no network is touched" pattern Round 6's `IngestMode::
  RebuildNormalized` test established, here proving a robots-disallowed or
  already-completed URL is never requested a second time, not merely
  asserting it after the fact. The 429-backoff test recomputes each expected
  timestamp via `rfc3339_from_unix` rather than parsing RFC3339 back to an
  integer (no parser exists or is needed elsewhere) and asserts the delay
  sequence is strictly `[1, 2, 4]` seconds. The ETag test seeds a stored ETag
  by direct SQL rather than by chaining off an earlier successful fetch — with
  success now permanently retiring an item, "fetch the same completed item
  twice" is no longer a real scenario the worker itself produces; a
  pre-existing stored ETag on an as-yet-unfetched item (e.g. carried over by
  a future seeding/migration step) is the honest way to exercise conditional
  GET. The restart test reopens a **file-backed** `Connection` (not
  `:memory:`) to a temp path to prove durability, not just in-memory dedup,
  and gives the already-completed item the *higher* priority of the two so
  the assertion tests exclusion-by-completion rather than accidentally
  passing via priority ordering. The priority-boost test doesn't add any
  worker logic beyond what `claim_next`'s pre-existing `ORDER BY priority
  DESC` and `enqueue`'s pre-existing `MAX(priority, ...)` upsert already do —
  it exists to prove the wiring end-to-end, not to add a feature.

130 tests total (6 new — 5 in `store_fetch_worker.rs`, 1 `#[ignore]`d — plus
124 pre-existing; 128 run, 2 ignored — `real_mirror_fetch` from P1.9 and this
round's own real-mirror test). `cargo fmt --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, and the wasm32 `--no-default-features` check
all clean. Also smoke-tested the real `bam` binary (`ingest --offline`):
still reports 501 packages, unaffected by this round's store/http changes.

**Deviations for the next session to know about:**
- `fetch_queue::mark_success`'s signature changed (new `next_attempt_at`
  parameter) — a P4.1 boundary was touched, not just built on top of. Flagged
  per the project's own convention (Round 5, Round 20) for any change that
  reopens a prior round's "done" schema/function; both existing P4.1 test call
  sites were updated (`None`, preserving every existing assertion) rather than
  reinterpreted.
- No `type = 'archive'`-specific handling exists yet — `kind` is passed
  through untouched; P4.3's own scope is generic across whatever `kind` a
  caller enqueues. Nothing in §7 or the phase doc's test list distinguishes
  by kind, so none was added.
- The `#[ignore]`d real-mirror test builds up to 1,000 readme URLs from
  `index_sample.txt` (only 501 are available in that fixture — Round 1's own
  curated size) rather than fixture-manufacturing a full 1,000; guesses the
  Aminet convention `{dir}/{file}.readme` for each entry's readme URL (not
  independently verified against a real mirror this round — it is, after all,
  never run in CI). Not executed this round; run manually before trusting it.

---

## Round 23 — 2026-08-07 · Readme landing storage (P4.4)

**Done:**

- **P4.4** — `crates/bam-core/migrations/0004_landing_readme.sql`, migration 4
  registered in `store/migrations.rs`. `landing_readme(id, package_id, url,
  fetched_at, raw BLOB, detected_encoding)` — `raw` is BLOB not TEXT, same
  reasoning as `landing_index_line` (P1.2): encoding is detected later and
  must stay correctable without re-fetching. Unlike `landing_index_line`,
  which is append-only, this table is keyed by `UNIQUE url` and *upserted* —
  the task's own third test bullet ("re-fetching the same URL updates rather
  than duplicating") makes append-only wrong for this table specifically.
  `store::tables::{LandingReadme, insert_landing_readme, get_landing_readme}`
  follow P1.2's exact struct/insert/get shape; `insert_landing_readme` is an
  `INSERT ... ON CONFLICT(url) DO UPDATE ... RETURNING id` (one statement,
  same RETURNING-based id-recovery idiom P4.1's `claim_next` already
  established for this codebase, rather than a separate check-then-act
  select). `get_landing_readme` looks up by `url`, not `id` — the caller
  always knows the url it just fetched, and `url` is already the table's
  natural key. Three tests in the new `tests/store_readme.rs`, matching the
  phase doc's three bullets exactly: exact-byte round trip (including a raw
  byte sequence that isn't valid UTF-8, same style as `store.rs`'s existing
  `blob_roundtrips_invalid_utf8`), `detected_encoding` stored and read back,
  and a same-url double-insert asserted to return the same id, leave exactly
  one row (`COUNT(*) FROM landing_readme WHERE url = ?1`), and surface the
  second call's `raw`/`fetched_at`, not the first's.

  `tests/migrations.rs`'s `db_at_version_n_only_runs_migrations_above_n` hit
  the same false-vacuous-pass pattern Round 5/20 already flagged and fixed
  for migrations 2 and 3: updated to assert the sorted triple
  `["fetch_queue", "http_cache", "landing_readme"]`, caught and fixed this
  round rather than left for the next one to rediscover.

133 tests total (3 new + 130 pre-existing; 131 run, 2 ignored — the two
pre-existing real-mirror tests). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean.

**No deviations.**

---

## Round 24 — 2026-08-07 · Readme header parser (P4.5)

**Done:**

- **P4.5** — `crates/bam-core/src/ingest/readme.rs` (ungated — pure string
  parsing, no `rusqlite`, confirmed by the wasm32 check): `ReadmeHeader`
  (`short`/`author`/`uploader`/`type`/`version`/`requires`/`distribution`,
  all `Option<String>`) and `parse_readme_header(text: &str) -> ReadmeHeader`,
  infallible per the task's own leniency rule — nothing here can fail a file.
  The header block is defined as the contiguous run from the start of the
  text to the first blank line; within it, a line matching `Word[ Word]:` is
  either a recognised field (captured) or an unrecognised one (`Architecture:`
  and misspellings like `Distrubution:` in the real fixtures — silently
  dropped, not merged into the previous field); any other line is a wrapped
  continuation of whichever recognised field is currently open, joined with a
  space. `README_HEADER_KIND = "readme_header"` and
  `README_HEADER_PRODUCER_VERSION = 1` are exported constants for whichever
  future task actually writes the `enrichment` row (P1.2's schema) — nothing
  in this task's four test bullets calls for DB wiring, so none was added;
  `ReadmeHeader` already derives `Serialize`/`Deserialize` for that caller to
  use directly as the `payload`.

  Twenty real readmes fetched from `ftp.fau.de/aminet/` (2026-08-07, listed in
  `crates/bam-core/tests/fixtures/README.md`), two from each of the ten
  categories `index_sample.txt` covers. Fetching them surfaced a real-world
  correction: the readme URL is `{dir}/{stem}.readme` with the archive
  extension **stripped** from the filename, not `{dir}/{file}.readme` with it
  kept — Round 22's `#[ignore]`d real-mirror test in `store_fetch_worker.rs`
  had guessed the latter and would have 404'd on every URL; fixed in passing
  (still one 404 short of correct for `.tar.bz2` names, matching
  `split_name_version`'s own pre-existing single-extension limitation from
  P1.6 — not fixed there, out of scope for this round). Five tests in
  `tests/ingest_readme.rs`: the twenty fixtures table-driven with each
  fixture's recognised-field count pinned (computed by hand from each real
  file, doubling as the "parsed without error" bullet — same table-satisfies-
  two-bullets pattern as Round 8/10's precedent), a synthetic no-header-block
  case, a synthetic blank-first-line case, a synthetic wrapped-value case, and
  one more proving an unrecognised field line (`Architecture:`) doesn't get
  merged into the preceding recognised field's value.

138 tests total (5 new + 133 pre-existing; 136 run, 2 ignored — the two
pre-existing real-mirror tests, unaffected by the readme-URL fix beyond the
one line it touches). `cargo fmt --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, and the wasm32 `--no-default-features` check
all clean.

**Deviations for the next session to know about:**
- The real readme-URL convention (`{dir}/{stem}.readme`, extension stripped)
  wasn't previously verified against a real mirror — Round 22 flagged its own
  guess as unconfirmed. Now confirmed and fixed in `store_fetch_worker.rs`'s
  ignored test; not re-run this round (still manual-only).
- No `enrichment` row is actually written yet — `parse_readme_header` is pure,
  and the `README_HEADER_KIND`/`README_HEADER_PRODUCER_VERSION` constants are
  there for whichever task wires readme fetch (P4.3) → parse (this round) →
  store (`store::tables::insert_enrichment`, P1.2) together. That wiring
  isn't named as its own task in the phase doc; watch for it being assumed
  already done.

---

## Round 25 — 2026-08-07 · FTS5 index over description and readme (P4.6)

**Done:**

- **P4.6** — `crates/bam-core/migrations/0005_fts.sql`, migration 5 registered
  in `store/migrations.rs`: `CREATE VIRTUAL TABLE package_fts USING
  fts5(description, readme_text, content='')`. Read as **contentless**
  (`content=''`), not a literal external-content table synced by triggers —
  the phase doc's own text rules out relying on triggers at all ("provide an
  explicit rebuild path... a trigger-only design silently desynchronises"),
  and the searchable text spans two source tables (`package.description`,
  `landing_readme.raw`), so there's no single table an `external content`
  declaration could point at anyway. A contentless table gets the same
  outcome more simply: it indexes whatever text is passed at insert time and
  stores none of it back, so `package`/`landing_readme` stay the sole record
  of the actual text, with no triggers at all — the whole class of sync bug
  the phase doc warns about is sidestepped by never attempting incremental
  sync, not chased with more trigger code.

  New `store::fts::rebuild_fts(conn)` (native-gated, inside `store::` per I1):
  drops and recreates `package_fts`, then for every `package` row inserts
  `(rowid = package.id, description, readme_text)`, where `readme_text` is
  every `landing_readme.raw` for that `package_id` decoded via P1.5's
  `ingest::charset::decode` and joined with `\n` (a package's own readme
  isn't stored decoded anywhere yet — Round 24 flagged that the fetch → parse
  → store wiring for readmes isn't built — so decoding happens fresh on each
  rebuild rather than adding that wiring here). Always a full drop-and-
  repopulate, never an incremental update: simplest correct behavior given
  `normalize` (P1.6) already does full `package` rebuilds that can renumber
  ids, and nothing in the five tests needs anything more targeted.

  `store::compile::compile_fulltext` (`store/compile.rs`) is the one place
  P4.6 asks changed: `Predicate::FullText(text)` now compiles to `id IN
  (SELECT rowid FROM package_fts WHERE package_fts MATCH ?)`, the whole value
  quoted as one FTS5 phrase (`"..."`, internal `"` doubled) rather than
  treated as independently-matchable unordered terms — preserving the
  replaced `LIKE '%...%'` fallback's word-order-sensitive substring behavior
  instead of loosening it. The pre-existing `worked_examples_compile_and_
  return_expected_ids` test (Round 8) exercises `"tracker module editor"` as
  a `FullText` query; its fixture needed one added line,
  `store::fts::rebuild_fts(&conn)` after inserting all nine rows, since that
  test's expectations now depend on the FTS index being populated, not on
  `description` being scanned directly.

  Five tests in the new `tests/store_fts.rs`, matching the phase doc's five
  bullets exactly: a distinctive readme-only word finds exactly its package;
  dropping `package_fts` directly (raw SQL, not through `rebuild_fts`) and
  calling `rebuild_fts` again reproduces identical search results; a real
  landing line through `normalize` (P1.6) twice in a row — proving the
  renumbered-ids case, since a bare `DELETE FROM package` followed by
  re-insert reuses freed rowids — stays searchable by the same term after
  each `rebuild_fts`; a direct assertion that the compiled SQL names
  `package_fts`/`MATCH` and contains no `LIKE`; and a term present only in a
  readme, not the description, is found.

  `tests/migrations.rs`'s `db_at_version_n_only_runs_migrations_above_n` hit
  the same false-vacuous-pass pattern Round 5/20/23 already flagged and fixed
  for migrations 2-4 — a virtual FTS5 table registers itself plus four shadow
  tables (`package_fts`, `_data`, `_idx`, `_docsize`, `_config`) in
  `sqlite_master`, all five now asserted by name.

143 tests total (5 new + 138 pre-existing; 141 run, 2 ignored — the two
pre-existing real-mirror tests). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean. Also smoke-tested the real `bam`
binary (`ingest --offline` against a scratch DB): still reports 501 packages,
confirming migration 5 doesn't break a fresh DB open.

**Deviations for the next session to know about:**
- `package_fts` is contentless (`content=''`), not a true external-content
  FTS5 table (`content='package'`) synced by triggers — the phase doc's own
  header calls it "an external-content FTS5 table," but the two source
  columns span two different tables, and the doc's own body text (no
  triggers, explicit rebuild only) is what both readings actually agree on
  in behavior. Revisit if a future task wants incremental (non-rebuild)
  updates to the index — that would need real triggers on `package` and
  `landing_readme`, which don't exist.
- `rebuild_fts` re-decodes every readme's raw bytes on every call rather than
  reading a stored decoded copy, because no such copy exists yet (Round 24's
  own flagged gap: readme fetch → parse → `enrichment` storage isn't wired).
  Revisit together if that wiring lands and a decoded readme text becomes
  available to read instead of re-derived.
- `rebuild_fts` is O(packages) queries for readme text (one `SELECT ... WHERE
  package_id = ?` per package) rather than one joined query — simplest
  correct thing at the current fixture/test scale; revisit if a full-catalog
  rebuild (thousands of packages) is ever measured to be slow.

---

## Round 26 — 2026-08-07 · Prioritise readmes for filtered and visible entries (P4.7) — Phase 4 exit

**Done:**

- **P4.7** — `Session::enqueue_readmes(pred, visible_offset, visible_len)`
  (`crates/bam-core/src/store/session.rs`, native-gated). Runs the same
  `compiled_for(pred)` + `ORDER BY id` query `search_window` already uses (so
  "visible" here means exactly the rows a simultaneous `search_window(pred,
  visible_offset, visible_len)` call would show), and for each matching
  package: computes its readme url, skips it if `landing_readme` already has
  that url (`tables::landing_readme_exists`, new — a plain existence check
  rather than reusing `get_landing_readme`, which fetches and decodes the
  whole BLOB just to answer yes/no), and otherwise calls P4.1's
  `fetch_queue::enqueue` with `README_PRIORITY_VISIBLE` (10) inside the window
  or `README_PRIORITY_BACKGROUND` (0) outside it — both new exported
  constants, so a caller (and the tests) never hardcodes the boost amount.
  Two of the four test bullets fall out of P4.1's own existing semantics
  rather than needing new logic here: "does not duplicate" is `enqueue`'s
  pre-existing `ON CONFLICT ... priority = MAX(...)` upsert, and "already-
  fetched readmes are not re-enqueued" is the one new check this task adds.

  The readme-url computation itself (`{dir}/{stem}.readme`, extension
  stripped) already existed, duplicated, in Round 22's `#[ignore]`d
  real-mirror test — factored out to `ingest::readme::readme_url` (new,
  alongside a new `AMINET_BASE_URL` constant matching `store::ingest::
  INDEX_URL`'s mirror) rather than writing a third copy for this task; the
  ignored test in `tests/store_fetch_worker.rs` now calls the shared function
  too, so there is exactly one place this convention lives. No `api::`
  wrapper was added — same precedent as P4.1/P4.2/P4.3's queue/worker
  internals, which stay at the `store::`/`Session` level with no typed
  request/response pair until an actual UI caller needs one; nothing in this
  task's four bullets or hand-over asks for TUI wiring.

  Four tests in the new `tests/store_enqueue_readmes.rs`, matching the phase
  doc's four bullets exactly, against a real file-backed `Session` seeded
  with ten same-`dir` packages (same pattern as `store_session_window.rs`):
  a query's full result set matches the queue's urls exactly; every url
  inside `[visible_offset, visible_offset+visible_len)` carries
  `README_PRIORITY_VISIBLE` and every other url carries
  `README_PRIORITY_BACKGROUND`; running the same query twice leaves exactly
  ten rows in `fetch_queue`, not twenty; a package with a pre-existing
  `landing_readme` row is excluded from the queue entirely while its two
  siblings are enqueued.

147 tests total (4 new + 143 pre-existing; 145 run, 2 ignored — the two
pre-existing real-mirror tests, unaffected beyond the one shared-function line
each touches). `cargo fmt --check`, `cargo clippy --workspace --all-targets --
-D warnings`, and the wasm32 `--no-default-features` check all clean.

**No deviations.**

**Phase 4 exit reached.** Full-text search over real readme content,
harvested politely and resumably, with visible/filtered entries prioritised.

---
