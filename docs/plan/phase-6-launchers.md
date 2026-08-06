# Phase 6 — Emulator launchers

← [Implementation plan index](../../IMPLEMENTATION_PLAN.md)

`bam-handoff.md` §14 asked which emulator to target. The answer is that the
question dissolves: launchers are pluggable (invariant **I4**), and FS-UAE is
simply the first implementation because it runs on both primary platforms.

This phase is therefore **no longer blocked**.

---

### P6.1 — `Launcher` trait, registry, probing · **O**

```rust
trait Launcher {
    fn id(&self) -> &str;                          // "fs-uae"
    fn probe(&self) -> Availability;               // installed? where?
    fn capabilities(&self) -> LauncherCaps;
    fn launch(&self, req: LaunchRequest) -> Result<LaunchHandle>;
}

struct LauncherCaps {
    directory_volume: bool,     // can mount a host directory as a volume?
    uaem_sidecars: bool,        // honours .uaem attribute files?
    hardfile: bool,
    adf: bool,
}
```

`capabilities()` **drives behaviour, it does not merely report it.** vAmiga's
weak directory-volume support (§12) becomes a flag the core reads to choose
between the `.uaem` directory-volume path and building a disk image. Without
this, the emulator choice leaks into extraction logic and every future launcher
means touching Phase 5 code.

Registry selection: config override first, then the highest-preference
available launcher whose capabilities satisfy the request.

**Tests first:**
- Two stub launchers register; the registry picks by configured preference.
- A launcher whose `probe` reports unavailable is never selected.
- A request needing `directory_volume` skips a launcher lacking it, even when
  that launcher is otherwise preferred.
- With no launcher able to satisfy a request, the error names the capability
  that could not be met — not a generic "no launcher found".
- Config override wins, and overriding to an unavailable launcher errors
  clearly rather than falling back silently.

**Why O:** the third registry, and the one whose `capabilities` contract
determines whether Phase 5's extraction path stays emulator-agnostic. The
"capabilities drive behaviour" decision is the whole design; a cheaper model
would plausibly produce a registry that reports capabilities nobody reads.

**Hand over:** invariant I4, P2.2 and P5.3 as the established registry shape,
§12 and §12.1, the trait above, the five tests.

**Done when:** all five pass.

---

### P6.2 — FS-UAE launcher · **S**

Extract an archive to a scratch directory (P5.4/P5.5), write `.uaem` sidecars
(P5.7), generate an FS-UAE configuration pointing a directory volume at it,
spawn the process.

FS-UAE runs on both macOS and Linux, which is why it is first.

**Tests first:**
- Generated config for a known request matches an expected fixture, field for
  field.
- `probe` finds FS-UAE at the platform default path and reports unavailable
  when absent.
- `capabilities()` reports `directory_volume: true`, `uaem_sidecars: true`.
- Scratch directories are cleaned up when the handle is dropped.
- One `#[ignore]`d manual test: launching an archive containing a
  startup-sequence script shows the script **running** — the real end-to-end
  check of whether P5.6 and P5.7 actually worked.

**Why S:** config templating plus a process spawn, behind a trait that already
exists.

**Hand over:** FS-UAE's config format, P5.3's unpacker registry, P5.7's `.uaem`
writer, the `Launcher` trait, the five tests.

**Done when:** the four automated tests pass; the ignored one is run manually
once and its result recorded in `PROGRESS.md`.

> That manual test is the only place in the plan where P5.6's binary-header
> work is validated against reality rather than against `lha -v`. Do not skip
> it.

---

### P6.3 — Launcher configuration in `bam.toml` · **H**

Binary path, extra arguments, scratch directory, and a preference order across
launchers. Per-platform default candidate paths so a standard installation
needs no configuration at all:

- macOS: `/Applications/FS-UAE.app/Contents/MacOS/fs-uae`, Homebrew paths
- Linux: `/usr/bin/fs-uae`, `/usr/local/bin/fs-uae`, Flatpak path

**Tests first:**
- With no config, the platform default candidates are probed in order.
- An explicit path overrides the candidates.
- Extra arguments reach the spawned command line.
- An unknown launcher id in the preference list errors, naming it.

**Why H:** a config struct plus a per-platform default table.

**Hand over:** the field list, the candidate paths above, the existing
`bam.toml` parsing, the four tests.

**Done when:** all four pass.

---

### P6.4 — Launch a selection · **S**

Invariant **I7**'s first bulk consumer: launch every archive in a selection,
sequentially, with a confirmation when the selection exceeds a threshold.

**Tests first:**
- Launching a three-member selection produces three launches in order.
- A selection larger than the threshold prompts before proceeding.
- A failure on member two reports it and continues to three rather than
  aborting the batch — with a summary at the end naming what failed.
- Cancelling mid-batch stops cleanly and reports how many ran.

**Why S:** iteration over an API that exists, with the error-handling policy
stated.

**Hand over:** P2.7's selection API, the `Launcher` registry, the
continue-on-failure policy, the four tests.

**Done when:** all four pass.

---

## Follow-on launchers — unscheduled

Adding either touches no existing code; that is the point of P6.1.

| Launcher | Platform | Note |
|---|---|---|
| Amiberry | Linux | Good directory-volume support |
| vAmiga | macOS | Weak directory-volume support — the case `LauncherCaps` exists for; will route to the disk-image path |
| WinUAE | Windows | Only if Windows becomes a priority |

---

**Phase 6 exit:** an archive selected in the TUI opens in an emulator with its
contents mounted and its scripts actually runnable.
