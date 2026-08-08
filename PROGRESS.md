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

## Rounds 44–50 — 2026-08-08 · Phase 9: Vue frontend → `bam-server` HTTP/SSE → Tauri shell → package list/detail views → AmigaGuide parser → timeline → per-archive content viz

Full round-by-round log moved to
[docs/progress/phase-9-frontends.md](docs/progress/phase-9-frontends.md) to
keep this file scannable — it covers P9.1 through P9.7: the Vue frontend and
`BamClient` transport interface with generated request/response types
(P9.1), the `bam-server` HTTP/SSE adapter (P9.2), the Tauri desktop shell
(P9.3), package list/detail views and query input (P9.4), the `nom`-based
AmigaGuide parser (P9.7), upload timeline visualization (P9.5), and
per-archive content visualization (P9.6, Phase 9 exit). 242 tests by the end
of the phase (33 added across the seven rounds; frontend tests grew from 0
to 29 over the same span).

**Phase 9 exit reached.** A Vue frontend behind one `BamClient` seam, served
by both `bam-server` and `bam-tauri` with no fork between them. Phase 8 (the
extism WASM plugin host) is the only unclosed phase left in the plan.

---

## Round 51 — 2026-08-08 · Phase 8: contract versioning and manifest schema (P8.1)

`bam_core::plugin` module: `PluginManifest` (TOML, per §9's `name`/`version`/
`api_version`/`extension_point`/`claims` shape) with `HOST_API_VERSION = 1`
rejected-on-mismatch checking, single-wildcard `claims` glob matching, and
`contract_schema()` generating a `schemars`-derived JSON Schema per
extension point (`content_analyzer` wired now; others return `None` until
their input types exist). Host-independent — no `#[cfg(feature = "native")]`
gate — so it compiles under `--no-default-features` on `wasm32-unknown-unknown`
same as the rest of I1. 6 tests added (248 total): known/rejected
`api_version`, a malformed manifest naming its missing field, `claims`
filtering, and the schema/type round-trip.

**Next:** P8.2, the extism host loading a WASM module through the existing
registries (`UnpackerRegistry` et al.) with no call-site change — the task
that proves or disproves invariant I4 for a plugin backend.

---

## Round 52 — 2026-08-08 · Phase 8: extism host and registry integration (P8.2)

`WasmUnpacker<S: BlobStore>` (`bam_core::plugin::wasm`, native-only):
loads `manifest.toml` + `plugin.wasm` from a directory and implements the
plain `Unpacker` trait (P5.3) by calling an `extism::Plugin`'s `probe` and
`unpack` exports with JSON — `UnpackRequest`/`UnpackResponse`/
`UnpackProbeResponse` added to `bam_core::plugin` as P8.1's `contract_schema`
gains an `"unpacker"` case alongside `"content_analyzer"`. The plugin
proposes file paths and bytes; the host writes them, rejecting `..`/absolute
paths itself, same trust posture as P5.4/P5.5 — a plugin is less trusted
than in-tree code, not more. `claims` reuses P8.1's glob matcher against a
format-name pattern (`*.zip` etc.) rather than adding a second matching
mechanism.

I4 confirmed: `registry.register(Box::new(WasmUnpacker::load(dir, store)?))`
against the unchanged `UnpackerRegistry` — zero diff in `unpack/`, `launch/`,
or any call site (checked directly, not just tested). 5 tests added (253
total): register-and-select through normal selection logic, a host↔plugin
JSON+bytes round-trip, native-vs-WASM resolving purely by registration order
in both directions, idempotent double-load, plus an `unpacker` contract
schema round-trip test mirroring P8.1's `content_analyzer` one.

Test fixture: `tests/fixtures/plugins/echo-unpacker/` — a real
`extism-pdk` WASM plugin (source kept alongside for provenance under
`src-provenance/`, not part of the Cargo workspace) that reports available
and echoes its input bytes back as one file, enough to prove the mechanism
without needing real archive parsing inside WASM — that's P8.4.

**Next:** P8.3, the `content_analyzer` extension point — wiring `enrichment`
rows to a plugin's classification output, with per-plugin producer
versioning so a plugin upgrade reprocesses only its own rows.

---

## Round 53 — 2026-08-08 · Phase 8: `content_analyzer` extension point (P8.3)

