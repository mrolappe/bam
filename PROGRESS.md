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

## Rounds 27–34 — 2026-08-07 · Phase 5: blob cache → LRU eviction → unpacker registry → `unar`/`zip` backends → LHA header reader → `.uaem` sidecar → inventory enrichment

Full round-by-round log moved to
[docs/progress/phase-5-cache-extraction.md](docs/progress/phase-5-cache-extraction.md)
to keep this file scannable — it covers P5.1 through P5.8: the content-addressed
`BlobStore` trait and filesystem implementation (P5.1), LRU eviction with
pinning that never touches `enrichment` (P5.2), the `Unpacker` trait and
registry with magic-byte format detection (P5.3), the out-of-process `unar`
backend (P5.4), the in-process `zip` backend (P5.5), the LHA extended-header
reader with a flagged best-effort Amiga extension (P5.6), the `.uaem` sidecar
writer (P5.7), and archive inventory enrichment (P5.8, Phase 5 exit). 185
tests by the end of the phase (34 added across the eight rounds).

**Phase 5 exit reached.** Archives are cached by content hash, extracted
through a registry with two working backends, with Amiga attributes
preserved as `.uaem` sidecars and file inventories captured as enrichment —
§15's "hard core" is complete.

---

## Rounds 35–39 — 2026-08-08 · Phase 7: LLM provider → grammars → query prompt → embeddings/sqlite-vec → summaries

Full round-by-round log moved to
[docs/progress/phase-7-llm.md](docs/progress/phase-7-llm.md) to keep this
file scannable — it covers P7.1 through P7.5: the `LlmProvider` trait and
`OpenAiCompatibleProvider` covering llama.cpp/Ollama/cloud (P7.1), GBNF and
JSON Schema grammar generation from one source per query language (P7.2),
the natural-language-to-`bam-dsl` query generation prompt with the
always-editable, never-auto-run guarantee held structurally (P7.3), batched
resumable readme embeddings via sqlite-vec that make `Predicate::Similar`
real (P7.4), and batched resumable `llm_summary` enrichment with per-package
error isolation and a confirmation-gated cost estimate (P7.5, Phase 7
exit). 208 tests by the end of the phase (14 added across the five rounds).

**Phase 7 exit reached.** Natural-language search that emits inspectable
DSL, semantic similarity, and summaries — all runnable entirely locally
against a llama.cpp server, per §10's hard requirement.

---

## Rounds 40–43 — 2026-08-08 · Phase 6: `Launcher` trait/registry → FS-UAE launcher → launcher configuration → launch a selection

Full round-by-round log moved to
[docs/progress/phase-6-launchers.md](docs/progress/phase-6-launchers.md) to
keep this file scannable — it covers P6.1 through P6.4: the `Launcher`
trait, `LauncherCaps`, and `LauncherRegistry::select` with config-override
and capability-driven selection (P6.1), the `FsUaeLauncher` extracting
through P5.3's registry with `.uaem` sidecars and a `bam.fs-uae` config
(P6.2, plus a manual end-to-end run that found a real gap in the
`AMIGA_EXT_TYPE` placeholder), `bam.toml` launcher configuration —
candidate-path override, extra args, preference ordering (P6.3), and
sequential continue-on-failure launches over a selection with a
size-threshold confirmation gate and cooperative cancellation (P6.4, Phase
6 exit). 229 tests by the end of the phase (15 added across the four
rounds).

**Phase 6 exit reached** on the core side: a selection's cached archives
launch through a pluggable, capability-driven registry with
continue-on-failure and a confirmation gate. Wiring an actual TUI keybinding
(`S`) to this function is unscheduled follow-on work, not blocking — the
phase doc's four P6.4 tests target the core batch behavior only.

---

## Round 44 — 2026-08-08 · Phase 9: Vue frontend and transport interface (P9.1)

One `frontend/` package (Vue 3, `<script setup>` + TypeScript, Vite) with a
`BamClient` interface as the only seam components use — `TauriClient` and
`HttpClient` implement it, neither imported by anything under `components/`.
Request/response types are generated, not hand-written: `bam-core::api`
gained a `schema` module (`all_schemas()`, mirroring P7.2's
`bam_dsl_json_schema` pattern) and an `export_api_schema` example that
prints it as JSON; `frontend/scripts/gen-types.mjs` turns that into
`src/generated/types.ts` via `json-schema-to-typescript`, merging every
type's independent `schema_for!` output into one shared `definitions` map
first so shared types (`Predicate`, `Package`, ...) don't get emitted once
per referencing root and collide.

All five of P9.1's required tests pass: a `PackageList` component test
against a mock `BamClient` with no real transport present; one contract
suite (`describe.each`) run against both `HttpClient` and `TauriClient`,
covering request/response shape and progress-stream termination on both
`Finished` and `AbortSignal`; a staleness test that regenerates the types
in-memory and diffs against the checked-in file; and a grep-style test
over every file under `components/` rejecting `@tauri-apps/api` and direct
`fetch(` calls. CI gained a `frontend` job running `npm run typecheck` and
`npm test`. 230 Rust tests, 11 frontend tests.

