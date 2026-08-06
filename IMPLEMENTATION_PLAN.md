# bam — Implementation Plan

Derived from `bam-handoff.md`. Sequenced per §15, with each step annotated
with the **minimum model** required to implement it, and split so that
mechanical parts can be delegated to cheaper models than the parts that need
judgement.

---

## How to read this

### Model tiers

| Tier | Model | Use for |
|---|---|---|
| **H** | Haiku 4.5 | Mechanical, fully-specified work. No design latitude. Boilerplate, fixtures, DDL transcription, CI config, format conversions, test cases enumerated by someone else. |
| **S** | Sonnet 5 | Ordinary implementation against a written spec. Parsers with a known grammar, HTTP clients, TUI widgets, SQL queries, migrations. Anything where "what correct looks like" is already decided. |
| **O** | Opus 5 | Work where the design *is* the deliverable: interface seams, the DSL, the enrichment/versioning model, correctness-critical binary-format handling, anything whose mistakes are expensive to undo later. |

Rule of thumb applied throughout: **the model tier tracks how expensive a wrong
decision is, not how much code gets typed.** A 40-line file that fixes an
interface everything else depends on is Opus. A 400-line file of table-driven
tests is Haiku.

### Delegation protocol

Every task below has a **Hand over** list. When delegating, pass *only* those
items. Do not paste `bam-handoff.md` wholesale into a subtask — it is 500 lines
of rationale for decisions the subtask does not need to re-litigate.

Concretely: a Haiku task gets the file paths it writes, the exact signatures it
implements, and its acceptance check. It does not get the architecture section.

### Task ID format

`P<phase>.<n>` — referenced by `PROGRESS.md` so the next session can resume by ID.

---

## Phase 0 — Workspace scaffold

Goal: `cargo test` runs green on an empty workspace. Nothing else.

### P0.1 — Cargo workspace skeleton · **H**

Create the workspace root and two member crates.

```
Cargo.toml            # workspace, resolver = "2"
crates/bam-core/      # lib
crates/bam-tui/       # bin, name = "bam"
```

Root `Cargo.toml` declares `[workspace.dependencies]` so member crates pin
versions in one place. Initial shared deps: `serde` (derive), `thiserror`,
`rusqlite` (features: `bundled`, `fts5`).

**Why H:** entirely mechanical, no decisions. The dependency list is given.

**Hand over:** the directory tree above, the dependency list, crate names,
`rust-version = "1.85"`, edition 2024.

**Done when:** `cargo build --workspace` succeeds, `bam --version` prints.

> Deliberately *not* created yet: `bam-gui`, `bam-mcp`, `bam-plugins`. Empty
> crates that exist "for later" are scaffolding that rots. Add each when its
> phase starts.

### P0.2 — CI + rustfmt/clippy config · **H**

One GitHub Actions workflow: `cargo fmt --check`, `cargo clippy -- -D warnings`,
`cargo test --workspace`, on push and PR. Ubuntu only for now.

**Why H:** standard, copy-shaped.

**Hand over:** the four commands, the repo uses `main` as default branch.

**Done when:** the workflow passes on a push to `main`.

> Cross-platform CI (macOS/Windows) waits on the target-platform decision
> (`bam-handoff.md` §14). Adding a Windows runner before anyone has committed
> to Windows support is paying for a decision that hasn't been made.

---

## Phase 1 — INDEX parser, schema, incremental update

The hard core starts here. Everything is testable offline against fixtures —
no network dependency in the test suite.

### P1.1 — Capture real INDEX/RECENT/TREE fixtures · **H**

Download once from a mirror and commit trimmed fixtures to
`crates/bam-core/tests/fixtures/`:

- `index_sample.txt` — ~500 lines from a real `INDEX`, chosen to include the
  awkward cases: very long filenames, descriptions containing the column
  delimiter, non-ASCII bytes, a zero-size entry, the header/preamble lines.
- `recent_sample.txt` — a real `RECENT`.
- `tree_sample.txt` — a real `TREE`.

Also commit `fixtures/README.md` recording the mirror URL and the fetch date.

**Why H:** fetch, trim, commit. The *selection criteria* are given above, so no
judgement needed beyond following them.

**Hand over:** mirror base URL `https://ftp.fau.de/aminet/`, the three
filenames, the awkward-case list, the target directory.

**Done when:** the three fixtures exist and `index_sample.txt` demonstrably
contains at least one instance of each listed awkward case.

> Do this before writing the parser. A parser written against a remembered
> format and then tested against reality is a rewrite; the fixture is the spec.

### P1.2 — SQLite schema: landing + normalized · **O**

Write `crates/bam-core/src/schema.rs` (or `migrations/0001_init.sql`) defining
both layers.

Landing — append-only, exactly what the origin said:

