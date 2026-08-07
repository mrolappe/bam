# bam — Progress

Living status file, updated at the end of every implementation round.
Task ids refer to [`IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md) and the
phase documents under [`docs/plan/`](docs/plan/). A phase's round-by-round
log may be moved out to its own file under
[`docs/progress/`](docs/progress/) once it grows large, linked from a short
summary here in its place — see Phase 3's entry for the pattern.

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

## Round 6 — 2026-08-06 · `bam ingest` CLI + ProgressSink

**Done:**

- **P1.10** — `bam_core::progress` (new top-level module, ungated — pure
  `serde` data, confirmed wasm32-safe): `OperationId(u64)`, `Outcome`
  (`Success`/`Failed { message }`), `ProgressEvent`
  (`Started`/`Advanced`/`Finished`, verbatim per the phase doc's shape), and
  the `ProgressSink` trait (one `emit` method) implementing invariant I5's
  "typed, serializable progress, never a formatted string." Orchestration
  lives in `store::ingest::run_ingest`, `native`-gated alongside the rest of
  `store::`: it wires P1.9's `fetch_and_land` → P1.2's landing → P1.6's
  `normalize` behind one `IngestMode` enum (`Fetch` / `Offline` /
  `RebuildNormalized`), emitting `Started` (with a step-count `total`) →
  `Advanced` per step → `Finished`, with the error path emitting
  `Finished { outcome: Failed }` before propagating. `IngestMode::Offline`
  lands an `include_bytes!`-embedded copy of `index_sample.txt` — no
  filesystem fixture path needed at runtime, and it's the same fixture P1.1
  curated for exactly this purpose. `IngestMode::RebuildNormalized` never
  touches `client` at all, satisfying the phase doc's "prove no network is
  touched" requirement by construction rather than by a runtime check.
  `bam-tui/src/main.rs` is a thin wrapper: hand-rolled arg parsing (three
  flags don't justify a `clap` dependency), a `CliProgress` sink that
  `eprintln!`s each event, `ReqwestClient` for the real HTTP path, and
  `#[tokio::main]` for the async runtime the workspace already depends on.
  Added one small pure helper, `bam_core::now_rfc3339()`, reusing
  `ingest::normalize`'s existing (now `pub(crate)`) `civil_from_days` rather
  than adding a date crate for the CLI's `fetched_at` timestamp. Four tests
  in `tests/store_ingest.rs`, all driven through `run_ingest` directly rather
  than by spawning the compiled binary — consistent with every prior round's
  approach of testing core functions, and avoiding an `assert_cmd` dependency
  for what the phase doc's five bullets don't actually require: recording-sink
  event sequence, `ProgressEvent` round-trips through `serde_json`, `--offline`
  (i.e. `IngestMode::Offline`) populates a DB from the fixture, and
  `--rebuild-normalized` with a client that panics on any call — proving the
  no-network claim. The fifth bullet, P0.4's purity test, is the pre-existing
  test re-verified green, not a new one.

All 37 tests pass (4 new + 33 pre-existing). `cargo fmt --check`, `cargo
clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean. Also smoke-tested the actual `bam`
binary (`cargo run -p bam-tui -- ingest --offline`, then
`--rebuild-normalized`, both against a scratch DB): both report 501 packages
landed from the fixture, and the no-subcommand path still prints the version
banner.

**No deviations this round.**

---

## Round 7 — 2026-08-06 · Query IR + `QueryLanguage` trait (P2.1–P2.2)

**Done:**

- **P2.1** — `crates/bam-core/src/query/ir.rs` and `.../registry.rs`, plus
  `docs/query-ir.md` as one artifact per the task's own framing. `Predicate`,
  `CmpOp`, `Value`, `Pattern`, `SelectionRef` verbatim from the phase doc /
  invariant I2. `FieldId` wraps an owned `String`, not `&'static str`: a
  `Predicate` is built from arbitrary parsed or LLM-generated input and must
  round-trip through serde without borrowing from it, and a closed `FieldId`
  enum would make P2.8's "registering a field touches only the registry"
  claim false by construction. `FieldRegistry::resolve` matches name or
  alias; `check_compare` validates a `CmpOp` against `FieldDef.ops`;
  `check_match` validates `Match`/glob against `FieldDef.ty` — only `Text`
  fields permit it, so `size:~'foo'` is rejected at resolve time without a
  separate `matchable` flag. `package_fields()` maps eight fields to P1.2's
  `package` columns (`dir`, `file`, `name`, `version`, `size`/`size_bytes`,
  `date`/`uploaded_on`, `year`, `description`); `type` and `author` from
  `bam-handoff.md` §11's examples are deliberately absent — neither has a
  backing column yet (`type` awaits a derived category, `author` awaits
  Phase 4 harvesting) — recorded in the doc rather than stubbed. `year`
  shares `uploaded_on` with `date`; the doc notes the compiler must also
  consult `date_precision`, per P2.5's own "three non-obvious points." Five
  tests in `tests/query_ir.rs`, matching the five test bullets exactly (the
  operator/match-not-permitted bullet covers both a rejected `Match` on an
  `Int` field and a rejected `CmpOp` on a `Text` field, since the phase doc's
  one example — `size:~'foo'` — is the `Match` case specifically). The doc's
  "worked IR trees for a dozen queries" section includes two queries that
  don't yet compile (`type`/`author`-keyed ones) with the gap stated inline,
  rather than silently substituting a field that doesn't carry the same
  meaning.
- **P2.2** — `crates/bam-core/src/query/lang.rs`: `QueryLanguage` trait,
  `GrammarKind`, `ParseError`, `LanguageRegistry`. `ParseError` carries a
  `span: Option<(usize, usize)>` from this task rather than being added in
  P2.4 — the trait signature is the registered contract, and adding a field
  to it later would be the exact kind of breaking change pluggability (I2/I4)
  exists to avoid; P2.4 fills the span in, it doesn't add the field.
  `LanguageRegistry::get` takes `Option<&str>`, falling back to a
  constructor-supplied default id. Five tests in `tests/query_lang.rs`, two
  hand-rolled stub `QueryLanguage` impls (`EchoLang`, `MuteLang`) local to
  the test file — no real grammar exists yet (P2.3/P2.4), so stubs are
  correct here, not a shortcut.

Both modules are ungated (pure `serde` data plus a trait/registry, no
`rusqlite`), confirmed by the wasm32 `--no-default-features` check. Hit the
same purity-scanner false positive Round 4 flagged: a module-doc comment in
`ir.rs` originally named the excluded dependency by its literal crate name
and tripped P0.4's raw substring scan; reworded to "no database driver
dependency" — the scanner doesn't parse comments separately from code, and
this is now the second time a doc comment discussing invariant I1 has hit
it.

All 47 tests pass (10 new + 37 pre-existing). `cargo fmt --check`, `cargo
clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean.

**No deviations beyond the purity-scanner note above.**

---

## Round 8 — 2026-08-06 · `bam-dsl` grammar + parser + IR → SQL compiler (P2.3–P2.5)

**Done:**

- **P2.3** — `docs/lang-bam-dsl.md`: grammar, precedence table, fifteen
  worked examples with IR trees, and a malformed-input table with expected
  byte spans. Two design points not fully pinned down by the phase doc's
  grammar sketch, resolved and written down rather than guessed at
  implementation time: (1) `field:rhs` is one context-sensitive operator —
  `Match`/`Glob` if `rhs` contains `*`, else `Compare`/`Eq` — matching
  `docs/query-ir.md`'s own worked examples (`name:Deluxe*` vs `version:1.2`)
  where the phase doc's sketch reads as two separate forms; (2) the
  force-match operator is the two-character `:~` (`size:~'foo'`,
  matching `bam-handoff.md` §11.1's `author:~'Mustermann'` and the field
  registry's own error text), not a bare `~` — an early draft used bare `~`
  and was corrected before any code was written against it. Also documented:
  adjacent bareword terms merge into one `FullText` node spanning their
  combined source text (`docs/query-ir.md`'s own example 7,
  `tracker module editor` → one `FullText`, not three ANDed ones) — juxtaposition-as-AND
  has this one exception.
- **P2.4** — `crates/bam-core/src/query/bam_dsl.rs`, a hand-rolled recursive-
  descent parser (`nom` skipped per the phase doc's own note) implementing
  `QueryLanguage` as `BamDsl`. Byte spans on every `ParseError`.
  `FieldRegistry::field_names()` (registry.rs) is a small new accessor —
  every name and alias, for a Levenshtein-distance "nearest field" suggestion
  on `UnknownField`, not exposed before this task. `render` re-serializes to
  canonical `bam-dsl` text (`Eq` compares always render via `:`, other ops via
  their symbol; `Or`/`And` children are parenthesized under a same-or-higher-
  precedence parent so re-parsing can't reflatten a nested tree into a flat
  one) — its round-trip test asserts `parse(render(p)) == p` for each of the
  fifteen built predicates directly, not `render(parse(s)) == s`: byte-
  identical source reproduction isn't the invariant that matters for a UI
  "show and correct the generated query" round-trip, semantic fidelity is.
  Ten tests in `tests/query_bam_dsl.rs` (the phase doc's five groups; the
  fifteen-examples and malformed-input groups are table-driven, one test
  each, rather than fifteen-plus-six separate `#[test]` functions).
- **P2.5** — `crates/bam-core/src/store/compile.rs` (native-gated, inside
  `store::` per I1). `Predicate` → `SELECT id FROM package WHERE ...` plus a
  `Vec<rusqlite::types::Value>`; every literal is `params.push`ed, never
  formatted into the SQL string. `Predicate::InSelection` compiles directly
  to an `EXISTS` subquery over `selection`/`selection_member` (P1.2's
  schema) — see the deviation note below for why this, not
  `FieldRegistry`/`SqlSource`, is where that logic lives. `year>N` detects
  `field.name == "year"` and compiles a ±7-day window against
  `date_precision` (`CAST(strftime('%Y', CASE WHEN date_precision='exact'
  THEN uploaded_on ELSE date(uploaded_on, '±7 days') END) AS INTEGER)` at
  each edge), so a `week`-precision row whose uncertainty window straddles a
  year boundary is excluded rather than guessed into either side.
  `FullText` compiles to `description LIKE ? ESCAPE '\'` with `%`/`_`/`\`
  escaped — Aminet filenames routinely contain `_`, which is a `LIKE`
  wildcard otherwise. `SqlSource::Join` is left as
  `CompileError::UnsupportedJoinSource` — no current field uses it; see the
  deviation note. Six tests in `tests/store_compile.rs`, against a nine-row
  in-memory fixture DB built through P1.2's real insert functions (not
  hand-written SQL): the fourteen executable worked examples (`Similar` is
  compile-rejected by design, tested separately) against hand-computed
  expected id sets, a `'; DROP TABLE package; --` literal proven bound and
  the table proven intact, `GLOB` case-sensitivity, the year/`date_precision`
  boundary, `!(a OR b)` vs. `!a OR b` giving different results, and
  `Similar`'s rejection.

58 tests total (11 new + 47 pre-existing; one pre-existing `#[ignore]`d
real-mirror test not counted as newly run). `cargo fmt --check`, `cargo
clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean.

**Deviations for the next session to know about:**

- `bam-handoff.md` §11's own example and `docs/query-ir.md`'s worked examples
  3, 5, and 6 use `type:`/`author:`, neither of which is a registered field
  (`docs/query-ir.md`, "Deliberately absent," Round 7). A query using them
  can only ever produce `UnknownField`, so they cannot appear in P2.3's
  fifteen *successfully-parsing* examples. Worked example 4 substitutes
  `name` for `type` to preserve the example's real point (juxtaposition
  binding tighter than `OR`); `type:mod` itself is kept, unchanged, in the
  malformed-input table instead of silently vanishing. Flagged per the same
  doc/reality-mismatch convention as Round 3's INDEX-header case and Round
  4's missing size/version expected values.
- P2.8's task text (`docs/plan/phase-2-query-core.md`) describes `in:`/
  `marked` as "two entries in the field registry" resolving to an `EXISTS`
  subquery — but P2.1 (Round 7) already made `InSelection` its own `Predicate`
  variant, and P2.4's parser (this round) already parses `in:'x'`/`marked`
  directly to it, not to a `Compare`/`Match` against a registered field.
  P2.5 therefore compiles `InSelection` on its own, independent of
  `FieldRegistry`. P2.8's own remaining scope, when its round comes, is
  smaller than its task text implies (or may be redundant with what's
  already built) — worth a short reconciliation pass rather than following
  the task text literally, per that task's own instruction to report back
  rather than force a fit.
- `parse_size_bytes` (P1.6, `ingest/normalize.rs`) only recognizes uppercase
  `K`/`M`, correct for real Aminet INDEX data. The DSL's own grammar sketch
  calls for lowercase `k`/`M` in a *typed query value* (`size>100k`), which
  is user input, not INDEX data — `bam_dsl.rs`'s `typed_value` upper-cases
  before calling the shared function rather than forking a second size
  parser for one case difference.

---

## Round 9 — 2026-08-06 · `bam-core::api` use-case layer + selections (P2.6–P2.7)

**Done:**

- **P2.6** — `crates/bam-core/src/store/session.rs` (native-gated, inside
  `store::` per I1 — see the deviation note below for why the actual
  session logic lives there and not in `api::`) defines `Session`: one
  connection, one `FieldRegistry`, an eagerly-created ephemeral *working*
  selection row (`Drop` deletes it, cascading to its members via P1.2's FK —
  a named selection, `ephemeral = 0`, outlives the session), and an
  operation table (`Mutex<HashMap<OperationId, OperationStatus>>`) keyed by
  a session-local counter. `crates/bam-core/src/api/` (`mod.rs`, `types.rs`,
  `query.rs`, `selection.rs`, `ingest.rs`) is the thin, serializable-typed
  layer over it invariant I5 asks for: every request/response type derives
  `Serialize`/`Deserialize`/`schemars::JsonSchema` (new workspace
  dependency, ungated — `query::ir::Predicate`, embedded in
  `SearchPackagesRequest`, lives in the always-wasm-compiled part of the
  crate, confirmed by the `--no-default-features` wasm32 check still
  passing). `CancellationToken` is a new ungated `crate::cancel` module
  (`Arc<AtomicBool>`, two methods) rather than `tokio_util`'s — nothing here
  needed more than "cancel" and "is it cancelled," and a hand-rolled type
  keeps it usable with no async runtime for a future wasm caller.
  `ingest` (the only long-running operation that exists yet) is the vehicle
  proving the `CancellationToken`/`OperationId` rules against real work
  rather than a synthetic op built just to have something to cancel:
  `Session::run_ingest` checks `cancel` once before starting (`ingest::
  run_ingest` itself has only two coarse steps, fetch+land and normalize —
  no finer-grained point to poll mid-flight yet) and records every
  `ProgressEvent` into the operation table under a session-assigned id, so
  `operation_status(id)` answers after the call returns too, for a
  reconnecting client. `ingest::run_ingest` itself changed: it now takes a
  caller-assigned `OperationId` instead of a hardcoded `OperationId(0)` (4
  call sites updated — 3 in `tests/store_ingest.rs`, 1 in `bam-tui/src/
  main.rs`, all passing `OperationId(0)` to keep prior behavior). `Package`
  (P1.2) is reused directly as the API's package response type (added
  `Serialize`/`Deserialize`/`JsonSchema` derives) rather than duplicated
  into a DTO. Five tests in `tests/api_session.rs`.
- **P2.7** — `Session::{mark, unmark, toggle, clear, select_by_query,
  save_as, load, list_selections, delete_selection}`, all operating on the
  working selection P2.6 already built, exposed through `api::selection`
  (bare-`package_id` calls for `mark`/`unmark`/`toggle`/`clear` — one
  primitive argument doesn't earn a named request type the way
  `SelectByQueryRequest` or a by-name lookup does). `save_as` copies the
  working selection's current members into a new named row (`INSERT OR
  IGNORE ... SELECT`); `load` clears the working selection and copies from
  the named one — independent snapshots, not shared storage, so further
  `mark`/`unmark` on the working selection never mutates a saved one.
  `select_by_query`'s four `SelectionMode` variants reuse `mark`/`unmark`/
  `clear` rather than hand-rolling per-mode SQL. Six tests in
  `tests/api_selection.rs`, against real file-backed DBs (not `:memory:` —
  two independent `:memory:` connections can't demonstrate "shares a
  database but not session state," and the drop-cleanup test needs to
  reopen the same file after the `Session` that created it is gone).

69 tests total (11 new + 58 pre-existing). `cargo fmt --check`, `cargo
clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean. Also smoke-tested the `bam`
binary (`cargo run -p bam-tui -- ingest --offline`) against the
`run_ingest` signature change: still reports 501 packages, unchanged
progress output.

**Deviations for the next session to know about:**

- **All DB-touching session/selection code lives in `store::session`, not
  `api::`.** P0.4's purity scanner bans the literal substring `"rusqlite"`
  in any file outside `src/store/` — `Session` (and hence `Connection`,
  `rusqlite::Error`, bound-parameter queries) can't be named from `api/*.rs`
  without tripping it. `api::` therefore never touches SQL directly; it
  only calls `Session`'s plain-Rust methods and adapts typed request/
  response structs around them. This reads as the more correct shape
  anyway (I1's "rusqlite confined to `store::*`" already implied a
  session type living there), but it means `bam-core::api`'s own module
  doc originally tripped the same purity scanner by *naming*
  `println!`/`eprintln!` literally while describing rule 1 — same class of
  false positive Round 4 and Round 7 hit for `rusqlite`; reworded rather
  than special-cased in the scanner.
- Only `search_packages`, `get_package`, `list_categories`, and the P2.7
  selection ops got typed request/response wrappers, plus `start_ingest`/
  `operation_status` to give I5's cancellation/`OperationId` rules something
  real to prove themselves against — ingest isn't named in P2.6's task
  text, but no other long-running operation exists yet to exercise those
  two rules honestly.

---

## Round 10 — 2026-08-06 · `in:`/`marked` reconciliation (P2.8) — Phase 2 exit

**Done:**

- **P2.8** — Confirmed, by reading `query/registry.rs`, `store/compile.rs`, and
  `query/bam_dsl.rs` before touching anything, that Round 8/9's deviation note
  was right: `InSelection` is its own `Predicate` variant, the parser already
  parses `in:'x'`/`marked` straight to it, and the compiler already compiles
  it independent of `FieldRegistry`. There is nothing to add to the registry —
  P2.8's literal task text ("add two entries to the field registry") doesn't
  apply, and adding stub entries anyway would misrepresent how resolution
  actually works. Two of the task's three test bullets were already true and
  already covered (`tests/store_compile.rs::worked_examples_compile_and_return_expected_ids`
  exercises both `in:'tracker candidates'` and `marked !size<10k`). The third
  wasn't: `in:'nonexistent'` didn't error, it silently compiled to an `EXISTS`
  subquery that matches zero rows — `store::compile::compile` has no
  `Connection` to check existence with. Fixed by adding
  `Session::check_named_selections_exist` (`store/session.rs`), a small
  recursive walk over the predicate tree run once in `matching_ids` (shared by
  `search_packages` and `select_by_query`, so both routes get the check
  without duplicating it), erroring with the existing `SessionError::
  UnknownSelection` the same way `load` already does for the same condition.
  One new test, `tests/api_selection.rs::in_selection_naming_an_unknown_selection_errors`.
  The task's own acceptance criterion — "no file under `query/lang/` and no
  file in the compiler is modified" — holds: the fix lives in `store/
  session.rs`, the session layer, not the parser or `store/compile.rs`.

69 tests total (1 new + 68 pre-existing — Round 9's own "69 tests total" was
itself off by one; verified here directly with `git stash`/`cargo test`
rather than trusted). `cargo fmt --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, and the wasm32 `--no-default-features` check
all clean.

**Deviations for the next session to know about:**
- P2.8 turned out to be exactly the smaller pass Round 8's deviation note
  predicted, not the two-registry-entry task the phase doc describes.
- Round 9's stated test count ("69 tests total") was off by one (actual
  pre-existing count was 68) — no code discrepancy, just a miscount in that
  round's own report. Noted in case a future round's running total looks off
  by one again.

