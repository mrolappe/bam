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

## Round 35 — 2026-08-08 · Phase 7: `LlmProvider` trait + OpenAI-compatible implementation (P7.1)

`crates/bam-core/src/llm/`: `LlmProvider` trait (`complete`, `embed`,
`capabilities`) plus `OpenAiCompatibleProvider`, one implementation for
llama.cpp, Ollama, and cloud endpoints — they differ only in
`OpenAiCompatibleConfig` (`base_url`, `model`, `api_key`,
`grammar: GrammarSupport`). Built on P1.9's `HttpClient`, extended with a
`post` method (default-erroring, so the four existing GET-only test fakes
needed no changes) and a new `HttpPostRequest` type; `ReqwestClient` gained
a real `post`. `LlmError::ConnectionFailed { url }` distinguishes a
transport-level failure (`HttpError::Request`, server not listening) from a
bad HTTP status, and names the configured URL in its message. Embeddings
are reordered by the response's `index` field before returning, so a
provider that replies out of order still yields vectors in input order.
5 tests added in `crates/bam-core/tests/llm.rs` (4 offline + 1 `#[ignore]`d
real-server test), all passing; `cargo clippy --workspace --all-targets`
clean; `cargo build -p bam-core --no-default-features --target
wasm32-unknown-unknown` still compiles (I1 holds — the trait and provider
are plain code, no `native` gate needed since they go through `HttpClient`
generically rather than `reqwest` directly).

## Round 36 — 2026-08-08 · Phase 7: grammar generation per language (P7.2)

`crates/bam-core/src/query/grammar.rs` (new): `BamDsl::grammar(GrammarKind)`
(`bam_dsl.rs`, was a stubbed `None`) now generates both artifacts from one
production-rule description (`rules()`) transliterated from
`docs/lang-bam-dsl.md`'s CFG — one deliberate deviation: `value`'s doc-level
`number size_suffix? | date | string | bareword_value` breakdown collapses
to `rhs := string | bareword`, matching what `Parser::lex_rhs` actually
accepts (type-specific parsing happens later, in `typed_value`, not at the
lexical level) — a grammar meant to constrain generation has to match the
real parser, not the doc's more suggestive gloss. `field` stays a generic
`ident`, exactly as the doc defines it: field *validity* is a
`FieldRegistry` concern the real parser only checks after parsing, and
`grammar()` has no registry parameter to consult anyway (I2's trait
signature, unchanged).