```sql
CREATE TABLE landing_index_line (
  id            INTEGER PRIMARY KEY,
  fetched_at    TEXT NOT NULL,      -- RFC3339, when this INDEX was retrieved
  source_url    TEXT NOT NULL,
  line_no       INTEGER NOT NULL,
  raw           BLOB NOT NULL       -- bytes, NOT text: encoding is not yet known
);
```

Normalized — derived, droppable, rebuildable from landing with no network:

```sql
CREATE TABLE package (
  id             INTEGER PRIMARY KEY,
  dir            TEXT NOT NULL,     -- 'util/misc'
  file           TEXT NOT NULL,     -- 'Foo-1.2.lha'
  name           TEXT NOT NULL,     -- 'Foo'  canonical, version stripped
  version        TEXT,              -- '1.2'  NULL when unparseable
  size_bytes     INTEGER,
  uploaded_on    TEXT,              -- ISO date
  date_precision TEXT NOT NULL,     -- 'week' | 'exact' — see note below
  description    TEXT,
  landing_id     INTEGER NOT NULL REFERENCES landing_index_line(id),
  UNIQUE(dir, file)
);
```

Two things to get right here, both expensive to change later:

1. **`raw` is a BLOB, not TEXT.** Aminet INDEX lines are not reliably UTF-8
   (§13). Storing them as TEXT forces a lossy decode at ingest time and
   destroys the whole point of the landing layer.
2. **`date_precision`.** Aminet's INDEX gives *age in weeks* relative to the
   INDEX generation date, so a derived date is ±1 week. Directory listings and
   readme headers can later supply an exact date for the same package. Without
   this column the two are indistinguishable and "sort by date" quietly lies.
   The upgrade path is one-directional: `week` may be overwritten by `exact`,
   never the reverse.

Also define the `enrichment` table now (`bam-handoff.md` §5.2) — it is
referenced by every later phase and its shape (`producer_version` for selective
invalidation) is a real design decision:

```sql
CREATE TABLE enrichment (
  package_id       INTEGER NOT NULL REFERENCES package(id),
  kind             TEXT NOT NULL,     -- 'readme_header' | 'inventory' | ...
  producer_version INTEGER NOT NULL,
  produced_at      TEXT NOT NULL,
  payload          TEXT NOT NULL,     -- JSON
  PRIMARY KEY (package_id, kind)
);
```

**Why O:** this schema is the foundation every subsequent phase writes against.
The BLOB-vs-TEXT and `date_precision` calls are exactly the kind of thing a
cheaper model smooths over into a plausible-looking schema that costs a
migration and a re-ingest to fix.

**Hand over:** `bam-handoff.md` §5.1, §5.2, §13 (encoding paragraph only), and
the fixture from P1.1.

**Done when:** a migration applies to a fresh DB and a test asserts round-trip
insert/select on each table.

### P1.3 — Migration runner · **H**

Numbered `.sql` files in `migrations/`, applied in order, tracked in a
`schema_version` table. Embed with `include_str!`.

**Why H:** ~30 lines, thoroughly conventional.

**Hand over:** the `migrations/` path convention, "use `user_version` pragma or
a `schema_version` table", "no down-migrations".

**Done when:** applying twice is a no-op; a test proves it.

> Skipped: `refinery`/`sqlx-migrate`. A loop over `include_str!`ed files is
> less code than the dependency's configuration. Add one when migrations need
> branching or rollback, which is not now.

### P1.4 — INDEX line parser · **S**

`parse_index_line(raw: &[u8]) -> Result<IndexRecord, ParseError>` in
`crates/bam-core/src/ingest/index.rs`.

The INDEX is column-aligned, not delimiter-separated — descriptions contain
whitespace runs freely, so splitting on whitespace is wrong. Parse by byte
offset, derived from the header row, with a fallback to fixed offsets.

Return byte slices / `Cow`, not `String`: decoding is a separate step (P1.5),
because the landing layer must keep the original bytes.

**Why S:** the grammar is fully determined by the fixture from P1.1. This is
careful implementation against a known target, not design.

**Hand over:** the `index_sample.txt` fixture, the `IndexRecord` field list
(file, dir, size, age, description — all as raw byte ranges), the
column-offset-not-whitespace constraint, and the "return borrowed bytes"
requirement.

**Done when:** every line of the fixture parses, and the awkward cases from
P1.1 each have a named test asserting the expected field split.

### P1.5 — Charset decode helper · **S**

