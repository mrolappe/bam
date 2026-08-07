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

## Round 11 — 2026-08-06 · Input model (P3.1) — Phase 3 start

**Done:**

- **P3.1** — `docs/input-model.md` plus `crates/bam-tui/src/input/mod.rs`.
  Added a `[lib] name = "bam_tui"` target to `bam-tui/Cargo.toml` (it was
  bin-only) so `tests/` — and later P3.4's UI code — can depend on the input
  module as a library; `main.rs` is untouched, since v1 doesn't wire the
  resolver into the app loop yet (that starts at P3.4). `Mode` and `Action`
  are the phase doc's own sketch verbatim; `ActionKind` is a new type not in
  the sketch — bindings in `bam.toml` name a count-independent action
  (`"move_down"`), and only `G` needs the count itself to change *which*
  `Action` variant comes out (`GoToRow(n)` with a count, `GoBottom` without),
  so `Resolver` resolves `ActionKind` + `Option<usize>` → `Action` rather
  than keymap entries pointing at `Action` directly. `Key` (a keypress,
  decoupled from crossterm/ratatui — neither is a dependency yet, and this
  module doesn't need one to be tested) and `Keymap` (`HashMap<String,
  ActionKind>`) both use one canonical string token per key
  (`Key::Ctrl('d')` → `"ctrl-d"`) as both the `bam.toml` spelling and the
  resolver's own sequence-matching key, so `gg`-style multi-key bindings need
  no separate parser — matching is `HashMap` lookup plus a prefix scan for
  `Pending`. `0` is handled as vim does: a digit that starts a count, unless
  it's a leading `0` with no count yet pending, in which case it's the
  `LineStart` binding instead. Four tests in `crates/bam-tui/src/input/
  mod.rs` (inline `#[cfg(test)]`, matching the module-is-the-artifact framing
  the phase doc uses elsewhere — not a separate `tests/` file), matching the
  task's four bullets exactly.

