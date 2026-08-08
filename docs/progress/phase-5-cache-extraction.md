# Phase 5 progress — Archive cache, unpackers, extraction, `.uaem`

← [PROGRESS.md](../../PROGRESS.md)

Round-by-round log for Phase 5 (Rounds 27–34), extracted from the top-level
progress file to keep that file scannable. Task ids refer to
[`IMPLEMENTATION_PLAN.md`](../../IMPLEMENTATION_PLAN.md) and
[phase-5-cache-extraction.md](../plan/phase-5-cache-extraction.md).

---

## Round 27 — 2026-08-07 · `BlobStore` trait + filesystem implementation (P5.1) — Phase 5 start

**Done:**

- **P5.1** — `crates/bam-core/src/blob/mod.rs`: `BlobHash` (a hex-encoded
  BLAKE3 digest, `Serialize`/`Deserialize` since it'll later live in
  `package.archive_hash`) and the `BlobStore` trait (`put`/`get`/`remove`),
  ungated — following the `HttpClient` pattern (P1.9/Round 5) exactly:
  plain trait in the crate root so a fake can be tested with no `native`
  feature, real implementation behind it. `blake3` added as a new,
  unconditional workspace dependency (pure computation, no OS dependency —
  confirmed wasm32-safe by the `--no-default-features` check, same tier as
  `chardetng`/`encoding_rs`).

  `FsBlobStore` (`blob/fs_store.rs`, `native`-gated) stores files at
  `<root>/aa/bb/<full-hash>`, two-level fanout. `put` streams the reader into
  a temp file while hashing; the real hash — and hence the real path — isn't
  known until the read completes, so an interrupted read can structurally
  never leave anything under a real hash name, satisfying that test without
  needing separate cleanup logic for the common case (the temp file itself is
  still removed on error, for hygiene, not because a test requires it).
  Identical content hashes identically, so a second `put` of the same bytes
  finds `dest.exists()` and drops its own temp copy rather than re-writing —
  dedup by construction, not a separate check. `get` reads the whole blob,
  recomputes its hash, and compares before returning a `Cursor` over the
  bytes — the after-the-fact verification §6 asks for, since Aminet
  publishes no checksums to check against up front; marked with a `ponytail:`
  comment (rehashes on every `get`, fine at Aminet archive sizes, revisit
  with streaming verification if that stops being true). Four tests in the
  new `tests/blob.rs`, matching the phase doc's four bullets exactly: a
  `Read` that yields some bytes then errors, asserted to leave zero files
  under the store root; identical bytes put twice yield the same hash and
  exactly one file on disk (the "two package references" half of that
  bullet is a `package`/`archive_hash` concern with no table wired to it
  yet — see the deviation note); a blob tampered with directly on disk
  fails `get` with `BlobError::Corrupted`; a hash that was never stored
  fails `get` with `BlobError::NotFound` rather than panicking.

151 tests total (4 new + 147 pre-existing; 149 run, 2 ignored — the two
pre-existing real-mirror tests). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean.

**Deviations for the next session to know about:**
- No `blobs` DB table or `package.archive_hash` column yet, despite §6
  describing both. P5.1's own hand-over list ("the trait, the fanout scheme,
  invariant I1's feature gating, the four tests") names no table, and none of
  the four tests need one — `BlobStore` alone is filesystem-only. P5.2's LRU
  eviction is the first task that actually needs `blobs(hash, size,
  last_used, pinned)` to operate on, so that migration is left for it rather
  than added speculatively here.
- P5.1's "storing identical bytes twice yields one blob with **two package
  references**" bullet is only half-tested: the blob-level half (one file,
  same hash) is; the package-level half needs `package.archive_hash`, which
  doesn't exist yet (see above). Flagged per the same convention as prior
  rounds' doc/reality gaps.

---

## Round 28 — 2026-08-07 · LRU eviction with pinning (P5.2)

**Done:**