**Phase 2 exit reached.**

---

## Rounds 11–19 — 2026-08-06–2026-08-07 · Phase 3: input model → TUI shell → selections → highlighting → help overlay

Full round-by-round log moved to
[docs/progress/phase-3-tui.md](docs/progress/phase-3-tui.md) to keep this
file scannable — it covers P3.1 through P3.9: the vim-style input resolver
(P3.1–P3.2), default keymap with user overrides (P3.3), the virtualized TUI
shell (P3.4), the query input line with inline error markers (P3.5),
selection UI and the `:` command line (P3.6), semantic-token → ratatui
styling (P3.7), hot-reloadable highlight rules (P3.8), and the help overlay
(P3.9, Phase 3 exit). 110 tests by the end of the phase (37 added across the
nine rounds).

**Phase 3 exit reached.** A usable daily-driver TUI with configurable
vim-style bindings, persistent selections, hot-reloadable highlighting, and a
help overlay. Everything after this is additive, per the phase doc's own
closing line.

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

## Round 27 — 2026-08-07 · `BlobStore` trait + filesystem implementation (P5.1) — Phase 5 start

**Done:**

- **P5.1** — `crates/bam-core/src/blob/mod.rs`: `BlobHash` (a hex-encoded
  BLAKE3 digest, `Serialize`/`Deserialize` since it'll later live in
  `package.archive_hash`) and the `BlobStore` trait (`put`/`get`/`remove`),
  ungated — following the `HttpClient` pattern (P1.9/Round 5) exactly:
  plain trait in the crate root so a fake can be tested with no `native`
  feature, real implementation behind it. `blake3` added as a new,
  unconditional workspace dependency (pure computation, no OS dependency —
  confirmed wasm32-safe by the `--no-default-features` check, same tier as
  `chardetng`/`encoding_rs`).

  `FsBlobStore` (`blob/fs_store.rs`, `native`-gated) stores files at
  `<root>/aa/bb/<full-hash>`, two-level fanout. `put` streams the reader into
  a temp file while hashing; the real hash — and hence the real path — isn't
  known until the read completes, so an interrupted read can structurally
  never leave anything under a real hash name, satisfying that test without
  needing separate cleanup logic for the common case (the temp file itself is
  still removed on error, for hygiene, not because a test requires it).
  Identical content hashes identically, so a second `put` of the same bytes
  finds `dest.exists()` and drops its own temp copy rather than re-writing —
  dedup by construction, not a separate check. `get` reads the whole blob,
  recomputes its hash, and compares before returning a `Cursor` over the
  bytes — the after-the-fact verification §6 asks for, since Aminet
  publishes no checksums to check against up front; marked with a `ponytail:`
  comment (rehashes on every `get`, fine at Aminet archive sizes, revisit
  with streaming verification if that stops being true). Four tests in the
  new `tests/blob.rs`, matching the phase doc's four bullets exactly: a
  `Read` that yields some bytes then errors, asserted to leave zero files
  under the store root; identical bytes put twice yield the same hash and
  exactly one file on disk (the "two package references" half of that
  bullet is a `package`/`archive_hash` concern with no table wired to it
  yet — see the deviation note); a blob tampered with directly on disk
  fails `get` with `BlobError::Corrupted`; a hash that was never stored
  fails `get` with `BlobError::NotFound` rather than panicking.

