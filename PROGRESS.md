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

## Next task

**Round 4 — P1.6–P1.7** (Sonnet then Haiku tier).
See [phase-1-ingest.md](docs/plan/phase-1-ingest.md) for the full task
entries.

1. **P1.6** — Normalizer: landing rows → `package` rows, applying P1.4 and
   P1.5. Derives `size_bytes` from `"134K"`/`"1.2M"` (K/M suffixes, base
   1024), `uploaded_on` from age-in-weeks plus `fetched_at`
   (`date_precision = 'week'`), and splits `name`/`version` from the
   filename — ambiguous split puts the whole stem in `name` and leaves
   `version` NULL rather than guessing. Hand over: the `package` schema, the
   three derivation rules, the idempotency requirement (`normalize()` run
   twice yields identical rows) and the offline-rebuild requirement
   (drop `package`, re-normalize from `landing_index_line` alone, network
   unavailable).
2. **P1.7** — Exhaustive table-driven tests for the two pure functions in
   P1.6 (size-suffix parse, name/version split), using the exact cases and
   expected values given in the phase doc. If a case's expected value looks
   wrong, report it back rather than editing the expectation.

**Round 4 ends when** P1.6's four tests pass (idempotent, offline-rebuildable,
sane dates at age 0 and the max observed age, every row `date_precision =
'week'`) and P1.7's table-driven tests pass against the given expected
values.

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
