# Input model

← [Implementation plan index](../IMPLEMENTATION_PLAN.md) · invariant I6

This document and `crates/bam-tui/src/input/mod.rs` are one artifact. If they
drift, this document is wrong.

Every keystroke in the TUI (and, later, any frontend with a modal keyboard
surface) routes through one resolver:

```
[count] [operator] {motion | object | command}
```

**v1 registers modes and motions only — zero operators, zero objects.** The
grammar slot exists and goes unused; that costs the types below now against
rewriting the input layer of a working TUI later.

## Modes

```rust
pub enum Mode { Normal, Insert, Visual, Command }
```

- **Normal** — the default; keys resolve to motions and commands.
- **Insert** — typing into the query line (entered by `/`).
- **Visual** — range selection over the package list (entered by `v`).
- **Command** — the `:` command line (`:mark`, `:save`, `:load`, ...; P3.6).

`Esc` returns to Normal from any other mode, and also clears any pending
count/sequence — the two are the same `Resolution::Rejected`-adjacent reset
path, not two separate mechanisms.

## The resolver

```rust
pub enum Resolution { Pending, Resolved(Action), Rejected(Reason) }
```

A `Resolver` holds a pending count and a pending key sequence. Each keypress:

1. A digit (except a leading `0`, see below) accumulates into the pending
   count and returns `Pending`.
2. Otherwise the key is appended to the pending sequence, and the sequence is
   looked up against the keymap:
   - **exact match** → the bound `ActionKind` combines with the pending count
     to produce a concrete `Action`; both pending fields clear.
   - **prefix of a bound sequence** (e.g. `g` before `gg` resolves) →
     `Pending`.
   - **no match** → `Rejected(Reason)`, and pending state clears. A rejection
     does not require the *next* key to also fail — the slate is clean.

There is no ambiguity timer. A prefix (`g`) stays `Pending` indefinitely,
exactly like `[count]` with nothing following it yet — surprise timeouts are
worse than an indefinite wait for the next key.

`ActionKind` is the *unresolved* action a binding names — count-independent.
`Action` is what actually gets executed, with the count applied. They are
almost the same enum; the one place they diverge is `G`: bound to
`ActionKind::GoToRowOrBottom`, which resolves to `Action::GoToRow(n)` when a
count preceded it and `Action::GoBottom` when none did. Every other
count-taking action just defaults the count to `1` when absent (`j` →
`MoveDown(1)`, `5j` → `MoveDown(5)`).

## Keymap config format

```rust
pub struct Keymap(pub HashMap<String, ActionKind>);
```

Bindings round-trip through `bam.toml` as a flat string-to-string(-ish) table
— the map key is the exact token sequence a user types, the value is the
action name:

```toml
[keys]
j = "move_down"
k = "move_up"
gg = "go_top"
"ctrl-d" = "half_page_down"
esc = "leave_mode"
```

A single keypress' canonical token (`Key::token`) is what both the resolver's
matching and the config format use — `Key::Char('j')` → `"j"`,
`Key::Ctrl('d')` → `"ctrl-d"`, `Key::Esc` → `"esc"`. A multi-key binding like
`gg` is just two tokens concatenated with no separator; nothing in v1's
binding list needs a separator to stay unambiguous (no `ctrl-`-prefixed key is
ever a prefix of another binding).

This module defines the *shape* only. P3.3 owns the actual default table (the
full v1 binding list: `j k gg G 0 $ ctrl-d ctrl-u ctrl-f ctrl-b H M L n N / ?
: space v Esc q`, each with a count where it makes sense) and the
user-override merge (`bam.toml`'s `[keys]` section layered over the defaults,
with unknown-action names rejected and defaults unbindable).

## Future object model

Not implemented in v1 — recorded here so nothing built now precludes it.
Objects resolve to **sets of packages**, not text ranges:

- `ic` / `ac` — inner/around category (the packages under the cursor's
  `dir` prefix, narrow vs. including siblings)
- `is` — the current (working) selection
- `ir` — the current result set

An operator applied to an object produces a selection — e.g. a hypothetical
`d` (subtract) operator over `ic` would remove that category's packages from
the working selection. This is why the object model composes directly with
**I7** (selections are a core, persisted concept) rather than being a
parallel mechanism: the operator/object grammar is, semantically, a shorthand
for selection set operations that could equally be typed as a `bam-dsl`
query and a `:mark`/`:unmark` command.

## Axis assumption

`0` and `$` are modeled as **horizontal** motions — line start/end when
editing the query input, leftmost/rightmost column in a wide table — with
`gg`/`G` covering the vertical axis (top/bottom of the list, or row `N` with
a count).

This is flagged as an assumption, not a settled fact: `0`/`$` could equally
have meant first/last *row* in a list-first UI where there's rarely a reason
to scroll a table horizontally. Bindings are configurable (P3.3), so getting
this wrong is cheap to revisit — worth recording precisely because it's the
kind of assumption that's easy to bake in silently otherwise.
