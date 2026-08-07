# Phase 2 progress — Query IR, `bam-dsl`, compiler, API layer, selections

← [PROGRESS.md](../../PROGRESS.md)

Round-by-round log for Phase 2 (Rounds 7–10), extracted from the top-level
progress file to keep that file scannable. Task ids refer to
[`IMPLEMENTATION_PLAN.md`](../../IMPLEMENTATION_PLAN.md) and
[phase-2-query-core.md](../plan/phase-2-query-core.md).

---

## Round 7 — 2026-08-06 · Query IR + `QueryLanguage` trait (P2.1–P2.2)

**Done:**

- **P2.1** — `crates/bam-core/src/query/ir.rs` and `.../registry.rs`, plus
  `docs/query-ir.md` as one artifact per the task's own framing. `Predicate`,
  `CmpOp`, `Value`, `Pattern`, `SelectionRef` verbatim from the phase doc /
  invariant I2. `FieldId` wraps an owned `String`, not `&'static str`: a
  `Predicate` is built from arbitrary parsed or LLM-generated input and must
  round-trip through serde without borrowing from it, and a closed `FieldId`
  enum would make P2.8's "registering a field touches only the registry"
  claim false by construction. `FieldRegistry::resolve` matches name or
  alias; `check_compare` validates a `CmpOp` against `FieldDef.ops`;
  `check_match` validates `Match`/glob against `FieldDef.ty` — only `Text`
  fields permit it, so `size:~'foo'` is rejected at resolve time without a
  separate `matchable` flag. `package_fields()` maps eight fields to P1.2's
  `package` columns (`dir`, `file`, `name`, `version`, `size`/`size_bytes`,
  `date`/`uploaded_on`, `year`, `description`); `type` and `author` from
  `bam-handoff.md` §11's examples are deliberately absent — neither has a
  backing column yet (`type` awaits a derived category, `author` awaits
  Phase 4 harvesting) — recorded in the doc rather than stubbed. `year`
  shares `uploaded_on` with `date`; the doc notes the compiler must also
  consult `date_precision`, per P2.5's own "three non-obvious points." Five
  tests in `tests/query_ir.rs`, matching the five test bullets exactly (the
  operator/match-not-permitted bullet covers both a rejected `Match` on an
  `Int` field and a rejected `CmpOp` on a `Text` field, since the phase doc's
  one example — `size:~'foo'` — is the `Match` case specifically). The doc's
  "worked IR trees for a dozen queries" section includes two queries that
  don't yet compile (`type`/`author`-keyed ones) with the gap stated inline,
  rather than silently substituting a field that doesn't carry the same
  meaning.
- **P2.2** — `crates/bam-core/src/query/lang.rs`: `QueryLanguage` trait,
  `GrammarKind`, `ParseError`, `LanguageRegistry`. `ParseError` carries a
  `span: Option<(usize, usize)>` from this task rather than being added in
  P2.4 — the trait signature is the registered contract, and adding a field
  to it later would be the exact kind of breaking change pluggability (I2/I4)
  exists to avoid; P2.4 fills the span in, it doesn't add the field.
  `LanguageRegistry::get` takes `Option<&str>`, falling back to a
  constructor-supplied default id. Five tests in `tests/query_lang.rs`, two
  hand-rolled stub `QueryLanguage` impls (`EchoLang`, `MuteLang`) local to
  the test file — no real grammar exists yet (P2.3/P2.4), so stubs are
  correct here, not a shortcut.

Both modules are ungated (pure `serde` data plus a trait/registry, no
`rusqlite`), confirmed by the wasm32 `--no-default-features` check. Hit the
same purity-scanner false positive Round 4 flagged: a module-doc comment in
`ir.rs` originally named the excluded dependency by its literal crate name
and tripped P0.4's raw substring scan; reworded to "no database driver
dependency" — the scanner doesn't parse comments separately from code, and
this is now the second time a doc comment discussing invariant I1 has hit
it.

