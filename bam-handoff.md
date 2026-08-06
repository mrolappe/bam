# bam — Browse AMinet — Handoff Document

Status: pre-implementation design summary
Purpose: input for detailed implementation planning of the first milestone
Audience: whoever picks up implementation (may be the same author, later)

## 1. Project goal

A tool to browse, filter, search, enrich, and act on the Aminet software
archive (Amiga software repository, ~84,000 packages). Core capabilities:

- Strong filtering while browsing (hide file types, hide subdirectories, etc.)
- Strong query capabilities for search, including LLM-assisted query construction
- Launching an Amiga emulator with an archive's content mounted as a virtual volume
- LLM-generated overviews/summaries of archive contents
- Visualizations: timeline views over ingested archives, and content visualizations
  for individual archives
- User-defined "semantic highlighting" of entries/search hits (gutter icons,
  background color, badges) based on arbitrary criteria

Initial delivery targets: a TUI and a desktop GUI, sharing one core. An MCP
server exposing the same functionality is a planned but later addition and
must be accounted for in the architecture from the start (see §8).

## 2. Naming convention

All project-level names use `bam` (short for "Browse AMinet") instead of
`aminet`, e.g.:

- Binary / CLI: `bam`
- Core crate: `bam-core`
- Frontend crates/packages: `bam-tui`, `bam-gui`, `bam-mcp` (later)
- Config file: `bam.toml`
- Cache/data directories: e.g. `~/.cache/bam/`, `~/.local/share/bam/`
- Plugin manifests, extension points, etc. should be named/prefixed consistently
  with `bam` rather than `aminet` wherever they refer to the tool itself
  (data that genuinely refers to Aminet-the-archive, e.g. mirror URLs or the
  `INDEX` file, keeps its real name).

## 3. Data source (Aminet)

The Aminet `INDEX` is **not** the only source. Confirmed available on mirrors
(e.g. `ftp.fau.de/aminet/`, `de.aminet.net`):

- `INDEX` / `INDEX.gz` — full listing, regenerated daily. Historical columns:
  File, Dir, Size, Age, Description (description is truncated/terse).
- `RECENT` / `RECENT.gz` — small delta of new uploads, suited for incremental updates.
- `TREE` — full directory/category structure.
- Per-package `.readme` file next to each `.lha`/archive — contains the actual
  useful text (author, version, requirements, changelog). This is the primary
  source for full-text/semantic search and for LLM summarization input, since
  the INDEX description alone is too short.
- Plain HTTP directory listings on mirrors — give exact byte sizes and mtimes,
  crawlable without FTP.
- `aminet.net` package pages (`/package/<dir>/<name>`) exist but there is no
  documented/official JSON API.

No official bulk rsync access is documented. A full mirror (metadata + all
archives) is possible in principle but is many GB; whether rsync access is
available must be **requested from a specific mirror operator** (e.g.
ftp.fau.de) — do not assume.

Decision from this conversation: **local storage = metadata + selectively
cached archives** (not a full mirror). See content-addressed cache, §6.

## 4. Tech stack (decided)

**Rust** as the core language, for one shared core library powering both a
TUI and a desktop GUI without duplicated logic or an IPC layer.

- Core library crate (`bam-core`): pure logic, serializable types, no
  direct I/O to a terminal or GUI (see §8).
- **TUI**: `ratatui` for rendering, `crossterm` as the terminal backend.
  `crossterm` matters specifically for **Windows support** — it talks to the
  Win32 console API, unlike Unix-only alternatives (e.g. `termion`). If
  Windows is a target platform, `crossterm` is effectively required.
- **GUI**: `Tauri`, with a TypeScript frontend. Note: Tauri always has a Rust
  backend — "TypeScript + Tauri" means "Rust core, TypeScript UI", not
  "no Rust". This is why a pure-TypeScript stack (which would imply Node,
  hence Electron) was not recommended: Electron/Node is a poor fit for a
  tool that's primarily invoked as a terminal program (startup time, no
  single binary, and the TUI story via `Ink` still means dragging along a
  Node runtime). Rust-core + ratatui-TUI + Tauri/TypeScript-GUI gives
  TypeScript exactly where it's strong (visualizations, e.g. D3/ECharts)
  without giving up a native, fast core.
- **Store**: `rusqlite` with **FTS5** (full-text search) and **sqlite-vec**
  (vector/embedding search) as statically linked extensions — single binary,
  no runtime dependency.
