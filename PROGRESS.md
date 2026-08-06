# bam — Progress

Living status file, updated at the end of every implementation round.
Task ids refer to [`IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md) and the
phase documents under [`docs/plan/`](docs/plan/).

---

## Round 0 — 2026-08-06 · Setup and planning

**Done:**

- Git repo initialized; public GitHub repo created and pushed:
  https://github.com/mrolappe/bam
- First plan written from `bam-handoff.md`.
- Plan revised to absorb ten further requirements (below), then split into
  `IMPLEMENTATION_PLAN.md` as index plus conventions, and ten phase documents
  under `docs/plan/`.
- 67 tasks across 10 phases, each with a model tier, a *Tests first* list, a
  *Hand over* list, and a *Done when* check.

**No code yet.** The workspace does not exist.

### Requirements absorbed in the revision

TDD as the working method · a future web app variant alongside the desktop GUI
and TUI · pluggable query languages, with highlight rules naming theirs ·
pluggable archive unpackers · configurable keybindings with a vim-style input
model architected for modes, motions, and package-set "objects" · configurable
token-bucket rate limit · pluggable, cross-platform emulator launchers ·
persistent, queryable selections of packages · llama.cpp preferred over Ollama ·
Vue for the frontend.

---

## Round 1 — 2026-08-06 · Workspace scaffold + fixtures

**Done:**

- **P0.1** — Cargo workspace created: root `Cargo.toml` (resolver 2, edition
  2024, `rust-version = "1.85"`), `crates/bam-core` (lib, `default =
  ["native"]` gating `rusqlite`/`reqwest`/`tokio`), `crates/bam-tui` (bin
  `bam`). All three acceptance checks pass. rusqlite pinned to `0.32` —
  its `fts5` cargo feature doesn't exist (there's no separate toggle);
  `bundled` alone compiles FTS5 into the amalgamation
  (`-DSQLITE_ENABLE_FTS5` is unconditional in `libsqlite3-sys`'s bundled
  build), so the workspace `Cargo.toml`'s `rusqlite` feature list is just
  `["bundled"]`.
- **P0.2** — `.github/workflows/ci.yml`: fmt/clippy/test matrix on
  ubuntu-latest + macos-latest. Green locally; not yet observed green on a
  GitHub runner (see below).
- **P0.3** — wasm32 job added to the same workflow, required (not `continue-on-error`).
  **The plan's suggested sabotage doesn't reproduce a failure**: `use
  std::process::Command;` alone still passes `cargo check` for
  `wasm32-unknown-unknown`, because that target's std ships a real (always-
  erroring) `process` module rather than omitting it. Used an unconditional
  `use rusqlite::Connection;` instead — that's the sabotage that actually
  fails, and it's arguably the more faithful test of invariant I1 anyway.
  Amended in `phase-0-scaffold.md`.
- **P0.4** — Core purity test at `crates/bam-core/tests/purity.rs`, walking
  `src/` by hand (no new dependency). All four acceptance cases verified by
  temporary sabotage-and-revert, including the `src/store/`-exemption case.
- **P1.1** — Fixtures fetched from `https://ftp.fau.de/aminet/` (2026-08-06):
  `index_sample.txt` (506 lines, curated — see
  `crates/bam-core/tests/fixtures/README.md` for exactly which lines and why),
  `recent_sample.txt` (74 lines) and `tree_sample.txt` (381 lines), both
  committed in full.

All three CI jobs (`test (ubuntu-latest)`, `test (macos-latest)`, `wasm32`)
confirmed green on GitHub Actions after the push.

**Deviation for the next session to know about:** `grep` in this shell is
aliased to `ugrep -a`-less (`-I`, skip binary), which silently returns zero
matches on `fixtures/index_sample.txt` (detected binary, since it carries raw
Latin-1 bytes) and would do the same on any future INDEX-derived file. Use
`grep -a` or `perl -ne 'print if /pattern/'` against files under
`tests/fixtures/` or real Aminet data.

---

## Round 2 — 2026-08-06 · Schema + migration runner

**Done:**

- **P1.2** — Schema DDL in `crates/bam-core/migrations/0001_initial.sql`
  (all five tables, verbatim from the phase doc), with the Rust side —
  `Connection`-touching code, structs, insert/get functions — confined to
  `crates/bam-core/src/store/` per invariant I1, and the whole module gated
  `#[cfg(feature = "native")]` so `--no-default-features` wasm32 builds still
  compile with it absent. `store::tables` holds one struct plus
  insert/get pair per table (`LandingIndexLine`, `Package`, `Enrichment`,
  `Selection`, `SelectionMember`); insert takes `&T` and ignores the `id`
  field rather than adding a parallel `NewT` type per table. Five tests in
  `tests/store.rs`, one per acceptance bullet (not one per table) — round trip
  across all five tables in a single test, then unique-constraint,
  cascade-delete, drop/recreate-independence, and BLOB-fidelity as their own
  tests.
- **P1.3** — Migration runner in `crates/bam-core/src/store/migrations.rs`:
  a `const MIGRATIONS: &[Migration]` built with `include_str!`, applied in a
  loop keyed on `PRAGMA user_version` — no `refinery`/`sqlx-migrate`, per the
  plan's own steer. `store::open(path)` opens the connection, turns on
  `PRAGMA foreign_keys`, and applies migrations in one call; `":memory:"`
  works as `path` since `rusqlite::Connection::open` special-cases it, so no
  separate `open_in_memory()` was needed. Three tests in `tests/migrations.rs`.
  The "DB at version N only runs migrations > N" test needed a way to prove a
  no-op without a second real migration to peek at yet — it stamps
  `user_version = 1` on a table-less DB and asserts `apply_migrations` leaves
  it table-less, rather than asserting on a version number alone.

All eight new tests pass, plus the two pre-existing ones (purity, wasm32
`--no-default-features` check). `cargo fmt --check` and `cargo clippy
--workspace --all-targets -- -D warnings` both clean.

**Deviation for the next session to know about:** P1.2's "Round-trip
insert/select on each of the five tables" bullet was implemented as *one*
test exercising all five tables, not five separate tests — the task's "Done
when: the five tests pass" refers to the five acceptance bullets, and reading
it as five-tests-per-table would silently inflate the count to nine.

---

## Round 3 — 2026-08-06 · INDEX parser + charset decode

**Done:**

- **P1.4** — `parse_index_line(raw: &[u8]) -> Result<IndexRecord<'_>,
  ParseError>` in `crates/bam-core/src/ingest/index.rs`. The real fixture's
  3-line preamble (`fixtures/README.md`) turned out to carry **no
  column-title header row at all** — just a `|` banner — so "derive offsets
  from the header row" had nothing to derive from, and the phase doc's
  fallback clause ("fixed offsets when absent") applies. Pure fixed byte
  offsets don't work either: `file`/`dir`/`size`/`age` never contain a space,
  but an overlong filename (the `gcc-4.2.2-*-cygwin.tar.bz2` awkward-case
  lines) pushes every later column right by the overflow amount, so a fixed
  absolute offset for `dir` would misread those two lines. Implemented
  instead as a sequential scan for the first four whitespace-delimited
  tokens — correct regardless of column drift, since it doesn't depend on
  any absolute position — with the description taken as the exact remaining
  bytes (`&raw[pos..]`), untouched, so its internal double-spaces survive
  byte-for-byte. This satisfies the doc's real concern (don't
  `split_whitespace()` the *whole* line and rejoin, which would collapse the
  description's internal runs) without the brittleness of hardcoded column
  numbers. Preamble lines (`raw.first() == Some(&b'|')`) return
  `ParseError::Preamble`, a variant distinct from `Truncated`, so a caller
  can skip them without treating them as malformed data. Seven tests in
  `tests/ingest_index.rs`; the awkward-case lines are located in the fixture
  by filename prefix rather than hardcoded line number, so the tests stay
  correct if the fixture is ever re-curated.
- **P1.5** — `decode(bytes: &[u8]) -> (String, &'static Encoding)` in
  `crates/bam-core/src/ingest/charset.rs`. `chardetng` + `encoding_rs`, not
  gated behind `native` — both are pure computation with no OS dependency,
  confirmed by the wasm32 `--no-default-features` check passing. Verified
  `chardetng`'s actual source (`guess_assess` at
  `chardetng-0.1.17/src/lib.rs:2978`) rather than assuming its API: it
  returns `(&'static Encoding, bool)` where the bool is an explicit
  low-confidence signal, and for a generic/unknown TLD (we always pass
  `None`) its own default candidate is already `encoding_rs::WINDOWS_1252` —
  confirming `encoding_rs`'s modeling of legacy ISO-8859-1 as
  `WINDOWS_1252` (the WHATWG Encoding Standard's alias; the two differ only
  in the rarely-used 0x80-0x9F range) is the idiomatic fallback for this
  crate pairing, not a workaround. Made the fallback explicit in code anyway
  rather than relying on that implicit default, so intent doesn't silently
  depend on an internal detail of a dependency. Three tests in
  `tests/ingest_charset.rs`, not four: the phase doc's third bullet ("same
  code path serves both") isn't independently assertable at runtime — it's
  satisfied structurally by both the ISO-8859-1 and UTF-8 tests calling the
  same `decode()`, with no `decode_latin1`/`decode_utf8` pair to have
  diverged in the first place. Same reasoning as Round 2's five-bullets/one-
  test call for P1.2.

