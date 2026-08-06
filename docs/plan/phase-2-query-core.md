# Phase 2 — Query core: IR, languages, compiler, API, selections

← [Implementation plan index](../../IMPLEMENTATION_PLAN.md)

Mostly new material. Pluggable query languages (invariant **I2**) invert the
original design: the stable contract is a typed IR plus a field registry, and
surface syntaxes are interchangeable implementations over it.

No terminal code in this phase. The TUI is Phase 3.

---

### P2.1 — Query IR + field registry · **O**

Deliverable: `docs/query-ir.md` **and** the Rust types it describes, in
`crates/bam-core/src/query/ir.rs` and `.../registry.rs`. Doc and types are one
artifact; splitting them guarantees they drift.

```rust
enum Predicate {
    And(Vec<Predicate>), Or(Vec<Predicate>), Not(Box<Predicate>),
    Compare { field: FieldId, op: CmpOp, value: Value },
    Match   { field: FieldId, pattern: Pattern },   // glob / prefix
    FullText(String),
    InSelection(SelectionRef),
    Similar { text: String, threshold: f32 },       // parsed, rejected until P7.4
}
```

The **field registry** maps each name and its aliases to a type, the operators
that field permits, and a SQL source (a column, or a join plus column):

```rust
struct FieldDef {
    id: FieldId, name: &'static str, aliases: &'static [&'static str],
    ty: FieldType, ops: &'static [CmpOp], source: SqlSource,
}
```

Languages resolve names through the registry; the compiler emits SQL through
it. Registering a new field makes it queryable **without touching any language
or the compiler** — the property P2.8 exists to prove.

`Similar` is parsed and type-checked from the start but rejected at compile
time with "not yet supported" until P7.4. Reserving it costs nothing now;
adding a node to the IR later invalidates every generated grammar and every
few-shot prompt example.

The doc records: the IR, the registry shape, the initial field set mapped to
P1.2's columns, worked IR trees for a dozen queries, and a short
rejected-alternatives note.

**Tests first:**
- Every `Predicate` variant round-trips through serde.
- The registry resolves a field by name and by alias.
- An unknown field name errors, and the error names the field.
- An operator the field does not permit errors (`size:~'foo'` is rejected at
  resolve time, not at SQL time).
- `Similar` constructs and serializes, and is documented as compile-rejected.

**Why O:** this is the contract between every language, the compiler, the
highlight engine, selections, the LLM grammar generation, and eventually MCP.
Six consumers. It is the highest-leverage artifact in the project, and the one
place where getting the abstraction wrong is not recoverable cheaply.

**Hand over:** `bam-handoff.md` §11 and §11.1, invariants I2 and I7, the
`package` schema from P1.2.

**Done when:** the five tests pass and `docs/query-ir.md` covers the sections
listed above.

---

### P2.2 — `QueryLanguage` trait + registry · **O**

```rust
trait QueryLanguage {
    fn id(&self) -> &str;                                   // "bam-dsl"
    fn parse(&self, src: &str, reg: &FieldRegistry) -> Result<Predicate, ParseError>;
    fn render(&self, p: &Predicate) -> Option<String>;      // round-trip for UI display
    fn grammar(&self, kind: GrammarKind) -> Option<String>; // GBNF / JSON Schema, if supported
}
```

Plus a registry keyed by id, and a `default_query_language` config key.
`render` and `grammar` return `Option` because not every language can
round-trip or be constrained — that is a capability, not a failure.

**Tests first:**
- Two stub languages register and resolve by id.
- An unknown id errors, naming the requested id and listing what is available.
- The configured default is used when no id is given.
- A stub language parses a string to a known `Predicate`.
- A language returning `None` from `grammar` is handled without error by a
  caller that wanted one.

**Why O:** invariant I4's reference implementation — unpackers (P5.3) and
launchers (P6.1) follow the shape set here, and Phase 8 registers WASM-backed
implementations through it. Three registries inherit whatever is decided.

**Hand over:** invariants I2 and I4, the trait above, the IR from P2.1.

**Done when:** the five tests pass.

---

### P2.3 — bam-dsl grammar specification · **S**