`decode(bytes: &[u8]) -> (String, &'static Encoding)` using `chardetng` to
detect and `encoding_rs` to decode, defaulting to ISO-8859-1 when detection is
low-confidence (Aminet's de-facto encoding).

Store the detected encoding label alongside the text wherever it is persisted,
so a later correction does not require a re-fetch (§13).

**Why S:** two well-documented crates wired together, but the low-confidence
fallback and "persist the label" requirement are easy to omit if the task is
handed to a model that will just call `String::from_utf8_lossy`.

**Hand over:** §13's encoding paragraph, the signature, the ISO-8859-1 default,
and: `chardetng` needs the full text, not a prefix, for decent accuracy.

**Done when:** a test decodes a known ISO-8859-1 byte sequence containing `ö`
correctly, and a UTF-8 one correctly, without a hardcoded per-input branch.

### P1.6 — Normalizer: landing → package rows · **S**

Read landing rows, apply P1.4 + P1.5, derive:

- `size_bytes` from `"134K"` / `"1.2M"` (Aminet uses K/M suffixes, base 1024).
- `uploaded_on` from age-in-weeks + the landing row's `fetched_at`, with
  `date_precision = 'week'`.
- `name` + `version` split from the filename (`Foo-1.2.lha` → `Foo`, `1.2`).
  Aminet naming is inconsistent; when the split is ambiguous, put the whole
  stem in `name` and leave `version` NULL rather than guessing.

Idempotent: running it twice produces the same rows. Rebuilding from scratch
must require no network access — that is the entire justification for the
landing layer, so there is a test for it.

**Why S:** the transformations are specified. The one judgement call
(ambiguous version split → prefer NULL over a guess) is stated here.

**Hand over:** the target `package` schema from P1.2, the three derivation
rules above, the idempotency and offline-rebuild requirements.

**Done when:** `normalize()` twice over the fixture yields identical rows;
dropping `package` and re-normalizing restores it byte-identically with the
network unavailable.

### P1.7 — Size-suffix and version-split unit tests · **H**

Table-driven tests for the two pure functions from P1.6. Cover: `0`, `1K`,
`999K`, `1.2M`, missing suffix, garbage input; and version splits for
`Foo-1.2.lha`, `Foo1.2.lha`, `Foo.lha`, `Foo-2.0beta.lha`, `Mod.Foo.lha`
(a `mods/` naming convention where the leading `Mod.` is not a version).

**Why H:** the functions exist and the cases are enumerated. Pure input→output
transcription.

**Hand over:** the two function signatures and the case list above. Nothing else.

**Done when:** tests pass, or a case fails and is reported back rather than the
test being adjusted to match the buggy output.

> That last clause matters when delegating tests: a cheap model asked to make
> tests pass will happily rewrite the expectation. State the expected value in
> the handover so it cannot.

### P1.8 — RECENT-based incremental update · **S**

`RECENT` has the same line format as `INDEX` but only new uploads. Ingest it
into landing, normalize, upsert by `(dir, file)`.

**Why S:** reuses P1.4/P1.6 wholesale; the new part is the upsert and the
"which packages changed" return value the TUI will later use.

**Hand over:** "same format as INDEX, reuse the parser", the `(dir, file)`
uniqueness key, the `recent_sample.txt` fixture.

**Done when:** ingesting `RECENT` after `INDEX` adds only genuinely new rows
and leaves existing ones untouched.

### P1.9 — HTTP fetch of INDEX/RECENT with gzip · **S**

`reqwest` GET of `INDEX.gz` / `RECENT.gz`, gunzip, hand bytes to the ingest
path. Descriptive `User-Agent` with contact info (§16). Conditional GET via
`ETag`/`If-Modified-Since`, stored per URL, so a repeat run of an unchanged
`INDEX` costs one 304.

**Why S:** ordinary HTTP client work; the politeness requirements are spelled
out in the handoff and repeated here.

**Hand over:** §7's politeness list (rate limit is not needed for these three
files — it becomes relevant in Phase 3), the two URLs, "store ETag per URL for
reuse", the `User-Agent` format.

**Done when:** a real fetch populates landing; a second immediate run logs a
304 and inserts nothing.

### P1.10 — `bam ingest` CLI command · **S**

Wire P1.9 → P1.2 → P1.6 behind one subcommand with `--offline` (fixtures only)
and `--rebuild-normalized` (skip fetch, re-derive from landing).

**Why S:** small, but it is the first place the `ProgressSink` seam from §8
gets used — no `println!` inside `bam-core`, the CLI supplies the sink. Getting
that boundary right the first time is cheaper than retrofitting it.

**Hand over:** §8 rules 1 and 2 (ProgressSink, CancellationToken) — not the
whole MCP section, the two rules — plus the three flags.

**Done when:** `bam ingest --offline` populates a DB from fixtures with a
progress bar rendered by the CLI, and `grep -rn 'println!' crates/bam-core/src`
is empty.

**Phase 1 exit:** ~84,000 packages in SQLite from a real INDEX, rebuildable
offline, incrementally updatable. This is the milestone worth pausing on.