All 47 tests pass (10 new + 37 pre-existing). `cargo fmt --check`, `cargo
clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean.

**No deviations beyond the purity-scanner note above.**

---

## Round 8 — 2026-08-06 · `bam-dsl` grammar + parser + IR → SQL compiler (P2.3–P2.5)

**Done:**

- **P2.3** — `docs/lang-bam-dsl.md`: grammar, precedence table, fifteen
  worked examples with IR trees, and a malformed-input table with expected
  byte spans. Two design points not fully pinned down by the phase doc's
  grammar sketch, resolved and written down rather than guessed at
  implementation time: (1) `field:rhs` is one context-sensitive operator —
  `Match`/`Glob` if `rhs` contains `*`, else `Compare`/`Eq` — matching
  `docs/query-ir.md`'s own worked examples (`name:Deluxe*` vs `version:1.2`)
  where the phase doc's sketch reads as two separate forms; (2) the
  force-match operator is the two-character `:~` (`size:~'foo'`,
  matching `bam-handoff.md` §11.1's `author:~'Mustermann'` and the field
  registry's own error text), not a bare `~` — an early draft used bare `~`
  and was corrected before any code was written against it. Also documented:
  adjacent bareword terms merge into one `FullText` node spanning their
  combined source text (`docs/query-ir.md`'s own example 7,
  `tracker module editor` → one `FullText`, not three ANDed ones) — juxtaposition-as-AND
  has this one exception.
- **P2.4** — `crates/bam-core/src/query/bam_dsl.rs`, a hand-rolled recursive-
  descent parser (`nom` skipped per the phase doc's own note) implementing
  `QueryLanguage` as `BamDsl`. Byte spans on every `ParseError`.
  `FieldRegistry::field_names()` (registry.rs) is a small new accessor —
  every name and alias, for a Levenshtein-distance "nearest field" suggestion
  on `UnknownField`, not exposed before this task. `render` re-serializes to
  canonical `bam-dsl` text (`Eq` compares always render via `:`, other ops via
  their symbol; `Or`/`And` children are parenthesized under a same-or-higher-
  precedence parent so re-parsing can't reflatten a nested tree into a flat
  one) — its round-trip test asserts `parse(render(p)) == p` for each of the
  fifteen built predicates directly, not `render(parse(s)) == s`: byte-
  identical source reproduction isn't the invariant that matters for a UI
  "show and correct the generated query" round-trip, semantic fidelity is.
  Ten tests in `tests/query_bam_dsl.rs` (the phase doc's five groups; the
  fifteen-examples and malformed-input groups are table-driven, one test
  each, rather than fifteen-plus-six separate `#[test]` functions).
- **P2.5** — `crates/bam-core/src/store/compile.rs` (native-gated, inside
  `store::` per I1). `Predicate` → `SELECT id FROM package WHERE ...` plus a
  `Vec<rusqlite::types::Value>`; every literal is `params.push`ed, never
  formatted into the SQL string. `Predicate::InSelection` compiles directly
  to an `EXISTS` subquery over `selection`/`selection_member` (P1.2's
  schema) — see the deviation note below for why this, not
  `FieldRegistry`/`SqlSource`, is where that logic lives. `year>N` detects
  `field.name == "year"` and compiles a ±7-day window against
  `date_precision` (`CAST(strftime('%Y', CASE WHEN date_precision='exact'
  THEN uploaded_on ELSE date(uploaded_on, '±7 days') END) AS INTEGER)` at
  each edge), so a `week`-precision row whose uncertainty window straddles a
  year boundary is excluded rather than guessed into either side.
  `FullText` compiles to `description LIKE ? ESCAPE '\'` with `%`/`_`/`\`
  escaped — Aminet filenames routinely contain `_`, which is a `LIKE`
  wildcard otherwise. `SqlSource::Join` is left as
  `CompileError::UnsupportedJoinSource` — no current field uses it; see the
  deviation note. Six tests in `tests/store_compile.rs`, against a nine-row
  in-memory fixture DB built through P1.2's real insert functions (not
  hand-written SQL): the fourteen executable worked examples (`Similar` is
  compile-rejected by design, tested separately) against hand-computed
  expected id sets, a `'; DROP TABLE package; --` literal proven bound and
  the table proven intact, `GLOB` case-sensitivity, the year/`date_precision`
  boundary, `!(a OR b)` vs. `!a OR b` giving different results, and
  `Similar`'s rejection.