- Supporting crates: `reqwest` (HTTP ingest), `serde` (+ JSON Schema
  generation), `tokio` (async), `nom` (parsers for INDEX and AmigaGuide),
  `encoding_rs` + `chardetng` (charset handling), `blake3` (content hashing
  for the cache).
- **Archive extraction is out-of-process**, behind an `Unpacker` trait —
  there is no production-quality native Rust LHA/LZX implementation:
  - LHA: `lhasa` (Simon Howard, ISC license) — good, embeddable as a library.
  - LZX: `unar` / XADMaster (LGPL) — used as an external process.
  - **Why not just use the classic `unlzx.c`**: it has no clear/explicit
    license text despite decades of informal redistribution. That's fine to
    compile privately, but becomes a real problem the moment you *distribute*
    a binary built with it. Using `unar`/XADMaster avoids this; the licensing
    question then lives at the "user installs their own unpacker" boundary
    instead of in the distributed binary.
- **Plugins**: WASM via `extism` (see §9) — language-agnostic, sandboxed,
  avoids native ABI breakage across compiler versions.

Open question (unresolved): target OS platforms (Linux / macOS / Windows /
all three). Tauri and the external unpackers behave most awkwardly on
Windows, so this affects packaging decisions.

## 5. Architecture overview

Layered, ingest → store → enrichment → frontends, each stage idempotent and
independently re-runnable (enrichment is the expensive part and must be
resumable without re-fetching from the network).

```
Mirror ingest (INDEX/RECENT/TREE)     Readme harvesting (async/batch, throttled)
                 \                                 /
                  v                               v
                     bam-core (Rust library)
              SQLite: landing store + normalized store
              FTS5 full text + sqlite-vec embeddings
              Filter/Query DSL, highlighting engine
                            |
              ---------------------------
              |                          |
        Enrichment: extraction    Enrichment: LLM
        file inventory, type      queries, summaries
        detection                 (local or cloud)
                            |
              ---------------------------
              |            |             |
        bam-tui       bam-gui       bam-mcp (later)
       (ratatui)   (Tauri + TS)     (MCP server)
```

### 5.1 Landing store vs. normalized store

Two explicit layers in SQLite, kept separate on purpose:

- **Landing**: exactly what was fetched, unmodified — the raw `INDEX` line,
  the raw readme text plus its detected encoding, and the timestamp of
  retrieval. This layer is the source of truth about what the origin
  actually said.
- **Normalized**: parsed out of landing into typed, queryable form —
  category/subdirectory as separate typed columns, size as bytes (not
  `"134K"`), Aminet's relative "Age" converted into a real date, canonical
  package name without version suffix, etc.

The reason for the split: if the parser has a bug, you re-derive the
normalized layer from landing without any network traffic. Landing is cheap
and append-only; normalized can be safely dropped and rebuilt.

### 5.2 Enrichment stages

Everything *derived*, in increasing cost order, each versioned so a single
stage can be selectively invalidated/reprocessed:

| Stage | Produces |
|---|---|
| Readme header parsing | Author, Version, Requires, Type, Distribution (Aminet readmes have a semi-standardized header block) |
| Archive inventory | File list with paths, sizes, detected types |
| File-type analyzers | e.g. MOD/tracker sample names, IFF/ILBM resolution, executable hunk headers |
| Embeddings | Vectors for semantic search |
| LLM summary | Human-readable overview/summary of an archive's contents |

Schema sketch:

```sql
enrichment(package_id, kind, producer_version, produced_at, payload)
```

Versioning per producer means "the MOD analyzer got better" triggers
reprocessing of only MOD-related rows — not the (expensive) LLM summaries.

## 6. Content-addressed cache

Cached archive bytes are addressed by content hash (BLAKE3), not by path:

```
cache/blobs/a3/f9/a3f92c...   (the actual bytes)
```

A DB table maps `packages(dir, file) → archive_hash → blobs(hash, size,
last_used, pinned)`.

Benefits:
- Deduplication (identical archives appear under multiple names on Aminet).
- Integrity checking comes for free (recompute hash, compare).
- Atomic writes: write to a temp file, rename to the hash on success — never
  a partially-written archive under a "real" cache key.
- LRU eviction operates on blobs, independent of how many package records
  reference them.

Caveat: Aminet does not publish checksums, so verification is *after* the
fact (detect corruption post-download), not pre-download. Size + mtime from
the directory listing are the only pre-download change signals available.