---

## Phase 2 — TUI and the filter DSL

Sequenced second per §15 because it validates the data model against actual use
rather than against assumptions.

### P2.1 — DSL grammar definition · **O**

Define the grammar formally, once, in `docs/dsl.md` — it is the source for four
downstream artifacts (parser, SQL compiler, GBNF for llama.cpp, JSON Schema for
cloud APIs; §10, §11).

Scope for v1:

```
query   := or_expr
or_expr := and_expr ( 'OR' and_expr )*
and_expr:= unary ( ' ' unary )*          # juxtaposition is AND
unary   := '!'? atom
atom    := '(' query ')' | term
term    := field ':' pattern | field op value | bareword
op      := '<' | '>' | '<=' | '>='
field   := 'dir'|'file'|'name'|'author'|'type'|'size'|'year'|'desc'
```

`bareword` is a full-text term over description+readme. Values support `*`
globs and `k`/`M` size suffixes.

Decisions to make here and record with rationale:

- Whether `similar:'...' > 0.82` (vector predicates, §11.1) is in the grammar
  now or added later. **Recommendation: reserve the syntax, reject it at
  compile time with "not yet supported".** Grammar changes invalidate the GBNF
  and every few-shot prompt example; a reserved keyword costs nothing now and
  avoids a breaking change in Phase 6.
- Precedence and whether juxtaposition-as-AND binds tighter than `OR`. It must,
  or `dir:util/* !type:mod OR year>2000` parses surprisingly.

**Why O:** this grammar is the contract between the TUI, the SQL compiler, the
LLM prompt, the highlight rules, and eventually the MCP tool schema. Five
consumers, and changing it later means changing all five plus any query the
user has saved. This is the highest-leverage single document in the project.

**Hand over:** §11 and §11.1 in full, §10's grammar-constraint paragraph, and
the `package` schema from P1.2 (the field list must map to real columns).

**Done when:** `docs/dsl.md` exists with the grammar, precedence table, ~15
worked examples spanning every construct, and a short "rejected alternatives"
note.

### P2.2 — DSL parser · **S**

Hand-rolled tokenizer + precedence-climbing parser producing an AST, in
`crates/bam-core/src/dsl/parse.rs`. Errors carry byte spans.

**Why S:** the grammar is now fully written down (P2.1), so this is
implementation. Spans matter because §11 requires showing the user an editable,
correctable query — an error without a position is not correctable.

**Hand over:** `docs/dsl.md`, the AST type definition, "errors must carry byte
spans".

**Done when:** all 15 examples from `docs/dsl.md` parse to the documented AST,
and each malformed input in a small error-case list reports a span pointing at
the offending token.

> Skipped: `nom`. This grammar is ~10 productions and precise error spans are a
> hard requirement; a hand-rolled precedence climber is shorter here than the
> nom combinators plus the error-mapping layer needed to get spans out of it.
> `nom` still earns its place for AmigaGuide (P7.x), which is genuinely
> line-command-heavy.

### P2.3 — DSL → SQL compiler · **O**

AST → parameterized SQL over `package` (+ FTS5 once Phase 3 lands). Every
literal becomes a bound parameter — no string interpolation anywhere, which is
what makes LLM-generated queries safe by construction (§11).

The non-obvious parts: glob patterns map to `GLOB` not `LIKE` (case sensitivity
differs and Aminet paths are case-significant); `year>2000` must respect
`date_precision`; a bareword term needs a different join depending on whether
FTS5 is populated yet.

**Why O:** this is the injection boundary. §11's entire safety argument —
"the LLM never emits SQL" — rests on this file being right. It also sets the
query shape that indexes get designed around.

**Hand over:** the AST type, the `package` schema, §11's safety rationale, the
three non-obvious points above.

**Done when:** every example in `docs/dsl.md` compiles to SQL that executes
against the Phase 1 DB and returns plausible rows; a test asserts that a query
containing `'; DROP TABLE package; --` inside a value is bound, not
interpolated, and the table survives.

### P2.4 — Query API in `bam-core::api` · **O**

The use-case layer from §8: `search_packages(SearchRequest) -> SearchResponse`,
`get_package(...)`, `list_categories(...)`. Serializable request/response
types, `CancellationToken` parameter, `ProgressSink` where relevant.

**Why O:** this is the three-adapter seam. Get it wrong and `bam-mcp` is a
rewrite instead of a thin adapter — which is the one thing §8 exists to
prevent. Small file, large consequences.

**Hand over:** §8 in full (this is the task it was written for), the DSL
compiler signature from P2.3.

**Done when:** the types derive `Serialize`/`Deserialize`/`JsonSchema`, no
function in the module touches stdout, and every long-running one takes a
cancellation token.

