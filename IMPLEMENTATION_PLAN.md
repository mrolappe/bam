# bam — Implementation Plan

Derived from [`bam-handoff.md`](bam-handoff.md). This document is the index and
the shared conventions; the tasks live in one document per phase under
[`docs/plan/`](docs/plan/).

Current status and the next task to pick up are in [`PROGRESS.md`](PROGRESS.md).

---

## How to read this

### Model tiers

| Tier | Model | Use for |
|---|---|---|
| **H** | Haiku 4.5 | Mechanical, fully-specified work. No design latitude. Boilerplate, fixtures, DDL transcription, CI config, format conversion, test tables whose expected values someone else determined. |
| **S** | Sonnet 5 | Ordinary implementation against a written spec. Parsers with a known grammar, HTTP clients, TUI widgets, SQL, migrations. Anything where "what correct looks like" is already decided. |
| **O** | Opus 5 | Work where the design *is* the deliverable: interface seams, the query IR, registries others register into, versioned public contracts, correctness-critical binary formats. |

The tier tracks **how expensive a wrong decision is, not how much code gets
typed.** A 40-line file that fixes an interface everything else depends on is
Opus. A 400-line table of test cases is Haiku.

### TDD is the working method

Red → green → refactor, per task. Consequences that shape every task entry:

- Each task carries a **Tests first** list. That list *is* the specification;
  the prose only says how to satisfy it.
- Fixtures land before the parsers that consume them.
- The default `cargo test` run touches **no network**. Live-mirror and
  live-model tests are `#[ignore]`d and run explicitly.
- **When delegating, hand over the test list with its expected values.** A
  cheaper model asked to make tests pass will otherwise edit the expectation.
  Several tasks say so in as many words; the rule is general.

### Delegation protocol

Every task has a **Hand over** list. Pass *only* those items. Do not paste
`bam-handoff.md` wholesale into a subtask — it is 500 lines of rationale for
decisions the subtask does not need to re-litigate.

A Haiku task gets the paths it writes, the exact signatures it implements, and
its acceptance check. It does not get the architecture.

### Task entry format

```
### PX.Y — Title · **Tier**
what to build, and the non-obvious constraints
**Tests first:** the specification
**Why <tier>:** why this model and not a cheaper one
**Hand over:** exactly the context a delegate needs
**Done when:** the acceptance check
```

`PX.Y` ids are referenced from `PROGRESS.md`, so a session can resume by id.

---

## Architectural invariants

These are why the plan is shaped as it is. Each has a mechanical check, because
an invariant nobody can test is a preference.

### I1 — `bam-core` compiles to `wasm32-unknown-unknown`

Every host capability sits behind a trait whose native implementation is
feature-gated:

```
bam-core (default = ["native"])
  trait BlobStore    → FsBlobStore    (native)  │ OPFS      (wasm, later)
  trait HttpClient   → ReqwestClient  (native)  │ fetch     (wasm, later)
  trait Unpacker     → process/in-proc (native) │ unavailable in wasm
  trait Launcher     → process        (native)  │ unavailable in wasm
```

Enforced by a CI job (P0.3) added while the core is still empty. Discipline
decays across sessions; a red CI job does not.

**`rusqlite` is confined, not abstracted.** All SQL lives in
`bam_core::store::*`, the only module permitted to name `rusqlite`, checked by
P0.4. A future wasm backend re-implements one module rather than the
application. A full `Store` trait over every query would be a large abstraction
serving one hypothetical consumer; confinement buys the same option for
roughly nothing.

### I2 — The query IR is the contract, not the syntax

The stable artifact is a typed predicate IR plus a field registry; surface
syntaxes are interchangeable implementations over it (P2.1, P2.2).

```rust
enum Predicate {
    And(Vec<Predicate>), Or(Vec<Predicate>), Not(Box<Predicate>),
    Compare { field: FieldId, op: CmpOp, value: Value },
    Match   { field: FieldId, pattern: Pattern },
    FullText(String),
    InSelection(SelectionRef),
    Similar { text: String, threshold: f32 },   // parsed, rejected until P7.4
}
```

