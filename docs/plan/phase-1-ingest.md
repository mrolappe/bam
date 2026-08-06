# Phase 1 — INDEX ingest, schema, incremental update

← [Implementation plan index](../../IMPLEMENTATION_PLAN.md)

The hard core starts here. Everything is testable offline against fixtures —
the default test run makes no network calls.

---

### P1.1 — Capture real INDEX/RECENT/TREE fixtures · **H**

Download once from a mirror and commit trimmed fixtures to
`crates/bam-core/tests/fixtures/`:

- `index_sample.txt` — ~500 lines from a real `INDEX`, chosen to include the
  awkward cases: very long filenames, descriptions containing runs of internal
  whitespace, non-ASCII bytes, a zero-size entry, and the header/preamble lines.
- `recent_sample.txt` — a real `RECENT`.
- `tree_sample.txt` — a real `TREE`.

Plus `fixtures/README.md` recording the mirror URL and the fetch date.

**Tests first:** none of its own — this task *is* the input to P1.4's tests.
Acceptance is the manual checklist below. Inventing a test that asserts a
fixture file is non-empty would be ceremony, not verification.

**Why H:** fetch, trim, commit. The selection criteria are given, so no
judgement is required beyond following them.

**Hand over:** mirror base URL `https://ftp.fau.de/aminet/`, the three
filenames, the awkward-case list, the target directory.

**Done when:** the three fixtures exist and a human has confirmed
`index_sample.txt` contains at least one instance of each listed awkward case.

> Do this before writing the parser. A parser written against a remembered
> format and then met with reality is a rewrite; the fixture is the spec.

---

### P1.2 — Schema: landing, normalized, enrichment, selections · **O**

All DDL lives under `crates/bam-core/src/store/` (invariant **I1**).

**Landing** — append-only, exactly what the origin said:

```sql
CREATE TABLE landing_index_line (
  id         INTEGER PRIMARY KEY,
  fetched_at TEXT NOT NULL,          -- RFC3339, when this INDEX was retrieved
  source_url TEXT NOT NULL,
  line_no    INTEGER NOT NULL,
  raw        BLOB NOT NULL           -- bytes: the encoding is not yet known
);
```

**Normalized** — derived, droppable, rebuildable from landing with no network:

```sql
CREATE TABLE package (
  id             INTEGER PRIMARY KEY,
  dir            TEXT NOT NULL,      -- 'util/misc'
  file           TEXT NOT NULL,      -- 'Foo-1.2.lha'
  name           TEXT NOT NULL,      -- 'Foo'  canonical, version stripped
  version        TEXT,               -- '1.2'  NULL when unparseable
  size_bytes     INTEGER,
  uploaded_on    TEXT,               -- ISO date
  date_precision TEXT NOT NULL,      -- 'week' | 'exact'
  description    TEXT,
  landing_id     INTEGER NOT NULL REFERENCES landing_index_line(id),
  UNIQUE(dir, file)
);
```

**Enrichment** — everything derived, versioned per producer so one stage can be
invalidated without touching the expensive ones:

```sql
CREATE TABLE enrichment (
  package_id       INTEGER NOT NULL REFERENCES package(id) ON DELETE CASCADE,
  kind             TEXT NOT NULL,    -- 'readme_header' | 'inventory' | 'llm_summary' | ...
  producer_version INTEGER NOT NULL,
  produced_at      TEXT NOT NULL,
  payload          TEXT NOT NULL,    -- JSON
  PRIMARY KEY (package_id, kind)
);
```

**Selections** — invariant **I7**; a core concept, not TUI state:

```sql
CREATE TABLE selection (
  id         INTEGER PRIMARY KEY,
  name       TEXT UNIQUE,            -- NULL for the unnamed working selection
  created_at TEXT NOT NULL,
  ephemeral  INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE selection_member (
  selection_id INTEGER NOT NULL REFERENCES selection(id) ON DELETE CASCADE,
  package_id   INTEGER NOT NULL REFERENCES package(id)   ON DELETE CASCADE,
  PRIMARY KEY (selection_id, package_id)
);
```

Three decisions here are expensive to reverse:

1. **`raw` is BLOB, not TEXT.** Aminet INDEX lines are not reliably UTF-8
   (`bam-handoff.md` §13). TEXT forces a lossy decode at ingest and destroys
   the point of having a landing layer at all.
2. **`date_precision`.** Aminet's INDEX gives *age in weeks* relative to the
   INDEX generation date, so a derived date is ±1 week; directory listings and
   readme headers can later supply an exact one. Without this column the two
   are indistinguishable and "sort by date" quietly lies. The upgrade is
   one-directional: `week` may be overwritten by `exact`, never the reverse.
3. **Enrichment survives package churn but cascades on delete.** Re-deriving
   the normalized layer must not discard LLM summaries — see P5.2, where the
   same invariant reappears for blob eviction.