### P2.5 — TUI shell: layout, event loop, list widget · **S**

`ratatui` + `crossterm`. A scrollable virtualized package list, a query input
line, a detail pane. Only the visible window is rendered and only visible rows
are queried (§11.1's evaluation-cost note).

**Why S:** conventional ratatui work. The virtualization requirement is stated,
so it is spec-following.

**Hand over:** the `api::search_packages` signature from P2.4, the three-pane
layout, "render only the visible window", crossterm-for-Windows-compat note.

**Done when:** browsing 84,000 rows scrolls without perceptible lag and memory
does not scale with result-set size.

### P2.6 — Live query input with error display · **S**

Type a DSL query, see parse errors inline with the span underlined, results
update on a debounce. Invalid query keeps the last valid result set rather than
blanking the list.

**Why S:** straightforward given P2.2's spans and P2.5's list.

**Hand over:** the parser's error type with spans, the debounce interval
(150ms), the keep-last-valid-results rule.

**Done when:** typing `dir:util/* size>` shows an error under the trailing
`>` and the previous results remain visible.

### P2.7 — Key bindings + help overlay · **H**

Vim-ish bindings (`j`/`k`/`gg`/`G`/`/`), `?` opens a help overlay listing them,
`q` quits. Bindings in one table that the overlay renders from, so they cannot
drift apart.

**Why H:** a keymap table and a widget that prints it.

**Hand over:** the binding list, "overlay renders from the same table",
the ratatui version in use.

**Done when:** `?` lists exactly the active bindings.

### P2.8 — Semantic token → ratatui `Style` mapping · **S**

Core emits tokens (`gutter: "user"`, `background: "accent-subtle"`); the TUI
maps them to styles and gutter characters (§11.1). One mapping table.
Conflict resolution: background exclusive by highest priority, gutters/badges
stack capped at 3.

**Why S:** the model is fully described in §11.1; this implements it.

**Hand over:** §11.1's token model and conflict rules only — not the plugin
half of that section, which is Phase 6+.

**Done when:** two rules matching one row resolve deterministically and a test
pins the outcome.

### P2.9 — Highlight rules from `bam.toml` + hot reload · **S**

Parse `[[highlight]]` blocks (§11.1's TOML shape), compile each `when` with the
DSL parser, evaluate against visible rows. Watch the file and reload on change
— §11.1 calls the restart-per-tweak loop out explicitly as friction to avoid.

**Why S:** TOML parse + reuse P2.2 + `notify` for the watch.

**Hand over:** the TOML example from §11.1, the DSL parser signature, "hot
reload, debounce the watcher, a rule that fails to compile is reported in the
UI and skipped — it does not take down the app".

**Done when:** editing `bam.toml` while running updates the highlighting
without a restart, and a syntactically broken rule shows an error instead of
crashing.

**Phase 2 exit:** a usable daily-driver TUI. Everything after this is additive.

---

## Phase 3 — Readme harvesting and full-text search

### P3.1 — `fetch_queue` schema + claim/complete operations · **S**

The table from §7, plus the operations a worker needs: atomically claim the
next due item (so concurrent workers cannot double-fetch), mark
success/failure, schedule a retry.

**Why S:** the schema is given; the atomic-claim pattern
(`UPDATE ... RETURNING` with a status transition) is standard SQLite.

**Hand over:** §7's table definition, "claim must be atomic under concurrent
workers", the retry-scheduling fields.

**Done when:** two threads claiming simultaneously get different rows; a test
proves it.

### P3.2 — Token-bucket rate limiter · **H**

~2 req/s, configurable. Pure logic over an injected clock so it is testable
without sleeping.

**Why H:** a well-known ~40-line algorithm with an explicit testability
constraint.

**Hand over:** "token bucket, configurable rate and burst, take a clock as a
parameter rather than calling `Instant::now` internally".

**Done when:** a test drives 100 requests through a fake clock and asserts the
observed rate, in milliseconds of wall time.

### P3.3 — Background fetch worker · **S**

Pulls from the queue, honours the rate limiter, conditional GET via stored
ETag, exponential backoff on 429/5xx, respects `robots.txt`, descriptive
`User-Agent`. Priority boost for whatever the user is currently viewing so
browsing stays responsive under a bulk run (§7).

**Why S:** every requirement is enumerated in §7. Assembly against a checklist.

**Hand over:** §7 in full — it is exactly one task's worth of spec — plus the
queue operations from P3.1 and the limiter from P3.2.

**Done when:** a 1,000-readme run against a real mirror completes at the
configured rate with no 429s, and interrupting mid-run resumes without
re-fetching completed items.

> Before running the full 84,000-item harvest, ask about mirror rsync access
> (§14). A single `rsync --include='*.readme'` pass is dramatically more
> mirror-friendly than 84,000 HTTP requests, and the answer changes whether
> this worker is the bulk path or only the incremental one. Worth an email
> before worth 12 hours of someone else's bandwidth.

### P3.4 — Readme landing storage · **S**

Store raw bytes + detected encoding (P1.5) + fetch timestamp + source URL in a
`landing_readme` table. Bytes, not text, for the same reason as P1.2.

**Why S:** mirrors the P1.2 pattern already established.

**Hand over:** the `landing_index_line` table as the pattern to follow, the
decode helper signature.

**Done when:** a fetched readme round-trips to its original bytes exactly.

### P3.5 — Readme header parser · **S**

Aminet readmes carry a semi-standardized header block (`Short:`, `Author:`,
`Uploader:`, `Type:`, `Version:`, `Requires:`, `Distribution:`). Parse into the
`enrichment` table with `kind = 'readme_header'`, `producer_version = 1`.

"Semi-standardized" is doing real work in that sentence — field order varies,
some files omit the block, some use different capitalization, some wrap values
across lines. Parse leniently: extract what is recognizable, record nothing for
the rest, never fail the whole file over one bad field.

**Why S:** messy but bounded, and the leniency rule is stated rather than left
to judgement.

**Hand over:** the field list, the leniency rule, the `enrichment` row shape,
and ~20 real readmes sampled across categories as fixtures.

**Done when:** the 20 fixtures parse, with a test recording how many fields
each yielded — so a later parser improvement shows up as a diff in that count.

### P3.6 — FTS5 index over description + readme · **S**

External-content FTS5 table, populated from `package.description` and readme
text, with triggers or an explicit rebuild step. Wire the DSL's `bareword` and
`desc:` terms (P2.3) to it.

**Why S:** FTS5 is well-documented; the integration point is already defined by
the compiler.

**Hand over:** the FTS5 external-content pattern, which columns feed it, the
compiler's bareword hook from P2.3, "provide an explicit `rebuild` path — do
not rely solely on triggers, because normalized rows get bulk-rebuilt (P1.6)".

**Done when:** searching a distinctive word from a known readme finds exactly
that package, and a full rebuild after `DROP`ping the FTS table restores it.

### P3.7 — Prioritize readmes for filtered/visible entries · **S**

§7's requirement: every entry surviving the active filters gets its readme
queued. Enqueue on query execution, boost priority for the visible window.

**Why S:** connects two existing pieces.

**Hand over:** the queue's priority column semantics, the search API from P2.4,
"enqueue the filtered set, boost the visible window".

**Done when:** running a query queues its results and the visible ones arrive
first.

**Phase 3 exit:** full-text search over real readme content.

---

## Phase 4 — Archive cache, extraction, `.uaem`

### P4.1 — Content-addressed blob store · **S**

BLAKE3-addressed files at `cache/blobs/aa/bb/<full-hash>`, write-to-temp then
rename (§6). `blobs(hash, size, last_used, pinned)` table, `package.archive_hash`
foreign key.

**Why S:** §6 specifies the layout, the atomicity approach, and the table.

**Hand over:** §6's first half (through "LRU eviction"), the two-level fanout
path scheme, "rename only after the full body is written and hashed".

**Done when:** an interrupted download leaves no file under a real hash name,
and storing identical bytes twice yields one blob with two package references.

### P4.2 — LRU eviction with pinning · **S**

Evict least-recently-used unpinned blobs down to a configured budget. Critically
(§6): enrichment rows survive eviction — otherwise re-fetching means paying for
LLM processing twice.

**Why S:** simple policy, but that last constraint is the kind of thing an
eviction routine written from the obvious mental model deletes by accident.

**Hand over:** the `blobs` table, "budget in bytes, configurable", and the
enrichment-survives rule stated as a hard invariant.

**Done when:** evicting to a small budget removes blobs, keeps pinned ones, and
leaves every `enrichment` row intact — asserted by a test.

### P4.3 — `Unpacker` trait + `unar` backend · **S**

Trait with an out-of-process implementation shelling out to `unar` (§4). Detect
availability at startup and degrade with a clear message rather than failing
opaquely at first use.

**Why S:** process spawning with a defined interface.

**Hand over:** §4's unpacker paragraph (the licensing rationale is context the
implementer does not need — just "out of process, via `unar`"), the trait
signature, "probe availability at startup".

**Done when:** an `.lha` and an `.lzx` both extract, and with `unar` absent the
error names the missing binary and how to install it.

> One implementation, one trait — normally a ponytail violation. It earns the
> trait here because `lhasa`-as-a-library is a real planned second
> implementation (§4) and because the out-of-process boundary needs to be
> mockable in tests that cannot depend on `unar` being installed on CI.

### P4.4 — LHA extended-header reader · **O**

Read protection bits (`HSPARWED`) and file comments from LHA level-1/level-2
extended headers (§12.1). This is binary format work: header level detection,
per-level offset differences, extended-header chaining, and the Amiga-specific
extension records.

**Why O:** binary parsing with no schema to check against and failure modes
that are silent — wrong offsets yield plausible-looking garbage rather than an
error. §12.1 makes this the difference between "content is viewable" and
"content actually boots", so a subtly wrong reader is worse than none.

**Hand over:** §12.1 in full, the LHA header format specification (level 0/1/2
layouts), and two real `.lha` fixtures known to carry S-bit scripts.

**Done when:** the S-bit and E-bit are read correctly from both fixtures,
verified against `lha -v` output as an independent reference.

### P4.5 — `.uaem` sidecar writer · **H**

Given the attributes from P4.4, write `Foo.uaem` next to `Foo`:

```
----rwed 2001-03-11 22:15:00.00 Some comment
```

**Why H:** pure formatting, and the exact output format is given.

**Hand over:** the format line above, the flag-character order (`hsparwed`,
lowercase = set), "timestamp is the archive's mtime, fractional part is
hundredths", the attribute struct from P4.4.

**Done when:** a known attribute set formats to a byte-exact expected string.

### P4.6 — Archive inventory enrichment · **S**

Extract to a temp dir, walk it, record file list with paths/sizes/detected
types into `enrichment` with `kind = 'inventory'`, `producer_version = 1`.
Discard the temp extraction; the inventory is the durable artifact.

**Why S:** composes P4.3 with the enrichment table.

**Hand over:** the unpacker trait, the enrichment row shape, "temp dir is
discarded, inventory persists".

**Done when:** an archive's inventory survives blob eviction (ties back to
P4.2's invariant).

**Phase 4 exit:** §15's "hard core" is complete. Reassess sequencing here —
5–7 are explicitly resequenceable.

---

## Phase 5 — Emulator launcher

**Blocked on §14's emulator decision.** Do not start this phase until the
target is chosen; FS-UAE and vAmiga differ enough in directory-volume support
that building against the wrong one is rework, not adaptation.

### P5.1 — Emulator config generation · **S**

Extract an archive to a scratch directory, generate the emulator config
pointing a directory volume at it, launch the process.

**Why S:** config templating plus a process spawn, once the target is fixed.

**Hand over:** the chosen emulator's config format, the extraction path from
P4.3, the `.uaem` writer from P4.5.

**Done when:** launching an archive containing a script shows the script
running — which is the actual test of whether P4.4/P4.5 worked.

### P5.2 — Launcher config in `bam.toml` · **H**

Emulator binary path, extra arguments, scratch directory. Sensible per-platform
defaults for the binary path.

**Why H:** config struct plus defaults.

**Hand over:** the field list, the existing `bam.toml` parsing setup.

**Done when:** a launch works with no explicit config on a machine with the
emulator installed at its default location.

---

## Phase 6 — LLM layer

### P6.1 — `LlmProvider` trait + OpenAI-compatible implementation · **S**

The trait from §10. One implementation covers Ollama, llama.cpp, and cloud
OpenAI-compatible endpoints — they share a wire format; the differences live in
`capabilities()`.

**Why S:** the trait is already written out in §10; the HTTP client is ordinary.

**Hand over:** §10's trait definition, "one impl for all OpenAI-compatible
endpoints, differences expressed via `capabilities()`", base-URL and model
name as config.

**Done when:** the same code path completes against a local Ollama and against
a cloud endpoint, distinguished only by config.

### P6.2 — Grammar generation from the DSL · **O**

Generate GBNF (llama.cpp) and JSON Schema (cloud) from the single grammar
definition in `docs/dsl.md` (P2.1). §10 is explicit that both must derive from
one source — hand-maintaining two representations of one grammar is how they
drift.

**Why O:** this is the mechanism that makes the provider genuinely swappable,
and it requires understanding how GBNF's and JSON Schema's expressiveness
differ (GBNF constrains token-by-token; JSON Schema constrains structure) and
what to do where they do not line up.

**Hand over:** `docs/dsl.md`, §10's capabilities paragraph, "both artifacts
generated from one source, with a test that they accept the same language".

**Done when:** every example in `docs/dsl.md` validates against both generated
artifacts, and a deliberately malformed query is rejected by both.

### P6.3 — DSL query generation prompt · **S**

Prompt template with the TREE category vocabulary, a file-type dictionary, and
few-shot examples (§11). Output is grammar-constrained via P6.2. The generated
DSL is always shown to the user and always editable before it runs — §11's
stated safety net, not an optional nicety.

**Why S:** prompt assembly against a specified recipe.

**Hand over:** §11's "why 7–8B is enough" paragraph (it names exactly what the
prompt must contain), the TREE data from P1.1, the provider trait.

