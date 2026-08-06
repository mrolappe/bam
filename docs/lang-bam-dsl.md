# `bam-dsl`

← [Implementation plan index](../IMPLEMENTATION_PLAN.md) · [Query IR](query-ir.md) · invariant I2

The default surface syntax over the [query IR](query-ir.md). One of
potentially several `QueryLanguage` implementations (P2.2) — nothing here is
privileged at the IR or compiler level.

## Grammar

```
query      := or_expr
or_expr    := and_expr ( SP+ 'OR' SP+ and_expr )*
and_expr   := unary ( SP+ unary )*
unary      := '!' unary | atom
atom       := '(' query ')' | special_term | term
special_term := in_term | marked_kw | similar_term
in_term    := 'in' ':' string
marked_kw  := 'marked'
similar_term := 'similar' ':' string ( SP* '>' SP* float )?
term       := field ':' '~'? rhs | field cmp_op value | bareword
field      := ident
cmp_op     := '=' | '!=' | '<=' | '>=' | '<' | '>'
rhs        := value | pattern
value      := number size_suffix? | date | string | bareword_value
pattern    := anything containing '*' (Glob), matched literally otherwise
size_suffix:= 'k' | 'K' | 'm' | 'M'
string     := "'" ... "'"          # single-quoted, for values with spaces
bareword   := ident, not matching any field name followed by ':' / cmp_op,
              and not 'marked'
```

Whitespace (`SP`) is the only juxtaposition operator and the only token
separator; there is no comma.

## Precedence