58 tests total (11 new + 47 pre-existing; one pre-existing `#[ignore]`d
real-mirror test not counted as newly run). `cargo fmt --check`, `cargo
clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean.

**Deviations for the next session to know about:**

- `bam-handoff.md` §11's own example and `docs/query-ir.md`'s worked examples
  3, 5, and 6 use `type:`/`author:`, neither of which is a registered field
  (`docs/query-ir.md`, "Deliberately absent," Round 7). A query using them
  can only ever produce `UnknownField`, so they cannot appear in P2.3's
  fifteen *successfully-parsing* examples. Worked example 4 substitutes
  `name` for `type` to preserve the example's real point (juxtaposition
  binding tighter than `OR`); `type:mod` itself is kept, unchanged, in the
  malformed-input table instead of silently vanishing. Flagged per the same
  doc/reality-mismatch convention as Round 3's INDEX-header case and Round
  4's missing size/version expected values.
- P2.8's task text (`docs/plan/phase-2-query-core.md`) describes `in:`/
  `marked` as "two entries in the field registry" resolving to an `EXISTS`
  subquery — but P2.1 (Round 7) already made `InSelection` its own `Predicate`
  variant, and P2.4's parser (this round) already parses `in:'x'`/`marked`
  directly to it, not to a `Compare`/`Match` against a registered field.
  P2.5 therefore compiles `InSelection` on its own, independent of
  `FieldRegistry`. P2.8's own remaining scope, when its round comes, is
  smaller than its task text implies (or may be redundant with what's
  already built) — worth a short reconciliation pass rather than following
  the task text literally, per that task's own instruction to report back
  rather than force a fit.
- `parse_size_bytes` (P1.6, `ingest/normalize.rs`) only recognizes uppercase
  `K`/`M`, correct for real Aminet INDEX data. The DSL's own grammar sketch
  calls for lowercase `k`/`M` in a *typed query value* (`size>100k`), which
  is user input, not INDEX data — `bam_dsl.rs`'s `typed_value` upper-cases
  before calling the shared function rather than forking a second size
  parser for one case difference.

---

## Round 9 — 2026-08-06 · `bam-core::api` use-case layer + selections (P2.6–P2.7)

**Done:**

- **P2.6** — `crates/bam-core/src/store/session.rs` (native-gated, inside
  `store::` per I1 — see the deviation note below for why the actual
  session logic lives there and not in `api::`) defines `Session`: one
  connection, one `FieldRegistry`, an eagerly-created ephemeral *working*
  selection row (`Drop` deletes it, cascading to its members via P1.2's FK —
  a named selection, `ephemeral = 0`, outlives the session), and an
  operation table (`Mutex<HashMap<OperationId, OperationStatus>>`) keyed by
  a session-local counter. `crates/bam-core/src/api/` (`mod.rs`, `types.rs`,
  `query.rs`, `selection.rs`, `ingest.rs`) is the thin, serializable-typed
  layer over it invariant I5 asks for: every request/response type derives
  `Serialize`/`Deserialize`/`schemars::JsonSchema` (new workspace
  dependency, ungated — `query::ir::Predicate`, embedded in
  `SearchPackagesRequest`, lives in the always-wasm-compiled part of the
  crate, confirmed by the `--no-default-features` wasm32 check still
  passing). `CancellationToken` is a new ungated `crate::cancel` module
  (`Arc<AtomicBool>`, two methods) rather than `tokio_util`'s — nothing here
  needed more than "cancel" and "is it cancelled," and a hand-rolled type
  keeps it usable with no async runtime for a future wasm caller.
  `ingest` (the only long-running operation that exists yet) is the vehicle
  proving the `CancellationToken`/`OperationId` rules against real work
  rather than a synthetic op built just to have something to cancel:
  `Session::run_ingest` checks `cancel` once before starting (`ingest::
  run_ingest` itself has only two coarse steps, fetch+land and normalize —
  no finer-grained point to poll mid-flight yet) and records every
  `ProgressEvent` into the operation table under a session-assigned id, so
  `operation_status(id)` answers after the call returns too, for a
  reconnecting client. `ingest::run_ingest` itself changed: it now takes a
  caller-assigned `OperationId` instead of a hardcoded `OperationId(0)` (4
  call sites updated — 3 in `tests/store_ingest.rs`, 1 in `bam-tui/src/
  main.rs`, all passing `OperationId(0)` to keep prior behavior). `Package`
  (P1.2) is reused directly as the API's package response type (added
  `Serialize`/`Deserialize`/`JsonSchema` derives) rather than duplicated
  into a DTO. Five tests in `tests/api_session.rs`.
- **P2.7** — `Session::{mark, unmark, toggle, clear, select_by_query,
  save_as, load, list_selections, delete_selection}`, all operating on the
  working selection P2.6 already built, exposed through `api::selection`
  (bare-`package_id` calls for `mark`/`unmark`/`toggle`/`clear` — one
  primitive argument doesn't earn a named request type the way
  `SelectByQueryRequest` or a by-name lookup does). `save_as` copies the
  working selection's current members into a new named row (`INSERT OR
  IGNORE ... SELECT`); `load` clears the working selection and copies from
  the named one — independent snapshots, not shared storage, so further
  `mark`/`unmark` on the working selection never mutates a saved one.
  `select_by_query`'s four `SelectionMode` variants reuse `mark`/`unmark`/
  `clear` rather than hand-rolling per-mode SQL. Six tests in
  `tests/api_selection.rs`, against real file-backed DBs (not `:memory:` —
  two independent `:memory:` connections can't demonstrate "shares a
  database but not session state," and the drop-cleanup test needs to
  reopen the same file after the `Session` that created it is gone).

69 tests total (11 new + 58 pre-existing). `cargo fmt --check`, `cargo
clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean. Also smoke-tested the `bam`
binary (`cargo run -p bam-tui -- ingest --offline`) against the
`run_ingest` signature change: still reports 501 packages, unchanged
progress output.

