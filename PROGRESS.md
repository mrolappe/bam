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

## Round 40 — 2026-08-08 · Phase 6: `Launcher` trait, registry, probing (P6.1)

`crates/bam-core/src/launch/mod.rs` adds the third I4 registry, mirroring
`query::lang::LanguageRegistry` and `unpack::UnpackerRegistry`: `Launcher`
(`id`/`probe`/`capabilities`/`launch`), `LauncherCaps` (`directory_volume`,
`uaem_sidecars`, `hardfile`, `adf`), and `LauncherRegistry::select` — config
override first, else the first registered (preference-ordered) launcher
that is both available and capability-sufficient. `Availability` is reused
from `unpack` rather than redefined. Selection failure names the specific
unmet capability (`LauncherError::CapabilityUnmet`) rather than a generic
"no launcher found"; an unavailable override errors as `Unavailable(id)`
instead of silently falling back. 6 tests in `tests/launch.rs` cover the
phase doc's five plus config-override-wins. 214 tests total (6 added).
`cargo fmt`, `clippy`, and the wasm32 build all clean.

## Round 41 — 2026-08-08 · Phase 6: FS-UAE launcher (P6.2)

`crates/bam-core/src/launch/fs_uae.rs` adds `FsUaeLauncher<S: BlobStore>`:
extracts an archive to a scratch directory via the P5.3 `UnpackerRegistry`,
writes `.uaem` sidecars (P5.7's `write_sidecar`) for entries whose LHA
header carried Amiga protection/comment data, renders an FS-UAE config
(`hard_drive_0 = <scratch>/volume`, FS-UAE's own documented directory-as-
hard-drive support), and spawns `fs-uae` against it. `probe`/`launch` take
their binary candidates via `with_candidates` (default: per-platform
hardcoded paths) rather than baking in real system paths, so both are
testable without a real FS-UAE install.

Getting sidecars right needed one gap closed first: nothing previously
walked a *multi-entry* LHA archive — only `parse_lha_header` for one header
at a time existed. `unpack::lha_header` gained a `compressed_size` field
(the header's own documented base-layout field, offset 7..11, common to all
three levels) and `list_headers`, which repeatedly parses-and-skips to
collect every entry's header; `launch()` reads the archive's raw bytes back
out of the `BlobStore`, walks them, and matches entries to extracted files
by filename.

`LaunchRequest` gained `archive: Option<LaunchArchive>` (blob hash +
format — P6.1 only exercised capability-based selection, never what to
launch) and `LaunchHandle` gained `scratch_dir: Option<PathBuf>` with a
`Drop` impl that removes it, so a launched archive's extracted copy doesn't
outlive the handle. Both changes are additive; all six P6.1 tests still
pass unchanged bar two struct-literal updates for the new fields.

6 new automated tests (4 in `tests/launch_fs_uae.rs` per the phase doc's
list, plus 2 covering `list_headers` in `tests/unpack_lha_header.rs`) —
220 passing total, up from 214, plus the one `#[ignore]`d manual test
below. `cargo fmt`, `cargo clippy --workspace --all-targets -- -D
warnings`, and the wasm32 `--no-default-features` build are all clean.

**Manual test — run, 2026-08-08. Script ran; the attribute round-trip did
not get validated — a real gap found, not closed.** FS-UAE turned out to
already be installed locally (`/Applications/FS-UAE.app/...` — `probe()`'s
default candidates found it; `which fs-uae` alone missed it since it ships
as a `.app` bundle, not on `PATH`). The fixture at
`tests/fixtures/archives/startup_sequence.lha` (`s/startup-sequence`,
`echo "bam!"`) is genuinely Amiga-built — packed with `lha`/`lharc` running
inside FS-UAE itself, not a host tool. `manual_launch_runs_the_startup_sequence_script`
launched FS-UAE and the script **did run**, confirmed visually.

But tracing what `list_headers` actually read from this real archive's
bytes shows why that pass doesn't mean what it looks like it means:
`LhaFileHeader { protection: None, comment: None, .. }`. The header's
level-1 extended-header chain holds a directory-name block (type `0x02`,
"s") and a 2-byte block of type `0x00` — almost certainly the
generic LHA header-CRC extension every `lha` build emits, not Amiga
protection data. Neither matches `AMIGA_EXT_TYPE = 0x47`, the placeholder
guess `unpack::lha_header`'s module doc has flagged since Round 32 as
"untested against real Amiga data." With `protection`/`comment` both
`None`, `write_sidecars` wrote **no `.uaem` sidecar** — the script running
anyway is best explained by FS-UAE's synthesized directory volume
defaulting an attribute-less extracted file to executable-permitted (the
same default AmigaDOS itself uses for a file with no recorded protection),
not by the sidecar mechanism working. **First real evidence the
`AMIGA_EXT_TYPE`/`AMIGA_OS_ID` placeholder is wrong** (or at least doesn't
match what this Amiga-native `lha` produces) — real data the module
previously had none of, but the actual protection-bit extension format is
still unknown. Left as a known, flagged gap rather than guessed at further;
`tests/fixtures/archives/startup_sequence.lha` is committed as a real data
point for whoever picks this up (try an archive with `Protect FILE -e` run
first and diff the header bytes against this one).

## Round 42 — 2026-08-08 · Phase 6: launcher configuration (P6.3)

`crates/bam-core/src/launch/mod.rs` adds `LaunchConfig`/`LauncherOverride`
(`Deserialize`, mirroring `bam_tui::input::KeymapConfig`'s pattern — `bam-core`
gains no `toml` dependency, only `serde`; the caller in `bam-tui` does the
actual `toml::from_str`), `resolve_candidates` (an explicit configured path
replaces the platform-default list outright, else defaults are probed in
order), and `LauncherRegistry::apply_preference` (reorders registered
launchers by a preference list, unlisted ones keeping their relative
registration order after the preferred ones; errors naming the id on any
unregistered entry). `FsUaeLauncher` gained `with_candidates_and_args` and an
`extra_args` field threaded into the spawned `Command`. `with_candidates`
still exists unchanged (delegates to the new constructor with empty args),
so all six P6.1/P6.2 tests still pass untouched.

5 new tests in `tests/launch_config.rs` (the phase doc's four, plus one
covering `apply_preference` reordering `select`'s outcome directly) — 225
passing total, up from 220. `cargo fmt`, `cargo clippy --workspace
--all-targets --features native -- -D warnings`, and the wasm32
`--no-default-features` build are all clean.

## Round 43 — 2026-08-08 · Phase 6: launch a selection (P6.4, Phase 6 exit)

`crates/bam-core/src/store/launch_selection.rs` adds `launch_selection`:
given `package_ids` (resolving a selection to ids stays the caller's job,
same division of labor as P7.5's `summaries::run_batch`), it resolves each
member's cached archive (`tables::get_archive_hash` plus a 16-byte
`BlobStore::get` read into `unpack::detect_format` — no need to read a whole
archive twice when the chosen `Launcher` re-reads it fully anyway), asks the
P6.1 `LauncherRegistry` to launch it, sequentially, and continues past a
per-member failure rather than aborting the batch. `package_ids.len() >
threshold` without `confirmed: true` errors
`LaunchSelectionError::ConfirmationRequired` before anything launches — the
same structural gate `summaries::run_batch` uses for its cost estimate,
here over a plain count instead of a token estimate. `cancel:
&CancellationToken` is checked before each member, so a mid-batch cancel
stops cleanly and `LaunchSelectionOutcome` still reports what ran. Lives at
the `store::` level, not wired into `bam_core::api`, since nothing else at
that layer needs it yet — `summaries`/`embeddings` are free functions for
the same reason (I1's rusqlite confinement is what forces `store::`, not
the API layer's session-scoped contract).

4 new tests in `tests/launch_selection.rs` (the phase doc's four) — 229
passing total, up from 225. `cargo fmt`, `cargo clippy --workspace
--all-targets --features native -- -D warnings`, and the wasm32
`--no-default-features` build are all clean.

**Phase 6 exit reached** on the core side: a selection's cached archives
launch through a pluggable, capability-driven registry with
continue-on-failure and a confirmation gate. Wiring an actual TUI keybinding
(`S`) to this function is unscheduled follow-on work, not blocking — the
phase doc's four P6.4 tests target the core batch behavior only.

## Next task

Phases 8 and 9 remain open and unscheduled behind Phase 6 (additive,
resequenceable per `IMPLEMENTATION_PLAN.md`'s phase table) — Phase 8 is the
extism WASM plugin host (5 tasks), Phase 9 is the Vue/`bam-server`/Tauri
frontends (7 tasks).

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