**Tests first:**
- Round-trip insert/select on each of the five tables.
- `UNIQUE(dir, file)` rejects a duplicate.
- With `PRAGMA foreign_keys = ON`, deleting a package cascades to
  `enrichment` and `selection_member`.
- Dropping and recreating `package` leaves `landing_index_line` untouched.
- A BLOB containing invalid UTF-8 round-trips byte-identically.

**Why O:** every later phase writes against this schema. The BLOB-vs-TEXT and
`date_precision` calls are exactly what a cheaper model smooths into a
plausible-looking schema that costs a migration and a full re-ingest to fix.

**Hand over:** `bam-handoff.md` §5.1, §5.2, §13's encoding paragraph;
invariants I1 and I7; the P1.1 fixture.

**Done when:** the five tests pass against a fresh database.

---

### P1.3 — Migration runner · **H**

Numbered `.sql` files in `crates/bam-core/migrations/`, applied in order,
tracked via SQLite's `user_version` pragma. Embedded with `include_str!`.
No down-migrations.

**Tests first:**
- Applying to a fresh DB creates every table.
- Applying twice is a no-op.
- A DB at version N only runs migrations > N.

**Why H:** roughly thirty lines, thoroughly conventional.

**Hand over:** the `migrations/` path, `user_version` as the tracking
mechanism, "no down-migrations", the three tests.

**Done when:** the three tests pass.

> Skipped: `refinery`, `sqlx-migrate`. A loop over `include_str!`ed files is
> shorter than either dependency's configuration. Add one when migrations need
> branching or rollback.

---

### P1.4 — INDEX line parser · **S**

`parse_index_line(raw: &[u8]) -> Result<IndexRecord<'_>, ParseError>` in
`crates/bam-core/src/ingest/index.rs`.

The INDEX is **column-aligned, not delimiter-separated** — descriptions contain
whitespace runs freely, so splitting on whitespace is wrong. Derive column
offsets from the header row, falling back to fixed offsets when absent.

Return borrowed byte ranges, not `String`. Decoding is a separate step (P1.5)
because the landing layer must retain the original bytes.

**Tests first:**
- Every line of `index_sample.txt` parses without error.
- One named test per awkward case from P1.1, each asserting the exact field
  split: long filename, description with internal whitespace runs, non-ASCII
  bytes, zero-size entry, preamble lines skipped.
- A truncated line yields `ParseError`, not a panic and not silent garbage.

**Why S:** the grammar is fully determined by the P1.1 fixture. Careful
implementation against a known target, not design.

**Hand over:** the fixture, the `IndexRecord` field list (file, dir, size, age,
description — all as borrowed byte ranges), the
column-offsets-not-whitespace constraint, the borrow requirement.

**Done when:** the tests above pass.

---

### P1.5 — Charset decode helper · **S**

`decode(bytes: &[u8]) -> (String, &'static Encoding)` — `chardetng` to detect,
`encoding_rs` to decode, defaulting to **ISO-8859-1** when detection is
low-confidence, since that is Aminet's de-facto encoding.

The returned encoding label is persisted alongside the text everywhere it is
stored, so a later correction never requires a re-fetch (§13).

**Tests first:**
- A known ISO-8859-1 sequence containing `ö` decodes correctly.
- A UTF-8 sequence decodes correctly.
- Neither case is handled by an input-specific branch — the same code path
  serves both.
- Ambiguous short input falls back to ISO-8859-1 and reports that label.

**Why S:** two well-documented crates wired together, but the low-confidence
fallback and the persist-the-label requirement are easy to drop in favour of
`String::from_utf8_lossy`, which would be wrong and would look fine.

**Hand over:** §13's encoding paragraph, the signature, the ISO-8859-1 default,
and the note that `chardetng` wants the full text rather than a prefix.

**Done when:** the four tests pass.

---

### P1.6 — Normalizer: landing → package rows · **S**

Read landing rows, apply P1.4 and P1.5, derive:

- `size_bytes` from `"134K"` / `"1.2M"` — Aminet uses K/M suffixes, base 1024.
- `uploaded_on` from age-in-weeks plus the landing row's `fetched_at`, with
  `date_precision = 'week'`.
- `name` + `version` split from the filename. Aminet naming is inconsistent;
  **when the split is ambiguous, put the whole stem in `name` and leave
  `version` NULL** rather than guessing.

**Tests first:**
- Running `normalize()` twice over the fixture yields identical rows.
- Dropping `package` and re-normalizing restores it identically **with the
  network unavailable** — this is the entire justification for the landing
  layer, so it gets an explicit test.
- Age 0 weeks and the maximum observed age both produce sane dates.
- Every produced row has `date_precision = 'week'`.

