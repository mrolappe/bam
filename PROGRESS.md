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

## Next task

**Round 1 — P0.1–P0.4 and P1.1** (all Haiku tier).
See [phase-0-scaffold.md](docs/plan/phase-0-scaffold.md) and
[phase-1-ingest.md](docs/plan/phase-1-ingest.md) for the full task entries.

1. **P0.1** — Cargo workspace. Root `Cargo.toml` (resolver 2, edition 2024,
   `rust-version = "1.85"`), `crates/bam-core` (lib), `crates/bam-tui` (bin,
   named `bam`). `bam-core` gets `default = ["native"]` with `rusqlite`,
   `reqwest` and `tokio` behind it. Do **not** create `bam-gui`, `bam-server`,
   `bam-mcp`, or `frontend/`.
2. **P0.2** — CI: `cargo fmt --check`, `cargo clippy -- -D warnings`,
   `cargo test --workspace`, on Linux **and** macOS. Windows deliberately
   excluded.
3. **P0.3** — wasm32 check job: `cargo check -p bam-core --target
   wasm32-unknown-unknown --no-default-features`. **Required, not advisory.**
   Prove it bites by temporarily adding `use std::process::Command;` to
   `bam-core` and confirming the job fails, then revert.
4. **P0.4** — Core purity test: a `#[test]` that fails if `rusqlite` appears
   outside `src/store/`, or if `println!`/`eprintln!` appears anywhere in
   `bam-core`.
5. **P1.1** — Fixtures in `crates/bam-core/tests/fixtures/` from
   `https://ftp.fau.de/aminet/`: a ~500-line `index_sample.txt` plus
   `recent_sample.txt` and `tree_sample.txt`. The INDEX sample must contain
   long filenames, descriptions with internal whitespace runs, non-ASCII bytes,
   a zero-size entry, and the header/preamble lines. Record the source URL and
   fetch date in `fixtures/README.md`.

**Round 1 ends when** `cargo build --workspace` succeeds, all four CI jobs are
green, P0.3 has been demonstrated to fail on a sabotaged tree, and the three
fixtures are committed.

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

- **Mirror rsync access** — unconfirmed. Ask a mirror operator (e.g.
  ftp.fau.de) whether bulk rsync of `.readme` files is available. A single
  rsync pass beats 84,000 HTTP requests. Decides whether P4.3 is the bulk
  harvesting path or only the incremental one. Worth an email well before
  Phase 4 — the answer takes days, the harvest takes twelve hours.