**Next:** P9.2, the `bam-server` HTTP/SSE adapter that gives `HttpClient` a
real backend to talk to (currently only exercised against mocks).

---

## Round 45 — 2026-08-08 · Phase 9: `bam-server` HTTP/SSE adapter (P9.2)

A new `bam-server` crate, routed exactly to the paths and JSON shapes
`frontend/src/transport/HttpClient.ts` already assumed from Round 44 — no
frontend changes needed. Sessions are cookie-based (`bam_session`, set on
first response); every `bam_core::api` call goes through a `SessionHandle`
that hands a closure to the session's own dedicated OS thread rather than
sharing `Session` across axum's multi-threaded executor — `Session` wraps a
`rusqlite::Connection`, which is deliberately not `Sync` (I1's purity check
never asked it to be; its only prior caller, `bam-tui`, is single-threaded),
so this keeps `bam-core` untouched rather than forcing thread-safety onto it.
An ingest's `active` broadcast-sender slot lives in its own
`std::sync::Mutex`, outside the actor's job queue: an ingest occupies the
actor thread for its whole duration, so a progress subscription that had to
queue behind it would only ever learn about progress after the ingest had
already finished — reading that mutex directly instead lets a reconnecting
SSE client subscribe immediately while the ingest is still running, or fall
back to a synthesized terminal event from `operation_status` once it's not.

All five acceptance items hold: every `bam_core::api` operation reachable
over HTTP and round-tripping its types (one test walking parse → search →
mark → select → save/load/delete → clear across the real routes); two
sessions (two cookie jars) confirmed not to observe each other's marks;
SSE delivering a real `Started`/`Advanced`/`Finished` sequence for an
offline ingest; a client that disconnects and reconnects with the same
`OperationId` resolving to `Finished` either way, with the ingest itself
proven to have actually completed rather than being orphaned by the first
disconnect; and a grep-based purity test in the spirit of P0.4 confirming
no `rusqlite` name and no raw SQL keyword anywhere in `bam-server/src`. 235
Rust tests project-wide (5 added), CI's existing `--workspace` fmt/clippy/
test steps cover the new crate with no workflow changes.

**Next:** P9.3, the Tauri shell — a thin host providing `TauriClient` for
the same `frontend/` build `bam-server` now serves over HTTP.

---

## Round 46 — 2026-08-08 · Phase 9: Tauri shell (P9.3)