The field registry maps names and aliases to a type, permitted operators, and a
SQL source. Registering a field makes it queryable without touching any
language or the compiler — P2.8 exists to prove exactly that.

One consequence worth stating: the surface grammar dropped from Opus to Sonnet
tier. The IR now absorbs the multi-consumer risk, so a language that turns out
wrong is replaceable. **Pluggability lowered the cost of getting one language
wrong, which is most of its value here.**

### I3 — Highlight rules name their language

```toml
[[highlight]]
name = "my own uploads"
lang = "bam-dsl"        # optional; falls back to default_query_language
when = "author:~'Mustermann'"
gutter = "user"
priority = 10
```

A rule compiles to the same IR as a search query, so highlighting, search, and
selection-by-criteria share one evaluation path.

### I4 — Registries, uniformly shaped

Query languages (P2.2), unpackers (P5.3), and launchers (P6.1) all use the same
pattern: a trait, a registry implementations register into, config-driven
selection, and runtime probing where availability varies. Phase 8 registers
WASM-backed implementations through the same registries without touching call
sites — P8.2 is where that claim is proven.

Two details fixed early because they are expensive later:

- **Unpacker selection is by magic bytes, not extension.** Aminet filenames lie
  routinely.
- **`Launcher::capabilities()` drives behaviour, not just reporting.** vAmiga's
  weak directory-volume support becomes a flag the core reads to choose between
  the `.uaem` path and a disk image, instead of the emulator choice leaking
  into extraction logic.

### I5 — The API layer is session-scoped and transport-agnostic

Extending `bam-handoff.md` §8's three rules for the web variant:

1. No `println!`/`eprintln!` in the core (checked by P0.4).
2. Every long-running operation takes a `CancellationToken`.
3. Request/response types are `Serialize` + `Deserialize` + `JsonSchema`.
4. **No global mutable state** — every call takes an explicit session handle.
5. **Progress events are typed and serializable**, never formatted strings. A
   string cannot become a web progress bar, be styled per frontend, or be
   translated.
6. **Long operations carry an `OperationId`**, so a reconnecting web client
   re-attaches instead of orphaning a running ingest.

### I6 — Input is a resolver, not a match arm

```
[count] [operator] {motion | object | command}
```

v1 registers modes and motions only — zero operators, zero objects. The grammar
slot exists unused, costing a few dozen lines now against rewriting the input
layer of a working TUI later. Objects, when they arrive, resolve to **sets of
packages** rather than text ranges (`ic`/`ac` category, `is` selection, `ir`
result set), which composes directly with I7: an operator applied to an object
produces a selection.

### I7 — Selections are a core concept

```sql
selection(id, name, created_at, ephemeral)
selection_member(selection_id, package_id)
```

Not TUI state — the GUI, the web variant, and MCP all need it, and bulk actions
take a selection as their target. Exposed to the query language as `in:'name'`
and `marked`, so selections compose with search rather than sitting beside it.
Set operations: `replace | union | intersect | subtract`.

### I8 — TDD

See "TDD is the working method" above.

---

## Phases

| Phase | Document | Contents | Tasks |
|---|---|---|---|
| 0 | [phase-0-scaffold.md](docs/plan/phase-0-scaffold.md) | Workspace, CI, the two invariant guards | 4 |
| 1 | [phase-1-ingest.md](docs/plan/phase-1-ingest.md) | INDEX/RECENT parsing, schema, incremental update | 10 |
| 2 | [phase-2-query-core.md](docs/plan/phase-2-query-core.md) | Query IR, languages, SQL compiler, API layer, selections | 8 |
| 3 | [phase-3-tui.md](docs/plan/phase-3-tui.md) | Input model, keymap, list, selection UI, highlighting | 9 |
| 4 | [phase-4-harvest-search.md](docs/plan/phase-4-harvest-search.md) | Readme harvesting queue, FTS5 | 7 |
| 5 | [phase-5-cache-extraction.md](docs/plan/phase-5-cache-extraction.md) | Blob cache, unpacker registry, `.uaem` | 8 |
| 6 | [phase-6-launchers.md](docs/plan/phase-6-launchers.md) | Launcher registry, FS-UAE | 4 |
| 7 | [phase-7-llm.md](docs/plan/phase-7-llm.md) | llama.cpp provider, grammars, embeddings, summaries | 5 |
| 8 | [phase-8-plugin-host.md](docs/plan/phase-8-plugin-host.md) | extism WASM host, contracts | 5 |
| 9 | [phase-9-frontends.md](docs/plan/phase-9-frontends.md) | Vue frontend, `bam-server`, Tauri, visualizations | 7 |