151 tests total (4 new + 147 pre-existing; 149 run, 2 ignored — the two
pre-existing real-mirror tests). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean.

**Deviations for the next session to know about:**
- No `blobs` DB table or `package.archive_hash` column yet, despite §6
  describing both. P5.1's own hand-over list ("the trait, the fanout scheme,
  invariant I1's feature gating, the four tests") names no table, and none of
  the four tests need one — `BlobStore` alone is filesystem-only. P5.2's LRU
  eviction is the first task that actually needs `blobs(hash, size,
  last_used, pinned)` to operate on, so that migration is left for it rather
  than added speculatively here.
- P5.1's "storing identical bytes twice yields one blob with **two package
  references**" bullet is only half-tested: the blob-level half (one file,
  same hash) is; the package-level half needs `package.archive_hash`, which
  doesn't exist yet (see above). Flagged per the same convention as prior
  rounds' doc/reality gaps.

---

## Round 28 — 2026-08-07 · LRU eviction with pinning (P5.2)

**Done:**

- **P5.2** — `crates/bam-core/migrations/0006_blobs.sql`, migration 6:
  `blobs(hash PRIMARY KEY, size, last_used, pinned)` plus `ALTER TABLE package
  ADD COLUMN archive_hash TEXT REFERENCES blobs(hash)` — the §6 mapping table
  P5.1 deliberately deferred (Round 27's own note: "P5.2's LRU eviction is the
  first task that actually needs `blobs`"). `archive_hash` was **not** added
  to the shared `Package` struct/`insert_package`/`get_package` — every
  pre-P5.2 caller across the codebase constructs a `Package` literal (17 call
  sites, confirmed by trying the struct-field approach first and watching it
  break all of them), and nothing outside this round's own eviction code needs
  to read or write the column yet. Two small raw accessors,
  `tables::{set_archive_hash, get_archive_hash}`, cover exactly what P5.2
  needs instead.

  New `store::blob_cache` (native-gated, inside `store::` per I1):
  `record_blob`/`touch`/`set_pinned` (thin upserts/updates on `blobs`), and
  `evict_to_budget<B: BlobStore>(conn, store, budget_bytes)` — generic over
  the trait (not `dyn`, since `BlobStore::get` returns `impl Read`, not
  object-safe), so a test can pass a real `FsBlobStore`. Loops: while total
  `blobs` size exceeds budget, pick the least-recently-used **unpinned** blob
  (`ORDER BY last_used ASC LIMIT 1 WHERE pinned = 0`), remove its bytes via
  `store.remove`, clear `archive_hash` on every package that pointed to it,
  then delete its `blobs` row — **in that order**: deleting the `blobs` row
  before clearing the referencing `package.archive_hash` trips the FK added
  by this same migration (`foreign_keys` is on by default via `store::open`)
  and was caught by the first test run, not reasoned out in advance. Never
  touches `package`(rows) or `enrichment` — the eviction loop has no DELETE
  against either table, so the hard invariant ("enrichment rows survive
  eviction... the single most expensive mistake available in this codebase")
  holds by the code simply not containing the capability to violate it, not
  by a guard checking for it. Runs out of unpinned blobs before the budget is
  met → `EvictionError::BudgetNotMet`, evicting nothing further; a pinned
  blob is never a candidate the loop even considers, not one it considers and
  rejects.

  Five tests in the new `tests/store_blob_cache.rs`, matching the phase doc's
  five bullets exactly, all against a real `FsBlobStore` over a temp
  directory and real inserted blobs (not hand-written `blobs` rows with no
  backing file) so `store.get`/`store.remove` genuinely succeed or fail:
  a two-blob DB evicted to the pinned blob's own byte size keeps the pinned
  file on disk and removes the other; an `enrichment` row on the evicted
  package's own id is read back unchanged after eviction; `get_package`
  still returns the row and `archive_hash` reads back `None` (not the stale
  hash); three blobs with strictly increasing `last_used`, evicted to a
  one-blob budget, removes the two oldest in age order, not insertion or
  hash order; an all-pinned single-blob DB evicted to budget 0 returns
  `BudgetNotMet` and the blob is still readable afterward.

  `tests/migrations.rs`'s `db_at_version_n_only_runs_migrations_above_n` hit
  a new variant of the false-vacuous-pass pattern Round 5/20/23/25 already
  flagged for migrations 2-5: this is the first migration whose DDL
  (`ALTER TABLE package`) requires a *prior* migration's table to actually
  exist, which the test's own "stamp `user_version = 1`, never run migration
  1's DDL" setup doesn't provide — caught as a real test failure (`no such
  table: package`), not a false pass. Fixed by running migration 1's DDL
  directly (bypassing `apply_migrations`, so it isn't the thing under test)
  before stamping the version; the test still proves migration 1 itself
  doesn't *re*-run (a second `CREATE TABLE package` would collide and the
  `unwrap()` would fail) while assertion now covers the full table set.

154 tests total (5 new + verified pre-existing baseline; summed directly via
`cargo test --workspace 2>&1 | grep "test result: ok" | ...`, not taken from
the prior round's stated count — Round 10/14's own caution, and Round 10's
own count was off by one previously). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean. Also smoke-tested the real `bam`
binary (`ingest --offline`): still reports 501 packages, unaffected by this
round's schema/store-only changes.

**Deviations for the next session to know about:**
- `package.archive_hash` exists in the schema but is read/written only by
  this round's own `store::blob_cache` and its raw `tables::` accessors —
  not by the shared `Package` struct, `insert_package`, or `get_package`.
  Whichever future task actually sets it on a real fetch (P5.4/P5.5's
  unpacker backends, or a caching layer over `store::fetch`) will need either
  these same raw accessors or a considered decision to fold the field into
  `Package` at that point, updating all 17 existing call sites.
- `evict_to_budget` recomputes `SELECT SUM(size)` from `blobs` on every loop
  iteration rather than tracking a running total in memory — same
  "simplest correct thing at current scale" call Round 25 made for
  `rebuild_fts`'s per-package readme lookups; revisit if eviction over a
  large `blobs` table is ever measured to be slow.

---

## Next task

**Phase 5** — next is **P5.3**, `Unpacker` trait, registry, magic-byte
detection — see
[phase-5-cache-extraction.md](docs/plan/phase-5-cache-extraction.md).

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
