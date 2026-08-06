# Phase 5 — Archive cache, unpackers, extraction, `.uaem`

← [Implementation plan index](../../IMPLEMENTATION_PLAN.md)

---

### P5.1 — `BlobStore` trait and filesystem implementation · **S**

Invariant **I1**: the core does not touch `std::fs` directly.

```rust
trait BlobStore {
    fn put(&self, bytes: impl Read) -> Result<BlobHash>;   // hashes while writing
    fn get(&self, hash: &BlobHash) -> Result<impl Read>;
    fn remove(&self, hash: &BlobHash) -> Result<()>;
}
```

`FsBlobStore` (behind `native`) stores BLAKE3-addressed files at
`cache/blobs/aa/bb/<full-hash>` with two-level fanout, writing to a temp file
and renaming only after the full body is written and hashed. A DB table
`blobs(hash, size, last_used, pinned)` plus `package.archive_hash` maps
packages to blobs (§6).

**Tests first:**
- An interrupted write leaves no file under a real hash name.
- Storing identical bytes twice yields one blob with two package references.
- A corrupted blob is detected by recomputing its hash (§6: verification is
  after the fact, since Aminet publishes no checksums).
- `get` on a missing hash errors rather than panicking.

**Why S:** §6 specifies the layout, the atomicity approach, and the table.

**Hand over:** §6 through "LRU eviction", the trait, the fanout scheme,
invariant I1's feature gating, the four tests.

**Done when:** all four pass.

---

### P5.2 — LRU eviction with pinning · **S**

Evict least-recently-used unpinned blobs down to a configured byte budget.

**Hard invariant** (§6): enrichment rows survive eviction. Otherwise
re-evicting and re-fetching means paying for LLM summarisation twice — the
single most expensive mistake available in this codebase.

**Tests first:**
- Evicting to a small budget removes unpinned blobs and keeps pinned ones.
- **Every `enrichment` row survives eviction** — asserted explicitly.
- `package` rows survive; only `archive_hash` is cleared.
- Eviction order is genuinely least-recently-used.
- Evicting with everything pinned reports that the budget cannot be met rather
  than silently evicting a pinned blob.

**Why S:** a simple policy, but the enrichment-survival rule is exactly what an
eviction routine written from the obvious mental model deletes by accident.

**Hand over:** the `blobs` table, "budget in bytes, configurable", the
enrichment-survival rule stated as a hard invariant, the five tests.

**Done when:** all five pass.

---

### P5.3 — `Unpacker` trait, registry, magic-byte detection · **S**

Invariant **I4**, following the shape set by P2.2.

```rust
trait Unpacker {
    fn id(&self) -> &str;
    fn handles(&self, format: ArchiveFormat) -> bool;
    fn probe(&self) -> Availability;               // is the backend usable here?
    fn unpack(&self, blob: &BlobHash, dest: &Path) -> Result<Vec<ExtractedFile>>;
}
```

**Format detection is by magic bytes, not extension.** Aminet filenames lie
routinely — `.lha` files that are actually LZX are common enough to break an
extension-keyed registry on real data.

Registry selection: config override first, then the first available unpacker
claiming the detected format.

**Tests first:**
- A file named `.lha` whose magic bytes say LZX routes to the LZX unpacker.
- An unknown format errors, naming the leading bytes it could not identify.
- A config override wins over the automatic choice.
- An unpacker whose `probe` reports unavailable is skipped, not attempted.
- With no available unpacker for a format, the error names the format and how
  to install a backend.

**Why S:** the registry pattern already exists in P2.2; this applies it. The
magic-byte requirement is stated.

**Hand over:** invariant I4, P2.2's registry as the reference shape, the trait,
"magic bytes not extension", the five tests.

**Done when:** all five pass.

---

### P5.4 — `unar` backend (out of process) · **S**

Shells out to `unar` (§4). Probes availability at startup and degrades with a
clear message rather than failing opaquely at first use.

**Tests first:**
- An `.lha` and an `.lzx` both extract to the expected file list.
- With `unar` absent, `probe` reports unavailable and the error names the
  missing binary and how to install it.