Phases 0–5 are the hard core. 6–9 are additive and can be resequenced.

---

## Round sizing

One phase is too much for one session. These are stopping points that leave the
tree green.

| Round | Tasks | Ends with |
|---|---|---|
| 1 | P0.1–P0.4, P1.1 | Workspace builds; wasm and purity checks green; fixtures committed |
| 2 | P1.2–P1.3 | Schema + migrations |
| 3 | P1.4–P1.7 | Parser + normalizer, tested offline |
| 4 | P1.8–P1.10 | `bam ingest` end to end |
| 5 | P2.1–P2.2 | Query IR + language trait — design, no surface syntax |
| 6 | P2.3–P2.5 | bam-dsl parses and compiles to SQL |
| 7 | P2.6–P2.8 | API layer + selections |
| 8 | P3.1–P3.3 | Input model + keymap |
| 9 | P3.4–P3.9 | Usable TUI |
| 10+ | Phase 4 onward | Reassess after real use |

Rounds 5 and 8 are design-heavy with little code, deliberately. Both produce
documents several later phases depend on, and bundling them with implementation
work is how such documents end up written hastily.

---

## Model tier summary

67 tasks.

| Tier | Count | Tasks |
|---|---|---|
| **Opus** | 12 | P1.2, P2.1, P2.2, P2.5, P2.6, P3.1, P5.6, P6.1, P7.2, P8.1, P8.2, P9.1 |
| **Sonnet** | 41 | the bulk of implementation |
| **Haiku** | 14 | P0.1–P0.4, P1.1, P1.3, P1.7, P2.8, P3.3, P3.9, P4.2, P5.5, P5.7, P6.3 |

Opus tasks cluster where a wrong answer propagates: the schema, the query IR
and its compiler, the API seam, the input model, the two other registries,
the one silent binary format, the grammar generator, the plugin contract, and
the frontend transport seam.

That is 12 of 67, up from 6 of 45 in the previous revision. **Pluggability
concentrates risk into the abstraction rather than removing it** — five of the
six new Opus tasks are the contracts that pluggability introduced.

---

## Deferred

Not scheduled. Each has a concrete trigger.

| Item | Trigger |
|---|---|
| `bam-mcp` | When someone wants MCP access. I5 is enforced from P1.10 onward, so the adapter stays thin — that is what makes deferring it safe rather than a bet. |
| WASM browser build of `bam-core` | When a read-only web viewer is wanted. I1's CI job keeps it reachable; the remaining work is an OPFS-backed `store` module. |
| `lhasa` in-process LHA unpacker | When `unar` spawn overhead measurably hurts a bulk run. P5.3's registry makes it additive. |
| Vim operators and objects | When motions alone start to feel limiting. I6 leaves the grammar slot open. |
| Amiberry / vAmiga launchers | On demand. P6.1's registry means adding one touches no existing code. |
| `highlight_hits` materialization | When visible-window evaluation measurably lags — realistically only once P7.4's vector predicates exist. |
| Windows support and CI | If Windows becomes a priority. `crossterm` keeps it compiling meanwhile; nothing tests it. |

---

## Open questions

All four of `bam-handoff.md` §14's questions are now resolved (see
`PROGRESS.md`). Mirror rsync access is decided low-priority: P4.3 is built as
the incremental, on-demand path regardless of whether bulk access is ever
confirmed.