73 tests total (4 new + 69 pre-existing). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check (unaffected — `bam-tui` isn't part of it) all
clean.

**No deviations.**

---

## Round 12 — 2026-08-06 · Input resolver state machine (P3.2)

**Done:**

- **P3.2** — Read `Resolver::handle_key` (`crates/bam-tui/src/input/mod.rs`,
  built in Round 11) against the five test groups in `phase-3-tui.md` before
  writing anything: pending-count accumulation, prefix-sequence matching, and
  clear-on-reject were all already implemented as part of P3.1's own
  deliverable — P3.1 went further than its task text asked (which only
  required the *types*) and built the working state machine too. So P3.2's
  "implement the resolver" has no code left to do; its actual remaining scope
  is the five test groups themselves, which P3.1's four narrower tests didn't
  fully cover. Added five tests to the existing `#[cfg(test)]` module (no new
  test file, matching P3.1's own inline-test framing): `count_prefix_motions_
  resolve` (adds the `12G` → `GoToRow(12)` case P3.1 didn't test), `g_prefix_
  state_machine` (adds the `gg` → `GoTop` middle case — P3.1's own tests
  covered the `Pending` and `Rejected` ends of that sequence but never the
  resolved middle), `esc_clears_pending_state_from_any_partial_sequence`
  (both a pending count and a pending key-prefix, proving neither survives
  into the next resolution), `mode_transitions` (`v`/`Esc`/`:`/`/` against a
  keymap binding all four, none of which P3.1's `test_keymap()` included),
  and `count_with_no_following_key_remains_pending_indefinitely` (three
  digits in a row, still `Pending`, then resolves with the full accumulated
  count). No production code changed — confirmed by running the new tests
  against the unmodified `Resolver` before writing this note, not assumed.

78 tests total (5 new + 73 pre-existing). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check (unaffected — `bam-tui` isn't part of it) all
clean.

**Deviation for the next session to know about:** P3.2's task text frames
this round as an implementation task ("Implement the resolver from P3.1"),
but by the time this round started there was nothing left to implement —
Round 11 had already built it in full while delivering P3.1's own narrower
scope. Flagged per the same convention as Round 8/10's P2.8 note: read the
current state before assuming a task's text still matches what's left to do.

---

## Round 13 — 2026-08-06 · Default keymap + user override merge (P3.3)

**Done:**

- **P3.3** — `default_keymap()` (`crates/bam-tui/src/input/mod.rs`) builds
  the full 22-binding v1 table from the phase doc's list. One token had no
  home: `?` names no existing `ActionKind`, so a new variant,
  `ActionKind::OpenHelp` / `Action::OpenHelp`, was added — a small,
  necessary addition (P3.7's help overlay is its future consumer), not scope
  creep, since the task's own first test bullet ("the default table contains
  every binding listed above") is false without it. `space` needed the same
  treatment as `Key::Esc` already got: `Key::token()` special-cases
  `Key::Char(' ')` to `"space"` rather than emitting a literal space
  character, matching `docs/plan/phase-3-tui.md`'s own naming of it as a
  distinct token alongside `Esc`. `KeymapConfig { keys: HashMap<String,
  String> }` is the `[keys]` section's shape — deliberately just that section,
  not a full `bam.toml` aggregate struct, since highlight (P3.6) and launcher
  (P6.3) config are later tasks' own scope to add, not this one's to
  anticipate. `merge_keymap` layers overrides over the default table,
  recognizing the sentinel string `"unbind"` (chosen here — no prior doc fixed
  one) to remove a binding rather than replace it, and otherwise resolves an
  override's action name via `ActionKind`'s own `Deserialize` (through
  `serde_json::from_value` on a JSON string) rather than a second hand-written
  name table that could drift from the enum. Added the `toml` crate
  (workspace-pinned, `0.8`) as a real dependency, not just for this test: it's
  the format every `bam.toml`-parsing task from here on (P3.3, P3.6, P6.3)
  needs, and P3.3 is the first to actually need it, confirmed by
  `toml::from_str::<KeymapConfig>("")` in the fifth test rather than
  simulating "no `[keys]` section" with a bare empty `HashMap`. Promoted
  `serde_json` from `bam-tui`'s dev-dependencies to a real dependency, since
  `merge_keymap` (not just its tests) now calls it. Five new tests in the
  existing inline `#[cfg(test)]` module, matching the five bullets exactly.

83 tests total (5 new + 78 pre-existing). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check (unaffected — `bam-tui` isn't part of it) all
clean.

**Deviations for the next session to know about:**
- Added `ActionKind::OpenHelp`/`Action::OpenHelp` — not in P3.1's or P3.2's
  enum, needed because `?` (explicitly in P3.3's binding list) had nothing to
  bind to otherwise. No overlay logic consumes it yet; P3.7 is its intended
  consumer.
- The `"unbind"` sentinel string and the `[keys]`-only shape of
  `KeymapConfig` are both this round's own design choices, not dictated by
  `docs/input-model.md` or the phase doc — flagged in case a later task (or a
  real `bam.toml` loader) assumes a different convention.

---

## Round 14 — 2026-08-06 · TUI shell and virtualized list (P3.4)

**Done:**

- **P3.4** — The phase doc's three tests presuppose a windowed query
  primitive that didn't exist yet: `Session::search_packages` (P2.6)
  materializes every match into a `Vec<Package>`, which is exactly what
  "memory does not scale with result-set size" rules out. Added
  `Session::search_window(pred, offset, limit) -> (Vec<Package>, usize)`
  (`crates/bam-core/src/store/session.rs`) — wraps the existing compiled
  `SELECT id FROM package WHERE ...` in `SELECT COUNT(*) FROM (...)` for the
  total and `... ORDER BY id LIMIT ? OFFSET ?` for the page, reusing
  `compile::compile` rather than duplicating predicate-compilation logic;
  `matching_ids` and the new method now share a `compiled_for` helper that
  factors out the existing-named-selection check. Exposed as
  `api::search_window` (`SearchWindowRequest`/`SearchWindowResponse`,
  `crates/bam-core/src/api/`) alongside P2.6's `search_packages`, not
  replacing it — a full, unpaginated result list is still the right shape
  for a future `type:`/CLI/MCP caller that isn't rendering a scrolling list.
  Two tests in `tests/store_session_window.rs`, against 25 and 5 real
  inserted rows: a page's ids match the corresponding slice of the full
  unpaginated result, and an out-of-range offset returns an empty page with
  the correct total still reported.

  `crates/bam-tui` gained `ratatui`+`crossterm` (new workspace
  dependencies) and three new modules. `store::PackageStore` is a small
  trait (`window(pred, offset, limit) -> WindowResult{packages, total}`) —
  narrower than `bam_core::api::Session` so a test can inject a fake that
  counts calls without a database; `store::SessionStore` is the real
  implementation, adapting `api::search_window`. `app::App<S: PackageStore>`
  holds a `cursor` (absolute row) and a `top`/loaded `window`, re-querying
  only when a `move_down`/`move_up`/`go_top`/`go_bottom` call would move the
  cursor outside the currently loaded page — a scroll that stays inside the
  page costs nothing, and one that doesn't costs exactly one
  viewport-sized query, never the whole result set. `ui::render` draws three
  panes (query line, package list, detail) from `App`'s already-loaded
  `window` alone — it never queries. `app::all_packages()` (a `dir GLOB '*'`
  match, `dir` being `NOT NULL`) stands in for a real query until P3.5 wires
  the input line to the parser. Three tests in `crates/bam-tui/tests/
  tui_shell.rs`: a counting fake store proves the initial query is
  viewport-sized, that scrolling within the loaded page issues no further
  query, and that crossing the page boundary issues exactly one more
  viewport-sized (never total-sized) query; a 100-total and an 84,000-total
  fake store both leave `App` holding exactly 20 `Package` records; a
  `TestBackend` buffer snapshot for a 3-package fixture asserts all three
  panes' expected text is present.

  Also wired the shell into the `bam` binary as a new `tui` subcommand
  (`crates/bam-tui/src/main.rs`) — Round 11's own note ("v1 doesn't wire the
  resolver into the app loop yet... that starts at P3.4") flagged this as
  P3.4's implied scope, and a "TUI shell" that only exists in tests isn't
  one. Loads `bam.toml`'s `[keys]` section from `~/.config/bam/bam.toml`
  (new — P3.3 built `KeymapConfig`/`merge_keymap` but nothing read a real
  file yet) via P3.3's own merge function, falling back to the defaults on
  a missing file or an unknown-action error. The crossterm event loop
  converts key events to P3.1's `Key` type, feeds them through P3.2's
  `Resolver`, and dispatches only `MoveDown`/`MoveUp`/`GoTop`/`GoBottom`/
  `Quit` — every other resolved `Action` (modes, marking, help) is later
  rounds' scope (P3.5-P3.9) and is silently accepted but not acted on yet.

88 tests total (5 new — 2 in `store_session_window.rs`, 3 in `tui_shell.rs`
— plus 83 pre-existing; verified directly via `cargo test --workspace 2>&1 |
grep "test result:"` and summed, not hand-counted, per Round 10's own
caution about this). `cargo fmt --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, and the wasm32 `--no-default-features` check
(unaffected — `bam-tui` isn't part of it) all clean. Also smoke-tested the
real `bam` binary: `ingest --offline` still reports 501 packages against a
scratch DB, and `bam tui` against the same DB with no TTY attached (this
environment has none) hits the `enable_raw_mode` failure path cleanly rather
than panicking — the actual interactive rendering and key-handling loop is
**not** verified against a real terminal this round; say so explicitly per
the project's own standing rule on claiming UI features work.

**Deviations for the next session to know about:**
- `Session::search_window` and `api::search_window` are additions the phase
  doc's P2.6/P2.5 text never named — they exist only because P3.4's own test
  bullets are unsatisfiable without a paginated query primitive underneath
  the virtualized list. Flagged in case a later phase (P4's harvest/search
  work) expects `search_packages` alone to still be the one query surface.
- The `tui` subcommand, its `~/.config/bam/bam.toml` config path, and the
  choice to silently ignore non-navigation actions are this round's own
  design choices, not dictated by any phase doc — same convention as Round
  13's flagged `KeymapConfig`/`"unbind"` choices.

---

## Round 15 — 2026-08-06 · Query input line with inline errors (P3.5)

**Done:**

- **P3.5** — `Session::parse_query` (`crates/bam-core/src/store/session.rs`,
  native-gated) parses query-line text through `BamDsl` directly against the
  session's own `FieldRegistry` — no `LanguageRegistry` (P2.2) involved: it's
  the only registered surface syntax so far, and wiring a registry for one
  entry would be speculative ahead of a second language or a real
  `default_query_language` config key (both still doc-only, confirmed by
  grep before writing this). `SessionError` gained a `Parse(#[from]
  ParseError)` variant so the call composes with `?` like every other
  session method. `api::parse_query` (`crates/bam-core/src/api/query.rs`) is
  the thin typed wrapper (`ParseQueryRequest{ src }` /
  `ParseQueryResponse{ predicate }`), following P2.6's existing pattern
  rather than having `bam-tui` call `Session` directly — `store.rs`'s
  `SessionStore` already goes through `api::` for `window`, not `Session`
  itself, so this keeps one convention rather than two.

  `bam-tui`'s `PackageStore` trait (`crates/bam-tui/src/store.rs`) gained
  `fn parse(&self, src: &str) -> Result<Predicate, ParseError>` — returning
  the parser's own span-carrying `ParseError`, not `StoreError`, since the
  inline error marker needs the byte span, not a flattened string.
  `SessionStore::parse` unwraps `api::parse_query`'s `SessionError` back down
  to a `ParseError` (the only variant `parse_query` can actually produce;
  other variants get a spanless fallback rather than a `panic!`/`unwrap`).

  `App` (`crates/bam-tui/src/app.rs`) gained `query_text`, `query_error`, and
  `debounce_deadline: Option<Instant>`, plus `edit_query(text, now)` (records
  the text and resets a 150 ms deadline without querying) and `tick(now)`
  (applies the pending edit once the deadline has passed: a successful parse
  replaces `predicate`/`window` and resets the cursor; a `ParseError` is
  stored and `predicate`/`window` are left exactly as they were — the
  "keep last valid result set" rule). Both take an explicit `Instant` rather
  than reading the clock internally, so the four tests drive debounce timing
  without a real `sleep`. `ui::render` (`crates/bam-tui/src/ui.rs`) grew a
  second one-line row under the query line: when `query_error` is set, it
  renders spaces up to the error's column (`span.0` clamped to the text's
  last character, so an operator with nothing after it — `size>`, whose real
  parser span is one-past-the-end — still marks the `>` itself, not the
  column after it) followed by `^` and the message.

  `bam-tui/src/input/mod.rs` gained `Key::Backspace` (mapped from
  crossterm's `KeyCode::Backspace` in `main.rs`) — a small, necessary
  addition, same class as Round 13's `OpenHelp`: without it there's no way
  to correct a typo in the query line, which would make the feature
  built-but-unusable rather than merely v1-scoped. `main.rs`'s `run_loop`
  now tracks a local `Mode` (Normal by default): while `Mode::Insert` (`/`
  in the default keymap resolves to `Action::EnterMode(Mode::Insert)`, which
  `run_loop` catches before `apply_action` to flip the mode rather than
  falling through to `apply_action`'s ignored-action catch-all), keys append
  to or backspace out of the query text directly instead of resolving
  through the keymap — `Esc` returns to `Mode::Normal` and clears the
  resolver's pending state. `event::read()`'s indefinite block was replaced
  with `event::poll(50ms)` so `app.tick(Instant::now())` still runs (and a
  settled debounce still fires a query) even while no key arrives; the four
  tests themselves don't touch `main.rs` at all, matching every prior TUI
  round's precedent of testing `App`/`ui::render` directly.

  Four tests in the new `crates/bam-tui/tests/tui_query_line.rs`, matching
  the phase doc's four bullets exactly. `FakeStore::parse` calls the real
  `BamDsl`/`FieldRegistry` (both pure, no database) rather than a hand-rolled
  stub, so the error-span test exercises the actual parser's span for
  `dir:util/* size>` (byte offset 16, one past the trailing `>` at index 15)
  instead of an invented one; `FakeStore::window` only distinguishes the
  initial `all_packages()` predicate from any other, which is enough to prove
  a valid edit changes what's rendered without reimplementing glob/compare
  semantics in a fake. `crates/bam-tui/tests/tui_shell.rs`'s pre-existing
  `FakeStore` (P3.4) needed a one-line placeholder `parse` impl to satisfy
  the now-larger trait — unused by those three tests, noted inline.

92 tests total (4 new + 88 pre-existing). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check (unaffected — `bam-tui` isn't part of it) all
clean. Also smoke-tested the real `bam` binary: `ingest --offline` still
reports 501 packages, and `bam tui` against the same scratch DB still hits
the clean `enable_raw_mode` failure path with no TTY attached — the real
interactive typing/debounce/error-marker loop is **not** verified against a
real terminal this round, same standing caveat as every prior TUI round.

**Deviations for the next session to know about:**
- No `LanguageRegistry` wiring for the search box — `Session::parse_query`
  calls `BamDsl` directly. `docs/plan/phase-2-query-core.md` and
  `phase-3-tui.md` both mention a `default_query_language` config key, but
  it exists only in docs (confirmed by grep before writing this round's
  code), and P3.8 (highlight rules, invariant I3) is the task that actually
  needs to select among multiple registered languages. Revisit if a second
  query language is registered before then.
- `Key::Backspace` is a v1-necessary addition beyond what P3.1-P3.4 named,
  same class as Round 13's `ActionKind::OpenHelp` — flagged in case a future
  round assumes `Key`'s variant list is exactly what P3.1's doc enumerated.

---

## Round 16 — 2026-08-06 · Selection UI and `:` command line (P3.6)

**Done:**

- **P3.6** — `crates/bam-tui/src/store.rs`'s `PackageStore` trait grew seven
  methods (`toggle`/`is_marked`/`mark`/`select_by_query`/`save_as`/`load`/
  `list_selections`), each a thin `SessionStore` pass-through to P2.7's
  `bam_core::api` functions — "everything routes through the API" per the
  phase doc, same convention P3.4/P3.5 already established for `window`/
  `parse`. Two of those API functions had gaps found while wiring this up:
  `api::is_marked` didn't exist yet (added to `api/selection.rs`, same
  bare-`package_id` shape as `mark`/`unmark`/`toggle`), and `api::list` —
  built in Round 9 — was never re-exported from `api::mod`'s `pub use
  selection::{...}` list, a pre-existing miss with no prior caller to trip
  it; fixed by adding it to the same re-export line.

  `App` (`crates/bam-tui/src/app.rs`) gained a `marked: Vec<bool>` field
  parallel to `window.packages` — a *rendering cache* refreshed from
  `store.is_marked` after every window change (`new`, `tick`, `sync_window`),
  never an independent record of membership; the working selection in
  `store::session` stays the sole source of truth, the same relationship
  `window: WindowResult` already has to the real result set (P3.4). Added
  `visual_anchor: Option<usize>` (`enter_visual`/`leave_visual`), and
  `toggle_mark()`, which toggles the single row under the cursor normally
  but — when an anchor is set — marks every row in `[anchor, cursor]` via
  `mark_range` (a fresh `store.window(pred, start, len)` fetch of exactly
  that span, not the currently-loaded viewport, so a Visual selection wider
  than the viewport still marks correctly) and consumes the anchor.
  `command_text`/`status` are the `:`-line's own editing/output state
  (same class as `query_text`/`debounce_deadline`, not selection state), and
  `run_command(&str) -> Result<CommandOutcome, StoreError>` parses the five
  commands (`mark`/`unmark` reuse `store.parse` + `select_by_query` with
  `SelectionMode::Union`/`Subtract`; `save`/`load` unquote an optionally
  `"quoted name"`; `selections` returns the summaries) — `CommandOutcome`
  is a small enum so a test can assert on the result directly rather than a
  formatted string.

  `bam-tui/src/input/mod.rs` gained `Key::Enter` — necessary once a command
  line needs an explicit submit key, same class of small addition as
  Round 13's `OpenHelp` and Round 15's `Backspace`. `main.rs`: `apply_action`
  now takes `&mut Mode` and handles `ToggleMark`/`EnterMode(Visual|Command|
  Insert)`/`LeaveMode` (previously resolved but silently ignored beyond
  `Insert`, per Round 14's own note); a new `edit_command_line` mirrors
  `edit_query_line` for `Mode::Command` (`Enter` runs the command and
  surfaces `CommandOutcome`/error via `app.set_status`, `Esc` cancels).
  `ui::render` shows a `"* "` marker before a marked row's label (only when
  marked, so an all-unmarked buffer renders byte-identical to before this
  round — confirmed by Round 14/15's pre-existing snapshot tests staying
  green unmodified) and, in the row under the query line, the in-progress
  command text or the last status message when there's no query error.

  Four tests in the new `crates/bam-tui/tests/tui_selection.rs`, matching
  the phase doc's four bullets exactly: a `FakeStore` backed by a shared
  `HashSet<i64>` proves `space` toggles membership and the rendered buffer
  gains a `*`; entering Visual, moving down 3, then confirming proves
  exactly 4 rows (not 3, not 5) get marked; `run_command("mark dir:mus/*")`
  against a fake that only recognizes that one predicate string proves the
  `:mark` union; and a real file-backed `Session`/`SessionStore` (temp-path
  pattern from Round 9's `api_selection.rs`) proves `:save "tracker
  candidates"` then a *fresh* `Session::open` on the same path plus
  `:load "tracker candidates"` restores the marked state. The fifth bullet
  ("no selection state in the TUI") is a diff-review claim, not a test:
  `App`'s new fields are either rendering caches refreshed from the store
  (`marked`) or UI editing/output state (`visual_anchor`, `command_text`,
  `status`) — none of them is itself the record of what's selected; that
  stays in `store::session`'s `selection`/`selection_member` tables,
  reached only through `PackageStore`. Round 14/15's two pre-existing
  `FakeStore`s (`tui_shell.rs`, `tui_query_line.rs`) needed placeholder
  impls of the seven new trait methods to keep compiling — `is_marked`
  specifically returns `Ok(false)` rather than `unimplemented!()`, since
  `App::new` now calls it for every loaded row regardless of which test is
  running.

96 tests total (4 new + 92 pre-existing — the wasm32 check is unaffected by
either fix in `api/`, since the whole `api` module is `#[cfg(feature =
"native")]`-gated at the crate root, never compiled into that build).
`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
warnings`, and the wasm32 `--no-default-features` check all clean. Also
smoke-tested the real `bam` binary: `ingest --offline` still reports 501
packages, and `bam tui` against the same scratch DB still hits the clean
`enable_raw_mode` failure path with no TTY attached — the real interactive
Visual-mode/command-line loop is **not** verified against a real terminal
this round, same standing caveat as every prior TUI round.

**Deviations for the next session to know about:**
- `api::is_marked` and the `api::list` re-export fix are both P3.6-driven
  additions/fixes to `bam-core::api`, not named by the phase doc's P3.6 text
  (which only asks for the TUI-side wiring) — flagged in case a later round
  assumes `api::mod`'s re-export list was already complete.
- `App::marked`'s "rendering cache, not selection state" framing is this
  round's own resolution of the diff-review bullet — worth a second look if
  a future round finds it awkward that marked-state can go briefly stale
  between a `mark`/`unmark` elsewhere (e.g. a future GUI client sharing the
  same DB) and this session's next window refresh; P2.6's session-scoped
  model (I5) already accepts that two sessions don't observe each other
  live, so this is consistent with existing behavior, not a new gap.

---

## Round 17 — 2026-08-06 · Semantic token → ratatui style (P3.7)

**Done:**

- **P3.7** — `bam_core::highlight` (new top-level module, ungated — pure data
  and logic, confirmed by the wasm32 `--no-default-features` check):
  `Decoration` (`gutter`/`badge`/`background: Option<String>` +
  `priority: i32`, verbatim the plugin-output shape from `bam-handoff.md`
  §11.1) and `resolve(&[Decoration]) -> RowTokens`, the one conflict-
  resolution implementation both DSL rules (P3.8, not built yet) and plugin
  output (Phase 8) will feed. `background` is exclusive: a strict `>`
  comparison while folding left-to-right means the *first* decoration to
  reach the max priority wins ties, not sort/hash order — deterministic and
  stable by construction, so no separate tiebreak field or sort key was
  needed. `gutters`/`badges` stack, `sort_by_key(Reverse(priority))` (stable,
  preserving input order on ties) then `.take(3)`. `MARKED_GUTTER`/
  `MARKED_PRIORITY` (`i32::MAX`) are the constants marked-state rendering
  uses to build its own `Decoration` — the module doesn't special-case
  marked state itself, callers do, per the phase doc's "marked state flows
  through the same token path" instruction. Three tests in
  `tests/highlight.rs` (highest-priority background wins with the winner
  pinned; equal priorities resolve to the first-seen one, not hash order;
  four stacking gutters render exactly three, highest-priority-first).

  `bam_tui::tokens` (new module) is the one mapping table from token string
  to ratatui presentation — `background_style`, `gutter_char`, `badge_text`
  — each falling through to an unstyled/blank default on an unrecognized
  token rather than panicking (one inline test). `App::row_tokens(idx)`
  (`crates/bam-tui/src/app.rs`) builds the decoration list for a window-local
  row (currently just the marked-state `Decoration`, since P3.8's rule
  evaluation doesn't exist yet to contribute more) and calls
  `highlight::resolve` — the same function a future rule-driven decoration
  list will call, not a parallel path. `ui::render` (`crates/bam-tui/src/
  ui.rs`) now builds each row's gutter prefix, background `Style`, and badge
  suffix from `row_tokens` instead of the previous ad hoc `if marked {"* "}`
  check; since `gutter_char("marked") == '*'` and an unmarked row still
  resolves to an empty gutter list, the rendered buffer is byte-identical to
  before this round for every existing case — confirmed by Round 14's
  pre-existing snapshot test (`tui_shell.rs::buffer_snapshot_for_a_small_
  fixture`) passing unmodified. One integration test,
  `tests/tui_tokens.rs::a_marked_row_resolves_through_the_same_path_as_a_
  highlight_rule`, builds a `FakeStore`, marks a row, and asserts
  `app.row_tokens` for that row equals `resolve()` called directly on a
  hand-built `Decoration` carrying the same marked gutter/priority — proving
  the "same path" claim rather than asserting it only by code reading.

101 tests total (5 new — 3 in `highlight.rs`, 1 in `tokens`'s inline test, 1
in `tui_tokens.rs` — plus 96 pre-existing). `cargo fmt --check`, `cargo
clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean. Also smoke-tested the real `bam`
binary: `ingest --offline` still reports 501 packages against a scratch DB,
unaffected by this round's TUI-only rendering change.

**No deviations.**

---

## Round 18 — 2026-08-06 · Highlight rules with hot reload (P3.8)

**Done:**

- **P3.8** — `Session` (`crates/bam-core/src/store/session.rs`) gained a
  `langs: LanguageRegistry` field (built in `from_connection` — `bam-dsl` the
  only registered id, same as before, but now through the actual P2.2
  registry instead of a hardcoded `BamDsl.parse` call), `parse_query_lang`
  (`lang: Option<&str>` selects the language, `SessionError::Language(
  #[from] LanguageError)` new), and `matching_ids_among(pred, ids)` — matches
  restricted to a caller-supplied id list rather than the whole table, so the
  highlight engine only asks about the currently *visible* rows. Deliberately
  compiles regardless of whether `ids` is empty (checks `ids.is_empty()`
  *after* `compiled_for`, not before) so `ids: &[]` doubles as a load-time
  validation trial — catches a predicate that parses fine but doesn't compile
  (`Similar`, not yet supported) without needing a real row. `bam_core::api`
  gained `filter_ids` (`api/query.rs`) wrapping it, and `ParseQueryRequest`
  gained a `#[serde(default)] lang: Option<String>` field (`api/types.rs`) —
  the two existing call sites (`api::query::parse_query` itself,
  `bam-tui`'s `SessionStore::parse`) updated to pass `lang: None`.

  `bam-tui`'s `PackageStore` trait (`crates/bam-tui/src/store.rs`) gained
  `parse_lang` and `matching_ids`, both thin `SessionStore` pass-throughs to
  the new API calls — `parse_lang` returns a flat `StoreError` rather than
  `parse`'s span-carrying `ParseError`, since a highlight rule's error is
  reported as one line per rule, not an inline caret under a byte offset.

  New module `crates/bam-tui/src/rules.rs`: `HighlightRules` parses
  `[[highlight]]` blocks (`RuleConfig`: `name`, `lang`, `when`, `gutter`,
  `badge`, `background`, `priority` — the phase doc's shape verbatim) via
  `toml`, compiling each `when` through `store.parse_lang` and validating it
  through `store.matching_ids(&pred, &[])` (the empty-trial-compile use of
  the method above); a rule that fails either step is dropped and its
  message (`"{name}: {error}"`) recorded in `errors()` instead of aborting
  the reload — one bad rule cannot disable the others. Watched by **polling
  file content**, not a filesystem-event crate (`notify` was never added):
  nothing here needs more than "did the bytes change," and content-diffing
  sidesteps `mtime` granularity flakiness a real notify-based test would
  have to work around. `poll(now, store)` reuses P3.5's own debounce shape —
  a content change starts a timer, a *different* change while pending resets
  it (same as query-line edits), and only content that has held steady for
  `RELOAD_DEBOUNCE` (300ms) triggers a reload, so two rapid writes from one
  editor save collapse into one.

  `App` (`crates/bam-tui/src/app.rs`) gained `rules: HighlightRules`
  (`HighlightRules::empty()` until wired) and `rule_hits: Vec<Vec<usize>>` —
  a rendering cache, same relationship P3.6's `marked` already has to the
  working selection, refreshed alongside it by a renamed `refresh_marked` →
  `refresh_row_caches` (all five call sites — `sync_window`, `tick`, both
  `run_command` branches, `select_by_query_command` — updated together, a
  mechanical rename since rule hits depend on window contents the same way
  marked-state does). `set_highlight_config(path)` (new, not called by
  `App::new` — every existing test/caller that never calls it keeps
  pre-P3.8 behaviour exactly) loads rules and refreshes the caches once;
  `tick` polls `rules` every call (unconditionally, ahead of the query
  debounce check) and refreshes the caches when `poll` reports a reload
  happened. `row_tokens` now folds each hit rule's own `Decoration` into the
  list passed to `highlight::resolve` alongside the marked-state one —
  P3.7's "same path" claim now has a second real producer, not just marked
  state and a hand-built test double. `highlight_errors()` exposes
  `rules.errors()`; `ui::render` (`crates/bam-tui/src/ui.rs`) shows them
  (joined by `"; "`) in the row-1 slot, at the lowest priority (query error
  > command line > status > highlight errors).

  `main.rs` gained `resolve_config_path(flags)` (extracted out of
  `load_keymap`, which now takes the resolved path directly) so the `[keys]`
  and `[[highlight]]` sections of the same `bam.toml` are resolved once, not
  twice; `tui()` calls `app.set_highlight_config(&config_path)` right after
  `App::new`, failing the same way the DB-open and initial-query steps
  already do on a real error (a rule-level error never reaches this path —
  only a genuine `StoreError`, e.g. a DB failure, does).

  Ten tests: six in the new `crates/bam-tui/tests/tui_highlight.rs`, matching
  the phase doc's six bullets exactly. The first four (default/explicit/
  unregistered language, a bad `when` reported-and-skipped) drive a real
  `Session`/`SessionStore` against a seeded one-row DB — genuine parser/
  registry/compiler wiring is what's under test, not a fake's own hardcoded
  logic, same reasoning as `tui_selection.rs`'s save/load round trip. The
  last two (hot reload, debounce) use a `FakeStore` that echoes `when` back
  as `FullText` and counts `parse_lang` calls, driven by synthetic `Instant`s
  exactly like P3.5's own debounce tests — deterministic timing instead of
  real filesystem-event races. The other four now-existing `FakeStore`s
  (`tui_shell.rs`, `tui_query_line.rs`, `tui_selection.rs`, `tui_tokens.rs`)
  each needed placeholder `parse_lang`/`matching_ids` impls to keep
  compiling, same convention as every prior round's trait growth.

107 tests total (6 new in `tui_highlight.rs` + 101 pre-existing; verified via
`cargo test --workspace 2>&1 | grep "test result:"` and summed, per Round
10/14's own caution about hand-counting). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean. Also
smoke-tested the real `bam` binary: `ingest --offline` still reports 501
packages, and `bam tui --config <a file with one [[highlight]] rule>`
against the same scratch DB still hits the clean `enable_raw_mode` failure
path with no TTY attached — proving `set_highlight_config` runs without
erroring against a real config file, not just the test doubles; the real
interactive hot-reload loop is **not** verified against a real terminal this
round, same standing caveat as every prior TUI round.

**Deviations for the next session to know about:**
- No `default_query_language` key was added to `bam.toml`. The phase doc's
  own sample comment (`# optional; falls back to default_query_language`)
  reads as a config key, but with exactly one language registered, the
  `LanguageRegistry`'s own constructor-supplied default id (`"bam-dsl"`,
  set once in `Session::from_connection`) already satisfies "the configured
  default" — a `bam.toml` key that lets a user choose *among* registered
  languages is speculative until a second one exists, same YAGNI call
  Round 15's deviation note made about wiring the registry at all. Revisit
  together with that note once a second language is registered.
- "A rule whose `when` fails to compile" is read broadly: both a parse-time
  rejection (`FieldRegistry`'s type checks, e.g. `size:~'foo'`) and a
  predicate that parses but fails the separate IR→SQL compile step (only
  reachable today via `Similar`, not yet supported) are caught at rule-load
  time and treated the same way — recorded in `errors()`, rule dropped. This
  is why `reload` calls `store.matching_ids(&pred, &[])` as a trial after a
  successful parse, not just `parse_lang` alone.
- `HighlightRules` polls file *content* (a full read-and-compare each tick),
  not `mtime` — a deliberate simplification over the more obvious
  mtime-based watch, made specifically so the debounce test doesn't depend
  on filesystem timestamp granularity. Fine at `bam.toml`'s size; revisit if
  a much larger watched file ever makes a full read-per-tick measurably
  costly.

**Phase 3 remaining:** P3.9 (help overlay).

---

## Round 19 — 2026-08-07 · Help overlay (P3.9) — Phase 3 exit

**Done:**

- **P3.9** — `App` (`crates/bam-tui/src/app.rs`) gained `help: Option<Keymap>`
  and `open_help(keymap)`/`close_help()`/`help_open()`/`help_bindings()`.
  `open_help` takes the caller's own live `Keymap` by value rather than
  reading a copy `App` holds independently — the phase doc's "render from the
  same table P3.3 loads, so the overlay and the real keymap cannot drift
  apart" is satisfied by construction: there is only ever the one `Keymap`
  value, passed in, not a second copy `App` could fall out of sync with. This
  also avoided threading a `Keymap` through `App::new`'s constructor (and
  therefore every existing test call site across five test files) — the same
  "grow via a setter, not the constructor" convention P3.8's
  `set_highlight_config` already established for a caller-optional feature.
  `ui::render` (`crates/bam-tui/src/ui.rs`) draws the overlay, when open, as a
  full-frame bordered block listing every `"{token}  {action}"` line
  (`serde_json`-serialized `ActionKind` name, e.g. `"move_down"` — matching
  `bam.toml`'s own spelling — rather than a hand-written display table that
  could drift from `ActionKind`'s real variant names), sorted by token for a
  stable render order (`Keymap`'s underlying `HashMap` has none).

  `crates/bam-tui/src/main.rs`: `apply_action` gained a `keymap: &Keymap`
  parameter and an `Action::OpenHelp` arm calling `app.open_help(keymap.
  clone())`; `run_loop` gained a `keymap` parameter too and, ahead of both the
  existing Insert/Command line-editing intercepts, a check that closes the
  overlay on `Esc` or `q` and swallows the keypress — without it, `q` would
  still resolve through the keymap to `Action::Quit` while the overlay is
  open, since the overlay isn't a `Mode` variant the keymap's own bindings
  already exclude. `tui()` now builds `keymap` once and clones it into both
  `Resolver::new` and `run_loop`, rather than `Resolver` owning the only copy
  (it didn't expose one) — the smallest change that gives both the resolver
  and the overlay a value to work from without duplicating `load_keymap`'s
  file-read.

  Three tests in the new `crates/bam-tui/tests/tui_help.rs`, matching the
  phase doc's three bullets exactly: `overlay_binding_set_equals_the_active_
  keymap` asserts set equality between `help_bindings()`'s keys and the
  source `Keymap`'s keys (not a hardcoded list of the 22 tokens); `user_
  override_shows_the_users_key_not_the_default` merges a `KeymapConfig`
  override through P3.3's own `merge_keymap` first, then asserts the overlay
  carries the override's key, not a separate hand-built `Keymap`; `open_and_
  close_toggle_help_open` drives `open_help`/`close_help` directly. All three
  test `App` alone (no `FakeStore` behaviour beyond satisfying the trait) —
  the `?`-opens/`Esc`-or-`q`-closes wiring itself lives in `main.rs`'s
  `run_loop`, untested by any prior round's convention for interactive
  key-loop code (every TUI round's own standing caveat: the real terminal
  loop isn't verified against a real terminal).

110 tests total (3 new + 107 pre-existing). `cargo fmt --check`, `cargo
clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean. Also smoke-tested the real `bam`
binary: `ingest --offline` still reports 501 packages against a scratch DB,
and `bam tui` against the same DB with no TTY attached still hits the clean
`enable_raw_mode` failure path — the real interactive help-overlay loop is
**not** verified against a real terminal this round, same standing caveat as
every prior TUI round.

**No deviations.**

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

## Next task

P4.4 is done. Next is **P4.5** (readme header parser) — see
[phase-4-harvest-search.md](docs/plan/phase-4-harvest-search.md).

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
