# Phase 9 — Frontends: round-by-round log

← [PROGRESS.md](../../PROGRESS.md) · [docs/plan/phase-9-frontends.md](../plan/phase-9-frontends.md)

## Round 44 — 2026-08-08 · Phase 9: Vue frontend and transport interface (P9.1)

One `frontend/` package (Vue 3, `<script setup>` + TypeScript, Vite) with a
`BamClient` interface as the only seam components use — `TauriClient` and
`HttpClient` implement it, neither imported by anything under `components/`.
Request/response types are generated, not hand-written: `bam-core::api`
gained a `schema` module (`all_schemas()`, mirroring P7.2's
`bam_dsl_json_schema` pattern) and an `export_api_schema` example that
prints it as JSON; `frontend/scripts/gen-types.mjs` turns that into
`src/generated/types.ts` via `json-schema-to-typescript`, merging every
type's independent `schema_for!` output into one shared `definitions` map
first so shared types (`Predicate`, `Package`, ...) don't get emitted once
per referencing root and collide.

All five of P9.1's required tests pass: a `PackageList` component test
against a mock `BamClient` with no real transport present; one contract
suite (`describe.each`) run against both `HttpClient` and `TauriClient`,
covering request/response shape and progress-stream termination on both
`Finished` and `AbortSignal`; a staleness test that regenerates the types
in-memory and diffs against the checked-in file; and a grep-style test
over every file under `components/` rejecting `@tauri-apps/api` and direct
`fetch(` calls. CI gained a `frontend` job running `npm run typecheck` and
`npm test`. 230 Rust tests, 11 frontend tests.

**Next:** P9.2, the `bam-server` HTTP/SSE adapter that gives `HttpClient` a
real backend to talk to (currently only exercised against mocks).

---

## Round 45 — 2026-08-08 · Phase 9: `bam-server` HTTP/SSE adapter (P9.2)

A new `bam-server` crate, routed exactly to the paths and JSON shapes
`frontend/src/transport/HttpClient.ts` already assumed from Round 44 — no
frontend changes needed. Sessions are cookie-based (`bam_session`, set on
first response); every `bam_core::api` call goes through a `SessionHandle`
that hands a closure to the session's own dedicated OS thread rather than
sharing `Session` across axum's multi-threaded executor — `Session` wraps a
`rusqlite::Connection`, which is deliberately not `Sync` (I1's purity check
never asked it to be; its only prior caller, `bam-tui`, is single-threaded),
so this keeps `bam-core` untouched rather than forcing thread-safety onto it.
An ingest's `active` broadcast-sender slot lives in its own
`std::sync::Mutex`, outside the actor's job queue: an ingest occupies the
actor thread for its whole duration, so a progress subscription that had to
queue behind it would only ever learn about progress after the ingest had
already finished — reading that mutex directly instead lets a reconnecting
SSE client subscribe immediately while the ingest is still running, or fall
back to a synthesized terminal event from `operation_status` once it's not.

All five acceptance items hold: every `bam_core::api` operation reachable
over HTTP and round-tripping its types (one test walking parse → search →
mark → select → save/load/delete → clear across the real routes); two
sessions (two cookie jars) confirmed not to observe each other's marks;
SSE delivering a real `Started`/`Advanced`/`Finished` sequence for an
offline ingest; a client that disconnects and reconnects with the same
`OperationId` resolving to `Finished` either way, with the ingest itself
proven to have actually completed rather than being orphaned by the first
disconnect; and a grep-based purity test in the spirit of P0.4 confirming
no `rusqlite` name and no raw SQL keyword anywhere in `bam-server/src`. 235
Rust tests project-wide (5 added), CI's existing `--workspace` fmt/clippy/
test steps cover the new crate with no workflow changes.

**Next:** P9.3, the Tauri shell — a thin host providing `TauriClient` for
the same `frontend/` build `bam-server` now serves over HTTP.

---

## Round 46 — 2026-08-08 · Phase 9: Tauri shell (P9.3)

