# Phase 0 — Workspace scaffold

← [Implementation plan index](../../IMPLEMENTATION_PLAN.md)

Goal: `cargo test` runs green on an empty workspace, and the two invariant
checks that guard the architecture are in place before there is any
architecture to guard.

---

### P0.1 — Cargo workspace skeleton · **H**

Create the workspace root and two member crates.

```
Cargo.toml            # workspace, resolver = "2"
crates/bam-core/      # lib
crates/bam-tui/       # bin, name = "bam"
```

Root `Cargo.toml` declares `[workspace.dependencies]` so members pin versions
in one place. `edition = "2024"`, `rust-version = "1.85"`.

`bam-core` gates every host capability behind a feature (invariant **I1**):

```toml
[features]
default = ["native"]
native  = ["dep:rusqlite", "dep:reqwest", "dep:tokio"]
```

Unconditional dependencies: `serde` (derive), `serde_json`, `thiserror`.
Behind `native`: `rusqlite` (`bundled`, `fts5`), `reqwest`, `tokio`.

**Tests first:**
- `cargo build --workspace` succeeds.
- `cargo build -p bam-core --no-default-features` succeeds.
- `bam --version` prints the version from `Cargo.toml` (not a hardcoded string).

**Why H:** entirely mechanical. The tree, the dependency list, and the feature
split are all given.

**Hand over:** the directory tree, the dependency list and which are gated
behind `native`, crate names, edition and MSRV.

**Done when:** all three checks above pass.

> Deliberately **not** created: `bam-gui`, `bam-server`, `bam-mcp`,
> `frontend/`. Empty crates that exist "for later" rot. Each is created when
> its phase starts.

---

### P0.2 — CI: fmt, clippy, test · **H**

One GitHub Actions workflow running `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo test --workspace` on a matrix of `ubuntu-latest` and `macos-latest`,
triggered on push and pull request.

Windows is absent on purpose — it is a non-priority platform. `crossterm` keeps
it compiling; nobody tests it.

**Tests first:** the workflow is the test. It must be green on `main` before
Phase 1 begins.

**Why H:** standard, copy-shaped.

**Hand over:** the three commands, the two-OS matrix, `main` as the default
branch, and the explicit note that Windows is intentionally excluded.

**Done when:** green on both runners.

---

### P0.3 — wasm32 target check · **H**

A CI job proving invariant **I1**:

```yaml
- run: rustup target add wasm32-unknown-unknown
- run: cargo check -p bam-core --target wasm32-unknown-unknown --no-default-features
```

This is a **required** job, not advisory. It is added now, when `bam-core` is
empty and passing is trivial, precisely so that the first time a host
dependency leaks into the core the build goes red on that commit rather than
six months later when unpicking it is a project.

**Tests first:**
- The job is green on the empty core.
- **Verify the check actually bites**: temporarily add an unconditional
  `use rusqlite::Connection;` (plus a call, so it isn't optimized away as
  unused) to `bam-core/src/lib.rs`, confirm the job fails, then revert. A
  guard nobody has seen fail is not known to work.

> Verified 2026-08-06: `use std::process::Command;` alone does **not** fail
> this check — `wasm32-unknown-unknown`'s std ships a real (always-erroring)
> `std::process` module, so it type-checks fine. An unconditional `rusqlite`
> reference is the sabotage that actually reproduces I1's failure mode, since
> the optional dependency is absent entirely under `--no-default-features`.

**Why H:** six lines of YAML. The reasoning is supplied here; the task is
transcription.

**Hand over:** the YAML above, "this job is required", and the
prove-it-fails step with its exact revert instruction.

**Done when:** green on the real tree, and demonstrated red on the sabotaged
one.

---

### P0.4 — Core purity test · **H**

A `#[test]` in `bam-core` that walks `src/` and fails on two conditions:

1. `rusqlite` named anywhere outside `src/store/` — invariant **I1**'s
   confinement rule, which is what keeps a future wasm backend a
   one-module job rather than a rewrite.
2. `println!` or `eprintln!` anywhere in the crate — invariant **I5**;
   progress leaves the core as typed events, never as text written to a stream
   a library has no business owning.

A test, not a shell script, so it runs under `cargo test` on every platform and
in every contributor's editor.

**Tests first:**
- Passes on the current tree.
- Adding `use rusqlite::Connection;` to `src/lib.rs` makes it fail.
- Adding `println!("x")` to any core file makes it fail.
- A file *inside* `src/store/` naming `rusqlite` does **not** fail it.

**Why H:** roughly twenty lines of directory walk and substring search. Both
rules are stated exactly.

**Hand over:** the two rules, the `src/store/` exemption, "implement as
`#[test]`", and the four cases above as the acceptance list.

**Done when:** all four cases behave as listed.

> Substring matching over source text is crude — it will flag the word
> `rusqlite` in a comment. That is the correct trade: false positives are
> visible and take five seconds to resolve, whereas a real parser here would be
> more machinery than the rule it enforces.

---

**Phase 0 exit:** an empty workspace that builds on Linux and macOS, compiles
to wasm32 without default features, and fails the build if either core
invariant is violated.
