# Query IR

← [Implementation plan index](../IMPLEMENTATION_PLAN.md) · invariant I2

The stable contract between every consumer of "a query": surface query
languages (`bam-dsl` and whatever follows it), the highlight engine (§11.1),
selections (`in:`/`marked`, P2.8), the SQL compiler (P2.5), and eventually an
LLM grammar generator (P7.2) and MCP. None of them talk to each other
directly — they all go through `Predicate` and `FieldRegistry`.

This document and `crates/bam-core/src/query/ir.rs` +
`.../query/registry.rs` are one artifact. If they drift, this document is
wrong.

## The IR

```rust
pub struct FieldId(pub String);

pub enum CmpOp { Eq, Ne, Lt, Le, Gt, Ge }

pub enum Value {
    Text(String),
    Int(i64),
    Date(String),   // ISO 8601 "YYYY-MM-DD", compared lexicographically
}

pub enum Pattern {
    Glob(String),
    Prefix(String),
}

pub enum SelectionRef {
    Named(String),
    Marked,          // the current working selection
}

pub enum Predicate {
    And(Vec<Predicate>),
    Or(Vec<Predicate>),
    Not(Box<Predicate>),
    Compare { field: FieldId, op: CmpOp, value: Value },
    Match   { field: FieldId, pattern: Pattern },
    FullText(String),
    InSelection(SelectionRef),
    Similar { text: String, threshold: f32 },
}
```

Everything here is owned data — no borrowed lifetimes, no `&'static str` in
the tree itself — because a `Predicate` is built from arbitrary user input
(typed, LLM-generated, or loaded from a saved highlight rule) and must
round-trip through serde without borrowing from whatever produced it. That's
why `FieldId` wraps an owned `String` rather than the `&'static str` a
`FieldDef`'s own `name` uses — the registry's field *definitions* are static,
but a *reference* to one inside a parsed predicate is not.

`Match` is glob/prefix matching, not free-text search — `FullText` is the
bareword/full-text case, with no field of its own; it's a whole-row search
that compiles to a `description LIKE` fallback until P4.6's FTS5 table
exists (P2.5).

`Similar` is parsed and type-checked from the start but rejected by the
compiler with "not yet supported" until P7.4 implements vector similarity.
Reserving the node now avoids invalidating every generated grammar and
few-shot prompt example a later phase would otherwise have to regenerate.

## The field registry

```rust
pub enum FieldType { Text, Int, Date }

pub enum SqlSource {
    Column(&'static str),
    Join { join_sql: &'static str, column: &'static str },
}

pub struct FieldDef {
    pub id: FieldId,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub ty: FieldType,
    pub ops: &'static [CmpOp],
    pub source: SqlSource,
}

pub struct FieldRegistry { /* Vec<FieldDef>, built once at startup */ }
```

`FieldRegistry::resolve(name)` looks a field up by its primary name or any
alias and errors, naming the field, if nothing matches. `check_compare`
validates a `CmpOp` against `ops`; `check_match` validates a glob/prefix
predicate against `ty` — only `Text` fields permit `Match` at all, so
`size:~'foo'` is rejected at resolve time rather than producing SQL that
silently matches nothing. Both checks happen before compilation, per §11's
"the generated query is inspectable and correctable" requirement — a
rejected predicate never reaches the compiler.

Languages resolve names through the registry; the compiler (P2.5) reads
`source` to emit SQL. **Registering a new `FieldDef` is the only change
needed to make a field queryable** — no language and no compiler file
changes. P2.8 (`in:`/`marked`) exists to prove this for a field whose source
is a `Join`, not just a plain `Column`.

## Initial field set

Mapped directly to the `package` table from P1.2 (`crates/bam-core/migrations/0001_initial.sql`):

| Field | Aliases | Type | Source | Ops |
|---|---|---|---|---|
| `dir` | — | Text | `dir` | Eq, Ne |
| `file` | — | Text | `file` | Eq, Ne |
| `name` | — | Text | `name` | Eq, Ne |
| `version` | `ver` | Text | `version` | Eq, Ne |
| `size` | `size_bytes` | Int | `size_bytes` | Eq, Ne, Lt, Le, Gt, Ge |
| `date` | `uploaded_on` | Date | `uploaded_on` | Eq, Ne, Lt, Le, Gt, Ge |
| `year` | — | Int | `uploaded_on` | Eq, Ne, Lt, Le, Gt, Ge |
| `description` | `desc` | Text | `description` | Eq, Ne |