`docs/lang-bam-dsl.md`. The default surface syntax.

```
query   := or_expr
or_expr := and_expr ( 'OR' and_expr )*
and_expr:= unary ( ' ' unary )*          # juxtaposition is AND
unary   := '!'? atom
atom    := '(' query ')' | term
term    := field ':' pattern | field op value | bareword
op      := '<' | '>' | '<=' | '>='
```

Juxtaposition-as-AND **must** bind tighter than `OR`, or
`dir:util/* !type:mod OR year>2000` parses surprisingly. Values support `*`
globs and `k`/`M` size suffixes. A bareword is a `FullText` node.

Contents: the grammar, a precedence table, fifteen worked examples each paired
with **its expected IR tree**, and a list of malformed inputs with the span the
error should point at.

**Tests first:** none of its own — this is a specification, and its fifteen
examples *become* P2.4's test table. The one automatable check belongs to P2.4:
every example listed here must appear there with the IR tree this document
claims for it, so a stale example fails the build rather than misleading a
reader. Writing a test that asserts a Markdown file contains fifteen headings
would be ceremony, not verification.

**Why S** (downgraded from Opus in the previous plan): the IR now carries the
five-consumer risk that made this document expensive to get wrong. A surface
language that turns out badly is replaceable — which is most of what
pluggability bought. The grammar is a consumer of the contract, not the
contract.

**Hand over:** `docs/query-ir.md`, §11, the grammar sketch above, "fifteen
examples, each with its IR tree" and "malformed inputs with expected error
spans".

**Done when:** the document exists with all four sections and every example
names the IR it produces.

---

### P2.4 — bam-dsl parser · **S**

Hand-rolled tokenizer plus precedence-climbing parser producing a `Predicate`
directly — no private AST in between. Errors carry byte spans.

Spans are non-negotiable: §11 requires the generated query to be shown to the
user and be correctable, and an error without a position is not correctable.

**Tests first:**
- All fifteen examples from `docs/lang-bam-dsl.md` parse to their documented
  `Predicate`.
- Each malformed input reports a span pointing at the offending token.
- `dir:util/* !type:mod OR year>2000` parses with AND binding tighter.
- An unknown field errors naming the field and suggesting near-matches from
  the registry.
- `render(parse(s))` round-trips each of the fifteen examples.

**Why S:** the grammar is written down and the expected outputs are enumerated.
Implementation against a fixed target.

**Hand over:** `docs/lang-bam-dsl.md`, the `QueryLanguage` trait, "byte spans
required", the five test groups.

**Done when:** all five pass.

> Skipped: `nom`. Ten productions and a hard requirement for precise error
> spans — a precedence climber is shorter here than the combinators plus the
> error-mapping needed to extract spans. `nom` still earns its place in P9.7
> for AmigaGuide, which is genuinely line-command-heavy.

---

### P2.5 — IR → SQL compiler · **O**

`Predicate` → parameterized SQL over `package`, in
`crates/bam-core/src/store/compile.rs` (inside the `store` module, per I1).

**Every literal is a bound parameter.** No string interpolation anywhere. This
is what makes §11's central safety claim — "the LLM never emits SQL" — true by
construction rather than by review.

Three non-obvious points:

- Globs compile to `GLOB`, not `LIKE`. Case sensitivity differs and Aminet
  paths are case-significant.
- `year>2000` must respect `date_precision`: a `week`-precision row near a year
  boundary cannot be asserted into the range.
- `FullText` has no FTS5 table until P4.6. Until then it compiles to a
  `description LIKE` fallback, and the switch happens in one place.

**Tests first:**
- Every example in `docs/lang-bam-dsl.md` compiles to SQL that executes against
  a Phase 1 fixture DB and returns the expected row ids.
- A value containing `'; DROP TABLE package; --` is **bound**, and `package`
  still exists afterwards.
- Glob patterns emit `GLOB`; a case-differing path does not match.
- `year>2000` excludes `week`-precision rows whose ±1-week window straddles
  the boundary.
- Nested `Not`/`Or` parenthesize correctly — `!(a OR b)` is not `!a OR b`.
- `Similar` returns a "not yet supported" error rather than silently producing
  no predicate.