**Done when:** a local 7–8B model produces valid DSL for ten natural-language
test queries, and the TUI shows the result as editable text rather than
executing it directly.

### P6.4 — Embeddings + sqlite-vec · **S**

Add the `sqlite-vec` extension, embed readme text via the provider's `embed()`,
store vectors, wire the reserved `similar:` predicate from P2.1 into the
compiler.

**Why S:** the integration points were all reserved in earlier phases.

**Hand over:** the `similar:` syntax reserved in P2.1, the provider's `embed`
signature, "batch the embedding calls; 84,000 sequential round-trips is a
night of compute for no reason".

**Done when:** a semantic query returns plausibly related packages that a
keyword search for the same phrase misses.

### P6.5 — LLM summaries · **S**

`kind = 'llm_summary'` enrichment from readme + inventory. Rate-limited,
resumable, cost-visible before starting a bulk run.

**Why S:** the enrichment machinery exists; this is another producer.

**Hand over:** the enrichment row shape, the provider trait, "resumable — an
interrupted bulk run must not re-summarize completed packages", §16's cost
warning.

**Done when:** interrupting a 100-package summarization run and restarting
processes only the remainder.

---

## Phase 7 — GUI and visualizations

**Blocked on §14's target-platform decision** (affects Tauri packaging).

