# Phase 3 progress — Input model, TUI shell, selections, highlighting, help

← [PROGRESS.md](../../PROGRESS.md)

Round-by-round log for Phase 3 (Rounds 11–19), extracted from the top-level
progress file to keep that file scannable. Task ids refer to
[`IMPLEMENTATION_PLAN.md`](../../IMPLEMENTATION_PLAN.md) and
[phase-3-tui.md](../plan/phase-3-tui.md).

---

## Round 11 — 2026-08-06 · Input model (P3.1) — Phase 3 start

**Done:**

- **P3.1** — `docs/input-model.md` plus `crates/bam-tui/src/input/mod.rs`.
  Added a `[lib] name = "bam_tui"` target to `bam-tui/Cargo.toml` (it was
  bin-only) so `tests/` — and later P3.4's UI code — can depend on the input
  module as a library; `main.rs` is untouched, since v1 doesn't wire the
  resolver into the app loop yet (that starts at P3.4). `Mode` and `Action`
  are the phase doc's own sketch verbatim; `ActionKind` is a new type not in
  the sketch — bindings in `bam.toml` name a count-independent action
  (`"move_down"`), and only `G` needs the count itself to change *which*
  `Action` variant comes out (`GoToRow(n)` with a count, `GoBottom` without),
  so `Resolver` resolves `ActionKind` + `Option<usize>` → `Action` rather
  than keymap entries pointing at `Action` directly. `Key` (a keypress,
  decoupled from crossterm/ratatui — neither is a dependency yet, and this
  module doesn't need one to be tested) and `Keymap` (`HashMap<String,
  ActionKind>`) both use one canonical string token per key
  (`Key::Ctrl('d')` → `"ctrl-d"`) as both the `bam.toml` spelling and the
  resolver's own sequence-matching key, so `gg`-style multi-key bindings need
  no separate parser — matching is `HashMap` lookup plus a prefix scan for
  `Pending`. `0` is handled as vim does: a digit that starts a count, unless
  it's a leading `0` with no count yet pending, in which case it's the
  `LineStart` binding instead. Four tests in `crates/bam-tui/src/input/
  mod.rs` (inline `#[cfg(test)]`, matching the module-is-the-artifact framing
  the phase doc uses elsewhere — not a separate `tests/` file), matching the
  task's four bullets exactly.

