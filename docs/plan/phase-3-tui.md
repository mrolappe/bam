# Phase 3 — TUI

← [Implementation plan index](../../IMPLEMENTATION_PLAN.md)

`ratatui` + `crossterm`. The primary daily-use surface, and the thing that
validates the Phase 1 data model against actual use rather than assumption.

---

### P3.1 — Input model · **O**

Deliverable: `docs/input-model.md` plus the types in
`crates/bam-tui/src/input/mod.rs`.

Invariant **I6**: configurable bindings plus a future vim grammar mean input is
a pending-state resolver from the first commit, never a flat `match key`.

```
[count] [operator] {motion | object | command}
```

```rust
enum Mode { Normal, Insert, Visual, Command }
enum Resolution { Pending, Resolved(Action), Rejected(Reason) }
```

**v1 registers modes and motions only — zero operators, zero objects.** The
grammar slot exists and goes unused. That costs a few dozen lines now against
rewriting the entire input layer later.

The document must also specify the **future** object model, so nothing built
now precludes it: objects resolve to **sets of packages**, not text ranges —
`ic`/`ac` inner/around category, `is` current selection, `ir` current result
set. An operator applied to an object produces a selection, which is why this
composes directly with I7 rather than being a parallel mechanism.

**Axis assumption to record explicitly:** `0` and `$` are *horizontal* motions
— line start/end in the query input, leftmost/rightmost column in a wide table
— with `gg`/`G` covering the vertical axis. Flagged as an assumption because
`0`/`$` could equally have meant first/last row; bindings are configurable, so
it is cheap to revisit.

**Tests first:**
- `Mode`, `Action` and the keymap types round-trip serde (bindings come from
  config, so they must deserialize).
- A count prefix parses: `5` then `j` yields `MoveDown(5)`.
- A prefix key yields `Pending`: `g` alone resolves to nothing.
- An unmatched sequence yields `Rejected` and clears pending state.

**Why O:** every keystroke in the application routes through this, and it must
accommodate a grammar that will not be built for a long time. Getting the shape
wrong means rewriting the input layer of a working TUI — the expensive kind of
wrong, arrived at long after the decision.

**Hand over:** invariants I6 and I7, the binding list from P3.3, §11.1's
selection interaction.

**Done when:** the four tests pass and the document covers modes, the resolver
grammar, the keymap config format, the future object model, and the axis
assumption.

---

### P3.2 — Input resolver state machine · **S**

Implement the resolver from P3.1. Pending state, count accumulation, mode
transitions, timeout-free (a prefix stays pending until resolved or cleared —
no ambiguity timers, which are a source of surprise).

**Tests first:**
- `j` → `MoveDown(1)`; `5j` → `MoveDown(5)`; `12G` → `GoToRow(12)`.
- `g` → `Pending`; `gg` → `GoTop`; `gx` → `Rejected` with pending cleared.
- `Esc` clears pending state from any partial sequence.
- Mode transitions: `v` enters Visual, `Esc` leaves, `:` enters Command,
  `/` enters Insert on the query line.
- A count with no following key remains `Pending` indefinitely.

**Why S:** a state machine against a written specification, with every
transition enumerated above.

**Hand over:** `docs/input-model.md`, the types from P3.1, the five test groups.

**Done when:** all five pass.

---

### P3.3 — Default keymap + user override merge · **H**

One table of default bindings; user overrides from `bam.toml` merged over it.

Defaults: `j` `k` `gg` `G` `0` `$` `ctrl-d` `ctrl-u` `ctrl-f` `ctrl-b` `H` `M`
`L` `n` `N` `/` `?` `:` `space` `v` `Esc` `q`, with counts.

**Tests first:**
- The default table contains every binding listed above.
- A user override of `j` wins over the default.
- An override naming an unknown action errors, naming the action.
- An override can unbind a default.
- A config with no `[keys]` section yields exactly the defaults.

**Why H:** a table plus a merge, with the full binding list and all five cases
given.

**Hand over:** the binding list, the merge semantics, the `bam.toml` parsing
already in place, the five tests.

**Done when:** all five pass.

---

### P3.4 — TUI shell and virtualized list · **S**

Three panes: scrollable package list, query input line, detail pane. **Only the
visible window is queried and rendered** — §11.1's evaluation-cost note, and
the difference between browsing 84,000 rows and loading them.

**Tests first** (via ratatui's `TestBackend`, which renders into an assertable
buffer):
- Rendering with a counting fake store issues queries for the visible window
  only — scrolling by one row does not re-query the whole set.
- Memory does not scale with result-set size: a 84,000-row result and a
  100-row result hold comparable numbers of package records.
- A buffer snapshot for a small fixture matches expected layout.