All eight fields also permit `Match` (they're `Text`) except `size`, `date`,
and `year`.

`year` shares `uploaded_on` with `date`: the compiler extracts the year and
must additionally consult `date_precision` (P1.2's ±1-week vs. exact
distinction) before deciding whether a `week`-precision row near a year
boundary can be asserted into a `year>2000` range. That logic lives in the
compiler, not the registry — the registry only says *where the value comes
from*, not how a comparison against it is evaluated.

**Deliberately absent:** `type` and `author`, both used in
`bam-handoff.md` §11's illustrative examples. Neither has a backing column
yet — `type` awaits a derived category (from `dir` or a file-extension
table), `author` awaits Phase 4's readme harvesting. Registering a field
with no real `SqlSource` would be speculative, not initial; both are added
the round each becomes backed by real data, with no change to this design.

## Worked examples

Each maps a `bam-handoff.md` §11-style query to the IR tree it should
produce. These aren't yet parseable — `bam-dsl` doesn't exist until
P2.3/P2.4 — they're the target the parser is written against, and (per
P2.3) become that task's test table verbatim.

1. `dir:util/*`
   `Match { field: "dir", pattern: Glob("util/*") }`

2. `size>100k`
   `Compare { field: "size", op: Gt, value: Int(102400) }`
   (`k`/`M` suffixes are a language-level convenience — resolved to a plain
   `Int` before reaching the IR.)

3. `!type:mod` — not yet expressible; `type` has no field until it exists
   (see "Deliberately absent" above). Listed to record the gap, not to
   claim it compiles.

4. `year>2000`
   `Compare { field: "year", op: Gt, value: Int(2000) }`

5. `dir:util/* !type:mod OR year>2000` — juxtaposition binds tighter than
   `OR` (P2.3):
   `Or([ And([ Match{dir, Glob("util/*")}, Not(<type — gap>) ]), Compare{year, Gt, Int(2000)} ])`

6. `author:~'Mustermann'` (from the §11.1 highlight-rule example) — same
   gap as `type`; recorded for the same reason.

7. `tracker module editor` (a bareword search)
   `FullText("tracker module editor")`

8. `name:Deluxe* version:1.2`
   `And([ Match{name, Glob("Deluxe*")}, Compare{version, Eq, Text("1.2")} ])`

9. `in:'tracker candidates'`
   `InSelection(Named("tracker candidates"))`

10. `marked !size<10k`
    `And([ InSelection(Marked), Not(Compare{size, Lt, Int(10240)}) ])`

11. `similar:'tracker module editor' > 0.82`
    `Similar { text: "tracker module editor", threshold: 0.82 }`
    (constructs and serializes today; the compiler rejects it until P7.4.)

12. `dir:mus/* (year<1995 OR year>2000)`
    `And([ Match{dir, Glob("mus/*")}, Or([ Compare{year, Lt, Int(1995)}, Compare{year, Gt, Int(2000)} ]) ])`

## Rejected alternatives

- **`FieldId` as a closed enum**, one variant per field. Rejected: it would
  make P2.8's "registering a field touches only registry setup" claim false
  by construction — a new field would need a new enum variant in `ir.rs`,
  which is exactly the file P2.8 must not touch. An owned-string newtype
  keeps `ir.rs` closed over field additions.
- **`FieldId` as `&'static str`.** Considered to avoid the allocation, but a
  `Predicate` built from arbitrary parsed or LLM-generated input can't
  produce a `'static` reference, and serde can't deserialize into one
  without unsafely leaking memory or tying the `Predicate`'s lifetime to
  its input buffer. The allocation cost is irrelevant at query-parse
  frequency.
- **Folding `Match` into `Compare` via a `Like`/`Glob` `CmpOp` variant.**
  Rejected: `Compare`'s `Value` is typed per-field (`Int`/`Date`/`Text`),
  while `Match`'s `Pattern` is inherently textual regardless of field type —
  keeping them separate variants means the registry's `check_match` can
  reject non-Text fields structurally instead of by a runtime type
  mismatch inside `Value`.
- **A `matchable: bool` flag on `FieldDef`**, independent of `ty`. Rejected
  as redundant: every field in the initial set that should support glob
  matching is already `Text`, and no field is `Text` but un-matchable. A
  flag earns its place only once such a field exists.
