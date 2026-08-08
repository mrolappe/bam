# Phase 6 — Emulator launchers: round-by-round log

← [PROGRESS.md](../../PROGRESS.md) · [docs/plan/phase-6-launchers.md](../plan/phase-6-launchers.md)

## Round 40 — 2026-08-08 · Phase 6: `Launcher` trait, registry, probing (P6.1)

`crates/bam-core/src/launch/mod.rs` adds the third I4 registry, mirroring
`query::lang::LanguageRegistry` and `unpack::UnpackerRegistry`: `Launcher`
(`id`/`probe`/`capabilities`/`launch`), `LauncherCaps` (`directory_volume`,
`uaem_sidecars`, `hardfile`, `adf`), and `LauncherRegistry::select` — config
override first, else the first registered (preference-ordered) launcher
that is both available and capability-sufficient. `Availability` is reused
from `unpack` rather than redefined. Selection failure names the specific
unmet capability (`LauncherError::CapabilityUnmet`) rather than a generic
"no launcher found"; an unavailable override errors as `Unavailable(id)`
instead of silently falling back. 6 tests in `tests/launch.rs` cover the
phase doc's five plus config-override-wins. 214 tests total (6 added).
`cargo fmt`, `clippy`, and the wasm32 build all clean.

## Round 41 — 2026-08-08 · Phase 6: FS-UAE launcher (P6.2)