`WasmContentAnalyzer` (`bam_core::plugin::wasm`, native-only): loads a
`content_analyzer` manifest/`plugin.wasm` pair the same way P8.2's
`WasmUnpacker` does, and calls its `analyze` export per file. The output is
read as a raw string and `serde_json`-parsed on the host rather than through
extism's typed `Json<T>` convert on the guest side, so a plugin returning
malformed JSON surfaces as `AnalyzeError::MalformedOutput` instead of a
panic — same trust posture as P8.2's unpacker (I4: a plugin is less trusted
than in-tree code).

`bam_core::store::content_analysis::analyze_files` is the DB half: one
`enrichment` row per `(package, plugin, file)` — `kind =
"content_analyzer:{plugin_id}:{path}"` — so bumping a plugin's version
reprocesses only that plugin's rows for the files it claims, never another
plugin's rows or `llm_summary` (checked directly by a test that seeds a
summary row, reprocesses the analyzer twice with different plugin versions,
and asserts the summary payload is untouched). `claims` prefiltering
(P8.1) happens in this function, before any WASM call, so an unclaimed file
is never handed to the plugin. `producer_version`'s column is `INTEGER`
but a plugin's version is a free-form string; `analyze_files` hashes it with
`DefaultHasher::new()` (fixed keys, so deterministic across runs) rather
than adding a second version-comparison column to the shared `enrichment`
table.

`store::fts::rebuild_fts` gained a third `package_fts` column,
`content_analysis`, populated by concatenating `searchable_text` out of
every package's `content_analyzer:*` enrichment payloads — P4.6's
whole-row `MATCH` needed no compiler change to pick it up.

Test fixture: `tests/fixtures/plugins/echo-analyzer/` — a real
`extism-pdk` WASM plugin (source under `src-provenance/`, same convention
as P8.2's echo-unpacker) that classifies any `.mod` file as `kind: "echo"`
with `searchable_text` from its decoded bytes, and deliberately returns
malformed JSON for a path ending `broken.mod` to exercise the host's error
path. 5 tests added (258 total): FTS5 discovery of a classified file's
`searchable_text`, plugin name/version stored as producer, version-bump
reprocessing that leaves `llm_summary` untouched, malformed output
reported and skipped without writing a row, and `claims` prefiltering
proven by the unclaimed file never producing an enrichment row.

Both extension points named in §9 (`unpacker`, P8.2; `content_analyzer`,
P8.3) now have a working WASM backend through the plugin host.

**Next:** P8.4, a WASM-backed unpacker doing real archive extraction (vs.
P8.2's echo fixture) — proving I4 against a second extension point end to
end, including format routing and path-traversal rejection inside the
sandbox.

---

## Round 54 — 2026-08-08 · Phase 8: WASM-backed unpacker with real archive extraction (P8.4)

`tests/fixtures/plugins/zip-unpacker/`: a real `extism-pdk` WASM plugin
(source under `src-provenance/`, same convention as P8.2/P8.3's fixtures)
that reads genuine ZIP archives via the `zip` crate inside WASM — the first
plugin fixture that does real format parsing rather than echoing bytes back.
Registers into `UnpackerRegistry` and is selected through `detect_format`'s
ordinary magic-byte routing (P5.3), unchanged by P8.2. A second fixture,
`unavailable-unpacker/`, always reports itself unavailable, isolating the
probe-honoured test from the extraction test — P8.2 only ever exercised the
available path.

Extracting this fixture surfaced a real gap: `WasmUnpacker::unpack`
(`bam_core::plugin::wasm`) wrote each returned file to `dest` as it decoded
it, so a malicious entry ordered after a safe one left partial output behind
— unlike `unar`/`zip`'s scratch-then-move pattern (P5.4/P5.5), which never
had this exposure. Fixed by validating every entry's path and base64 first,
then writing only once the whole batch decodes clean — same no-partial-
extraction guarantee as the native backends, extended to plugins even though
the sandbox itself is per-call rather than per-file. 4 tests added (262
total): real extraction against a two-file fixture, magic-byte routing
through the unchanged registry, a `probe`-honoured negative case via
`unavailable-unpacker`, and traversal rejection with the fixed atomicity
verified directly (`dest` empty after the error).

**Next:** P8.5, plugin loading configuration and failure isolation — the
phase's last task: panics, time limits, and memory limits caught per-plugin
without taking down the host, plugin discovery/enable config, and a plugin
that fails to load reported at startup without blocking it.

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