Juxtaposition (AND) **must** bind tighter than `OR`, matching every other
shell-like or search-bar DSL (and the phase doc's own worked case):
`dir:util/* !name:mod OR year>2000` reads as `(dir:util/* AND !name:mod) OR
year>2000`, not `dir:util/* AND (!name:mod OR year>2000)`.

From loosest to tightest:

| Precedence | Construct | Associativity |
|---|---|---|
| 1 (loosest) | `OR` | left |
| 2 | juxtaposition (AND) | left |
| 3 | `!` (unary not) | prefix |
| 4 (tightest) | `( ... )`, terms | — |

## The `:` operator is context-sensitive

`field:rhs` means **glob match** if `rhs` contains `*`, and **equality
compare** otherwise:

- `dir:util/*` → contains `*` → `Match { pattern: Glob("util/*") }`
- `version:1.2` → no `*` → `Compare { op: Eq, value: Text("1.2") }`

This single rule (not two syntaxes) is what makes `field:literal` read as
"equals" without a separate `=` most of the time, while `field:glob*` still
reads as a pattern — both are the same operator, disambiguated by the value.

`field:~rhs` — a `~` immediately after the colon — **always** matches
(never compares), even without a `*`: `size:~'foo'` is
`Match { pattern: Prefix("foo") }` in shape (see below for why it doesn't
actually succeed); `field:~foo*bar` is `Match { pattern: Glob("foo*bar") }`.
This is the exact syntax used in `docs/query-ir.md` and
`bam-handoff.md` §11.1 (`author:~'Mustermann'`) and in the field registry's
own error text — it's what lets a forced match reach the registry against a
non-`Text` field, so `check_match` can reject it. `size:~'foo'` is listed
under Malformed inputs below: it parses to a well-formed *attempt*, but that
attempt is rejected at resolve time, so parsing this DSL string never
succeeds end to end.

`cmp_op` (`= != <= >= < >`) always compares, never matches, and is rejected
by `check_compare` if the field's `ops` don't include it (e.g. `dir<'a'` —
`dir` only permits `Eq`/`Ne`).

## Bareword runs merge into one `FullText`

`and_expr`'s juxtaposition-is-AND rule has one exception: **adjacent
bareword terms are not separate `FullText` conjuncts ANDed together — they
merge into a single `FullText` node** spanning their combined source text.
`tracker module editor` is `FullText("tracker module editor")`, not
`And([FullText("tracker"), FullText("module"), FullText("editor")])`. A run
ends at the first typed term, `OR`, `!`, or `(`; a typed term appearing
between two bareword runs starts a new run rather than joining them. This
matches how a search bar reads free text as one query, while still letting
`size>100k tracker module dir:mus/*` AND a free-text run with typed filters.

If an `and_expr` reduces to a single conjunct after this merge (the common
case — one term, or one bareword run), no `And` wrapper is emitted; `size>100k`
compiles to a bare `Compare`, not `And([Compare])`. Same for `or_expr`
reducing to a single `and_expr`.

## Worked examples

Each of the fifteen appears verbatim in P2.4's parser test table. Trees are
written as the `Predicate` constructors from `docs/query-ir.md`.

1. `dir:util/*`
   `Match { field: "dir", pattern: Glob("util/*") }`

2. `size>100k`
   `Compare { field: "size", op: Gt, value: Int(102400) }`

3. `year>2000`
   `Compare { field: "year", op: Gt, value: Int(2000) }`

4. `dir:util/* !name:mod OR year>2000`
   `Or([ And([ Match{dir, Glob("util/*")}, Not(Compare{name, Eq, Text("mod")}) ]), Compare{year, Gt, Int(2000)} ])`

5. `tracker module editor`
   `FullText("tracker module editor")`

6. `name:Deluxe* version:1.2`
   `And([ Match{name, Glob("Deluxe*")}, Compare{version, Eq, Text("1.2")} ])`

7. `in:'tracker candidates'`
   `InSelection(Named("tracker candidates"))`

8. `marked !size<10k`
   `And([ InSelection(Marked), Not(Compare{size, Lt, Int(10240)}) ])`

9. `similar:'tracker module editor' > 0.82`
   `Similar { text: "tracker module editor", threshold: 0.82 }`

10. `dir:mus/* (year<1995 OR year>2000)`
    `And([ Match{dir, Glob("mus/*")}, Or([ Compare{year, Lt, Int(1995)}, Compare{year, Gt, Int(2000)} ]) ])`

11. `!(dir:mus/* OR dir:cla/*)`
    `Not(Or([ Match{dir, Glob("mus/*")}, Match{dir, Glob("cla/*")} ]))`

12. `date>2020-01-01`
    `Compare { field: "date", op: Gt, value: Date("2020-01-01") }`

13. `description:~'demo'`
    `Match { field: "description", pattern: Prefix("demo") }`

14. `version!=1.0`
    `Compare { field: "version", op: Ne, value: Text("1.0") }`

15. `file:*.lha`
    `Match { field: "file", pattern: Glob("*.lha") }`

## Malformed inputs

Each error names the offending field or token and carries a byte span
`[start, end)` into the source.

| Input | Error | Span |
|---|---|---|
| `siz:100` | unknown field `'siz'`; nearest known field: `size` | `0..3` (`siz`) |
| `type:mod` | unknown field `'type'` | `0..4` (`type`) — the doc's own recorded gap (`docs/query-ir.md`, "Deliberately absent") surfacing as a parse error, not silently resolved to anything |
| `dir:util/* (year<2000` | unbalanced `(` | `11..12` (the open paren) |
| `size<` | expected a value after `<` | `5..5` (end of input) |
| `in:'tracker` | unterminated string literal | `3..11` (from the opening `'` to end of input) |
| `size:~'foo'` | field `'size'` does not support glob/prefix matching | `0..4` (`size`) — parses to a well-formed `Match` attempt, rejected by `FieldRegistry::check_match` |

## Deviation from `bam-handoff.md` §11 / `docs/query-ir.md`

§11's own example (`dir:util/* !type:mod OR year>2000`) and `query-ir.md`'s
worked examples 3, 5, and 6 use `type:` and `author:` — neither field is
registered yet (see `docs/query-ir.md`, "Deliberately absent"), so a query
using them cannot reach a successful parse, only `UnknownField`. Worked
example 4 above substitutes `name` for `type` to preserve the example's real
point — juxtaposition binding tighter than `OR` — without a field that
doesn't exist; `type:mod` itself is kept, unchanged, in the malformed-input
table instead of being dropped, so the gap stays documented rather than
disappearing. `author:~'Mustermann'` is not reproduced here; `size:~'foo'`
already covers the same `check_match`-rejection shape with a field that
exists.
