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

## Next task

**P2.8** — register `in:`/`marked` as query fields. Per Round 8's deviation
note, P2.1 (Round 7) already made `InSelection` its own `Predicate` variant
and P2.5 (Round 8) already compiles it directly, independent of
`FieldRegistry` — check what P2.8's task text actually still needs to add
(if anything) before implementing it literally; it may be a smaller
reconciliation pass rather than new registry entries. See
[phase-2-query-core.md](docs/plan/phase-2-query-core.md).

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