A new `bam-tauri` crate: a thin Tauri v2 host with no UI of its own —
`tauri.conf.json`'s `frontendDist` points straight at `../../frontend/dist`,
the same build `npm run build` produces, so there is exactly one `frontend/`
package rather than a bundled fork. `#[tauri::command]` handlers mirror
`TauriClient.ts`'s `invoke` calls one-for-one (P9.1), each adapting a
request straight onto `bam_core::api`. Unlike `bam-server`, a desktop app
has exactly one user, so there's no cookie-keyed session map: one
`SessionHandle` is spawned at startup and shared by every command —
`SessionHandle::spawn` (P9.2's per-session actor thread, keeping
`bam-core`'s non-`Sync` `Session` off axum's/Tauri's async runtime) is now
`pub` in `bam-server::state` so this crate reuses it rather than
reimplementing the same actor-thread machinery. `start_ingest` spawns a
relay task forwarding the session's progress broadcast to a
`progress:{operation}` Tauri event until `Finished`, the exact event name
`TauriClient::progress` already listens for.

All three of P9.3's tests hold: the app builds and launches on macOS,
confirmed by a manual run (opens and stays open — no panic, no error
output) plus `cargo build -p bam-tauri`; the shared transport contract
suite from P9.1 (`contract.test.ts`) already runs `TauriClient` against a
real command surface (Round 44 wrote it against both transports up front);
and `tauri.conf.json`'s build config is the no-fork evidence for the third.
Linux launch itself is unverified locally (no Linux machine in this
session) but now builds in CI — added `libwebkit2gtk-4.1-dev`,
`libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev` to the
`ubuntu-latest` CI step so `cargo test --workspace`/`clippy --workspace`
(now covering `bam-tauri` as a workspace member) actually links there
instead of failing on missing system libs. 235 Rust tests (unchanged — no
new Rust test code; the acceptance surface here is build/launch and the
already-existing frontend contract suite), CI gained the Linux Tauri
system deps, no other workflow changes.

**Next:** P9.4, package list and detail views — the first real Vue
components (virtualized list, detail pane, query input with P3.5-style
inline error display) landing in `frontend/components/`, consumed by both
`bam-server` and `bam-tauri` at once.

---

## Round 47 — 2026-08-08 · Phase 9: package list and detail views (P9.4)

Three new `frontend/components/`: `PackageList.vue` (virtualized — renders
only the rows within a configurable `viewportHeight` of the scroll offset
plus overscan, so an 84,000-row result still only ever mounts a few dozen
`<li>`s), `PackageDetail.vue` (fetches and renders the selected package via
`getPackage`), and `QueryInput.vue` (150ms-debounced `parseQuery`, matching
`bam-tui`'s `DEBOUNCE` constant exactly, P3.5's reference).

Getting the error-span test to mean anything surfaced a real gap: `BamClient`
had no way to report *where* a bad query failed, and both backends were
flattening `ParseError` down to a bare string before it ever left Rust.
Fixed at the source rather than worked around in the component: `bam-server`'s
`ApiError` and `bam-tauri`'s new `CmdError` both now carry the `Option<(usize,
usize)>` span from `SessionError::Parse` alongside the message, and
`BamClient` gained a `BamApiError` (message + optional span) that `HttpClient`
and `TauriClient` throw uniformly — one error shape regardless of transport,
same seam P9.1 established. `BamClient` also grew `toggle()` (I7), plumbed
through a `bam-tauri` `toggle` command that didn't exist before this round
(the HTTP side already had the route from P9.2; parity was the actual gap).

All four of P9.4's tests hold, plus one added for the span fix: component
tests for list/detail/query-input against a shared `mockClient` test helper;
the 84,000-row virtualization bound; query errors rendering the offending
byte range in a `<mark>` with the previous predicate left in place on
failure; mark-toggle round-tripping through the injected client; and a new
Rust test (`parse_error_span_survives_over_http`) proving the span isn't
lost in `bam-server`'s JSON flatten. 236 Rust tests (1 added), 22 frontend
tests (11 added), CI unchanged — no new dependency, no new workflow step.

**Next:** P9.7, the AmigaGuide parser — `nom` into a custom AST over
`text/hyper/` fixtures from Aminet (line commands, inline attributes, link
resolution, graceful degradation on malformed markup, P1.5 encoding
handling). Picked over P9.5/P9.6 because it's a standalone parser with no
Vue/UI dependency — order among the three remaining Phase 9 items (P9.5
timeline viz, P9.6 archive content viz, P9.7 AmigaGuide) is otherwise free;
P9.7 just doesn't block on or share code with the other two, so it can run
independently. Phase 9 exit needs all three closed.

---

## Round 48 — 2026-08-08 · Phase 9: AmigaGuide parser (P9.7)

A `nom` parser into a custom AST (`bam_core::ingest::amigaguide`):
`GuideDocument { database, nodes }`, each `GuideNode` carrying
`name`/`title`/`next`/`prev`/`toc` plus a `body: Vec<Inline>` of
`Text`/`Styled(Style, children)`/`Link` — nesting is real tree structure, not
a flat run list, built from a tokenized chunk stream via an explicit
open-style stack so `@{b}...@{i}...@{ui}...@{ub}` closes into
bold-containing-italic rather than two flat spans. `nom` earns its place
here the way P2.4 said it wouldn't for `bam-dsl`: line commands and inline
attribute codes are many small, near-identical productions, which is what
combinators are for. Nothing in the module returns `Result` — malformed
input (an unmatched `@{`, an attribute code that isn't recognised, a style
never closed before `@endnode`) degrades to literal text or gets flushed at
end-of-node rather than failing the parse, matching the plan's "thirty-year-
old files, some are simply broken" framing directly in the type signatures.

The real fixture is Commodore's own `Amigaguide.guide` — fetched from
Aminet's `text/hyper/amigaguidedocs.lha` (`unar`-extracted) rather than
synthesized, since it already exercises 8 `@node`/`@endnode` pairs,
navigation fields, both style and link attributes, and one escaped `\@{...}`
inside a `@MACRO` example. Parsing it against that fixture surfaced a real
bug before any test needed to name it: a body line starting with `@{` at
column 0 (the fixture's own `@{b}$VER: ...@{ub}`) was being swallowed whole
as an unrecognised line command, because `@{...}` and `@command` share the
same leading `@`. Fixed at the one place that distinguishes them —
`command_line` now returns `None` for anything starting `@{`, so it falls
through to body text — rather than special-casing it in each call site.
`nom` was not yet a workspace dependency; added at `"7"` (pure Rust, so I1's
`wasm32-unknown-unknown --no-default-features` build for `bam-core` still
holds, checked directly rather than assumed).

All five of P9.7's tests hold: the real fixture parses to the expected
8-node AST with the front page's 7 links and 1 italic span; every one of
those links resolves to a real node name via `GuideDocument::find_node`;
a small literal case proves bold-containing-italic-containing-link nests as
a real tree; an unrecognised attribute code and an unclosed style both
degrade to literal text without panicking; and a Latin-1-encoded byte in a
node body decodes through P1.5's `decode` (not assumed UTF-8) via the same
`WINDOWS_1252` path `charset.rs` already uses for ISO-8859-1. 241 Rust tests
(5 added), `cargo fmt`/`clippy --workspace --all-targets` clean, no other
workflow changes.

**Next:** P9.5 or P9.6 — the two remaining Phase 9 items, both Vue chart
components over data that already exists (`package.date_precision` for the
timeline, P5.8's inventory payload for per-archive content). Order between
them is free, same as noted last round. Phase 9 exit needs both closed.

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