**Why S:** conventional ratatui work; the virtualization requirement is stated
rather than left to be inferred.

**Hand over:** `api::search_packages` from P2.6, the three-pane layout, "render
and query the visible window only", `TestBackend` as the test vehicle.

**Done when:** the three tests pass.

---

### P3.5 — Query input line with inline errors · **S**

Type a query, see parse errors inline with the offending span underlined,
results update on a 150 ms debounce. **An invalid query keeps the last valid
result set** rather than blanking the list.

**Tests first:**
- `dir:util/* size>` renders an error marker under the trailing `>`.
- The previous results remain visible while the query is invalid.
- Debounce coalesces rapid keystrokes into one query.
- A valid query replaces results.

**Why S:** straightforward given P2.4's spans and P3.4's list.

**Hand over:** the parser's span-carrying error type, the 150 ms debounce, the
keep-last-valid rule.

**Done when:** the four tests pass.

---

### P3.6 — Selection UI and `:` command line · **S**

Invariant **I7** at the surface. `space` toggles the row under the cursor,
Visual mode marks a range, and the command line supports `:mark <query>`,
`:unmark <query>`, `:save <name>`, `:load <name>`, `:selections`.

Everything routes through P2.7's API — no selection state lives in the TUI.

**Tests first:**
- `space` toggles membership through the core API, and the row's rendering
  updates.
- Visual mode over N rows marks exactly N.
- `:mark dir:mus/*` unions the query's results into the working selection.
- `:save "tracker candidates"` persists, and `:load` in a fresh session
  restores it.
- No selection state is stored in TUI structs — verified by review of the diff.

**Why S:** UI over an API that already exists and is already tested.

**Hand over:** P2.7's operation list, the command syntax above, "no selection
state in the TUI", the five acceptance items.

**Done when:** the four tests pass and the diff review confirms the fifth.

---

### P3.7 — Semantic token → ratatui style · **S**

The core emits semantic tokens (`gutter: "user"`, `background: "accent-subtle"`,
`badge: "XL"`); the TUI maps them to `Style` values and gutter characters. One
mapping table, so theming stays centralized instead of duplicated per frontend
(§11.1).

Marked state (I7) is emitted as a token like any other, so selections render
through this path rather than a special case.

Conflict resolution: background is exclusive, highest `priority` wins; gutters
and badges stack, capped at 3.

**Tests first:**
- Two rules matching one row resolve deterministically — highest priority
  background wins, and the test pins which.
- Equal priorities resolve stably (define the tiebreak; do not leave it to
  hash order).
- Four stacking gutters render three.
- A marked row renders its token through the same path as a highlight rule.
- An unknown token renders as unstyled rather than panicking.

**Why S:** the model is fully described in §11.1; this implements it.

**Hand over:** §11.1's token model and conflict rules — not the plugin half of
that section, which is Phase 8 — plus the five tests.

**Done when:** all five pass.

---

### P3.8 — Highlight rules with hot reload · **S**

Parse `[[highlight]]` blocks from `bam.toml`, compile each `when` through the
query language named by its `lang` key (invariant **I3**), evaluate against
visible rows.

```toml
[[highlight]]
name = "my own uploads"
lang = "bam-dsl"        # optional; falls back to default_query_language
when = "author:~'Mustermann'"
gutter = "user"
priority = 10
```

Watch the file and reload on change — §11.1 names the restart-per-tweak loop as
friction that should not exist.

**Tests first:**
- A rule omitting `lang` uses the configured default.
- A rule naming an explicit language uses it.
- A rule naming an unregistered language errors, naming it.
- A rule whose `when` fails to compile is **reported in the UI and skipped** —
  it does not take down the app or disable the other rules.
- Editing the file while running updates highlighting without a restart.
- The watcher debounces (editors write twice; the reload must not).

**Why S:** TOML parsing plus P2.4 plus `notify`, all against stated rules.

**Hand over:** the TOML shape above, invariant I3, the language registry from
P2.2, the six tests.

**Done when:** all six pass.

---

### P3.9 — Help overlay · **H**

`?` opens an overlay listing the active bindings, rendered **from the same
table P3.3 loads**, so the two cannot drift.

**Tests first:**
- The overlay's binding set equals the active keymap's binding set — assert set
  equality, not a hardcoded list.
- A user override appears in the overlay with the user's key, not the default.
- `?` opens and `Esc`/`q` closes.

**Why H:** a widget that renders a table that already exists.

**Hand over:** the keymap table from P3.3, "render from the same table", the
three tests.

**Done when:** all three pass.

---

**Phase 3 exit:** a usable daily-driver TUI with configurable vim-style
bindings, persistent selections, and hot-reloadable highlighting. Everything
after this is additive.
