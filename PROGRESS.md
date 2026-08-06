# bam — Progress

Living status file. Updated at the end of every implementation round.
Task IDs refer to `IMPLEMENTATION_PLAN.md`.

---

## Round 0 — 2026-08-06 · Setup and planning

**Done:**

- Git repo initialized, `bam-handoff.md` committed.
- Public GitHub repo created and pushed: https://github.com/mrolappe/bam
- `IMPLEMENTATION_PLAN.md` written — ~45 tasks across 8 phases, each with a
  minimum model tier, the context to hand over when delegating, and a
  done-when check.
- Round-end workflow and the KeePassXC/SSH reminder recorded in project memory.

**No code yet.** The workspace does not exist.

---

## Next task

**Round 1 — P0.1, P0.2, P1.1** (all Haiku-tier)

1. **P0.1** — Cargo workspace: root `Cargo.toml` (resolver 2, edition 2024,
   `rust-version = "1.85"`), `crates/bam-core` (lib), `crates/bam-tui`
   (bin, named `bam`). Shared deps in `[workspace.dependencies]`: `serde`
   (derive), `thiserror`, `rusqlite` (`bundled`, `fts5`).
   Do **not** create `bam-gui`, `bam-mcp`, or a plugins crate.
2. **P0.2** — GitHub Actions: `cargo fmt --check`, `cargo clippy -- -D warnings`,
   `cargo test --workspace`. Ubuntu only — cross-platform CI waits on the
   target-OS decision.
3. **P1.1** — Fetch and commit fixtures to `crates/bam-core/tests/fixtures/`
   from `https://ftp.fau.de/aminet/`: a ~500-line `index_sample.txt`, plus
   `recent_sample.txt` and `tree_sample.txt`. The INDEX sample must include
   long filenames, descriptions with internal whitespace runs, non-ASCII
   bytes, a zero-size entry, and the header/preamble lines. Record the source
   URL and fetch date in `fixtures/README.md`.

**Round 1 ends when** `cargo build --workspace` succeeds, CI is green, and the
three fixtures are committed.

---

## Decisions carried forward

- Landing tables store **bytes (BLOB)**, never TEXT — encoding is detected
  later and must stay correctable without re-fetching.
- `package.date_precision` distinguishes INDEX-derived (±1 week) dates from
  exact ones. `week` may be upgraded to `exact`, never the reverse.
- No `println!`/`eprintln!` in `bam-core`, ever — progress goes through
  `ProgressSink`. Enforced from P1.10 onward; it is what keeps `bam-mcp` a thin
  adapter rather than a rewrite.
- The DSL grammar (P2.1) is written once in `docs/dsl.md` and is the source for
  the parser, the SQL compiler, GBNF, and JSON Schema. Do not fork it.

## Open questions — unanswered, and what they block

From `bam-handoff.md` §14. Nothing in Rounds 1–7 is blocked by these.

- **Target emulator** (FS-UAE / WinUAE / vAmiga) — blocks all of Phase 5.
- **Target OS platforms** — blocks cross-platform CI and all of Phase 7.
- **LLM provider default** (local vs. cloud) — affects Phase 6 emphasis only.
- **Mirror rsync access** — ask before the bulk readme harvest in P3.3. A
  single rsync pass beats 84,000 HTTP requests; worth an email to the ftp.fau.de
  operator well ahead of Phase 3.