Cache is bounded by an LRU budget (e.g. "max 20 GB of archives") plus a pin
flag for archives the user wants to keep. Enrichment results (inventory,
embeddings, summaries) survive independently of whether the archive blob
itself is still cached — otherwise re-evicting and re-fetching would mean
paying for LLM processing twice.

## 7. Readme harvesting (async / batch, mirror-polite)

Requirement: for every entry that survives the active filters (i.e. was not
explicitly excluded), the readme should be downloaded and cached. This
happens **asynchronously in the background and/or as a batch job**, not
inline with browsing.

Modeled as a persistent queue in SQLite:

```sql
fetch_queue(url, kind, priority, attempts, next_attempt_at, etag, last_status)
```

A background worker pulls from this queue with:

- A token-bucket rate limit (e.g. ~2 requests/second) over a single
  keep-alive connection.
- Conditional GET (ETag / If-Modified-Since) to avoid re-downloading unchanged files.
- Exponential backoff on 429/5xx.
- `robots.txt` respected.
- A descriptive `User-Agent` identifying the tool and providing contact info.
- Priority boost for whatever the user is currently looking at, so browsing
  still feels responsive while the bulk job runs underneath.

Rough sizing: ~84,000 readmes at ~1–3 KB each, at ~2 req/s ≈ ~12 hours as a
background batch — acceptable. Note: if a *complete* readme set is wanted
up front, a single `rsync --include='*.readme'` pass (if a mirror grants
access) is far more mirror-friendly than 84,000 individual HTTP requests —
worth a short request to a mirror operator (e.g. ftp.fau.de).

## 8. MCP-readiness (architectural constraint, not a feature yet)

An MCP server is a planned later addition. To make that a cheap add rather
than a rewrite, the core is built as a **use-case API layer with three thin
adapters**:

```
bam-core::api   — pure functions, serializable request/response types, no direct I/O
   ├─ bam-tui     (adapter)
   ├─ bam-gui     (adapter, Tauri)
   └─ bam-mcp     (adapter, later)
```

Rules that keep this true over time:

1. No `println!`/`eprintln!`/ANSI color codes inside the core. Progress is
   reported via a `ProgressSink` trait, implemented differently per adapter.
2. Every long-running operation accepts a `CancellationToken`. Both MCP
   clients and TUI/GUI need to be able to abort.
3. Request/response types are `Serialize`/`Deserialize` and JSON-Schema-able,
   so an MCP tool definition can be derived from the type rather than
   hand-written.

Likely first MCP tools once built: `search_packages`, `get_package`,
`get_readme`, `list_categories`, `summarize_package`,
`list_archive_contents`.

## 9. Plugin system

WASM via `extism` — language-agnostic, sandboxed, avoids native ABI churn.
Three distinct concepts:

- **Extension point**: the place in the pipeline where the host calls out,
  e.g. "after extraction, once per file in the archive." The host owns
  *when* it's called and what happens to the result.
- **Contract**: the versioned data schema for that call — input, output,
  error cases, tagged with an `api_version`. The host rejects plugins whose
  major contract version it doesn't recognize rather than letting them run
  half-broken. Example (content analyzer):

  ```json
  // input
  { "path": "mods/foo.mod", "size": 108234, "bytes_b64": "...", "hint": "audio" }
  // output
  { "kind": "protracker_module", "confidence": 0.97,
    "attributes": { "channels": 4, "patterns": 32, "samples": ["bass", "hihat"] },
    "searchable_text": "bass hihat ..." }
  ```

- **Entrypoint**: the concrete exported function the plugin provides and the
  host invokes (e.g. an exported `analyze` function taking/returning JSON via
  Extism). Alongside it, a manifest:

  ```toml
  name = "protracker-analyzer"
  version = "0.2.0"
  api_version = 1
  extension_point = "content_analyzer"
  claims = ["*.mod", "mod.*", "*.med"]
  ```

  `claims` lets the host pre-filter which files are even offered to a given
  plugin, instead of invoking every plugin for every file.

Planned initial extension points: `content_analyzer`, `ingest_source`,
`visualization`, `launcher` (emulator launchers), `highlight_provider`
(see §11).

## 10. LLM integration

A provider trait, so cloud and local models are interchangeable:

```rust
trait LlmProvider {
    async fn complete(&self, req: CompletionRequest) -> Result<String>;
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    fn capabilities(&self) -> Capabilities;  // grammar support? JSON schema? context size?
}
```