73 tests total (4 new + 69 pre-existing). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check (unaffected — `bam-tui` isn't part of it) all
clean.

**No deviations.**

---

## Round 12 — 2026-08-06 · Input resolver state machine (P3.2)

**Done:**

- **P3.2** — Read `Resolver::handle_key` (`crates/bam-tui/src/input/mod.rs`,
  built in Round 11) against the five test groups in `phase-3-tui.md` before
  writing anything: pending-count accumulation, prefix-sequence matching, and
  clear-on-reject were all already implemented as part of P3.1's own
  deliverable — P3.1 went further than its task text asked (which only
  required the *types*) and built the working state machine too. So P3.2's
  "implement the resolver" has no code left to do; its actual remaining scope
  is the five test groups themselves, which P3.1's four narrower tests didn't
  fully cover. Added five tests to the existing `#[cfg(test)]` module (no new
  test file, matching P3.1's own inline-test framing): `count_prefix_motions_
  resolve` (adds the `12G` → `GoToRow(12)` case P3.1 didn't test), `g_prefix_
  state_machine` (adds the `gg` → `GoTop` middle case — P3.1's own tests
  covered the `Pending` and `Rejected` ends of that sequence but never the
  resolved middle), `esc_clears_pending_state_from_any_partial_sequence`
  (both a pending count and a pending key-prefix, proving neither survives
  into the next resolution), `mode_transitions` (`v`/`Esc`/`:`/`/` against a
  keymap binding all four, none of which P3.1's `test_keymap()` included),
  and `count_with_no_following_key_remains_pending_indefinitely` (three
  digits in a row, still `Pending`, then resolves with the full accumulated
  count). No production code changed — confirmed by running the new tests
  against the unmodified `Resolver` before writing this note, not assumed.

78 tests total (5 new + 73 pre-existing). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check (unaffected — `bam-tui` isn't part of it) all
clean.

**Deviation for the next session to know about:** P3.2's task text frames
this round as an implementation task ("Implement the resolver from P3.1"),
but by the time this round started there was nothing left to implement —
Round 11 had already built it in full while delivering P3.1's own narrower
scope. Flagged per the same convention as Round 8/10's P2.8 note: read the
current state before assuming a task's text still matches what's left to do.

---

## Round 13 — 2026-08-06 · Default keymap + user override merge (P3.3)

**Done:**

- **P3.3** — `default_keymap()` (`crates/bam-tui/src/input/mod.rs`) builds
  the full 22-binding v1 table from the phase doc's list. One token had no
  home: `?` names no existing `ActionKind`, so a new variant,
  `ActionKind::OpenHelp` / `Action::OpenHelp`, was added — a small,
  necessary addition (P3.7's help overlay is its future consumer), not scope
  creep, since the task's own first test bullet ("the default table contains
  every binding listed above") is false without it. `space` needed the same
  treatment as `Key::Esc` already got: `Key::token()` special-cases
  `Key::Char(' ')` to `"space"` rather than emitting a literal space
  character, matching `docs/plan/phase-3-tui.md`'s own naming of it as a
  distinct token alongside `Esc`. `KeymapConfig { keys: HashMap<String,
  String> }` is the `[keys]` section's shape — deliberately just that section,
  not a full `bam.toml` aggregate struct, since highlight (P3.6) and launcher
  (P6.3) config are later tasks' own scope to add, not this one's to
  anticipate. `merge_keymap` layers overrides over the default table,
  recognizing the sentinel string `"unbind"` (chosen here — no prior doc fixed
  one) to remove a binding rather than replace it, and otherwise resolves an
  override's action name via `ActionKind`'s own `Deserialize` (through
  `serde_json::from_value` on a JSON string) rather than a second hand-written
  name table that could drift from the enum. Added the `toml` crate
  (workspace-pinned, `0.8`) as a real dependency, not just for this test: it's
  the format every `bam.toml`-parsing task from here on (P3.3, P3.6, P6.3)
  needs, and P3.3 is the first to actually need it, confirmed by
  `toml::from_str::<KeymapConfig>("")` in the fifth test rather than
  simulating "no `[keys]` section" with a bare empty `HashMap`. Promoted
  `serde_json` from `bam-tui`'s dev-dependencies to a real dependency, since
  `merge_keymap` (not just its tests) now calls it. Five new tests in the
  existing inline `#[cfg(test)]` module, matching the five bullets exactly.

83 tests total (5 new + 78 pre-existing). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check (unaffected — `bam-tui` isn't part of it) all
clean.

**Deviations for the next session to know about:**
- Added `ActionKind::OpenHelp`/`Action::OpenHelp` — not in P3.1's or P3.2's
  enum, needed because `?` (explicitly in P3.3's binding list) had nothing to
  bind to otherwise. No overlay logic consumes it yet; P3.7 is its intended
  consumer.
- The `"unbind"` sentinel string and the `[keys]`-only shape of
  `KeymapConfig` are both this round's own design choices, not dictated by
  `docs/input-model.md` or the phase doc — flagged in case a later task (or a
  real `bam.toml` loader) assumes a different convention.

---

## Round 14 — 2026-08-06 · TUI shell and virtualized list (P3.4)

**Done:**

- **P3.4** — The phase doc's three tests presuppose a windowed query
  primitive that didn't exist yet: `Session::search_packages` (P2.6)
  materializes every match into a `Vec<Package>`, which is exactly what
  "memory does not scale with result-set size" rules out. Added
  `Session::search_window(pred, offset, limit) -> (Vec<Package>, usize)`
  (`crates/bam-core/src/store/session.rs`) — wraps the existing compiled
  `SELECT id FROM package WHERE ...` in `SELECT COUNT(*) FROM (...)` for the
  total and `... ORDER BY id LIMIT ? OFFSET ?` for the page, reusing
  `compile::compile` rather than duplicating predicate-compilation logic;
  `matching_ids` and the new method now share a `compiled_for` helper that
  factors out the existing-named-selection check. Exposed as
  `api::search_window` (`SearchWindowRequest`/`SearchWindowResponse`,
  `crates/bam-core/src/api/`) alongside P2.6's `search_packages`, not
  replacing it — a full, unpaginated result list is still the right shape
  for a future `type:`/CLI/MCP caller that isn't rendering a scrolling list.
  Two tests in `tests/store_session_window.rs`, against 25 and 5 real
  inserted rows: a page's ids match the corresponding slice of the full
  unpaginated result, and an out-of-range offset returns an empty page with
  the correct total still reported.

  `crates/bam-tui` gained `ratatui`+`crossterm` (new workspace
  dependencies) and three new modules. `store::PackageStore` is a small
  trait (`window(pred, offset, limit) -> WindowResult{packages, total}`) —
  narrower than `bam_core::api::Session` so a test can inject a fake that
  counts calls without a database; `store::SessionStore` is the real
  implementation, adapting `api::search_window`. `app::App<S: PackageStore>`
  holds a `cursor` (absolute row) and a `top`/loaded `window`, re-querying
  only when a `move_down`/`move_up`/`go_top`/`go_bottom` call would move the
  cursor outside the currently loaded page — a scroll that stays inside the
  page costs nothing, and one that doesn't costs exactly one
  viewport-sized query, never the whole result set. `ui::render` draws three
  panes (query line, package list, detail) from `App`'s already-loaded
  `window` alone — it never queries. `app::all_packages()` (a `dir GLOB '*'`
  match, `dir` being `NOT NULL`) stands in for a real query until P3.5 wires
  the input line to the parser. Three tests in `crates/bam-tui/tests/
  tui_shell.rs`: a counting fake store proves the initial query is
  viewport-sized, that scrolling within the loaded page issues no further
  query, and that crossing the page boundary issues exactly one more
  viewport-sized (never total-sized) query; a 100-total and an 84,000-total
  fake store both leave `App` holding exactly 20 `Package` records; a
  `TestBackend` buffer snapshot for a 3-package fixture asserts all three
  panes' expected text is present.

  Also wired the shell into the `bam` binary as a new `tui` subcommand
  (`crates/bam-tui/src/main.rs`) — Round 11's own note ("v1 doesn't wire the
  resolver into the app loop yet... that starts at P3.4") flagged this as
  P3.4's implied scope, and a "TUI shell" that only exists in tests isn't
  one. Loads `bam.toml`'s `[keys]` section from `~/.config/bam/bam.toml`
  (new — P3.3 built `KeymapConfig`/`merge_keymap` but nothing read a real
  file yet) via P3.3's own merge function, falling back to the defaults on
  a missing file or an unknown-action error. The crossterm event loop
  converts key events to P3.1's `Key` type, feeds them through P3.2's
  `Resolver`, and dispatches only `MoveDown`/`MoveUp`/`GoTop`/`GoBottom`/
  `Quit` — every other resolved `Action` (modes, marking, help) is later
  rounds' scope (P3.5-P3.9) and is silently accepted but not acted on yet.

88 tests total (5 new — 2 in `store_session_window.rs`, 3 in `tui_shell.rs`
— plus 83 pre-existing; verified directly via `cargo test --workspace 2>&1 |
grep "test result:"` and summed, not hand-counted, per Round 10's own
caution about this). `cargo fmt --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, and the wasm32 `--no-default-features` check
(unaffected — `bam-tui` isn't part of it) all clean. Also smoke-tested the
real `bam` binary: `ingest --offline` still reports 501 packages against a
scratch DB, and `bam tui` against the same DB with no TTY attached (this
environment has none) hits the `enable_raw_mode` failure path cleanly rather
than panicking — the actual interactive rendering and key-handling loop is
**not** verified against a real terminal this round; say so explicitly per
the project's own standing rule on claiming UI features work.

**Deviations for the next session to know about:**
- `Session::search_window` and `api::search_window` are additions the phase
  doc's P2.6/P2.5 text never named — they exist only because P3.4's own test
  bullets are unsatisfiable without a paginated query primitive underneath
  the virtualized list. Flagged in case a later phase (P4's harvest/search
  work) expects `search_packages` alone to still be the one query surface.
- The `tui` subcommand, its `~/.config/bam/bam.toml` config path, and the
  choice to silently ignore non-navigation actions are this round's own
  design choices, not dictated by any phase doc — same convention as Round
  13's flagged `KeymapConfig`/`"unbind"` choices.

---

## Round 15 — 2026-08-06 · Query input line with inline errors (P3.5)

**Done:**

- **P3.5** — `Session::parse_query` (`crates/bam-core/src/store/session.rs`,
  native-gated) parses query-line text through `BamDsl` directly against the
  session's own `FieldRegistry` — no `LanguageRegistry` (P2.2) involved: it's
  the only registered surface syntax so far, and wiring a registry for one
  entry would be speculative ahead of a second language or a real
  `default_query_language` config key (both still doc-only, confirmed by
  grep before writing this). `SessionError` gained a `Parse(#[from]
  ParseError)` variant so the call composes with `?` like every other
  session method. `api::parse_query` (`crates/bam-core/src/api/query.rs`) is
  the thin typed wrapper (`ParseQueryRequest{ src }` /
  `ParseQueryResponse{ predicate }`), following P2.6's existing pattern
  rather than having `bam-tui` call `Session` directly — `store.rs`'s
  `SessionStore` already goes through `api::` for `window`, not `Session`
  itself, so this keeps one convention rather than two.

  `bam-tui`'s `PackageStore` trait (`crates/bam-tui/src/store.rs`) gained
  `fn parse(&self, src: &str) -> Result<Predicate, ParseError>` — returning
  the parser's own span-carrying `ParseError`, not `StoreError`, since the
  inline error marker needs the byte span, not a flattened string.
  `SessionStore::parse` unwraps `api::parse_query`'s `SessionError` back down
  to a `ParseError` (the only variant `parse_query` can actually produce;
  other variants get a spanless fallback rather than a `panic!`/`unwrap`).

  `App` (`crates/bam-tui/src/app.rs`) gained `query_text`, `query_error`, and
  `debounce_deadline: Option<Instant>`, plus `edit_query(text, now)` (records
  the text and resets a 150 ms deadline without querying) and `tick(now)`
  (applies the pending edit once the deadline has passed: a successful parse
  replaces `predicate`/`window` and resets the cursor; a `ParseError` is
  stored and `predicate`/`window` are left exactly as they were — the
  "keep last valid result set" rule). Both take an explicit `Instant` rather
  than reading the clock internally, so the four tests drive debounce timing
  without a real `sleep`. `ui::render` (`crates/bam-tui/src/ui.rs`) grew a
  second one-line row under the query line: when `query_error` is set, it
  renders spaces up to the error's column (`span.0` clamped to the text's
  last character, so an operator with nothing after it — `size>`, whose real
  parser span is one-past-the-end — still marks the `>` itself, not the
  column after it) followed by `^` and the message.

  `bam-tui/src/input/mod.rs` gained `Key::Backspace` (mapped from
  crossterm's `KeyCode::Backspace` in `main.rs`) — a small, necessary
  addition, same class as Round 13's `OpenHelp`: without it there's no way
  to correct a typo in the query line, which would make the feature
  built-but-unusable rather than merely v1-scoped. `main.rs`'s `run_loop`
  now tracks a local `Mode` (Normal by default): while `Mode::Insert` (`/`
  in the default keymap resolves to `Action::EnterMode(Mode::Insert)`, which
  `run_loop` catches before `apply_action` to flip the mode rather than
  falling through to `apply_action`'s ignored-action catch-all), keys append
  to or backspace out of the query text directly instead of resolving
  through the keymap — `Esc` returns to `Mode::Normal` and clears the
  resolver's pending state. `event::read()`'s indefinite block was replaced
  with `event::poll(50ms)` so `app.tick(Instant::now())` still runs (and a
  settled debounce still fires a query) even while no key arrives; the four
  tests themselves don't touch `main.rs` at all, matching every prior TUI
  round's precedent of testing `App`/`ui::render` directly.

  Four tests in the new `crates/bam-tui/tests/tui_query_line.rs`, matching
  the phase doc's four bullets exactly. `FakeStore::parse` calls the real
  `BamDsl`/`FieldRegistry` (both pure, no database) rather than a hand-rolled
  stub, so the error-span test exercises the actual parser's span for
  `dir:util/* size>` (byte offset 16, one past the trailing `>` at index 15)
  instead of an invented one; `FakeStore::window` only distinguishes the
  initial `all_packages()` predicate from any other, which is enough to prove
  a valid edit changes what's rendered without reimplementing glob/compare
  semantics in a fake. `crates/bam-tui/tests/tui_shell.rs`'s pre-existing
  `FakeStore` (P3.4) needed a one-line placeholder `parse` impl to satisfy
  the now-larger trait — unused by those three tests, noted inline.

92 tests total (4 new + 88 pre-existing). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check (unaffected — `bam-tui` isn't part of it) all
clean. Also smoke-tested the real `bam` binary: `ingest --offline` still
reports 501 packages, and `bam tui` against the same scratch DB still hits
the clean `enable_raw_mode` failure path with no TTY attached — the real
interactive typing/debounce/error-marker loop is **not** verified against a
real terminal this round, same standing caveat as every prior TUI round.

**Deviations for the next session to know about:**
- No `LanguageRegistry` wiring for the search box — `Session::parse_query`
  calls `BamDsl` directly. `docs/plan/phase-2-query-core.md` and
  `phase-3-tui.md` both mention a `default_query_language` config key, but
  it exists only in docs (confirmed by grep before writing this round's
  code), and P3.8 (highlight rules, invariant I3) is the task that actually
  needs to select among multiple registered languages. Revisit if a second
  query language is registered before then.
- `Key::Backspace` is a v1-necessary addition beyond what P3.1-P3.4 named,
  same class as Round 13's `ActionKind::OpenHelp` — flagged in case a future
  round assumes `Key`'s variant list is exactly what P3.1's doc enumerated.

---

## Round 16 — 2026-08-06 · Selection UI and `:` command line (P3.6)

**Done:**

- **P3.6** — `crates/bam-tui/src/store.rs`'s `PackageStore` trait grew seven
  methods (`toggle`/`is_marked`/`mark`/`select_by_query`/`save_as`/`load`/
  `list_selections`), each a thin `SessionStore` pass-through to P2.7's
  `bam_core::api` functions — "everything routes through the API" per the
  phase doc, same convention P3.4/P3.5 already established for `window`/
  `parse`. Two of those API functions had gaps found while wiring this up:
  `api::is_marked` didn't exist yet (added to `api/selection.rs`, same
  bare-`package_id` shape as `mark`/`unmark`/`toggle`), and `api::list` —
  built in Round 9 — was never re-exported from `api::mod`'s `pub use
  selection::{...}` list, a pre-existing miss with no prior caller to trip
  it; fixed by adding it to the same re-export line.

  `App` (`crates/bam-tui/src/app.rs`) gained a `marked: Vec<bool>` field
  parallel to `window.packages` — a *rendering cache* refreshed from
  `store.is_marked` after every window change (`new`, `tick`, `sync_window`),
  never an independent record of membership; the working selection in
  `store::session` stays the sole source of truth, the same relationship
  `window: WindowResult` already has to the real result set (P3.4). Added
  `visual_anchor: Option<usize>` (`enter_visual`/`leave_visual`), and
  `toggle_mark()`, which toggles the single row under the cursor normally
  but — when an anchor is set — marks every row in `[anchor, cursor]` via
  `mark_range` (a fresh `store.window(pred, start, len)` fetch of exactly
  that span, not the currently-loaded viewport, so a Visual selection wider
  than the viewport still marks correctly) and consumes the anchor.
  `command_text`/`status` are the `:`-line's own editing/output state
  (same class as `query_text`/`debounce_deadline`, not selection state), and
  `run_command(&str) -> Result<CommandOutcome, StoreError>` parses the five
  commands (`mark`/`unmark` reuse `store.parse` + `select_by_query` with
  `SelectionMode::Union`/`Subtract`; `save`/`load` unquote an optionally
  `"quoted name"`; `selections` returns the summaries) — `CommandOutcome`
  is a small enum so a test can assert on the result directly rather than a
  formatted string.

  `bam-tui/src/input/mod.rs` gained `Key::Enter` — necessary once a command
  line needs an explicit submit key, same class of small addition as
  Round 13's `OpenHelp` and Round 15's `Backspace`. `main.rs`: `apply_action`
  now takes `&mut Mode` and handles `ToggleMark`/`EnterMode(Visual|Command|
  Insert)`/`LeaveMode` (previously resolved but silently ignored beyond
  `Insert`, per Round 14's own note); a new `edit_command_line` mirrors
  `edit_query_line` for `Mode::Command` (`Enter` runs the command and
  surfaces `CommandOutcome`/error via `app.set_status`, `Esc` cancels).
  `ui::render` shows a `"* "` marker before a marked row's label (only when
  marked, so an all-unmarked buffer renders byte-identical to before this
  round — confirmed by Round 14/15's pre-existing snapshot tests staying
  green unmodified) and, in the row under the query line, the in-progress
  command text or the last status message when there's no query error.

  Four tests in the new `crates/bam-tui/tests/tui_selection.rs`, matching
  the phase doc's four bullets exactly: a `FakeStore` backed by a shared
  `HashSet<i64>` proves `space` toggles membership and the rendered buffer
  gains a `*`; entering Visual, moving down 3, then confirming proves
  exactly 4 rows (not 3, not 5) get marked; `run_command("mark dir:mus/*")`
  against a fake that only recognizes that one predicate string proves the
  `:mark` union; and a real file-backed `Session`/`SessionStore` (temp-path
  pattern from Round 9's `api_selection.rs`) proves `:save "tracker
  candidates"` then a *fresh* `Session::open` on the same path plus
  `:load "tracker candidates"` restores the marked state. The fifth bullet
  ("no selection state in the TUI") is a diff-review claim, not a test:
  `App`'s new fields are either rendering caches refreshed from the store
  (`marked`) or UI editing/output state (`visual_anchor`, `command_text`,
  `status`) — none of them is itself the record of what's selected; that
  stays in `store::session`'s `selection`/`selection_member` tables,
  reached only through `PackageStore`. Round 14/15's two pre-existing
  `FakeStore`s (`tui_shell.rs`, `tui_query_line.rs`) needed placeholder
  impls of the seven new trait methods to keep compiling — `is_marked`
  specifically returns `Ok(false)` rather than `unimplemented!()`, since
  `App::new` now calls it for every loaded row regardless of which test is
  running.

96 tests total (4 new + 92 pre-existing — the wasm32 check is unaffected by
either fix in `api/`, since the whole `api` module is `#[cfg(feature =
"native")]`-gated at the crate root, never compiled into that build).
`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
warnings`, and the wasm32 `--no-default-features` check all clean. Also
smoke-tested the real `bam` binary: `ingest --offline` still reports 501
packages, and `bam tui` against the same scratch DB still hits the clean
`enable_raw_mode` failure path with no TTY attached — the real interactive
Visual-mode/command-line loop is **not** verified against a real terminal
this round, same standing caveat as every prior TUI round.

**Deviations for the next session to know about:**
- `api::is_marked` and the `api::list` re-export fix are both P3.6-driven
  additions/fixes to `bam-core::api`, not named by the phase doc's P3.6 text
  (which only asks for the TUI-side wiring) — flagged in case a later round
  assumes `api::mod`'s re-export list was already complete.
- `App::marked`'s "rendering cache, not selection state" framing is this
  round's own resolution of the diff-review bullet — worth a second look if
  a future round finds it awkward that marked-state can go briefly stale
  between a `mark`/`unmark` elsewhere (e.g. a future GUI client sharing the
  same DB) and this session's next window refresh; P2.6's session-scoped
  model (I5) already accepts that two sessions don't observe each other
  live, so this is consistent with existing behavior, not a new gap.

---

## Round 17 — 2026-08-06 · Semantic token → ratatui style (P3.7)

**Done:**

- **P3.7** — `bam_core::highlight` (new top-level module, ungated — pure data
  and logic, confirmed by the wasm32 `--no-default-features` check):
  `Decoration` (`gutter`/`badge`/`background: Option<String>` +
  `priority: i32`, verbatim the plugin-output shape from `bam-handoff.md`
  §11.1) and `resolve(&[Decoration]) -> RowTokens`, the one conflict-
  resolution implementation both DSL rules (P3.8, not built yet) and plugin
  output (Phase 8) will feed. `background` is exclusive: a strict `>`
  comparison while folding left-to-right means the *first* decoration to
  reach the max priority wins ties, not sort/hash order — deterministic and
  stable by construction, so no separate tiebreak field or sort key was
  needed. `gutters`/`badges` stack, `sort_by_key(Reverse(priority))` (stable,
  preserving input order on ties) then `.take(3)`. `MARKED_GUTTER`/
  `MARKED_PRIORITY` (`i32::MAX`) are the constants marked-state rendering
  uses to build its own `Decoration` — the module doesn't special-case
  marked state itself, callers do, per the phase doc's "marked state flows
  through the same token path" instruction. Three tests in
  `tests/highlight.rs` (highest-priority background wins with the winner
  pinned; equal priorities resolve to the first-seen one, not hash order;
  four stacking gutters render exactly three, highest-priority-first).

  `bam_tui::tokens` (new module) is the one mapping table from token string
  to ratatui presentation — `background_style`, `gutter_char`, `badge_text`
  — each falling through to an unstyled/blank default on an unrecognized
  token rather than panicking (one inline test). `App::row_tokens(idx)`
  (`crates/bam-tui/src/app.rs`) builds the decoration list for a window-local
  row (currently just the marked-state `Decoration`, since P3.8's rule
  evaluation doesn't exist yet to contribute more) and calls
  `highlight::resolve` — the same function a future rule-driven decoration
  list will call, not a parallel path. `ui::render` (`crates/bam-tui/src/
  ui.rs`) now builds each row's gutter prefix, background `Style`, and badge
  suffix from `row_tokens` instead of the previous ad hoc `if marked {"* "}`
  check; since `gutter_char("marked") == '*'` and an unmarked row still
  resolves to an empty gutter list, the rendered buffer is byte-identical to
  before this round for every existing case — confirmed by Round 14's
  pre-existing snapshot test (`tui_shell.rs::buffer_snapshot_for_a_small_
  fixture`) passing unmodified. One integration test,
  `tests/tui_tokens.rs::a_marked_row_resolves_through_the_same_path_as_a_
  highlight_rule`, builds a `FakeStore`, marks a row, and asserts
  `app.row_tokens` for that row equals `resolve()` called directly on a
  hand-built `Decoration` carrying the same marked gutter/priority — proving
  the "same path" claim rather than asserting it only by code reading.

101 tests total (5 new — 3 in `highlight.rs`, 1 in `tokens`'s inline test, 1
in `tui_tokens.rs` — plus 96 pre-existing). `cargo fmt --check`, `cargo
clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean. Also smoke-tested the real `bam`
binary: `ingest --offline` still reports 501 packages against a scratch DB,
unaffected by this round's TUI-only rendering change.

**No deviations.**

---

## Round 18 — 2026-08-06 · Highlight rules with hot reload (P3.8)

**Done:**

- **P3.8** — `Session` (`crates/bam-core/src/store/session.rs`) gained a
  `langs: LanguageRegistry` field (built in `from_connection` — `bam-dsl` the
  only registered id, same as before, but now through the actual P2.2
  registry instead of a hardcoded `BamDsl.parse` call), `parse_query_lang`
  (`lang: Option<&str>` selects the language, `SessionError::Language(
  #[from] LanguageError)` new), and `matching_ids_among(pred, ids)` — matches
  restricted to a caller-supplied id list rather than the whole table, so the
  highlight engine only asks about the currently *visible* rows. Deliberately
  compiles regardless of whether `ids` is empty (checks `ids.is_empty()`
  *after* `compiled_for`, not before) so `ids: &[]` doubles as a load-time
  validation trial — catches a predicate that parses fine but doesn't compile
  (`Similar`, not yet supported) without needing a real row. `bam_core::api`
  gained `filter_ids` (`api/query.rs`) wrapping it, and `ParseQueryRequest`
  gained a `#[serde(default)] lang: Option<String>` field (`api/types.rs`) —
  the two existing call sites (`api::query::parse_query` itself,
  `bam-tui`'s `SessionStore::parse`) updated to pass `lang: None`.

  `bam-tui`'s `PackageStore` trait (`crates/bam-tui/src/store.rs`) gained
  `parse_lang` and `matching_ids`, both thin `SessionStore` pass-throughs to
  the new API calls — `parse_lang` returns a flat `StoreError` rather than
  `parse`'s span-carrying `ParseError`, since a highlight rule's error is
  reported as one line per rule, not an inline caret under a byte offset.

  New module `crates/bam-tui/src/rules.rs`: `HighlightRules` parses
  `[[highlight]]` blocks (`RuleConfig`: `name`, `lang`, `when`, `gutter`,
  `badge`, `background`, `priority` — the phase doc's shape verbatim) via
  `toml`, compiling each `when` through `store.parse_lang` and validating it
  through `store.matching_ids(&pred, &[])` (the empty-trial-compile use of
  the method above); a rule that fails either step is dropped and its
  message (`"{name}: {error}"`) recorded in `errors()` instead of aborting
  the reload — one bad rule cannot disable the others. Watched by **polling
  file content**, not a filesystem-event crate (`notify` was never added):
  nothing here needs more than "did the bytes change," and content-diffing
  sidesteps `mtime` granularity flakiness a real notify-based test would
  have to work around. `poll(now, store)` reuses P3.5's own debounce shape —
  a content change starts a timer, a *different* change while pending resets
  it (same as query-line edits), and only content that has held steady for
  `RELOAD_DEBOUNCE` (300ms) triggers a reload, so two rapid writes from one
  editor save collapse into one.

  `App` (`crates/bam-tui/src/app.rs`) gained `rules: HighlightRules`
  (`HighlightRules::empty()` until wired) and `rule_hits: Vec<Vec<usize>>` —
  a rendering cache, same relationship P3.6's `marked` already has to the
  working selection, refreshed alongside it by a renamed `refresh_marked` →
  `refresh_row_caches` (all five call sites — `sync_window`, `tick`, both
  `run_command` branches, `select_by_query_command` — updated together, a
  mechanical rename since rule hits depend on window contents the same way
  marked-state does). `set_highlight_config(path)` (new, not called by
  `App::new` — every existing test/caller that never calls it keeps
  pre-P3.8 behaviour exactly) loads rules and refreshes the caches once;
  `tick` polls `rules` every call (unconditionally, ahead of the query
  debounce check) and refreshes the caches when `poll` reports a reload
  happened. `row_tokens` now folds each hit rule's own `Decoration` into the
  list passed to `highlight::resolve` alongside the marked-state one —
  P3.7's "same path" claim now has a second real producer, not just marked
  state and a hand-built test double. `highlight_errors()` exposes
  `rules.errors()`; `ui::render` (`crates/bam-tui/src/ui.rs`) shows them
  (joined by `"; "`) in the row-1 slot, at the lowest priority (query error
  > command line > status > highlight errors).

  `main.rs` gained `resolve_config_path(flags)` (extracted out of
  `load_keymap`, which now takes the resolved path directly) so the `[keys]`
  and `[[highlight]]` sections of the same `bam.toml` are resolved once, not
  twice; `tui()` calls `app.set_highlight_config(&config_path)` right after
  `App::new`, failing the same way the DB-open and initial-query steps
  already do on a real error (a rule-level error never reaches this path —
  only a genuine `StoreError`, e.g. a DB failure, does).

  Ten tests: six in the new `crates/bam-tui/tests/tui_highlight.rs`, matching
  the phase doc's six bullets exactly. The first four (default/explicit/
  unregistered language, a bad `when` reported-and-skipped) drive a real
  `Session`/`SessionStore` against a seeded one-row DB — genuine parser/
  registry/compiler wiring is what's under test, not a fake's own hardcoded
  logic, same reasoning as `tui_selection.rs`'s save/load round trip. The
  last two (hot reload, debounce) use a `FakeStore` that echoes `when` back
  as `FullText` and counts `parse_lang` calls, driven by synthetic `Instant`s
  exactly like P3.5's own debounce tests — deterministic timing instead of
  real filesystem-event races. The other four now-existing `FakeStore`s
  (`tui_shell.rs`, `tui_query_line.rs`, `tui_selection.rs`, `tui_tokens.rs`)
  each needed placeholder `parse_lang`/`matching_ids` impls to keep
  compiling, same convention as every prior round's trait growth.

107 tests total (6 new in `tui_highlight.rs` + 101 pre-existing; verified via
`cargo test --workspace 2>&1 | grep "test result:"` and summed, per Round
10/14's own caution about hand-counting). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean. Also
smoke-tested the real `bam` binary: `ingest --offline` still reports 501
packages, and `bam tui --config <a file with one [[highlight]] rule>`
against the same scratch DB still hits the clean `enable_raw_mode` failure
path with no TTY attached — proving `set_highlight_config` runs without
erroring against a real config file, not just the test doubles; the real
interactive hot-reload loop is **not** verified against a real terminal this
round, same standing caveat as every prior TUI round.

**Deviations for the next session to know about:**
- No `default_query_language` key was added to `bam.toml`. The phase doc's
  own sample comment (`# optional; falls back to default_query_language`)
  reads as a config key, but with exactly one language registered, the
  `LanguageRegistry`'s own constructor-supplied default id (`"bam-dsl"`,
  set once in `Session::from_connection`) already satisfies "the configured
  default" — a `bam.toml` key that lets a user choose *among* registered
  languages is speculative until a second one exists, same YAGNI call
  Round 15's deviation note made about wiring the registry at all. Revisit
  together with that note once a second language is registered.
- "A rule whose `when` fails to compile" is read broadly: both a parse-time
  rejection (`FieldRegistry`'s type checks, e.g. `size:~'foo'`) and a
  predicate that parses but fails the separate IR→SQL compile step (only
  reachable today via `Similar`, not yet supported) are caught at rule-load
  time and treated the same way — recorded in `errors()`, rule dropped. This
  is why `reload` calls `store.matching_ids(&pred, &[])` as a trial after a
  successful parse, not just `parse_lang` alone.
- `HighlightRules` polls file *content* (a full read-and-compare each tick),
  not `mtime` — a deliberate simplification over the more obvious
  mtime-based watch, made specifically so the debounce test doesn't depend
  on filesystem timestamp granularity. Fine at `bam.toml`'s size; revisit if
  a much larger watched file ever makes a full read-per-tick measurably
  costly.

**Phase 3 remaining:** P3.9 (help overlay).

---

## Round 19 — 2026-08-07 · Help overlay (P3.9) — Phase 3 exit

**Done:**

- **P3.9** — `App` (`crates/bam-tui/src/app.rs`) gained `help: Option<Keymap>`
  and `open_help(keymap)`/`close_help()`/`help_open()`/`help_bindings()`.
  `open_help` takes the caller's own live `Keymap` by value rather than
  reading a copy `App` holds independently — the phase doc's "render from the
  same table P3.3 loads, so the overlay and the real keymap cannot drift
  apart" is satisfied by construction: there is only ever the one `Keymap`
  value, passed in, not a second copy `App` could fall out of sync with. This
  also avoided threading a `Keymap` through `App::new`'s constructor (and
  therefore every existing test call site across five test files) — the same
  "grow via a setter, not the constructor" convention P3.8's
  `set_highlight_config` already established for a caller-optional feature.
  `ui::render` (`crates/bam-tui/src/ui.rs`) draws the overlay, when open, as a
  full-frame bordered block listing every `"{token}  {action}"` line
  (`serde_json`-serialized `ActionKind` name, e.g. `"move_down"` — matching
  `bam.toml`'s own spelling — rather than a hand-written display table that
  could drift from `ActionKind`'s real variant names), sorted by token for a
  stable render order (`Keymap`'s underlying `HashMap` has none).

  `crates/bam-tui/src/main.rs`: `apply_action` gained a `keymap: &Keymap`
  parameter and an `Action::OpenHelp` arm calling `app.open_help(keymap.
  clone())`; `run_loop` gained a `keymap` parameter too and, ahead of both the
  existing Insert/Command line-editing intercepts, a check that closes the
  overlay on `Esc` or `q` and swallows the keypress — without it, `q` would
  still resolve through the keymap to `Action::Quit` while the overlay is
  open, since the overlay isn't a `Mode` variant the keymap's own bindings
  already exclude. `tui()` now builds `keymap` once and clones it into both
  `Resolver::new` and `run_loop`, rather than `Resolver` owning the only copy
  (it didn't expose one) — the smallest change that gives both the resolver
  and the overlay a value to work from without duplicating `load_keymap`'s
  file-read.

  Three tests in the new `crates/bam-tui/tests/tui_help.rs`, matching the
  phase doc's three bullets exactly: `overlay_binding_set_equals_the_active_
  keymap` asserts set equality between `help_bindings()`'s keys and the
  source `Keymap`'s keys (not a hardcoded list of the 22 tokens); `user_
  override_shows_the_users_key_not_the_default` merges a `KeymapConfig`
  override through P3.3's own `merge_keymap` first, then asserts the overlay
  carries the override's key, not a separate hand-built `Keymap`; `open_and_
  close_toggle_help_open` drives `open_help`/`close_help` directly. All three
  test `App` alone (no `FakeStore` behaviour beyond satisfying the trait) —
  the `?`-opens/`Esc`-or-`q`-closes wiring itself lives in `main.rs`'s
  `run_loop`, untested by any prior round's convention for interactive
  key-loop code (every TUI round's own standing caveat: the real terminal
  loop isn't verified against a real terminal).

110 tests total (3 new + 107 pre-existing). `cargo fmt --check`, `cargo
clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean. Also smoke-tested the real `bam`
binary: `ingest --offline` still reports 501 packages against a scratch DB,
and `bam tui` against the same DB with no TTY attached still hits the clean
`enable_raw_mode` failure path — the real interactive help-overlay loop is
**not** verified against a real terminal this round, same standing caveat as
every prior TUI round.

**No deviations.**

**Phase 3 exit reached.** A usable daily-driver TUI with configurable
vim-style bindings, persistent selections, hot-reloadable highlighting, and a
help overlay. Everything after this is additive, per the phase doc's own
closing line.

---