- **P5.2** — `crates/bam-core/migrations/0006_blobs.sql`, migration 6:
  `blobs(hash PRIMARY KEY, size, last_used, pinned)` plus `ALTER TABLE package
  ADD COLUMN archive_hash TEXT REFERENCES blobs(hash)` — the §6 mapping table
  P5.1 deliberately deferred (Round 27's own note: "P5.2's LRU eviction is the
  first task that actually needs `blobs`"). `archive_hash` was **not** added
  to the shared `Package` struct/`insert_package`/`get_package` — every
  pre-P5.2 caller across the codebase constructs a `Package` literal (17 call
  sites, confirmed by trying the struct-field approach first and watching it
  break all of them), and nothing outside this round's own eviction code needs
  to read or write the column yet. Two small raw accessors,
  `tables::{set_archive_hash, get_archive_hash}`, cover exactly what P5.2
  needs instead.

  New `store::blob_cache` (native-gated, inside `store::` per I1):
  `record_blob`/`touch`/`set_pinned` (thin upserts/updates on `blobs`), and
  `evict_to_budget<B: BlobStore>(conn, store, budget_bytes)` — generic over
  the trait (not `dyn`, since `BlobStore::get` returns `impl Read`, not
  object-safe), so a test can pass a real `FsBlobStore`. Loops: while total
  `blobs` size exceeds budget, pick the least-recently-used **unpinned** blob
  (`ORDER BY last_used ASC LIMIT 1 WHERE pinned = 0`), remove its bytes via
  `store.remove`, clear `archive_hash` on every package that pointed to it,
  then delete its `blobs` row — **in that order**: deleting the `blobs` row
  before clearing the referencing `package.archive_hash` trips the FK added
  by this same migration (`foreign_keys` is on by default via `store::open`)
  and was caught by the first test run, not reasoned out in advance. Never
  touches `package`(rows) or `enrichment` — the eviction loop has no DELETE
  against either table, so the hard invariant ("enrichment rows survive
  eviction... the single most expensive mistake available in this codebase")
  holds by the code simply not containing the capability to violate it, not
  by a guard checking for it. Runs out of unpinned blobs before the budget is
  met → `EvictionError::BudgetNotMet`, evicting nothing further; a pinned
  blob is never a candidate the loop even considers, not one it considers and
  rejects.

  Five tests in the new `tests/store_blob_cache.rs`, matching the phase doc's
  five bullets exactly, all against a real `FsBlobStore` over a temp
  directory and real inserted blobs (not hand-written `blobs` rows with no
  backing file) so `store.get`/`store.remove` genuinely succeed or fail:
  a two-blob DB evicted to the pinned blob's own byte size keeps the pinned
  file on disk and removes the other; an `enrichment` row on the evicted
  package's own id is read back unchanged after eviction; `get_package`
  still returns the row and `archive_hash` reads back `None` (not the stale
  hash); three blobs with strictly increasing `last_used`, evicted to a
  one-blob budget, removes the two oldest in age order, not insertion or
  hash order; an all-pinned single-blob DB evicted to budget 0 returns
  `BudgetNotMet` and the blob is still readable afterward.

  `tests/migrations.rs`'s `db_at_version_n_only_runs_migrations_above_n` hit
  a new variant of the false-vacuous-pass pattern Round 5/20/23/25 already
  flagged for migrations 2-5: this is the first migration whose DDL
  (`ALTER TABLE package`) requires a *prior* migration's table to actually
  exist, which the test's own "stamp `user_version = 1`, never run migration
  1's DDL" setup doesn't provide — caught as a real test failure (`no such
  table: package`), not a false pass. Fixed by running migration 1's DDL
  directly (bypassing `apply_migrations`, so it isn't the thing under test)
  before stamping the version; the test still proves migration 1 itself
  doesn't *re*-run (a second `CREATE TABLE package` would collide and the
  `unwrap()` would fail) while assertion now covers the full table set.

154 tests total (5 new + verified pre-existing baseline; summed directly via
`cargo test --workspace 2>&1 | grep "test result: ok" | ...`, not taken from
the prior round's stated count — Round 10/14's own caution, and Round 10's
own count was off by one previously). `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean. Also smoke-tested the real `bam`
binary (`ingest --offline`): still reports 501 packages, unaffected by this
round's schema/store-only changes.

**Deviations for the next session to know about:**
- `package.archive_hash` exists in the schema but is read/written only by
  this round's own `store::blob_cache` and its raw `tables::` accessors —
  not by the shared `Package` struct, `insert_package`, or `get_package`.
  Whichever future task actually sets it on a real fetch (P5.4/P5.5's
  unpacker backends, or a caching layer over `store::fetch`) will need either
  these same raw accessors or a considered decision to fold the field into
  `Package` at that point, updating all 17 existing call sites.
- `evict_to_budget` recomputes `SELECT SUM(size)` from `blobs` on every loop
  iteration rather than tracking a running total in memory — same
  "simplest correct thing at current scale" call Round 25 made for
  `rebuild_fts`'s per-package readme lookups; revisit if eviction over a
  large `blobs` table is ever measured to be slow.

---

## Round 29 — 2026-08-07 · `Unpacker` trait, registry, magic-byte detection (P5.3)

**Done:**

- **P5.3** — `crates/bam-core/src/unpack/mod.rs`, new module, ungated per I1
  (mirrors `blob`'s shape: trait plus plain types compile to wasm32, only a
  real backend would be `native`-gated — P5.3 adds no backend, that's
  P5.4/P5.5). `Unpacker` trait (`id`/`handles`/`probe`/`unpack`) matches the
  phase doc's signature exactly. `UnpackerRegistry` follows P2.2's
  `LanguageRegistry` shape (`Vec<Box<dyn Unpacker>>`, not `dyn`-map) but its
  `select(format, override_id)` differs from `LanguageRegistry::get`'s plain
  id-lookup-with-default: it has two axes to satisfy (does an unpacker claim
  this *format*, and does it *probe* available) where the query-language
  registry only had one (id match), so selection is config override first —
  looked up by id, still required to `handle()` the format and `probe()`
  available, erroring rather than silently falling through if not — then the
  first registered unpacker that both claims the format and probes
  available.

  `detect_format` reads magic bytes only, never the filename: LZX archives
  open with the literal 4-byte signature `LZX\0`; LHA/LZH archives have no
  fixed leading signature (bytes 0–1 are header size and checksum) but always
  spell their method id as `-lh?-`/`-lz?-` at offset 2, which is what's
  checked. An unrecognized format's error carries the first 8 leading bytes
  for diagnosis rather than just saying "unknown".

  Six tests in the new `tests/unpack.rs` (the phase doc names five; a real
  magic-byte LHA-detects-as-LHA case was split out from the "lies about its
  extension" case as its own test, since the plan's "file named `.lha`
  routes to LZX" bullet doesn't by itself prove plain LHA bytes are also read
  correctly): a file with an LHA-shaped name but LZX magic bytes detects as
  LZX; genuine `-lh5-` magic bytes detect as LHA; unrecognized bytes error
  with the leading 8 bytes attached; a config override selects a specific
  registered unpacker over another that would otherwise win by list order;
  an unavailable unpacker (`probe` returning `Unavailable`) is skipped in
  favour of a working one rather than attempted; no available unpacker for a
  format produces an error whose text names both the format and an install
  hint. All six use fake in-test `Unpacker` impls returning `Ok(vec![])` from
  `unpack` — P5.3 is trait-plus-registry only, no real extraction is wired up
  or exercised yet (that's P5.4's `unar` backend and P5.5's `zip` backend).

160 tests total (6 new + 154 pre-existing, summed directly via `cargo test
--workspace 2>&1 | grep "test result: ok" | ...`). `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean. Also smoke-tested the real `bam`
binary (`ingest --offline`): still reports 501 packages, unaffected by this
round's new, self-contained module.

**Deviations for the next session to know about:**
- None. This round's scope matched the phase doc exactly — no table, no
  backend, no filesystem access; `ArchiveFormat` currently only has `Lha`
  and `Lzx` variants since those are the only two formats named anywhere in
  the plan (§4/P5.4/P5.5); a future zip-fixture round adds `Zip` when P5.5
  needs it.

---

## Round 30 — 2026-08-07 · `unar` backend, out of process (P5.4)

**Done:**

- **P5.4** — `crates/bam-core/src/unpack/unar.rs`, `native`-gated:
  `UnarUnpacker<S: BlobStore>` (generic the same way `evict_to_budget` is —
  `unpack()` only receives a `BlobHash`, so the backend has to hold a store
  to turn that hash into archive bytes; the trait itself stays object-safe
  since erasure happens at the concrete `S`). `unpack()` writes the blob to
  a private scratch dir, lists entries via `lsar -json` and rejects any
  whose name contains a `..`/root component *before* running `unar` at all,
  then extracts into a second scratch subdirectory and only moves files
  into `dest` once `unar` exits successfully — so a malformed or malicious
  archive structurally cannot leave partial or escaped output under `dest`,
  regardless of what `unar` itself did or didn't write, matching P5.1's
  "make the bad state unreachable, not cleaned up after" pattern.
  `probe()` shells `unar -v` and `lsar -v` (`lsar` is required for the
  traversal check, so its absence is unavailability too) and names
  whichever binary is missing plus an install hint (`brew`/`apt`).

  New `UnpackError` variants: `Blob` (wraps `BlobError` from
  `store.get`), `PathTraversal { entry }`, `ExtractionFailed { message }`.

  Fixtures added under `tests/fixtures/archives/`: `sample.lha` (two files,
  `a.txt`/`sub/b.txt`, built with the system `lha` tool), `malformed.lha`
  (arbitrary non-archive bytes), and `sample.lzx` (same two-file spec) —
  no LZX *compressor* is available on this machine or via Homebrew (`unar`
  only decompresses), so the user built this one in an Amiga emulator and
  dropped it into the fixtures directory mid-round.
  `lzx_fixture_extracts_to_expected_file_list` was written to check for the
  file's existence and skip with a message if absent rather than failing or
  being `#[ignore]`d, precisely so it would start asserting for real the
  moment the fixture landed with no test-code change — which is exactly
  what happened.

  Five new tests: `lha_fixture_extracts_to_expected_file_list` and the
  (currently-skipping) LZX counterpart in `tests/unpack_unar.rs`;
  `malformed_archive_errors_without_partial_extraction` (asserts `dest` has
  zero entries after the error, not just that an error occurred);
  `traversal_entry_in_zip_archive_is_rejected` — `lha`/`lzx` archivers both
  sanitize a leading `../` at creation time, so a genuinely malicious *LHA*
  fixture can't be built with the tools on hand; a `.zip` (which `unar`
  also reads, and which does not sanitize member names — verified by hand
  against `lsar -json` output first) stands in to prove the rejection is
  real end-to-end rather than only unit-tested against a crafted string;
  `unar_absent_reports_unavailable_naming_the_binary_and_install_hint` is
  split into its own test binary
  (`tests/unpack_unar_unavailable.rs`) since it's the one scenario that
  needs `PATH` genuinely broken and mutating process-global env is only
  safe with nothing else in the same process to race it.

  `.github/workflows/ci.yml` now installs `unar` (`apt`/`brew`, matrix-keyed
  on OS) before the test step on both runners — without it these tests
  would silently pass locally and fail (or need to skip) in CI; installing
  the real dependency was the smaller change than teaching every test to
  degrade.

165 tests total (5 new + 160 pre-existing, summed directly via `cargo test
--workspace 2>&1 | grep "test result: ok" | ...`). `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean (the new module is entirely
`native`-gated, so it doesn't touch the wasm32 build). Also smoke-tested the
real `bam` binary (`ingest --offline`): still reports 501 packages.

**Deviations for the next session to know about:**
- None. `sample.lzx` arrived (user-built in an Amiga emulator) before this
  round closed, so all five tests, including the LZX round trip, run and
  pass for real — see the updated count below.

---

## Round 31 — 2026-08-07 · `zip` backend, in process (P5.5)

**Done:**

- **P5.5** — `crates/bam-core/src/unpack/zip_backend.rs`, `native`-gated:
  `ZipUnpacker<S: BlobStore>`, generic the same way `UnarUnpacker` is. Adds
  `ArchiveFormat::Zip`, detected from the `PK\x03\x04` local-file-header
  magic bytes (§P5.3's "magic bytes, never extension" rule extended to the
  new format). `probe()` always reports `Available` — no external binary,
  the one difference the phase doc calls out between this backend and
  `unar`'s. Path-traversal rejection reuses `ZipFile::enclosed_name()`
  (returns `None` for a `../`/absolute member name) rather than
  reimplementing `unar`'s hand-rolled component check against `lsar -json`
  output — the crate already does the validation `unar` has to do by hand
  because it has no equivalent in-process. Checked over every entry in a
  first pass before any file is written, so a traversal entry anywhere in
  the archive leaves nothing under `dest`, matching `unar`'s all-or-nothing
  extraction from P5.4.

  `zip` added as a new workspace dependency, `default-features = false,
  features = ["deflate"]` — the default feature set pulls in bzip2/lzma/
  zstd/aes-crypto, none of which this backend's fixtures or Aminet's real
  `.zip` uploads need; `deflate` alone covers the standard method the
  system `zip` tool also uses. `native`-gated via `dep:zip` in
  `bam-core`'s `native` feature, same as `rusqlite`/`reqwest`/`tokio` —
  writing extracted files still goes through `std::fs`, so I1's wasm32
  boundary applies here exactly as it does to `unar`.

  Four tests in the new `tests/unpack_zip.rs`, matching the phase doc's
  four bullets exactly, fixtures built in-test with `zip::ZipWriter`
  rather than checked-in binary files (no external `zip`/`unzip` tool
  needed, unlike P5.4's LHA/LZX fixtures which had no in-process writer
  available): a two-file zip built at test time extracts to the expected
  file list; a registry with both `ZipUnpacker` and `UnarUnpacker`
  registered routes a zip-magic-bytes archive to `"zip"` and the existing
  `sample.lha` fixture to `"unar"` via `detect_format`; `probe()` reports
  `Available` with no setup; a zip containing one safe and one `../`
  entry is rejected with `PathTraversal` and leaves `dest` empty or
  absent.

169 tests total (4 new + 165 pre-existing, summed directly via `cargo test
--workspace 2>&1 | grep "test result: ok" | ...`). `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean (the new module is entirely
`native`-gated). Also smoke-tested the real `bam` binary (`ingest
--offline`): still reports 501 packages, unaffected by this round's
self-contained new backend.

**Deviations for the next session to know about:**
- None. This round's scope matched the phase doc exactly.

---

## Round 32 — 2026-08-07 · LHA extended-header reader (P5.6)

**Done:**

- **P5.6** — `crates/bam-core/src/unpack/lha_header.rs`, new module, ungated
  (pure byte-slice parsing, no I/O — same shape as `detect_format`).
  `parse_lha_header(bytes) -> (LhaFileHeader, bytes_consumed)` handles all
  three header levels. Levels 1/2 walk the documented extended-header chain
  (`[type][data][next-size]`, terminated by `next-size == 0`); level 0 reads
  the fixed fields directly and, if `header_size` claims more bytes than the
  standard layout accounts for, treats the trailing bytes as an OS-specific
  extension keyed by a leading OS-ID byte — the same mechanism level-0
  archives built by the system `lha` tool were found to use for embedding
  Unix permissions (confirmed by hand-decoding a real level-0 fixture against
  `lha v`'s own listing while writing this).

  **Deviation flagged in the code itself, not just here:** research this
  round found no authoritative spec for the *Amiga-specific* extended-header
  content (protection bits/comment) — neither the LHa-for-UNIX header spec
  nor libarchive's LHA reader documents it, and the real AmigaOS `LhA`
  archiver has no source available to consult. Per the user's explicit
  decision this round (asked directly, given a real spec gap: "best-effort +
  ponytail flag, real fixtures later" over waiting or skipping), the OS-ID
  byte `'A'` and the extension layout `[protection: u32 LE][comment_len: u8
  or via ext-type 0x47 for level 1/2][comment]` are this codebase's own
  invented placeholder, marked with a `ponytail:` comment naming exactly
  what's unverified and what would replace it (a real Amiga-built `.lha` and
  an `lha -v`-equivalent oracle). `ProtectionBits::from_amiga_u32`'s
  HSPARWED bit semantics (bits 0-3 inverted for d/e/w/r, bits 4-7 direct for
  a/p/s/h) are the one part of the Amiga side that *is* standard AmigaDOS
  `FIBB_*` protection-flag semantics, independent of the LHA encoding
  question.

  Seven tests in the new `tests/unpack_lha_header.rs`, covering the phase
  doc's five groups with this round's substitution made explicit in the test
  file's own doc comment: three real fixtures (`lha_header_level{0,1,2}.lha`,
  built with the system `lha` tool, one per level) parse correctly and yield
  `protection: None, comment: None` — satisfying "all three levels parse"
  and "no extended headers yields defaults, not garbage" against a genuine
  `lha v`-cross-checked oracle; two *synthetic* fixtures (hand-built by the
  test itself, not real Amiga archives) exercise the S-bit and E-bit paths
  through the placeholder encoding, standing in for the phase doc's "two real
  `.lha` fixtures... verified against `lha -v`" bullet until real ones exist;
  a comment round-trips through the same synthetic path; a truncated header
  and a malformed extension-chain block-size both error rather than reading
  past the buffer or panicking (added a `MalformedExtension` error variant
  once hand-tracing the real level-1 fixture's chain showed a
  `next_size < 3` value would otherwise underflow the slice index).

176 tests total (7 new + 169 pre-existing, summed directly via `cargo test
--workspace 2>&1 | grep "test result: ok" | ...`). `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean (the new module has no `native`
dependency, unlike P5.4/P5.5's backends). Also smoke-tested the real `bam`
binary (`ingest --offline`): still reports 501 packages.

**Deviations for the next session to know about:**
- The Amiga protection-bits/comment encoding (`AMIGA_OS_ID = 'A'`,
  `AMIGA_EXT_TYPE = 0x47`, the `[u32 protection][comment]` layout) is an
  unverified placeholder — see the `ponytail:` comment at the top of
  `unpack/lha_header.rs`. It has never been checked against a real
  Amiga-built archive or `lha -v`. Whoever next has access to a real
  Amiga-native `.lha` with S-bit scripts (or the actual AmigaOS `LhA`
  archiver source) should treat this as the first thing to correct, the way
  Round 30's `sample.lzx` corrected an until-then-skipped test the moment a
  real fixture arrived.
- `LhaFileHeader`/`parse_lha_header` are not yet wired into any unpacker's
  `unpack()` — P5.6 was scoped as parser-only per the phase doc. P5.7 (the
  `.uaem` sidecar writer) is what will actually consume `ProtectionBits`.

---

## Round 33 — 2026-08-07 · `.uaem` sidecar writer (P5.7)

**Done:**

- **P5.7** — `crates/bam-core/src/unpack/uaem.rs`, new module. Pure
  formatting (`format_uaem_line`) stays ungated per I1 — same split as
  `blob`/`unpack` themselves: only the actual file write
  (`write_sidecar`, native-gated) touches `std::fs`. `format_flags` walks
  `ProtectionBits` in the exact `hsparwed` field order the struct already
  declares (set-lowercase, unset `-`), reusing the struct P5.6 built
  rather than adding a second flag ordering anywhere. The timestamp comes
  from a `SystemTime` (the archive's mtime, per the phase doc's hand-over
  line) broken into y/m/d via `ingest::normalize::civil_from_days`
  (`pub(crate)`, already built for `date_from_age_weeks` in Round 1) rather
  than adding a date/time dependency the workspace has deliberately never
  taken — h/m/s and hundredths come from straightforward integer division
  on the Unix-epoch seconds and `subsec_millis() / 10`. A comment
  containing `\n`/`\r` is rejected with `UaemError::InvalidComment` before
  any formatting happens, never written raw, since it would corrupt the
  sidecar's one-line format; `write_sidecar` appends `.uaem` to the full
  target filename (`Foo` → `Foo.uaem`, `Foo.txt` → `Foo.txt.uaem`) via
  `OsString`, not `Path::with_extension` (which would replace `.txt`
  rather than append).

  Five tests in the new `tests/unpack_uaem.rs`, matching the phase doc's
  five bullets exactly, all built around the literal 2001-03-11 22:15:00
  UTC example line from §12.1 (Unix timestamp 984348900, cross-checked
  against `date -u -r`): a known r/w/e/d-only attribute set formats to
  that exact string byte-for-byte; an h/s-only set proves `hsparwed`
  ordering and dash-for-absent with a `starts_with` check distinct from
  the byte-exact test; 340ms added to the example time renders as `.34`,
  not `.340` or `.3`; no comment omits the trailing field with no dangling
  space; a comment with an embedded `\n` errors.

181 tests total (5 new + 176 pre-existing, summed directly via `cargo test
--workspace 2>&1 | grep "test result: ok" | ...`). `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean (`uaem.rs`'s formatting half has no
`native` dependency, matching `lha_header.rs`). Also smoke-tested the real
`bam` binary (`ingest --offline`): still reports 501 packages.

**Deviations for the next session to know about:**
- `write_sidecar` is not yet called from anywhere — P5.7 was scoped as
  formatter-plus-writer only, no caller. P5.8 (archive inventory
  enrichment) is the first task positioned to actually invoke it during
  real extraction; it will need to source a real `SystemTime` mtime from
  the unpacked file or the LHA header itself (P5.6's header currently
  carries no timestamp field — only `filename`/`protection`/`comment` —
  so whoever wires P5.8 should check whether the archive's own header
  timestamp or the extracted file's on-disk mtime is the intended source
  before assuming either).

---

## Round 34 — 2026-08-07 · Archive inventory enrichment (P5.8) — Phase 5 exit

**Done:**

- **P5.8** — split across two modules to keep invariant I1 (`rusqlite`
  confined to `store::`) intact, since the phase doc's "composes P5.3 with
  the enrichment table" spans both sides of that boundary:

  `unpack/inventory.rs` (native-gated, no DB access): `extract_inventory`
  extracts a blob via a `&UnpackerRegistry` into a private scratch
  directory — the same counter+PID scratch-dir scheme as `unar.rs` — and
  always removes it afterwards, success or error, before propagating any
  extraction error. Reuses `Unpacker::unpack`'s own returned
  `Vec<ExtractedFile>` (path+size) rather than re-walking `dest`, since the
  registry already produced exactly that list; the one thing missing was a
  "detected type", added as `detect_kind`, a plain extension→category map
  (`text`/`image`/`audio`/`icon`/`system`/`other`/`unknown`) over the
  *extracted* file's own name — deliberately not a magic-byte sniffer,
  since P5.3's "Aminet lies about extensions" problem is specifically about
  the outer archive's filename, not the contents' own names once unpacked.

  `store/inventory.rs` (native-gated, under `store::` per I1):
  `enrich_inventory` is the actual entry point — checks for an existing
  `kind = "inventory"` enrichment row first (`producer_version ==
  INVENTORY_PRODUCER_VERSION` → `InventoryOutcome::UpToDate`, no extraction
  attempted at all, not just a discarded one), otherwise calls
  `extract_inventory` and serializes the result with `serde_json` into a
  new `enrichment` row. Storing that row needed a genuine addition to
  `tables.rs`: the existing `insert_enrichment` is a plain `INSERT`, which
  collides with `enrichment`'s `PRIMARY KEY (package_id, kind)` on a
  reprocess — added `upsert_enrichment` (`INSERT ... ON CONFLICT DO
  UPDATE`, the same idiom P5.2's `record_blob` already uses for `blobs`)
  rather than widening `insert_enrichment` itself, since every existing
  caller's insert-only contract (erroring on a genuine duplicate) is still
  correct for them.

  `produced_at` reuses `bam_core::now_rfc3339()` (built in Round 1 for
  `fetched_at`) rather than adding a clock dependency.

  Four tests in the new `tests/unpack_inventory.rs`, matching the phase
  doc's four bullets exactly, all built around an in-test `ZipUnpacker` (no
  external `unar` binary needed, unlike P5.4's fixtures) plus the same
  `seed()` package/blob helper `tests/store_blob_cache.rs` already uses: a
  two-file zip's inventory round-trips through the enrichment payload with
  matching paths, sizes, and a `text` kind for `.txt`; a malformed-zip blob
  errors *and* leaves no `bam-inventory-scratch-*` directory under the
  system temp dir (a before/after glob-diff, guarded by a
  `static Mutex` all four tests take — the four tests in this file all
  create and remove their own scratch dirs under that shared prefix, and
  running in parallel threads without the lock made the glob-diff
  genuinely flaky, caught by a real intermittent failure during this
  round, not reasoned out in advance); the enrichment row survives
  `evict_to_budget(..., 0)` on the same blob, re-asserting P5.2's
  invariant from the consumer side per the phase doc's own wording; a row
  manually seeded at `producer_version = 0` is reprocessed to `1`
  (`Written`), and calling again is a no-op (`UpToDate`).

**Phase 5 exit reached.** Archives are cached by content hash (P5.1–P5.2),
extracted through a registry with two working backends (P5.3–P5.5), with
Amiga attributes preserved as `.uaem` sidecars (P5.6–P5.7) and file
inventories captured as enrichment (P5.8) — §15's "hard core" is complete
per the phase doc's own closing line.

185 tests total (4 new + 181 pre-existing, summed directly via `cargo test
--workspace 2>&1 | grep "test result: ok" | ...`). `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and the wasm32
`--no-default-features` check all clean (`unpack/inventory.rs` has no DB
dependency and would compile under wasm32 but for its scratch-dir `fs`
calls, so it stays native-gated like `unar.rs`/`zip_backend.rs`). Also
smoke-tested the real `bam` binary (`ingest --offline`): still reports 501
packages.

**Deviations for the next session to know about:**
- Round 33's hand-over note speculated P5.8 would be "the first task
  positioned to actually invoke [`write_sidecar`]". The actual P5.8 phase
  doc entry names only inventory extraction and the enrichment table —
  no `.uaem` sidecar involvement — so that speculation didn't hold;
  `write_sidecar` remains uncalled from anywhere. Nothing in Phase 5's own
  task list schedules its wiring; it would need a real extraction pipeline
  (real destination directory, not a discarded scratch one) as a caller,
  which no task before Phase 6 currently owns.
- `enrich_inventory`/`extract_inventory` are not yet called from any
  ingest or fetch pipeline — P5.8 was scoped, like P5.1/P5.3/P5.6/P5.7
  before it, as the enrichment mechanism itself, not its trigger. Whichever
  future phase adds a "process this archive" pipeline step is the first
  real caller.

---