**Why O:** this file is the injection boundary and it fixes the query shapes
that indexes will be designed around. The `date_precision` and parenthesization
cases are the kind of thing that produces subtly wrong result sets rather than
errors.

**Hand over:** the IR, the `package` schema, §11's safety rationale, the three
non-obvious points, the six test groups.

**Done when:** all six pass.

---

### P2.6 — `bam-core::api` use-case layer · **O**

The three-adapter seam from §8, extended by invariant **I5** for the web
variant: `search_packages`, `get_package`, `list_categories`, and the selection
operations from P2.7.

Rules this task establishes:

1. No `println!` in the core (already enforced by P0.4).
2. Every long-running operation takes a `CancellationToken`.
3. Request/response types are `Serialize` + `Deserialize` + `JsonSchema`.
4. **No global mutable state** — every call takes an explicit session handle.
   No singletons, no `static mut`, no thread-local connection.
5. **Progress events are typed and serializable** (P1.10's `ProgressEvent`).
6. **Long operations carry an `OperationId`**, so a reconnecting web client can
   re-attach rather than orphaning a running ingest.

**Tests first:**
- Every request and response type round-trips serde and produces a JSON schema.
- Two sessions operating concurrently do not observe each other's state.
- A cancelled operation stops within a bounded time and reports cancellation
  rather than an error.
- An `OperationId` returned by a long call can be used to query its status.
- No function in the module writes to stdout or stderr (P0.4 covers this;
  assert it stays green).

**Why O:** get this wrong and `bam-server` (P9.2) and `bam-mcp` become rewrites
instead of thin adapters — which is the single thing §8 exists to prevent. A
small module with disproportionate consequences.

**Hand over:** §8 in full, invariant I5, the compiler signature from P2.5.

**Done when:** the five tests pass.

---

### P2.7 — Selection store and operations · **S**

Invariant **I7**. Over the P1.2 tables: `mark`, `unmark`, `toggle`, `clear`,
`select_by_query(pred, mode)`, `save_as(name)`, `load(name)`, `list`, `delete`.

`mode` is `Replace | Union | Intersect | Subtract`, so selections compose with
search instead of merely coexisting with it.

**Tests first:**
- `mark` is idempotent; `toggle` twice returns to the original state.
- Each of the four modes over a known result set produces the expected
  membership.
- `save_as` then `load` in a fresh session returns the same members.
- Deleting a package removes its membership (P1.2's cascade).
- An ephemeral selection is cleaned up on session end; a named one is not.
- Two sessions each with a working selection do not interfere (ties to I5).

**Why S:** ordinary CRUD plus set algebra, over a schema that already exists
and against an API shape already fixed by P2.6.

**Hand over:** the two tables from P1.2, the operation list, the four modes,
the six test groups.

**Done when:** all six pass.

---

### P2.8 — Register `in:` and `marked` as query fields · **H**

Add two entries to the field registry: `in:'name'` resolving to an `EXISTS`
subquery over `selection_member` for the named selection, and `marked` for the
current working selection.

**Tests first:**
- `in:'tracker candidates'` parses, compiles, and returns exactly that
  selection's members.
- `in:'nonexistent'` errors naming the missing selection.
- `marked !type:mod` composes correctly.
- **The diff for this task touches only registry setup** — no file under
  `query/lang/` and no file in the compiler is modified. This is the acceptance
  criterion that proves invariant I2's central claim, so it is checked by
  reviewing the diff, not by a test.

**Why H:** two registry entries and their tests, once P2.1's `SqlSource`
supports a join. If it turns out not to, that is a P2.1 defect to report back —
not something to work around here.

**Hand over:** the `FieldDef` shape, the `selection_member` schema, the four
acceptance items, and the instruction to report back rather than patch the
compiler if a join cannot be expressed.

**Done when:** the three tests pass **and** the diff is confined to registry
setup.

---

**Phase 2 exit:** queries in a pluggable surface language compile to safe SQL
through a stable IR, selections persist and compose with search, and the API
layer is ready for a second and third adapter.