**Why S:** the transformations are specified, and the one judgement call
(ambiguous version → NULL, never a guess) is stated here.

**Hand over:** the `package` schema, the three derivation rules, the
idempotency and offline-rebuild requirements.

**Done when:** the four tests pass.

---

### P1.7 — Size-suffix and version-split test tables · **H**

Exhaustive table-driven tests for the two pure functions in P1.6.

Sizes: `0`, `1K`, `999K`, `1.2M`, no suffix, garbage.
Version splits: `Foo-1.2.lha` → (`Foo`, `1.2`); `Foo1.2.lha`; `Foo.lha` →
(`Foo`, NULL); `Foo-2.0beta.lha`; `Mod.Foo.lha` → (`Mod.Foo`, NULL), because in
`mods/` a leading `Mod.` is a naming convention, not a version.

**Tests first:** the task is the tests.

**Why H:** the functions exist and every case with its expected output is
enumerated. Pure transcription.

**Hand over:** the two signatures and the case list **with expected values**.
Nothing else.

**Done when:** the tests pass — or a case fails and is **reported back rather
than the expectation being edited**. A cheap model asked to make tests pass
will otherwise adjust the expectation, which is why the values are given rather
than derived.

---

### P1.8 — RECENT-based incremental update · **S**

`RECENT` shares the INDEX line format but lists only new uploads. Ingest into
landing, normalize, upsert by `(dir, file)`, and return which packages changed
— the TUI will want that later.

**Tests first:**
- Ingesting `RECENT` after `INDEX` adds only genuinely new rows.
- Existing rows are left untouched, including their `id` (so foreign keys from
  `enrichment` and `selection_member` survive).
- The changed-package list matches exactly what was added or updated.

**Why S:** reuses P1.4 and P1.6 wholesale; the new parts are the upsert and the
change report.

**Hand over:** "same format as INDEX, reuse the parser", the `(dir, file)` key,
the id-stability requirement, the `recent_sample.txt` fixture.

**Done when:** the three tests pass.

---

### P1.9 — `HttpClient` trait + reqwest implementation · **S**

Invariant **I1**: `bam-core` must not call `reqwest` directly.

```rust
trait HttpClient {
    async fn get(&self, req: HttpRequest) -> Result<HttpResponse, HttpError>;
}
```

`ReqwestClient` implements it behind the `native` feature. Fetch `INDEX.gz` /
`RECENT.gz`, gunzip, hand the bytes to the ingest path. A descriptive
`User-Agent` with contact information (§16). Conditional GET with `ETag` /
`If-Modified-Since` stored per URL, so a repeat run of an unchanged INDEX costs
one 304.

**Tests first:**
- A fake `HttpClient` returning the P1.1 fixture bytes drives a full ingest
  with **no network access**.
- The stored ETag is sent on the second request.
- A 304 response inserts nothing and is not an error.
- A 500 surfaces as `HttpError`, not a partial ingest.
- One `#[ignore]`d test hits a real mirror, run explicitly and never in CI.

**Why S:** ordinary HTTP client work; every requirement is enumerated.

**Hand over:** the trait signature, the two URLs, the `User-Agent` format,
"store ETag per URL", invariant I1's feature-gating rule, and the fake-client
test requirement.

**Done when:** the five tests behave as listed.

---

### P1.10 — `bam ingest` CLI command · **S**

Wire P1.9 → P1.2 → P1.6 behind one subcommand, with `--offline` (fixtures only)
and `--rebuild-normalized` (skip fetch, re-derive from landing).

This is where invariant **I5** first bites. Progress is a typed, serializable
enum emitted through a `ProgressSink`:

```rust
enum ProgressEvent {
    Started   { operation: OperationId, total: Option<u64> },
    Advanced  { operation: OperationId, done: u64 },
    Finished  { operation: OperationId, outcome: Outcome },
}
```

The CLI implements the sink and renders a progress bar. The core formats
nothing — a preformatted string cannot become a web progress bar, cannot be
styled per frontend, and cannot be translated.

**Tests first:**
- A recording sink captures the expected event sequence for a fixture ingest.
- `ProgressEvent` round-trips through serde.
- `bam ingest --offline` populates a DB from fixtures.
- `--rebuild-normalized` works with the fake client configured to panic on any
  call, proving no network is touched.
- P0.4's purity test still passes (no `println!` reached into the core).

**Why S:** small, but it establishes the ProgressSink boundary. Getting it
right on first use costs nothing; retrofitting it across every phase costs a
lot.

**Hand over:** invariant I5, the `ProgressEvent` shape above, the three flags,
the five tests.

**Done when:** the five tests pass.

---

**Phase 1 exit:** ~84,000 packages in SQLite from a real INDEX, rebuildable
offline, incrementally updatable, with no formatted output anywhere in the
core. Worth pausing on.
