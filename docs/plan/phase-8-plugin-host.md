# Phase 8 — WASM plugin host

← [Implementation plan index](../../IMPLEMENTATION_PLAN.md)

`extism`-based WASM plugin host, per `bam-handoff.md` §9. **Scheduled, not
deferred** — a deliberate decision, since third-party extensibility is a stated
goal rather than a speculative one.

It lands here rather than earlier because by this point **three registries
exist** — query languages (P2.2), unpackers (P5.3), launchers (P6.1) — so the
host generalises over trait shapes proven in use instead of shapes guessed at
in advance. Building the plugin ABI first would have meant versioning a
contract before anything consumed it.

---

### P8.1 — Contract versioning and manifest schema · **O**

§9's three concepts, made concrete: extension point, contract, entrypoint.

```toml
name = "protracker-analyzer"
version = "0.2.0"
api_version = 1
extension_point = "content_analyzer"
claims = ["*.mod", "mod.*", "*.med"]
```

The host **rejects plugins whose major contract version it does not
recognise**, rather than letting them run half-broken. `claims` lets the host
pre-filter which files reach a given plugin instead of invoking every plugin
for every file.

Contracts are JSON schemas per extension point, generated from the Rust types
where possible so they cannot drift from what the host actually passes.

**Tests first:**
- A manifest with a known `api_version` loads.
- A manifest with a higher major `api_version` is rejected, with an error
  naming both versions.
- A malformed manifest is rejected with a message naming the offending field.
- `claims` filtering: a plugin claiming `*.mod` is not offered a `.iff` file.
- The generated contract schema matches the host's actual input type — a
  round-trip test, so the two cannot diverge silently.

**Why O:** this is a versioned public contract. Once a third-party plugin
exists, changing it is no longer a local decision, and the escape hatch has to
be designed before the first plugin ships rather than after.

**Hand over:** §9 in full, the three registry traits from P2.2/P5.3/P6.1 as the
shapes to accommodate, the manifest above.

**Done when:** all five pass.

---

### P8.2 — extism host and registry integration · **O**

Load WASM modules and register them **through the existing registries**, so
call sites are unchanged:

```rust
registry.register(Box::new(WasmUnpacker::load(path)?));
```

This is invariant **I4**'s central claim, and this task is where it is proven
or disproven. If a WASM-backed implementation cannot satisfy an existing trait,
that is a finding to report — the correct response is to fix the trait, not to
add a parallel plugin dispatch path beside it.

**Tests first:**
- A WASM plugin registers into the unpacker registry and is selected by the
  normal selection logic, with no change to the selection code.
- Host and plugin exchange JSON per the P8.1 contract.
- **No call site changed** to accommodate WASM — verified by diff review.
- A plugin and a native implementation claiming the same format resolve by the
  documented preference order.
- Loading the same plugin twice is idempotent.

**Why O:** the boundary where the whole registry design either holds or does
not. It also fixes the sandboxing and resource-limit posture for every plugin
that follows.

**Hand over:** invariant I4, P8.1's contracts, the three registry traits,
extism's host API, the five acceptance items.

**Done when:** the four tests pass and the diff review confirms the third.

---

### P8.3 — `content_analyzer` extension point · **S**

The one §9 specifies concretely. The host calls a plugin per file after
extraction; the plugin returns a classification plus searchable text.

```json
// input
{ "path": "mods/foo.mod", "size": 108234, "bytes_b64": "...", "hint": "audio" }
// output
{ "kind": "protracker_module", "confidence": 0.97,
  "attributes": { "channels": 4, "patterns": 32, "samples": ["bass", "hihat"] },
  "searchable_text": "bass hihat ..." }
```

Results land in `enrichment` with the plugin's name and version as the
producer, so a plugin upgrade invalidates only its own rows.

**Tests first:**
- A sample analyzer plugin classifies a `.mod` fixture and its
  `searchable_text` becomes findable through FTS5 (P4.6).
- Results are stored with the plugin's name and version as producer.
- Bumping the plugin version reprocesses only that plugin's rows — LLM
  summaries are untouched.
- A plugin returning malformed JSON is reported and skipped.
- `claims` pre-filtering means a plugin is not invoked for files it cannot
  handle — asserted by call count.

**Why S:** the contract is specified in §9 and the host exists after P8.2.

**Hand over:** §9's content-analyzer example, P8.1's contract mechanism, the
`enrichment` row shape, the five tests.

**Done when:** all five pass.

---

### P8.4 — WASM-backed unpacker · **S**

A working unpacker as a WASM plugin, end to end. Proves I4's claim with a
second extension point rather than a single one, which is what distinguishes a
general host from a content-analyzer host with extra steps.

**Tests first:**
- A WASM unpacker extracts a fixture archive correctly.
- It participates in magic-byte format routing like any native backend.
- Its `probe` is honoured.
- Path traversal is rejected inside the sandbox, as in P5.4 — a plugin is less
  trusted than in-tree code, not more.

**Why S:** an application of P8.2 against an existing trait and existing tests.

**Hand over:** P5.3's `Unpacker` trait, P8.2's host, P5.4's test list as the
bar to clear, the four tests.

**Done when:** all four pass.

---

### P8.5 — Plugin loading configuration and failure isolation · **S**

Where plugins are discovered, how they are enabled, and — the important part —
**a broken plugin degrades one feature, never the application**.

**Tests first:**
- A plugin that panics is caught; the host continues and reports which plugin
  failed.
- A plugin exceeding a time limit is terminated, and the operation continues
  without it.
- A plugin exceeding a memory limit is terminated likewise.
- A plugin failing to load is reported at startup, naming the file, and does
  not prevent startup.
- Disabling a plugin in config prevents loading it entirely.

**Why S:** extism provides the sandboxing primitives; this is policy and wiring
over them, with every case enumerated.

**Hand over:** extism's resource-limit API, the five failure modes above, the
config shape.

**Done when:** all five pass.

---

**Phase 8 exit:** third parties can extend content analysis and unpacking in
any language that compiles to WASM, without a native ABI to break.
