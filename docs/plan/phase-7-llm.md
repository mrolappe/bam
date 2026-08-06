# Phase 7 — LLM layer

← [Implementation plan index](../../IMPLEMENTATION_PLAN.md)

Local models are a hard requirement, not an optional extra (§10). **llama.cpp
is the documented default**, because its native GBNF support means
grammar-constrained query generation works out of the box.

---

### P7.1 — `LlmProvider` trait and OpenAI-compatible implementation · **S**

```rust
trait LlmProvider {
    async fn complete(&self, req: CompletionRequest) -> Result<String>;
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    fn capabilities(&self) -> Capabilities;   // GBNF? JSON Schema? context size?
}
```

One implementation covers llama.cpp, Ollama, and cloud OpenAI-compatible
endpoints — they share a wire format. The meaningful difference lives entirely
in `capabilities()`: llama.cpp reports GBNF support, cloud endpoints report
JSON Schema.

Default configuration targets a local llama.cpp server. Ollama and cloud
providers are reachable by changing base URL and model name, nothing else.

Uses P1.9's `HttpClient` trait, so every test runs offline.

**Tests first:**
- The same code path completes against fake llama.cpp and fake cloud
  responses, distinguished only by config.
- `capabilities()` reports GBNF for llama.cpp and JSON Schema for a cloud
  endpoint.
- A connection failure to a local server produces an error naming the
  configured URL and suggesting the server may not be running — the single most
  common local-model failure, and worth a good message.
- Embedding a batch returns vectors in input order.
- One `#[ignore]`d test against a real local llama.cpp server.

**Why S:** the trait is written out in §10 and the HTTP client is ordinary.

**Hand over:** §10's trait definition, "one impl for all OpenAI-compatible
endpoints, differences via `capabilities()`", "llama.cpp is the default",
P1.9's `HttpClient`, the five tests.

**Done when:** the four offline tests pass.

---

### P7.2 — Grammar generation per language · **O**

Generate GBNF (llama.cpp) and JSON Schema (cloud) from
`QueryLanguage::grammar()` (invariant **I2**) — **not** from a hardcoded DSL.
Each language produces its own; a language that cannot be constrained returns
`None` and the caller falls back to unconstrained generation plus validation.

§10 is explicit that both representations must derive from one source.
Hand-maintaining two encodings of one grammar is how they drift, and the drift
is invisible until a model emits something the parser rejects.

**Tests first:**
- Every example in `docs/lang-bam-dsl.md` validates against both the generated
  GBNF and the generated JSON Schema.
- A deliberately malformed query is rejected by both.
- **The two artifacts accept the same language** — a property test over
  generated inputs, not just the fifteen examples. This is the drift check, and
  it is the reason the task is Opus rather than Sonnet.
- A language returning `None` is handled by the caller without error.

**Why O:** this is the mechanism that makes the provider genuinely swappable,
and it requires understanding where GBNF and JSON Schema differ in
expressiveness — GBNF constrains token by token, JSON Schema constrains
structure — and what to do where they do not line up.

**Hand over:** `docs/lang-bam-dsl.md`, `docs/query-ir.md`, P2.2's
`QueryLanguage` trait, §10's capabilities paragraph, the four tests including
the equivalence property test.

**Done when:** all four pass.

---

### P7.3 — Query generation prompt · **S**

A prompt template carrying the TREE category vocabulary, a file-type
dictionary, and few-shot examples (§11), with output grammar-constrained via
P7.2.

**The generated query is always shown to the user and always editable before it
runs.** §11 names this as the architectural safety net regardless of model
size: a model mistake becomes a visible, correctable suggestion rather than a
silent wrong result set.

**Tests first:**
- Ten natural-language test queries produce valid, parseable DSL against a
  fake provider returning canned completions.
- The prompt includes the category vocabulary from the TREE fixture.
- The result surfaces as editable text; **no code path executes a generated
  query without user confirmation** — verified by review of the call graph.
- A model returning unparseable output produces a clear error, not a panic.
- One `#[ignore]`d test runs the ten queries against a real local 7–8B model.

**Why S:** prompt assembly against a recipe that §11 spells out.

**Hand over:** §11's "why 7–8B is enough" paragraph — it names exactly what the
prompt must contain — the TREE fixture from P1.1, the provider trait, the
always-editable rule, the five tests.

**Done when:** the four offline tests pass and the call-graph review confirms
the third.

---

### P7.4 — Embeddings and sqlite-vec · **S**

Add `sqlite-vec`, embed readme text via `LlmProvider::embed`, store vectors,
and enable the `Similar` IR node reserved in P2.1 — the compiler stops
rejecting it.

**Batch the embedding calls.** 84,000 sequential round-trips is a night of
compute for no reason; batched locally it is a fraction of that (§10).

**Tests first:**
- `Similar` now compiles instead of erroring, and its threshold filters.
- A semantic query returns plausibly related packages that a keyword search for
  the same phrase misses — asserted over a small hand-labelled fixture set.
- Embedding is batched: a fake provider records call count, and 100 packages
  produce far fewer than 100 calls.
- An interrupted embedding run resumes without re-embedding completed packages.
- Vector dimension mismatch (switching embedding models) is detected and
  reported rather than silently returning nonsense.

**Why S:** every integration point was reserved in earlier phases; this
connects them.

**Hand over:** P2.1's `Similar` node, P2.5's rejection to remove, the provider's
`embed` signature, "batch, and make it resumable", the five tests.

**Done when:** all five pass.

---

### P7.5 — LLM summaries · **S**

`kind = 'llm_summary'` enrichment produced from readme plus inventory. Rate
limited, resumable, targetable at a selection (I7), and **cost-visible before a
bulk run starts** — §16 flags eager summarisation of 84,000 packages against a
paid API as a real hazard.

**Tests first:**
- Interrupting a 100-package run and restarting processes only the remainder.
- Bumping `producer_version` reprocesses; leaving it does not.
- A bulk run reports estimated token count and, for paid providers, estimated
  cost before starting, and requires confirmation.
- Summarising a selection touches only its members.
- A provider error on one package does not abort the batch.

**Why S:** the enrichment machinery exists; this is another producer over it.

**Hand over:** the `enrichment` row shape, the provider trait, P2.7's selection
API, §16's cost warning, the five tests.

**Done when:** all five pass.

---

**Phase 7 exit:** natural-language search that emits inspectable DSL, semantic
similarity, and summaries — all runnable entirely locally.
