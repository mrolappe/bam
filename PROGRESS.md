# bam — Progress

Living status file, updated at the end of every implementation round.
Task ids refer to [`IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md) and the
phase documents under [`docs/plan/`](docs/plan/). Each closed phase's
round-by-round log lives under [`docs/progress/`](docs/progress/), linked
from a short summary here in its place; the current, still-open phase stays
inline until it closes.

---

## Rounds 0–6 — 2026-08-06 · Phase 0/1: setup and planning → workspace scaffold → ingest pipeline

Full round-by-round log moved to
[docs/progress/phase-0-1-setup-ingest.md](docs/progress/phase-0-1-setup-ingest.md)
to keep this file scannable — it covers planning (Round 0), the Cargo
workspace/CI/purity scaffold plus fixtures (P0.1–P0.4, P1.1), the DB schema
and migration runner (P1.2–P1.3), the INDEX parser and charset decoder
(P1.4–P1.5), the normalizer (P1.6–P1.7), the RECENT upsert and `HttpClient`
(P1.8–P1.9), and the `bam ingest` CLI with typed `ProgressSink` events
(P1.10). 37 tests by the end of Round 6.

---

## Rounds 7–10 — 2026-08-06 · Phase 2: query IR → bam-dsl → SQL compiler → API layer → selections → `in:`/`marked` reconciliation

Full round-by-round log moved to
[docs/progress/phase-2-query-core.md](docs/progress/phase-2-query-core.md)
to keep this file scannable — it covers P2.1 through P2.8: the query IR and
field registry plus the `QueryLanguage` trait (P2.1–P2.2), the `bam-dsl`
grammar/parser/compiler (P2.3–P2.5), the `bam-core::api` use-case layer and
selections (P2.6–P2.7), and the `in:`/`marked` reconciliation pass (P2.8,
Phase 2 exit). 69 tests by the end of the phase (32 added across the four
rounds).

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

## Rounds 20–26 — 2026-08-07 · Phase 4: fetch queue → rate limiter → background worker → readme storage/parsing → FTS5 → readme prioritisation

Full round-by-round log moved to
[docs/progress/phase-4-harvest-search.md](docs/progress/phase-4-harvest-search.md)
to keep this file scannable — it covers P4.1 through P4.7: the `fetch_queue`
schema with atomic claim (P4.1), the token-bucket rate limiter (P4.2), the
background fetch worker with `robots.txt` and backoff (P4.3), readme landing
storage (P4.4), the readme header parser (P4.5), the FTS5 index over
description and readme text (P4.6), and prioritising readmes for
visible/filtered entries (P4.7, Phase 4 exit). 147 tests by the end of the
phase (31 added across the seven rounds).

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

## Round 29 — 2026-08-07 · `Unpacker` trait, registry, magic-byte detection (P5.3)

**Done:**

- **P5.3** — `crates/bam-core/src/unpack/mod.rs`, new module, ungated per I1
  (mirrors `blob`'s shape: trait plus plain types compile to wasm32, only a
  real backend would be `native`-gated — P5.3 adds no backend, that's
  P5.4/P5.5). `Unpacker` trait (`id`/`handles`/`probe`/`unpack`) matches the
  phase doc's signature exactly. `UnpackerRegistry` follows P2.2's
  `LanguageRegistry` shape (`Vec<Box<dyn Unpacker>>`, not `dyn`-map) but its
  `select(format, override_id)` differs from `LanguageRegistry::get`'s plain
  id-lookup-with-default: it has two axes to satisfy (does an unpacker claim
  this *format*, and does it *probe* available) where the query-language
  registry only had one (id match), so selection is config override first —
  looked up by id, still required to `handle()` the format and `probe()`
  available, erroring rather than silently falling through if not — then the
  first registered unpacker that both claims the format and probes
  available.

  `detect_format` reads magic bytes only, never the filename: LZX archives
  open with the literal 4-byte signature `LZX\0`; LHA/LZH archives have no
  fixed leading signature (bytes 0–1 are header size and checksum) but always
  spell their method id as `-lh?-`/`-lz?-` at offset 2, which is what's
  checked. An unrecognized format's error carries the first 8 leading bytes
  for diagnosis rather than just saying "unknown".

  Six tests in the new `tests/unpack.rs` (the phase doc names five; a real
  magic-byte LHA-detects-as-LHA case was split out from the "lies about its
  extension" case as its own test, since the plan's "file named `.lha`
  routes to LZX" bullet doesn't by itself prove plain LHA bytes are also read
  correctly): a file with an LHA-shaped name but LZX magic bytes detects as
  LZX; genuine `-lh5-` magic bytes detect as LHA; unrecognized bytes error
  with the leading 8 bytes attached; a config override selects a specific
  registered unpacker over another that would otherwise win by list order;
  an unavailable unpacker (`probe` returning `Unavailable`) is skipped in
  favour of a working one rather than attempted; no available unpacker for a
  format produces an error whose text names both the format and an install
  hint. All six use fake in-test `Unpacker` impls returning `Ok(vec![])` from
  `unpack` — P5.3 is trait-plus-registry only, no real extraction is wired up
  or exercised yet (that's P5.4's `unar` backend and P5.5's `zip` backend).

160 tests total (6 new + 154 pre-existing, summed directly via `cargo test
--workspace 2>&1 | grep "test result: ok" | ...`). `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean. Also smoke-tested the real `bam`
binary (`ingest --offline`): still reports 501 packages, unaffected by this
round's new, self-contained module.

**Deviations for the next session to know about:**
- None. This round's scope matched the phase doc exactly — no table, no
  backend, no filesystem access; `ArchiveFormat` currently only has `Lha`
  and `Lzx` variants since those are the only two formats named anywhere in
  the plan (§4/P5.4/P5.5); a future zip-fixture round adds `Zip` when P5.5
  needs it.

---

## Next task

**Phase 5** — next is **P5.4**, `unar` backend (out of process) — see
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