GBNF (llama.cpp) renders `rules()` into GBNF text — constrains raw
`bam-dsl` text token by token. JSON Schema (cloud) instead comes from
`Predicate`'s own `#[derive(JsonSchema)]` (already present on every IR type
since P2.1) via `schemars::schema_for!` — constrains a JSON encoding of the
same predicate tree, since that's what cloud "structured output" actually
constrains, not raw DSL text. Both are one source in the sense that matters
(the DSL's own grammar; the IR type itself), not two hand-copied encodings
of a third, invented representation.

Two interpreters exist for tests only (`gbnf_accepts`, `json_schema_accepts`
in `grammar.rs`): a backtracking matcher over the exact same `rules()` AST
the GBNF renderer walks (so a renderer bug shows up as a
matcher/renderer disagreement, not a silently-wrong string), and a ~60-line
Draft-07-subset JSON Schema validator (`$ref`, `oneOf`/`anyOf`/`allOf`,
`enum`, `type`, `properties`/`required`, `items` — exactly what `schemars`
0.8 emits for these types) rather than trusting `schema_for!` by
construction or adding a validator crate. `crates/bam-core/tests/
query_grammar.rs`: all fifteen `docs/lang-bam-dsl.md` examples validate
against both (P2.4's parser confirms each parses; this file only needs the
source text); a deliberately malformed input (unbalanced `(`; an unknown
`Predicate` variant tag) is rejected by both; the equivalence property
test — the reason this task was marked **O** — renders a hand-enumerated
spread of `Predicate`s (every variant, every `CmpOp`/`Value`/`Pattern`/
`SelectionRef`, several nestings) to `bam-dsl` text and to JSON and checks
both artifacts accept, then reparses the text and asserts it round-trips to
the same `Predicate` (no fuzzing crate in the workspace; the grammar is
small enough that a fixed spread exercises every construct without one); a
`QueryLanguage` with no grammar at all confirms `None` propagates cleanly.

Also closed P7.1's own loose end: `CompletionRequest` gained a
`json_schema: Option<String>` field (`llm/mod.rs`) alongside `grammar`, and
`OpenAiCompatibleProvider::complete` now wires it into
`response_format: {"type": "json_schema", ...}` when
`capabilities().grammar == GrammarSupport::JsonSchema` — the comment left
at that call site in Round 35 pointed straight at this round. One test
added in `llm.rs` confirming it's wired for the cloud config and absent for
llama.cpp's.

194 tests total (5 added: 4 in `query_grammar.rs`, 1 in `llm.rs`; three
pre-existing `CompletionRequest` call sites in `llm.rs` just picked up the
new `json_schema: None` field, not new tests). `cargo clippy --workspace
--all-targets` clean; `cargo build -p
bam-core --no-default-features --target wasm32-unknown-unknown` still
compiles (I1 holds — `grammar.rs` is plain code, no `native` gate, and
`schemars` was already an ungated dependency).

## Round 37 — 2026-08-08 · Phase 7: query generation prompt (P7.3)

`crates/bam-core/src/llm/query_prompt.rs` (new): `build_prompt` assembles
the natural-language-to-`bam-dsl` prompt — task instructions, the TREE
category vocabulary (`dir:` values, passed in by the caller since TREE data
is per-mirror-snapshot and nothing ingests it into the DB yet), a small
hardcoded file-type dictionary (`.lha`, `.lzx`, `.zip`, `.readme`, `.txt`,
`.dms`, `.info` — fixed across Aminet, so hardcoded rather than threaded
through as configuration), and five few-shot examples written against
`registry::package_fields`'s actual field set (`dir`, `year`, `size`,
`description`, `marked`) rather than `bam-handoff.md`'s illustrative
`type`/`author` examples, which have no backing column yet.