**Deviations for the next session to know about:**

- **All DB-touching session/selection code lives in `store::session`, not
  `api::`.** P0.4's purity scanner bans the literal substring `"rusqlite"`
  in any file outside `src/store/` — `Session` (and hence `Connection`,
  `rusqlite::Error`, bound-parameter queries) can't be named from `api/*.rs`
  without tripping it. `api::` therefore never touches SQL directly; it
  only calls `Session`'s plain-Rust methods and adapts typed request/
  response structs around them. This reads as the more correct shape
  anyway (I1's "rusqlite confined to `store::*`" already implied a
  session type living there), but it means `bam-core::api`'s own module
  doc originally tripped the same purity scanner by *naming*
  `println!`/`eprintln!` literally while describing rule 1 — same class of
  false positive Round 4 and Round 7 hit for `rusqlite`; reworded rather
  than special-cased in the scanner.
- Only `search_packages`, `get_package`, `list_categories`, and the P2.7
  selection ops got typed request/response wrappers, plus `start_ingest`/
  `operation_status` to give I5's cancellation/`OperationId` rules something
  real to prove themselves against — ingest isn't named in P2.6's task
  text, but no other long-running operation exists yet to exercise those
  two rules honestly.

---

## Round 10 — 2026-08-06 · `in:`/`marked` reconciliation (P2.8) — Phase 2 exit

**Done:**

- **P2.8** — Confirmed, by reading `query/registry.rs`, `store/compile.rs`, and
  `query/bam_dsl.rs` before touching anything, that Round 8/9's deviation note
  was right: `InSelection` is its own `Predicate` variant, the parser already
  parses `in:'x'`/`marked` straight to it, and the compiler already compiles
  it independent of `FieldRegistry`. There is nothing to add to the registry —
  P2.8's literal task text ("add two entries to the field registry") doesn't
  apply, and adding stub entries anyway would misrepresent how resolution
  actually works. Two of the task's three test bullets were already true and
  already covered (`tests/store_compile.rs::worked_examples_compile_and_return_expected_ids`
  exercises both `in:'tracker candidates'` and `marked !size<10k`). The third
  wasn't: `in:'nonexistent'` didn't error, it silently compiled to an `EXISTS`
  subquery that matches zero rows — `store::compile::compile` has no
  `Connection` to check existence with. Fixed by adding
  `Session::check_named_selections_exist` (`store/session.rs`), a small
  recursive walk over the predicate tree run once in `matching_ids` (shared by
  `search_packages` and `select_by_query`, so both routes get the check
  without duplicating it), erroring with the existing `SessionError::
  UnknownSelection` the same way `load` already does for the same condition.
  One new test, `tests/api_selection.rs::in_selection_naming_an_unknown_selection_errors`.
  The task's own acceptance criterion — "no file under `query/lang/` and no
  file in the compiler is modified" — holds: the fix lives in `store/
  session.rs`, the session layer, not the parser or `store/compile.rs`.

69 tests total (1 new + 68 pre-existing — Round 9's own "69 tests total" was
itself off by one; verified here directly with `git stash`/`cargo test`
rather than trusted). `cargo fmt --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, and the wasm32 `--no-default-features` check
all clean.

**Deviations for the next session to know about:**
- P2.8 turned out to be exactly the smaller pass Round 8's deviation note
  predicted, not the two-registry-entry task the phase doc describes.
- Round 9's stated test count ("69 tests total") was off by one (actual
  pre-existing count was 68) — no code discrepancy, just a miscount in that
  round's own report. Noted in case a future round's running total looks off
  by one again.

**Phase 2 exit reached.**

---
