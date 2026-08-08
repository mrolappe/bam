# Phase 7 progress — LLM layer

← [PROGRESS.md](../../PROGRESS.md)

Round-by-round log for Phase 7 (Rounds 35–39), extracted from the top-level
progress file to keep that file scannable. Task ids refer to
[`IMPLEMENTATION_PLAN.md`](../../IMPLEMENTATION_PLAN.md) and
[phase-7-llm.md](../plan/phase-7-llm.md).

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

## Round 39 — 2026-08-08 · Phase 7: LLM summaries (P7.5, Phase 7 exit)

`crates/bam-core/src/store/summaries.rs` (new): `kind = "llm_summary"`
enrichment, mirroring P7.4's `embeddings::run_batch` shape exactly —
`pending()` selects packages with a landed readme but no enrichment row at
the current `SUMMARY_PRODUCER_VERSION`, `run_batch` processes up to
`batch_size` of them per call, so an interrupted run resumes for free (the
next call's `pending()` just excludes whatever the previous call already
wrote). Unlike embeddings' single batched `embed` call, each package gets
its own `LlmProvider::complete` call — summaries are per-package prose, not
a batchable vector op — so a provider error on one package is caught into
`SummaryOutcome::failed` and the rest of the batch still runs, rather than
propagating and aborting everything after it.

Summary input is readme text plus, when an inventory enrichment (P5.8)
exists for the package, its file listing (`get_enrichment(..., INVENTORY_KIND)`
decoded back to `Inventory`) — both go into one prompt. Selection scoping
(I7) is a plain `package_ids: Option<&[i64]>` parameter on `pending`/
`run_batch`; resolving a selection name to ids is left to the caller
(`Session::search_packages`), the same division `store::compile` already
draws between the pure query layer and session-level context resolution —
`summaries.rs` has no dependency on `Session` or the query IR at all.

§16's cost-visibility requirement: `estimate_run` scans the *whole* pending
backlog (not just one batch) and reports package count, an estimated token
count (chars/4 — no `LlmProvider` exposes a real tokenizer, so this is a
`ponytail:`-flagged heuristic ceiling), and, when a `cost_per_1k_tokens`
price is passed, an estimated cost (`None` for local/free providers).
`run_batch` takes a `confirmed: bool` gate and returns
`SummaryError::ConfirmationRequired` without calling the provider at all
when `false` — a structural requirement that the caller obtained and showed
the estimate first, not just a documented convention.

`crates/bam-core/tests/store_summaries.rs`: interrupted/resumed run over
100 packages across four `batch_size = 40` calls (40/40/20/0); bumping
`producer_version` reprocesses a stale row while leaving it alone does not;
`estimate_run`'s token/cost numbers are asserted directly and an
unconfirmed `run_batch` call makes zero provider calls; a two-id selection
touches exactly those two packages and leaves a third alone; one failing
package (matched by a provider error trigger) doesn't block the other two
in the same batch, and is reported in `failed` rather than silently
dropped. All five of P7.5's tests pass, matching phase-7-llm.md's list
one-to-one. `cargo clippy --workspace --all-targets` clean. `cargo build -p
bam-core --no-default-features --target wasm32-unknown-unknown` still
compiles (I1 holds — `summaries.rs` lives entirely under `store`, already
native-gated the same way `embeddings.rs` and `inventory.rs` are). 208
tests total across the workspace (5 added).

**Phase 7 exit reached.** Natural-language search that emits inspectable
DSL (P7.3), semantic similarity (P7.4), and now summaries (P7.5) — all
runnable entirely locally against a llama.cpp server, per §10's hard
requirement.