`generate_query` picks `CompletionRequest.grammar` or `.json_schema` from
`provider.capabilities().grammar` (P7.1/P7.2's existing wiring), calls
`complete`, then routes the completion back through `BamDsl::parse` (GBNF
path) or `serde_json::from_str::<Predicate>` (JSON-Schema path) into a
`Predicate`, and returns `BamDsl::render`ed text. It touches nothing in
`bam-core::store` or `bam-core::api` — no import, no call — so §11's "always
shown, always editable, never auto-run" rule holds structurally: this
function has no way to run a query even if it wanted to, confirmed by
reading its full call graph rather than by a test that could pass for the
wrong reason.

`crates/bam-core/tests/llm_query_prompt.rs`: a `FakeProvider` implementing
`LlmProvider` directly (simpler than the HTTP-layer `FakeClient` P7.1's
tests use, since this layer never touches HTTP itself) drives ten
natural-language cases through both the GBNF and JSON-Schema paths, each
asserting the returned text re-parses under `BamDsl`; a prompt-content test
spot-checks real category strings from `tests/fixtures/tree_sample.txt`
(P1.1's `TREE` fixture, read and line-split by the test itself — no
production TREE parser exists yet, so none was added just for this); an
unbalanced-paren completion produces `QueryGenError::Unparseable`, not a
panic; one `#[ignore]`d test runs the same ten queries against a real local
llama.cpp server. 5 tests added (4 offline + 1 ignored). `cargo clippy
--workspace --all-targets` clean; `cargo build -p bam-core
--no-default-features --target wasm32-unknown-unknown` still compiles (I1
holds — `query_prompt.rs` only touches `query::ir`/`query::lang`/
`query::registry` and `llm`, none of which are `native`-gated).

## Round 38 — 2026-08-08 · Phase 7: embeddings and sqlite-vec (P7.4)

`package_embedding` (migration 0007): one row per package, `vector` packed
as raw little-endian float32 bytes — the exact layout sqlite-vec's
`vec_distance_cosine` reads without further encoding, confirmed against a
scratch crate before committing to the design (both a `vec0` virtual table
and a plain-table + scalar-function approach were tried; the plain table
won because `Similar`'s `threshold` is a similarity cutoff, not a top-`k`,
and `vec0` is a KNN index that wants a `k` bound). `store::open` registers
the extension once per process via `sqlite3_auto_extension`
(`store/mod.rs`), so every connection gets `vec_distance_cosine` for free.

`store::compile::compile` gains a `SimilarVectors` parameter (`text ->
Vec<f32>`, `HashMap`): `Predicate::Similar` now compiles to an `EXISTS`
against `package_embedding` (`1 - threshold` bound as the cosine-*distance*
cutoff, since sqlite-vec returns distance and the DSL's `threshold` is a
similarity) instead of `CompileError::SimilarNotSupported`, which is
removed. The compiler stays synchronous and never touches an
`LlmProvider` — resolving `text` to a vector is the caller's job
(`CompileError::MissingEmbedding` when it hasn't been), the same division
`marked_selection_id` already draws between the pure compiler and the
session layer that resolves context before calling it. `Session`'s own
`compiled_for` passes an empty map: no `Session` method has a provider to
resolve `Similar` with, so one still errors clearly rather than silently
matching nothing.

`store::embeddings::run_batch` (new): one call embeds up to `batch_size`
pending packages (readme landed, no embedding row yet) in exactly one
`LlmProvider::embed` call — P7.1's `embed` already takes a batch, so this
is chunking the input, not building new batching machinery. Mirrors
`fetch_worker::step`'s shape (P4.3): one step per call, not a loop, so the
caller controls pacing and an interrupted run resumes for free — the next
call's `SELECT ... NOT EXISTS (... package_embedding ...)` just excludes
whatever the previous call already wrote. A dimension change against
what's already stored (`tables::any_package_embedding_dim`) is
`EmbedError::DimensionMismatch`, not a silent write of an incompatible
vector.

`crates/bam-core/tests/store_compile.rs`: `similar_is_rejected_...` is gone
(the compiler no longer rejects it); replaced with a caller-side
`MissingEmbedding` test and a threshold-filtering test, three hand-picked
3-vectors and threshold 0.5 correctly keeping the near-identical one and
dropping the distant one. `crates/bam-core/tests/store_embeddings.rs`
(new): 100 packages batched into 5 `embed` calls; an "interrupted" run (one
batch, half the backlog) resumes and completes without a call count that
would imply re-embedding; a model switch mid-run is a reported
`DimensionMismatch`; and a hand-crafted-vector semantic search finds a
readme sharing no words with the query (a literal FTS5 search for the same
phrase misses it, checked in the same test) while excluding a
keyword-matching decoy whose embedding is deliberately far away. All five
of P7.4's tests pass. `cargo clippy --workspace --all-targets` clean
(one `missing_transmute_annotations` warning from the extension
registration's `transmute`, fixed with an explicit target type rather than
left as a warning). `cargo build -p bam-core --no-default-features
--target wasm32-unknown-unknown` still compiles (I1 holds — every P7.4
change lives under `store`, already entirely `native`-gated;
`sqlite-vec` is `native`-only in `Cargo.toml`, same as `rusqlite`).

## Next task

**P7.5 — LLM summaries** ([phase-7-llm.md](docs/plan/phase-7-llm.md)):
`kind = 'llm_summary'` enrichment from readme plus inventory. Rate limited,
resumable, targetable at a selection (I7), and cost-visible before a bulk
run starts (§16). Marked **S**.

What Round 38 leaves ready to build on:
- `store::embeddings::run_batch`'s one-step-per-call shape and its
  `NOT EXISTS`-against-the-output-table resumability pattern are the
  template P7.5's own resumable batch run can reuse directly.
- `tables::upsert_enrichment`'s `producer_version` reprocessing convention
  (P5.8) is exactly what P7.5's "bumping `producer_version` reprocesses"
  test needs — already built, not new for this phase.
- `Similar` is real now: a future frontend wiring natural-language search
  end-to-end needs to resolve query text to a vector (one `embed` call)
  before calling `store::compile::compile`, then supply it as
  `SimilarVectors` — no `Session` API change required to do that from the
  caller's side.

Still true from Round 35, unclaimed by this round:
- **P2.7's selection API** (I7) is what P7.5 targets a summarisation run at.
- **The `enrichment` table plus `upsert_enrichment`** (P5.8) is P7.5's
  `kind = 'llm_summary'` producer's mechanism.

§16's cost-visibility requirement (P7.5) remains a hard requirement worth
keeping in view — now due.

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