A new `bam-tauri` crate: a thin Tauri v2 host with no UI of its own —
`tauri.conf.json`'s `frontendDist` points straight at `../../frontend/dist`,
the same build `npm run build` produces, so there is exactly one `frontend/`
package rather than a bundled fork. `#[tauri::command]` handlers mirror
`TauriClient.ts`'s `invoke` calls one-for-one (P9.1), each adapting a
request straight onto `bam_core::api`. Unlike `bam-server`, a desktop app
has exactly one user, so there's no cookie-keyed session map: one
`SessionHandle` is spawned at startup and shared by every command —
`SessionHandle::spawn` (P9.2's per-session actor thread, keeping
`bam-core`'s non-`Sync` `Session` off axum's/Tauri's async runtime) is now
`pub` in `bam-server::state` so this crate reuses it rather than
reimplementing the same actor-thread machinery. `start_ingest` spawns a
relay task forwarding the session's progress broadcast to a
`progress:{operation}` Tauri event until `Finished`, the exact event name
`TauriClient::progress` already listens for.

All three of P9.3's tests hold: the app builds and launches on macOS,
confirmed by a manual run (opens and stays open — no panic, no error
output) plus `cargo build -p bam-tauri`; the shared transport contract
suite from P9.1 (`contract.test.ts`) already runs `TauriClient` against a
real command surface (Round 44 wrote it against both transports up front);
and `tauri.conf.json`'s build config is the no-fork evidence for the third.
Linux launch itself is unverified locally (no Linux machine in this
session) but now builds in CI — added `libwebkit2gtk-4.1-dev`,
`libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev` to the
`ubuntu-latest` CI step so `cargo test --workspace`/`clippy --workspace`
(now covering `bam-tauri` as a workspace member) actually links there
instead of failing on missing system libs. 235 Rust tests (unchanged — no
new Rust test code; the acceptance surface here is build/launch and the
already-existing frontend contract suite), CI gained the Linux Tauri
system deps, no other workflow changes.

**Next:** P9.4, package list and detail views — the first real Vue
components (virtualized list, detail pane, query input with P3.5-style
inline error display) landing in `frontend/components/`, consumed by both
`bam-server` and `bam-tauri` at once.

---

## Round 47 — 2026-08-08 · Phase 9: package list and detail views (P9.4)

Three new `frontend/components/`: `PackageList.vue` (virtualized — renders
only the rows within a configurable `viewportHeight` of the scroll offset
plus overscan, so an 84,000-row result still only ever mounts a few dozen
`<li>`s), `PackageDetail.vue` (fetches and renders the selected package via
`getPackage`), and `QueryInput.vue` (150ms-debounced `parseQuery`, matching
`bam-tui`'s `DEBOUNCE` constant exactly, P3.5's reference).

Getting the error-span test to mean anything surfaced a real gap: `BamClient`
had no way to report *where* a bad query failed, and both backends were
flattening `ParseError` down to a bare string before it ever left Rust.
Fixed at the source rather than worked around in the component: `bam-server`'s
`ApiError` and `bam-tauri`'s new `CmdError` both now carry the `Option<(usize,
usize)>` span from `SessionError::Parse` alongside the message, and
`BamClient` gained a `BamApiError` (message + optional span) that `HttpClient`
and `TauriClient` throw uniformly — one error shape regardless of transport,
same seam P9.1 established. `BamClient` also grew `toggle()` (I7), plumbed
through a `bam-tauri` `toggle` command that didn't exist before this round
(the HTTP side already had the route from P9.2; parity was the actual gap).

All four of P9.4's tests hold, plus one added for the span fix: component
tests for list/detail/query-input against a shared `mockClient` test helper;
the 84,000-row virtualization bound; query errors rendering the offending
byte range in a `<mark>` with the previous predicate left in place on
failure; mark-toggle round-tripping through the injected client; and a new
Rust test (`parse_error_span_survives_over_http`) proving the span isn't
lost in `bam-server`'s JSON flatten. 236 Rust tests (1 added), 22 frontend
tests (11 added), CI unchanged — no new dependency, no new workflow step.

**Next:** P9.7, the AmigaGuide parser — `nom` into a custom AST over
`text/hyper/` fixtures from Aminet (line commands, inline attributes, link
resolution, graceful degradation on malformed markup, P1.5 encoding
handling). Picked over P9.5/P9.6 because it's a standalone parser with no
Vue/UI dependency — order among the three remaining Phase 9 items (P9.5
timeline viz, P9.6 archive content viz, P9.7 AmigaGuide) is otherwise free;
P9.7 just doesn't block on or share code with the other two, so it can run
independently. Phase 9 exit needs all three closed.

---

## Round 48 — 2026-08-08 · Phase 9: AmigaGuide parser (P9.7)

A `nom` parser into a custom AST (`bam_core::ingest::amigaguide`):
`GuideDocument { database, nodes }`, each `GuideNode` carrying
`name`/`title`/`next`/`prev`/`toc` plus a `body: Vec<Inline>` of
`Text`/`Styled(Style, children)`/`Link` — nesting is real tree structure, not
a flat run list, built from a tokenized chunk stream via an explicit
open-style stack so `@{b}...@{i}...@{ui}...@{ub}` closes into
bold-containing-italic rather than two flat spans. `nom` earns its place
here the way P2.4 said it wouldn't for `bam-dsl`: line commands and inline
attribute codes are many small, near-identical productions, which is what
combinators are for. Nothing in the module returns `Result` — malformed
input (an unmatched `@{`, an attribute code that isn't recognised, a style
never closed before `@endnode`) degrades to literal text or gets flushed at
end-of-node rather than failing the parse, matching the plan's "thirty-year-
old files, some are simply broken" framing directly in the type signatures.

The real fixture is Commodore's own `Amigaguide.guide` — fetched from
Aminet's `text/hyper/amigaguidedocs.lha` (`unar`-extracted) rather than
synthesized, since it already exercises 8 `@node`/`@endnode` pairs,
navigation fields, both style and link attributes, and one escaped `\@{...}`
inside a `@MACRO` example. Parsing it against that fixture surfaced a real
bug before any test needed to name it: a body line starting with `@{` at
column 0 (the fixture's own `@{b}$VER: ...@{ub}`) was being swallowed whole
as an unrecognised line command, because `@{...}` and `@command` share the
same leading `@`. Fixed at the one place that distinguishes them —
`command_line` now returns `None` for anything starting `@{`, so it falls
through to body text — rather than special-casing it in each call site.
`nom` was not yet a workspace dependency; added at `"7"` (pure Rust, so I1's
`wasm32-unknown-unknown --no-default-features` build for `bam-core` still
holds, checked directly rather than assumed).

All five of P9.7's tests hold: the real fixture parses to the expected
8-node AST with the front page's 7 links and 1 italic span; every one of
those links resolves to a real node name via `GuideDocument::find_node`;
a small literal case proves bold-containing-italic-containing-link nests as
a real tree; an unrecognised attribute code and an unclosed style both
degrade to literal text without panicking; and a Latin-1-encoded byte in a
node body decodes through P1.5's `decode` (not assumed UTF-8) via the same
`WINDOWS_1252` path `charset.rs` already uses for ISO-8859-1. 241 Rust tests
(5 added), `cargo fmt`/`clippy --workspace --all-targets` clean, no other
workflow changes.

**Next:** P9.5 or P9.6 — the two remaining Phase 9 items, both Vue chart
components over data that already exists (`package.date_precision` for the
timeline, P5.8's inventory payload for per-archive content). Order between
them is free, same as noted last round. Phase 9 exit needs both closed.

---

## Round 49 — 2026-08-08 · Phase 9: timeline visualization (P9.5)

`PackageTimeline.vue`: uploads per year for the active query, fetched through
the same `client.searchPackages({ predicate })` call `PackageList.vue`
already uses — no new backend or generated types needed, `Package` already
carries `uploaded_on` and `date_precision` (P1.2). Each year renders as two
stacked segments, `precision-exact` and `precision-week`, so a `week`-precision
point (INDEX-derived, ±1 week) is never drawn as though it were exact — the
one constraint the phase doc called out by name.

All three of P9.5's tests hold: a four-package fixture spanning 2023–2026
buckets correctly by year, including a year with zero uploads producing no
bar; `exact` and `week` bars carry distinct CSS classes; and the timeline
re-fetches through the injected `predicate` prop rather than reading the
whole archive. 236 Rust tests (unchanged), 26 frontend tests (4 added), no
new dependency, no workflow changes.

**Next:** P9.6, per-archive content visualization — the last Phase 9 item.
Renders P5.8's inventory payload (file types, sizes, directory structure)
with a "not analyzed" state when no inventory exists yet. Phase 9 exit
follows once it closes.

---

## Round 50 — 2026-08-08 · Phase 9: per-archive content visualization (P9.6, Phase 9 exit)

`PackageContent.vue`, the last Phase 9 component: renders P5.8's inventory
payload (per-kind file counts/sizes, directory listing) or a "not analyzed"
state when no `inventory` enrichment row exists yet for the package. Getting
there needed a new API surface that hadn't existed before this round —
`Session::get_inventory` deserializes the `enrichment` row's JSON payload
through P5.8's existing `store::tables::get_enrichment`, returning `None` on
`QueryReturnedNoRows` rather than an error (the same pattern `get_package`
already uses), with a new `SessionError::Serde` variant for a payload that
fails to deserialize. `Inventory`/`InventoryEntry` (`unpack::inventory`)
gained `JsonSchema` derives — needed for `bam-core::api`'s schema export but
not before, since nothing crossed the API boundary carrying them. Wired
through every layer the existing `get_package` surface already established:
`api::get_inventory`, `bam-server`'s `POST /api/get-inventory` (one `route!`
macro line), `bam-tauri`'s `get_inventory` command, and `BamClient`/
`HttpClient`/`TauriClient`/`mockClient` in lockstep — no new pattern
introduced anywhere, just the existing one extended one more time.

Both of P9.6's tests hold, plus one added at the `Session`/`api` layer since
this round introduced real (if thin) new logic there, not just a component:
a component test renders file-type and directory groupings from a fixture
inventory without any blob present; a second shows the "not analyzed" state
when `getInventory` resolves `null`; and `get_inventory_reflects_enrichment_state`
proves `None` before enrichment exists and the deserialized payload once
`store::inventory` (P5.8) would have written one. 242 Rust tests (1 added),
29 frontend tests (3 added), `cargo fmt`/`clippy --workspace --all-targets`
clean, no new dependency, no workflow changes.

**Phase 9 exit reached.** All of P9.1–P9.7 are closed: a Vue frontend behind
one `BamClient` seam, served by both `bam-server` (HTTP/SSE) and `bam-tauri`
(desktop) hosts with no fork between them, covering package list/detail/
query input, timeline and per-archive content visualization, and a standalone
AmigaGuide parser. Everything in `IMPLEMENTATION_PLAN.md` through Phase 9 is
now closed; Phase 8 (the extism WASM plugin host, noted as a scheduled
follow-on since the 2026-08-06 open-questions session) is the only
unclosed phase left in the plan.