### P7.1 — Tauri scaffold + `bam-core` binding · **S**
### P7.2 — Package list and detail views · **S**
### P7.3 — Timeline visualization · **S**
### P7.4 — Per-archive content visualization · **S**
### P7.5 — AmigaGuide parser (`nom` → AST) · **S**

Deliberately low detail: Phase 2's TUI will have taught things about the data
model that should shape this, and planning it in detail now is planning against
what is currently believed rather than what will be known. Expand this phase
when Phase 6 closes.

---

## Deferred — and why

Not scheduled. Each has a concrete trigger for reconsidering it.

| Item | Trigger to build it |
|---|---|
| **`bam-mcp`** | When someone actually wants MCP access. §8's rules (no stdout in core, cancellation tokens, serializable types) are enforced from P1.10 onward, so the adapter stays thin — that is what makes deferring it safe rather than a bet. |
| **WASM plugin system (§9)** | When a second content analyzer exists that cannot live in-tree. One analyzer behind a plugin ABI is an ABI with one consumer; the extension-point design in §9 is sound but nothing yet needs it. |
| **`highlight_provider` plugins (§11.1B)** | When a highlight rule is genuinely inexpressible as a DSL predicate. The DSL path (P2.9) covers the documented examples. |
| **`lhasa` in-process unpacker** | When `unar` process spawn overhead measurably hurts a bulk operation. The trait (P4.3) is already there. |
| **`highlight_hits` materialization** | When visible-window evaluation is measurably too slow — which §11.1 ties specifically to vector predicates, so realistically after P6.4. |
| **Windows/macOS CI** | When the target-platform question (§14) is answered. |