**Local model support is a hard requirement, not an optional extra**, and
must be part of the initial design. Ollama and llama.cpp both expose
OpenAI-compatible endpoints, so one provider implementation covers most
cases. The meaningful difference is in `capabilities`: llama.cpp can enforce
GBNF grammars, cloud APIs work off JSON Schema — define the DSL grammar
(see §12) once, formally, and generate both representations from it so the
provider is genuinely swappable.

Embeddings should also be runnable locally (e.g. `bge-m3` or
`nomic-embed-text` via Ollama) — running 84,000 embeddings through a paid
cloud API is an unnecessary cost; locally it's roughly a night of compute.

Model size guidance for DSL query generation (see §12): a 7–8B instruct
model (e.g. Qwen2.5-7B-Instruct, Llama 3.1 8B) is sufficient **provided**
output is grammar-constrained and the prompt includes the category
vocabulary. Without grammar constraints, plan for ~14B instead.

## 11. Filter / Query DSL

A single DSL, not raw SQL, is used for three different purposes:

1. Manual filtering while browsing (hide file types, hide subdirectories, etc.)
2. Search queries, optionally LLM-assisted (the LLM emits DSL, not SQL directly)
3. Semantic highlighting rules (§11.1) — the same predicate language, plus a decoration

Example: `dir:util/* !type:mod size<100k year>2000`

The DSL compiles to SQL server-side; the LLM never emits SQL directly, which
keeps generated queries inspectable, editable by the user before running,
and avoids injection concerns entirely.

Why a 7–8B local model is enough (recap from §10): grammar-constrained
generation removes syntax errors entirely; the remaining difficulty is
*vocabulary mapping* ("music software from the nineties" → `dir:mus/* AND
year<2000`), addressed by including the TREE category list and a small
file-type dictionary plus a handful of few-shot examples in the prompt. The
architectural safety net regardless of model size: always show the
generated DSL and make it editable — a model mistake becomes a visible,
correctable suggestion, never a silent wrong result set.

### 11.1 Semantic highlighting — two realization paths

Two distinct mechanisms are both in scope, sharing one rendering layer:

**A. DSL-based rules** — a highlight rule is a DSL predicate (§11) plus a
decoration, evaluated declaratively:

```toml
[[highlight]]
name = "my own uploads"
when = "author:~'Mustermann'"
gutter = "user"
priority = 10

[[highlight]]
name = "large archive"
when = "size>5M"
badge = "XL"
priority = 5
```

Good for anything expressible as a predicate over stored/normalized fields
(including vector-similarity predicates like `similar:'tracker module
editor' > 0.82` if the DSL is extended to support them).

**B. Plugin-based highlight providers** — a `highlight_provider` extension
point (§9): the host calls the plugin per package (or per result set) with
the normalized record plus available enrichment data, and the plugin returns
decoration suggestions. This is for logic that isn't a clean predicate over
stored fields — arbitrary heuristics, external lookups, custom
classification/scoring logic a DSL predicate can't express.

```json
// output from a highlight_provider plugin, same shape regardless of source
[{ "gutter": "flag", "badge": null, "background": null,
   "priority": 7, "reason": "matches internal heuristic X" }]
```

Both paths feed the **same downstream model**: the core emits *semantic
tokens* (`gutter: "user"`, `background: "accent-subtle"`, etc.), never
colors directly. Each frontend maps tokens to its own presentation
(ratatui `Style` + a gutter character in the TUI; a CSS class in the GUI).
This keeps theming/dark-mode centralized instead of duplicated per frontend.

**Conflict resolution** (applies uniformly to both DSL rules and plugin
output): background color is exclusive — highest `priority` wins. Gutter
icons and badges may stack, but should be capped (e.g. 2–3) to keep dense
lists readable.

**Evaluation cost**: for ordinary list rendering, rules/providers are
applied to the currently visible window only — cheap even with several
rules active. Once a rule or provider involves vector similarity or other
non-trivial computation, results are precomputed and materialized instead:

```sql
highlight_hits(package_id, rule_id, decoration_json)
```

