# Phase 8 — extism WASM plugin host: round-by-round log

← [PROGRESS.md](../../PROGRESS.md) · [docs/plan/phase-8-plugin-host.md](../plan/phase-8-plugin-host.md)

## Round 51 — 2026-08-08 · Phase 8: contract versioning and manifest schema (P8.1)

`bam_core::plugin` module: `PluginManifest` (TOML, per §9's `name`/`version`/
`api_version`/`extension_point`/`claims` shape) with `HOST_API_VERSION = 1`
rejected-on-mismatch checking, single-wildcard `claims` glob matching, and
`contract_schema()` generating a `schemars`-derived JSON Schema per
extension point (`content_analyzer` wired now; others return `None` until
their input types exist). Host-independent — no `#[cfg(feature = "native")]`
gate — so it compiles under `--no-default-features` on `wasm32-unknown-unknown`
same as the rest of I1. 6 tests added (248 total): known/rejected
`api_version`, a malformed manifest naming its missing field, `claims`
filtering, and the schema/type round-trip.

**Next:** P8.2, the extism host loading a WASM module through the existing
registries (`UnpackerRegistry` et al.) with no call-site change — the task
that proves or disproves invariant I4 for a plugin backend.

---

## Round 52 — 2026-08-08 · Phase 8: extism host and registry integration (P8.2)

`WasmUnpacker<S: BlobStore>` (`bam_core::plugin::wasm`, native-only):
loads `manifest.toml` + `plugin.wasm` from a directory and implements the
plain `Unpacker` trait (P5.3) by calling an `extism::Plugin`'s `probe` and
`unpack` exports with JSON — `UnpackRequest`/`UnpackResponse`/
`UnpackProbeResponse` added to `bam_core::plugin` as P8.1's `contract_schema`
gains an `"unpacker"` case alongside `"content_analyzer"`. The plugin
proposes file paths and bytes; the host writes them, rejecting `..`/absolute
paths itself, same trust posture as P5.4/P5.5 — a plugin is less trusted
than in-tree code, not more. `claims` reuses P8.1's glob matcher against a
format-name pattern (`*.zip` etc.) rather than adding a second matching
mechanism.

I4 confirmed: `registry.register(Box::new(WasmUnpacker::load(dir, store)?))`
against the unchanged `UnpackerRegistry` — zero diff in `unpack/`, `launch/`,
or any call site (checked directly, not just tested). 5 tests added (253
total): register-and-select through normal selection logic, a host↔plugin
JSON+bytes round-trip, native-vs-WASM resolving purely by registration order
in both directions, idempotent double-load, plus an `unpacker` contract
schema round-trip test mirroring P8.1's `content_analyzer` one.

Test fixture: `tests/fixtures/plugins/echo-unpacker/` — a real
`extism-pdk` WASM plugin (source kept alongside for provenance under
`src-provenance/`, not part of the Cargo workspace) that reports available
and echoes its input bytes back as one file, enough to prove the mechanism
without needing real archive parsing inside WASM — that's P8.4.

**Next:** P8.3, the `content_analyzer` extension point — wiring `enrichment`
rows to a plugin's classification output, with per-plugin producer
versioning so a plugin upgrade reprocesses only its own rows.

---

## Round 53 — 2026-08-08 · Phase 8: `content_analyzer` extension point (P8.3)

`WasmContentAnalyzer` (`bam_core::plugin::wasm`, native-only): loads a
`content_analyzer` manifest/`plugin.wasm` pair the same way P8.2's
`WasmUnpacker` does, and calls its `analyze` export per file. The output is
read as a raw string and `serde_json`-parsed on the host rather than through
extism's typed `Json<T>` convert on the guest side, so a plugin returning
malformed JSON surfaces as `AnalyzeError::MalformedOutput` instead of a
panic — same trust posture as P8.2's unpacker (I4: a plugin is less trusted
than in-tree code).

`bam_core::store::content_analysis::analyze_files` is the DB half: one
`enrichment` row per `(package, plugin, file)` — `kind =
"content_analyzer:{plugin_id}:{path}"` — so bumping a plugin's version
reprocesses only that plugin's rows for the files it claims, never another
plugin's rows or `llm_summary` (checked directly by a test that seeds a
summary row, reprocesses the analyzer twice with different plugin versions,
and asserts the summary payload is untouched). `claims` prefiltering
(P8.1) happens in this function, before any WASM call, so an unclaimed file
is never handed to the plugin. `producer_version`'s column is `INTEGER`
but a plugin's version is a free-form string; `analyze_files` hashes it with
`DefaultHasher::new()` (fixed keys, so deterministic across runs) rather
than adding a second version-comparison column to the shared `enrichment`
table.

`store::fts::rebuild_fts` gained a third `package_fts` column,
`content_analysis`, populated by concatenating `searchable_text` out of
every package's `content_analyzer:*` enrichment payloads — P4.6's
whole-row `MATCH` needed no compiler change to pick it up.