---

## Suggested round sizing

One phase is too big for one session; these are natural stopping points that
leave the tree green:

| Round | Tasks | Ends with |
|---|---|---|
| 1 | P0.1–P0.2, P1.1 | Workspace builds, fixtures committed |
| 2 | P1.2–P1.3 | Schema + migrations |
| 3 | P1.4–P1.7 | Parser + normalizer, tested offline |
| 4 | P1.8–P1.10 | `bam ingest` works end to end |
| 5 | P2.1 | `docs/dsl.md` — design only, no code |
| 6 | P2.2–P2.4 | DSL parses and compiles to SQL |
| 7 | P2.5–P2.9 | Usable TUI |
| 8+ | Phase 3 onward | Reassess after real use |

Round 5 is deliberately a single task with no code. It is the document four
later phases depend on, and bundling it with implementation work is how it ends
up written hastily.

---

## Model tier summary

| Tier | Tasks |
|---|---|
| **Opus** | P1.2, P2.1, P2.3, P2.4, P4.4, P6.2 |
| **Sonnet** | P1.4, P1.5, P1.6, P1.8, P1.9, P1.10, P2.2, P2.5, P2.6, P2.8, P2.9, P3.1, P3.3, P3.4, P3.5, P3.6, P3.7, P4.1, P4.2, P4.3, P4.6, P5.1, P6.1, P6.3, P6.4, P6.5, P7.1–P7.5 |
| **Haiku** | P0.1, P0.2, P1.1, P1.3, P1.7, P2.7, P3.2, P4.5, P5.2 |

Six Opus tasks out of ~45. They cluster where a wrong answer propagates: the
schema, the DSL and its two compilers, the API seam, and the one binary format
whose errors are silent.