All 19 tests pass (10 new + 9 pre-existing). `cargo fmt --check`, `cargo
clippy --workspace --all-targets -- -D warnings`, and `cargo check -p
bam-core --no-default-features --target wasm32-unknown-unknown` all clean.

**Deviations for the next session to know about:**
- `chardetng` resolved to `0.1.17`; a `1.0.0` exists on crates.io but the
  workspace pin (`"0.1"`) was left as the lower, longer-established line
  rather than jumping a major version unprompted — revisit if P1.9's HTTP
  work wants something `1.0` added.
- No literal Aminet INDEX header row with column titles was found in real
  data (only the `|`-banner preamble) — if a future fixture (e.g. from a
  different mirror or format era) does carry one, `parse_index_line` doesn't
  use it; the token-scan approach doesn't need it, but the phase doc's
  "derive offsets from the header row" primary clause was never actually
  exercised.

---

## Round 4 — 2026-08-06 · Normalizer + size/version test tables

**Done:**

- **P1.6** — Pure derivation functions in
  `crates/bam-core/src/ingest/normalize.rs` (no `rusqlite`, so no `native`
  gate needed — confirmed by the wasm32 check): `parse_size_bytes` (K/M
  suffix, base 1024), `split_name_version` (splits on the *last* `-` only
  when what follows starts with an ASCII digit — Aminet's convention — else
  the whole stem is the name; naturally gives `Mod.Foo.lha` → `(Mod.Foo,
  NULL)` with no directory-awareness needed, since a bare `.` next to no
  dash never looks like a version split point), and `date_from_age_weeks`
  (age-in-weeks + `fetched_at` → ISO date). No date/calendar crate existed in
  the workspace and this is pure computation the wasm32 build must keep
  compiling, so date math is Howard Hinnant's `days_from_civil` /
  `civil_from_days` (public domain, ~20 lines) rather than adding a
  dependency for arithmetic this size. `normalize_line` combines these with
  P1.4's parser and P1.5's decoder into one landing-row → `NewPackage` step.
  The database half — `normalize(conn)` in `crates/bam-core/src/store/normalize.rs`
  — reads all `landing_index_line` rows, does a full `DELETE FROM package`
  then reinserts in landing order via `INSERT OR IGNORE` (so a `(dir, file)`
  collision keeps the first-seen row rather than erroring the whole rebuild —
  the real fixture has two: `WorldDATA.lha` and `agendafr.lha`, both in
  `biz/dbase`, artifacts of P1.1's curation splicing extra lines in). This is
  a full rebuild, not an upsert: it satisfies P1.6's own two DB tests, but
  does *not* by itself preserve `package.id` (and hence FK-linked
  `enrichment`) across a rebuild against a live DB — the phase doc's own text
  for P1.8 ("the new parts are the upsert and the change report") confirms
  that id-preserving upsert-by-`(dir, file)` is P1.8's job, layered on top of
  these same pure functions, not something P1.6 needs to solve. Four tests in
  `tests/store_normalize.rs` (idempotent, offline-rebuild via the same
  drop/recreate-`package` pattern as P1.2's store test, every row
  `date_precision = 'week'`, sane dates at age 0 and the fixture's observed
  max age of 999 weeks — real Aminet data, confirmed by scanning
  `index_sample.txt`'s age column).
- **P1.7** — Table-driven tests in `tests/ingest_normalize.rs`. The phase
  doc's case list is missing explicit expected values for more cases than it
  states: none of the six size cases have a `→`, and two of the five version
  splits (`Foo1.2.lha`, `Foo-2.0beta.lha`) don't either — only `Foo-1.2.lha`,
  `Foo.lha`, and `Mod.Foo.lha` do. Per Round 3's precedent for a doc/reality
  mismatch, this is reported rather than silently resolved: the missing
  values were derived from the doc's own stated rules (K/M-base-1024 for
  sizes; "ambiguous → whole stem, never guess" for splits) rather than
  invented — `Foo1.2.lha` has no `-` at all, so it's unambiguously the
  ambiguous case → `(Foo1.2, NULL)`; `Foo-2.0beta.lha`'s suffix after the
  last `-` starts with a digit, so it splits → `(Foo, 2.0beta)`. Flagged here
  for a human check rather than assumed correct.

All 25 tests pass (6 new + 19 pre-existing). `cargo fmt --check`, `cargo
clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean.

**Deviation for the next session to know about:** P0.4's purity test
(`tests/purity.rs`) does a raw substring scan for the literal text
`"rusqlite"` in any file outside `src/store/` — it doesn't parse comments
separately from code. A module-doc comment in `ingest/normalize.rs` that
*mentioned* `rusqlite` by name (explaining why the DB half lives elsewhere)
tripped it despite the file having no such dependency. Reworded to avoid the
literal string. Watch for this in any future `src/` doc comment that
discusses invariant I1.

---

## Round 5 — 2026-08-06 · RECENT upsert + HttpClient

**Done:**

- **P1.8** — `store::land::land_lines` factors out the split-body-into-lines
  → `insert_landing_index_line` loop that both this task and P1.9 need,
  shared rather than duplicated. `store::recent::upsert_recent` lands a
  RECENT body through it, then upserts each normalized row into `package` by
  `(dir, file)`: no existing row → insert; existing row whose derived fields
  differ → `UPDATE ... WHERE id = ?` (preserves `id`, so FK-linked
  `enrichment`/`selection_member` survive — the invariant P1.2's schema notes
  named and Round 4 deliberately deferred to this task); existing row with
  identical fields → left completely untouched, not even `landing_id` is
  rewritten, so re-listing an unchanged package costs nothing and never
  appears in the returned `Vec<ChangedPackage>`. Three tests in
  `tests/store_recent.rs`. The real fixtures turned out to have zero
  `(dir, file)` overlap (checked directly, 2026-08-06), so "existing rows
  untouched" is proven by running the same `recent_sample.txt` body twice and
  asserting the second run changes nothing; "changed-list matches exactly"
  needed a synthetic case, built by taking one real INDEX line
  (`A2KDeck.lha`) and swapping its size token — the parser is token-position
  based, so this doesn't require preserving column alignment — to prove an
  update is reported while an untouched duplicate of the same line, present
  in the same body, is not.
- **P1.9** — `http::HttpClient` (trait + `HttpRequest`/`HttpResponse`/
  `HttpError`) lives in the new `src/http/` module, ungated so a fake
  implementation needs neither the `native` feature nor a network to drive a
  test. `HttpClient::get` is `async fn` directly in the trait (native RPITIT,
  stable since 1.75) rather than `async-trait` — it's only ever called via
  `impl HttpClient`, never `dyn`, so the lint asking for explicit `Send`
  bounds is suppressed with a one-line `#[allow(async_fn_in_trait)]` rather
  than restructuring around a bound nothing here needs.
  `http::reqwest_client::ReqwestClient` (behind `native`) is the real
  implementation; `USER_AGENT` is `bam/{version} (+github repo URL)` — the
  handoff doc's §16 asks for a descriptive UA with contact info but doesn't
  fix a format, and a URL was chosen over an email address as the more
  conventional, less personally-identifying contact point for a bot UA.
  Gunzip is `ingest::gzip::gunzip`, ungated like P1.5's charset decode: it's
  `flate2` on its default `miniz_oxide` (pure-Rust) backend, confirmed
  wasm32-safe by the same `--no-default-features` check the other ingest
  modules pass. `store::fetch::fetch_and_land` is the one file that mixes
  `HttpClient` with `rusqlite` (I1 confines the *string* `rusqlite` to
  `store::`, not `reqwest` — the ETag lookup needs a `Connection`, so the
  orchestration has to live there even though `ReqwestClient` itself doesn't).
  It checks the stored ETag (new `http_cache(url, etag)` table, migration
  `0002`), sends conditional GET, returns `FetchOutcome::NotModified` on 304
  without touching landing at all, and only stores a new ETag and lands lines
  on 200 — an error from the client (e.g. mapped-from-500) propagates before
  any landing write, so a failed fetch can't leave a partial ingest. Five
  tests in `tests/http.rs`, including a `#[ignore = "..."]`d real-mirror test
  against `RECENT.gz`.

All 33 tests pass (8 new — 3 in `store_recent.rs`, 5 in `http.rs` with 1 of
those correctly `#[ignore]`d — plus 25 pre-existing). `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean.

**Deviation for the next session to know about:** adding migration `0002`
broke P1.3's `db_at_version_n_only_runs_migrations_above_n` test, which had
proved "migrations above N still run" by asserting *no* tables exist after
stamping `user_version = 1` on a table-less DB — that assertion was only ever
true because migration 1 was the *only* migration. Updated it to assert
exactly `http_cache` (migration 2's table) exists, which now proves both
halves of the invariant: migration 1 was skipped, migration 2 ran. Watch for
this same false-vacuous-pass pattern if a third migration lands.

---

## Next task

**Round 6 — P1.10** (Sonnet tier). See
[phase-1-ingest.md](docs/plan/phase-1-ingest.md) for the full task entry.

Wire P1.9 → P1.2 → P1.6 behind one `bam ingest` CLI subcommand, with
`--offline` (fixtures only) and `--rebuild-normalized` (skip fetch,
re-derive from landing). This is where invariant **I5** first bites: progress
is a typed, serializable `ProgressEvent` enum emitted through a
`ProgressSink` trait, not a preformatted string — the CLI implements the sink
and renders a progress bar, the core formats nothing. Hand over: invariant
I5, the `ProgressEvent` shape (`Started`/`Advanced`/`Finished`, each carrying
an `OperationId`), the three flags, the five tests (recording-sink event
sequence, `ProgressEvent` round-trips through serde, `--offline` populates a
DB from fixtures, `--rebuild-normalized` works with a fake client configured
to panic on any call — proving no network is touched, P0.4's purity test
still passes with no `println!` reaching into the core).

**Round 6 ends when** all five tests pass.

---

## Decisions carried forward

The eight architectural invariants are stated in full in
[`IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md). In brief:

- **I1** `bam-core` compiles to wasm32 with `--no-default-features`; host
  capabilities live behind traits; `rusqlite` is confined to
  `bam_core::store::*`.
- **I2** The query IR plus field registry is the contract; surface syntaxes are
  pluggable implementations over it.
- **I3** Highlight rules name their query language, defaulting from config.
- **I4** Query languages, unpackers, and launchers share one registry pattern.
  Unpackers dispatch on magic bytes, not extensions.
  `Launcher::capabilities()` drives behaviour, not just reporting.
- **I5** The API layer is session-scoped: no global state, typed serializable
  progress events, operation ids, cancellation tokens, no stdout in the core.
- **I6** Input is a `[count][operator]{motion|object|command}` resolver from
  the first commit; v1 registers motions only.
- **I7** Selections persist in SQLite and are queryable via `in:'name'` and
  `marked`.
- **I8** TDD: tests first, no network in the default run, and a delegated task
  may not weaken its test list.

Schema decisions that predate the invariants and still hold:

- Landing tables store **BLOB, never TEXT** — encoding is detected later and
  must stay correctable without re-fetching.
- `package.date_precision` distinguishes ±1-week INDEX-derived dates from exact
  ones. `week` may be upgraded to `exact`, never the reverse.

## Resolved open questions

From `bam-handoff.md` §14, answered 2026-08-06:

- **Target platforms** — macOS and Linux primary. Windows is not a priority:
  `crossterm` keeps it compiling, nothing tests it, no CI runner.
- **LLM provider default** — local, **llama.cpp**. Its native GBNF support is
  why it leads; Ollama and cloud endpoints work through the same
  OpenAI-compatible implementation, differing only in `capabilities()`.
- **Target emulator** — the question dissolved: launchers are pluggable
  (I4). FS-UAE is implemented first because it runs on both primary platforms.
  Amiberry and vAmiga are unscheduled follow-ons.

Also decided this session: web variant is a **Rust server plus browser UI**
(with a WASM read-only build kept reachable by I1); pluggability is **traits
and registries now, with the extism WASM host as scheduled Phase 8**;
selections are **persisted and queryable**; the frontend is **Vue**.

## Still open

Nothing blocking. Mirror rsync access is decided low-priority (2026-08-06):
on-demand queueing with priority boosting is acceptable as the baseline even
if a full incremental harvest takes several hours; see the note at the top of
[phase-4-harvest-search.md](docs/plan/phase-4-harvest-search.md).