invalidated when the rule set (or a plugin's version) changes. The rule/
plugin configuration file should be hot-reloaded — highlight rules are
written iteratively and restarting the app on every tweak is friction that
should not exist.

## 12. Emulator integration

Goal: launch an Amiga emulator with an archive's extracted content directly
accessible as a virtual volume.

FS-UAE (and WinUAE) support directory-based volumes well; vAmiga's support
for this is notably weaker — **which emulator is the reference target is an
open question** and affects feasibility directly (see §14).

### 12.1 `.info` files — clarification

`.info` files are ordinary files inside the archive and are extracted
normally like any other file; there is nothing special about them as files.

What *is* lost on a naive/host extraction are the **Amiga-specific file
attributes carried in the LHA archive headers**: protection bits (the
classic `HSPARWED` flags) and file comments. Concretely relevant: the
**S-bit (Script)** and **E-bit (Executable)** — a startup-sequence fragment
or AmigaDOS script without its S-bit won't run correctly. Host filesystems
(ext4/NTFS/APFS) have no equivalent attribute to preserve these in, so a
generic unpack step silently drops them.

FS-UAE/WinUAE address this for directory volumes via `.uaem` sidecar files —
next to `Foo` there's a `Foo.uaem` containing a line like:

```
----rwed 2001-03-11 22:15:00.00 Some comment
```

Plan: read the LHA extended headers during extraction and emit `.uaem`
sidecar files from them. This is a small, well-scoped piece of code, and
it's the difference between "content is viewable" and "content actually
boots/runs correctly" in the emulator.

## 13. Character encoding and AmigaGuide

- **Encoding detection**: most readmes are ISO-8859-1/Amiga-native, not
  UTF-8, but not uniformly — some non-English uploads use ISO-8859-2 or
  CP437. Use `encoding_rs` for decoding and `chardetng` (from the Firefox
  codebase, pure Rust) for detection; store the detected encoding per file
  in the landing layer so it's correctable later without re-fetching.
- **AmigaGuide markup**: no maintained Rust library exists for this. The
  format itself is modest — line commands (`@node`, `@endnode`, `@title`)
  and inline attributes (`@{b}`, `@{"Text" link Node}`) — a parser is a few
  hundred lines. Recommendation: write it directly (a `nom` parser into a
  custom AST), rather than port anything. Useful references: the historical
  `ag2html`/`guide2html` converters, and Aminet's own `text/hyper/` category,
  which has relevant material.

## 14. Open questions (need answers before/at implementation start)

- **Target emulator(s)**: FS-UAE vs. WinUAE vs. vAmiga vs. others. Directly
  determines whether directory-volume mounting is viable as designed.
- **LLM provider default**: local (Ollama) vs. cloud API as the out-of-box
  default — affects whether grammar-constraining work is front-loaded.
- **Target OS platforms**: Linux / macOS / Windows / all three — affects
  Tauri packaging and external-unpacker distribution strategy.
- **Mirror rsync access**: unconfirmed; requires directly contacting a
  mirror operator (e.g. ftp.fau.de) to find out whether bulk rsync
  (including `.readme` files or full archives) is available, rather than
  relying purely on the HTTP-based harvesting queue in §7.

## 15. Suggested build order

1. `INDEX` parser + SQLite schema (landing + normalized) + `RECENT`-based
   incremental update — testable without live network dependency.
2. TUI with the filter DSL — primary daily-use surface, and validates the
   data model early.
3. Readme harvesting queue (§7) + FTS5 full-text search.
4. Content-addressed archive cache (§6) + extraction + `.uaem` sidecar
   generation (§12.1).
5. Emulator launcher.
6. LLM layer — DSL query generation first, summaries second.
7. Tauri GUI with visualizations (timeline, per-archive content views).

Steps 1–4 form the hard core; 5–7 are additive and can be resequenced.

## 16. Pitfalls summary

- LZX has no clearly licensed free implementation to embed directly — use
  `unar`/XADMaster as an external process instead of bundling `unlzx.c`.
- Archive text is not reliably UTF-8 or even a single fixed encoding —
  detect and store per file.
- Naive extraction silently drops Amiga protection bits/comments needed for
  scripts to run — must be reconstructed from LHA extended headers.
- LLM cost at ~84,000 packages is nontrivial if done eagerly against a paid
  API — local model support (completion + embeddings) must be a first-class
  path, not an afterthought.
- No published checksums from Aminet — integrity is "detect corruption after
  the fact," not "verify before accepting."
- Be a polite mirror citizen: rate-limit, conditional GET, backoff, proper
  User-Agent, and prefer a single rsync pass over the harvesting queue when
  fetching in bulk, if access can be arranged.