Test fixture: `tests/fixtures/plugins/echo-analyzer/` — a real
`extism-pdk` WASM plugin (source under `src-provenance/`, same convention
as P8.2's echo-unpacker) that classifies any `.mod` file as `kind: "echo"`
with `searchable_text` from its decoded bytes, and deliberately returns
malformed JSON for a path ending `broken.mod` to exercise the host's error
path. 5 tests added (258 total): FTS5 discovery of a classified file's
`searchable_text`, plugin name/version stored as producer, version-bump
reprocessing that leaves `llm_summary` untouched, malformed output
reported and skipped without writing a row, and `claims` prefiltering
proven by the unclaimed file never producing an enrichment row.

Both extension points named in §9 (`unpacker`, P8.2; `content_analyzer`,
P8.3) now have a working WASM backend through the plugin host.

**Next:** P8.4, a WASM-backed unpacker doing real archive extraction (vs.
P8.2's echo fixture) — proving I4 against a second extension point end to
end, including format routing and path-traversal rejection inside the
sandbox.

---

## Round 54 — 2026-08-08 · Phase 8: WASM-backed unpacker with real archive extraction (P8.4)

`tests/fixtures/plugins/zip-unpacker/`: a real `extism-pdk` WASM plugin
(source under `src-provenance/`, same convention as P8.2/P8.3's fixtures)
that reads genuine ZIP archives via the `zip` crate inside WASM — the first
plugin fixture that does real format parsing rather than echoing bytes back.
Registers into `UnpackerRegistry` and is selected through `detect_format`'s
ordinary magic-byte routing (P5.3), unchanged by P8.2. A second fixture,
`unavailable-unpacker/`, always reports itself unavailable, isolating the
probe-honoured test from the extraction test — P8.2 only ever exercised the
available path.

Extracting this fixture surfaced a real gap: `WasmUnpacker::unpack`
(`bam_core::plugin::wasm`) wrote each returned file to `dest` as it decoded
it, so a malicious entry ordered after a safe one left partial output behind
— unlike `unar`/`zip`'s scratch-then-move pattern (P5.4/P5.5), which never
had this exposure. Fixed by validating every entry's path and base64 first,
then writing only once the whole batch decodes clean — same no-partial-
extraction guarantee as the native backends, extended to plugins even though
the sandbox itself is per-call rather than per-file. 4 tests added (262
total): real extraction against a two-file fixture, magic-byte routing
through the unchanged registry, a `probe`-honoured negative case via
`unavailable-unpacker`, and traversal rejection with the fixed atomicity
verified directly (`dest` empty after the error).

**Next:** P8.5, plugin loading configuration and failure isolation — the
phase's last task: panics, time limits, and memory limits caught per-plugin
without taking down the host, plugin discovery/enable config, and a plugin
that fails to load reported at startup without blocking it.

---

## Round 55 — 2026-08-08 · Phase 8: plugin loading configuration and failure isolation (P8.5, Phase 8 exit)

`PluginConfig` (`bam_core::plugin`, host-independent — same pattern as
`launch::LaunchConfig`): the `[plugins]` section of `bam.toml`, with
`disabled: Vec<String>` plus `timeout_ms`/`max_memory_pages` limits.
`WasmUnpacker`/`WasmContentAnalyzer` gained `load_with_config` alongside the
existing `load` (which now delegates to it with defaults, so none of the
~15 existing call sites needed to change): a disabled plugin's name is
checked against the parsed manifest before its `.wasm` is even read, and
`build_plugin` threads the limits into an `extism::Manifest` — `timeout_ms`
and `with_memory_max` — rather than the raw-bytes `Plugin::new` P8.2/P8.3
used, since `extism::Manifest` implements `Into<WasmInput>` as a drop-in.
Both limit kinds surface as an ordinary `Err` from `plugin.call` through
extism's own epoch-interrupt timer and `wasmtime` memory limiter — the same
path a guest panic (a WASM trap) already took, so no catch-unwind wrapper
was needed at any call site to satisfy "a broken plugin degrades one
feature, never the application."

`discover_unpackers`/`discover_content_analyzers` scan a directory of
plugin subdirectories, loading each independently and collecting failures
into a `PluginLoadReport` (naming the directory) instead of stopping — one
broken or disabled plugin never blocks the rest from loading. Required
`FsBlobStore: Clone` (added; it's one `PathBuf`) since discovery hands each
`WasmUnpacker` its own owned store instance.

Test fixture: `tests/fixtures/plugins/misbehaving-unpacker/` — one
`extism-pdk` WASM plugin whose `unpack` panics, infinite-loops, or hogs
memory depending on the archive bytes it's given (`b"panic"`/`b"loop"`/
`b"memory"`, decoded from `bytes_b64` the same way a real archive would be),
covering all three failure-isolation cases without three fixture binaries.
5 tests added (267 total): a panic caught without harming the plugin
instance for later calls, a `timeout_ms`-bounded call terminated well under
the suite's own patience, a `max_memory_pages`-bounded call terminated
against the memory hog, discovery reporting one malformed-manifest
directory by path while still loading a good plugin alongside it, and
`disabled` rejecting a plugin by name before any `.wasm` read.

**Phase 8 exit reached.** All five P8 tasks are done: manifest/contract
versioning (P8.1), the extism host wired into the unchanged `UnpackerRegistry`
(P8.2, I4 confirmed), the `content_analyzer` extension point (P8.3), a
WASM-backed unpacker doing real archive extraction (P8.4), and failure
isolation plus config (P8.5). Third parties can extend content analysis and
unpacking in any language that compiles to WASM, without a native ABI to
break — and a broken plugin never takes the host down with it. Every phase
in `IMPLEMENTATION_PLAN.md` is now closed.