- A malformed archive produces an error, not a partial extraction left on disk.
- Extraction paths containing `../` are rejected — archives are untrusted
  input, and a path-traversal write outside the destination is the one failure
  here with consequences beyond a bad result.

**Why S:** process spawning behind an interface that already exists.

**Hand over:** §4's "out of process, via `unar`" decision (the licensing
rationale is context this task does not need), the trait, "probe at startup",
the four tests.

**Done when:** all four pass.

---

### P5.5 — `zip` backend (in process) · **H**

The `zip` crate behind the same trait. Aminet does host `.zip` uploads, so this
is a genuinely useful backend — and it exercises the registry across **both**
mechanisms, in-process and out-of-process, which one backend alone cannot do.

**Tests first:**
- A `.zip` fixture extracts to the expected file list.
- The registry routes `.zip` here and `.lha` to `unar`.
- `probe` always reports available (no external binary).
- Path traversal is rejected, as in P5.4.

**Why H:** a crate call behind a trait that already exists, with the tests
enumerated.

**Hand over:** the `Unpacker` trait, the `zip` crate, the four tests, and the
path-traversal requirement copied from P5.4.

**Done when:** all four pass.

---

### P5.6 — LHA extended-header reader · **O**

Read protection bits (the classic `HSPARWED` flags) and file comments from LHA
level-0/1/2 extended headers (§12.1).

Binary format work: header-level detection, per-level offset differences,
extended-header chaining, and the Amiga-specific extension records.

**Tests first:**
- Two real `.lha` fixtures known to carry S-bit scripts have their S-bit and
  E-bit read correctly, **verified against `lha -v` output** as an independent
  reference rather than against this implementation's own belief.
- All three header levels parse, with a fixture for each.
- A file comment round-trips.
- A truncated header errors rather than reading past the buffer.
- An archive with no extended headers yields defaults, not garbage.

**Why O:** binary parsing with no schema to check against and silent failure
modes — wrong offsets yield plausible-looking garbage rather than an error.
§12.1 makes this the difference between "content is viewable" and "content
actually boots", so a subtly wrong reader is worse than no reader at all.

**Hand over:** §12.1 in full, the LHA level-0/1/2 header layout specification,
two `.lha` fixtures with S-bit scripts, and `lha -v` as the reference oracle.

**Done when:** all five test groups pass.

---

### P5.7 — `.uaem` sidecar writer · **H**

Given P5.6's attributes, write `Foo.uaem` next to `Foo`:

```
----rwed 2001-03-11 22:15:00.00 Some comment
```

**Tests first:**
- A known attribute set formats to a byte-exact expected string.
- Flags absent render as `-`; the order is `hsparwed`, lowercase when set.
- The fractional second field is hundredths.
- A file with no comment omits the trailing field cleanly.
- A comment containing a newline is rejected or escaped, never written raw —
  it would corrupt the sidecar format.

**Why H:** pure formatting, with the exact output format given.

**Hand over:** the format line, the flag order and casing rule, "timestamp is
the archive's mtime, fraction is hundredths", P5.6's attribute struct, the five
tests.

**Done when:** all five pass.

---

### P5.8 — Archive inventory enrichment · **S**

Extract to a temp directory, walk it, record the file list with paths, sizes
and detected types into `enrichment` with `kind = 'inventory'`,
`producer_version = 1`. Discard the temp extraction — the inventory is the
durable artifact.

**Tests first:**
- An archive's inventory matches its actual contents.
- The temp directory is removed afterwards, including on error.
- The inventory **survives blob eviction** (P5.2's invariant, re-asserted here
  from the consumer's side).
- Re-running with the same `producer_version` is a no-op; bumping the version
  reprocesses.

**Why S:** composes P5.3 with the enrichment table, both of which exist.

**Hand over:** the `Unpacker` trait, the enrichment row shape, "temp discarded,
inventory persists", the four tests.

**Done when:** all four pass.

---

**Phase 5 exit:** archives cached by content hash, extracted through a
registry with two working backends, with Amiga attributes preserved as `.uaem`
sidecars. This completes §15's "hard core" — reassess sequencing here.