`crates/bam-core/src/launch/fs_uae.rs` adds `FsUaeLauncher<S: BlobStore>`:
extracts an archive to a scratch directory via the P5.3 `UnpackerRegistry`,
writes `.uaem` sidecars (P5.7's `write_sidecar`) for entries whose LHA
header carried Amiga protection/comment data, renders an FS-UAE config
(`hard_drive_0 = <scratch>/volume`, FS-UAE's own documented directory-as-
hard-drive support), and spawns `fs-uae` against it. `probe`/`launch` take
their binary candidates via `with_candidates` (default: per-platform
hardcoded paths) rather than baking in real system paths, so both are
testable without a real FS-UAE install.

Getting sidecars right needed one gap closed first: nothing previously
walked a *multi-entry* LHA archive — only `parse_lha_header` for one header
at a time existed. `unpack::lha_header` gained a `compressed_size` field
(the header's own documented base-layout field, offset 7..11, common to all
three levels) and `list_headers`, which repeatedly parses-and-skips to
collect every entry's header; `launch()` reads the archive's raw bytes back
out of the `BlobStore`, walks them, and matches entries to extracted files
by filename.

`LaunchRequest` gained `archive: Option<LaunchArchive>` (blob hash +
format — P6.1 only exercised capability-based selection, never what to
launch) and `LaunchHandle` gained `scratch_dir: Option<PathBuf>` with a
`Drop` impl that removes it, so a launched archive's extracted copy doesn't
outlive the handle. Both changes are additive; all six P6.1 tests still
pass unchanged bar two struct-literal updates for the new fields.

6 new automated tests (4 in `tests/launch_fs_uae.rs` per the phase doc's
list, plus 2 covering `list_headers` in `tests/unpack_lha_header.rs`) —
220 passing total, up from 214, plus the one `#[ignore]`d manual test
below. `cargo fmt`, `cargo clippy --workspace --all-targets -- -D
warnings`, and the wasm32 `--no-default-features` build are all clean.

**Manual test — run, 2026-08-08. Script ran; the attribute round-trip did
not get validated — a real gap found, not closed.** FS-UAE turned out to
already be installed locally (`/Applications/FS-UAE.app/...` — `probe()`'s
default candidates found it; `which fs-uae` alone missed it since it ships
as a `.app` bundle, not on `PATH`). The fixture at
`tests/fixtures/archives/startup_sequence.lha` (`s/startup-sequence`,
`echo "bam!"`) is genuinely Amiga-built — packed with `lha`/`lharc` running
inside FS-UAE itself, not a host tool. `manual_launch_runs_the_startup_sequence_script`
launched FS-UAE and the script **did run**, confirmed visually.

But tracing what `list_headers` actually read from this real archive's
bytes shows why that pass doesn't mean what it looks like it means:
`LhaFileHeader { protection: None, comment: None, .. }`. The header's
level-1 extended-header chain holds a directory-name block (type `0x02`,
"s") and a 2-byte block of type `0x00` — almost certainly the
generic LHA header-CRC extension every `lha` build emits, not Amiga
protection data. Neither matches `AMIGA_EXT_TYPE = 0x47`, the placeholder
guess `unpack::lha_header`'s module doc has flagged since Round 32 as
"untested against real Amiga data." With `protection`/`comment` both
`None`, `write_sidecars` wrote **no `.uaem` sidecar** — the script running
anyway is best explained by FS-UAE's synthesized directory volume
defaulting an attribute-less extracted file to executable-permitted (the
same default AmigaDOS itself uses for a file with no recorded protection),
not by the sidecar mechanism working. **First real evidence the
`AMIGA_EXT_TYPE`/`AMIGA_OS_ID` placeholder is wrong** (or at least doesn't
match what this Amiga-native `lha` produces) — real data the module
previously had none of, but the actual protection-bit extension format is
still unknown. Left as a known, flagged gap rather than guessed at further;
`tests/fixtures/archives/startup_sequence.lha` is committed as a real data
point for whoever picks this up (try an archive with `Protect FILE -e` run
first and diff the header bytes against this one).

## Round 42 — 2026-08-08 · Phase 6: launcher configuration (P6.3)

`crates/bam-core/src/launch/mod.rs` adds `LaunchConfig`/`LauncherOverride`
(`Deserialize`, mirroring `bam_tui::input::KeymapConfig`'s pattern — `bam-core`
gains no `toml` dependency, only `serde`; the caller in `bam-tui` does the
actual `toml::from_str`), `resolve_candidates` (an explicit configured path
replaces the platform-default list outright, else defaults are probed in
order), and `LauncherRegistry::apply_preference` (reorders registered
launchers by a preference list, unlisted ones keeping their relative
registration order after the preferred ones; errors naming the id on any
unregistered entry). `FsUaeLauncher` gained `with_candidates_and_args` and an
`extra_args` field threaded into the spawned `Command`. `with_candidates`
still exists unchanged (delegates to the new constructor with empty args),
so all six P6.1/P6.2 tests still pass untouched.

5 new tests in `tests/launch_config.rs` (the phase doc's four, plus one
covering `apply_preference` reordering `select`'s outcome directly) — 225
passing total, up from 220. `cargo fmt`, `cargo clippy --workspace
--all-targets --features native -- -D warnings`, and the wasm32
`--no-default-features` build are all clean.

## Round 43 — 2026-08-08 · Phase 6: launch a selection (P6.4, Phase 6 exit)

`crates/bam-core/src/store/launch_selection.rs` adds `launch_selection`:
given `package_ids` (resolving a selection to ids stays the caller's job,
same division of labor as P7.5's `summaries::run_batch`), it resolves each
member's cached archive (`tables::get_archive_hash` plus a 16-byte
`BlobStore::get` read into `unpack::detect_format` — no need to read a whole
archive twice when the chosen `Launcher` re-reads it fully anyway), asks the
P6.1 `LauncherRegistry` to launch it, sequentially, and continues past a
per-member failure rather than aborting the batch. `package_ids.len() >
threshold` without `confirmed: true` errors
`LaunchSelectionError::ConfirmationRequired` before anything launches — the
same structural gate `summaries::run_batch` uses for its cost estimate,
here over a plain count instead of a token estimate. `cancel:
&CancellationToken` is checked before each member, so a mid-batch cancel
stops cleanly and `LaunchSelectionOutcome` still reports what ran. Lives at
the `store::` level, not wired into `bam_core::api`, since nothing else at
that layer needs it yet — `summaries`/`embeddings` are free functions for
the same reason (I1's rusqlite confinement is what forces `store::`, not
the API layer's session-scoped contract).

4 new tests in `tests/launch_selection.rs` (the phase doc's four) — 229
passing total, up from 225. `cargo fmt`, `cargo clippy --workspace
--all-targets --features native -- -D warnings`, and the wasm32
`--no-default-features` build are all clean.

**Phase 6 exit reached** on the core side: a selection's cached archives
launch through a pluggable, capability-driven registry with
continue-on-failure and a confirmation gate. Wiring an actual TUI keybinding
(`S`) to this function is unscheduled follow-on work, not blocking — the
phase doc's four P6.4 tests target the core batch behavior only.
