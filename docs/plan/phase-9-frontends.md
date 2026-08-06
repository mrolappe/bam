# Phase 9 — Desktop GUI, web server, visualizations

← [Implementation plan index](../../IMPLEMENTATION_PLAN.md)

**Framework: Vue** — Vue 3, `<script setup>` with TypeScript, Vite.

One `frontend/` package consumed by both the Tauri shell and `bam-server`.
Neither host gets its own copy of the UI; that duplication is exactly what
invariant **I5** and this phase's transport seam exist to prevent.

Kept deliberately lower-detail than earlier phases: Phase 3 will teach things
about the data model that should shape this, and planning it minutely now is
planning against belief rather than knowledge. Expand when Phase 8 closes.

---

### P9.1 — Vue frontend and transport interface · **O**

One Vue application, two transports.

```ts
interface BamClient {
  searchPackages(req: SearchRequest): Promise<SearchResponse>
  getPackage(id: PackageId): Promise<Package>
  // ...
  progress(op: OperationId): AsyncIterable<ProgressEvent>
}
```

Implementations: `TauriClient` over `invoke` plus Tauri events, `HttpClient`
over `fetch` plus SSE. Supplied through `provide`/`inject` and consumed via
composables. **Components never import a transport directly**, so no component
can quietly become Tauri-only — which is the failure mode that turns one
frontend back into two.

Progress arrives as an async iterable in both transports: Tauri events on one
side, SSE on the other, one consumer-facing shape.

**Types are generated, not hand-written.** I5 already requires `Serialize` +
`JsonSchema` on every request and response, so generate the TypeScript
definitions from those schemas and check the generated file into CI. A
hand-maintained mirror of Rust types drifts silently, and this frontend is
built against a core that is still moving.

**Tests first:**
- A component test with a mock `BamClient` renders results without either real
  transport present.
- Both transport implementations satisfy the same interface contract — one
  shared test suite run twice, once per implementation.
- The generated TypeScript types match the current Rust schemas; CI fails when
  they are stale.
- A grep test: no file under `components/` imports `@tauri-apps/api` or calls
  `fetch` directly.
- Progress iteration terminates cleanly on completion and on cancellation, in
  both transports.

**Why O:** this seam decides whether "desktop GUI and web app" is one codebase
or two that diverge over a year. It is also the only place the type-generation
pipeline can be established cheaply — retrofitting it after components exist
means rewriting every one of them.

**Hand over:** invariant I5, P2.6's API types, "Vue 3 `<script setup>` +
TypeScript + Vite", the interface above, the five tests.

**Done when:** all five pass.

---

### P9.2 — `bam-server` HTTP/SSE adapter · **S**

A Rust HTTP server exposing P2.6's API. Sessions, operation ids, and progress
streamed over SSE (invariant I5).

Being a thin adapter is the acceptance criterion: it translates transport to
API calls and contains no query, storage, or business logic.

**Tests first:**
- Every API operation is reachable over HTTP and round-trips its types.
- Two concurrent sessions do not observe each other's working selection.
- SSE delivers progress events for a long operation.
- A client disconnecting and reconnecting with the same `OperationId`
  re-attaches to a still-running operation rather than orphaning it.
- The crate contains no SQL and no query logic — verified by review, in the
  spirit of P0.4.

**Why S:** a conventional server over an API designed for exactly this.

**Hand over:** P2.6's API surface, invariant I5's three web rules, the five
acceptance items.

**Done when:** the four tests pass and review confirms the fifth.

---

### P9.3 — Tauri shell · **S**

A thin Tauri host: provides `TauriClient`, hosts the same Vue application,
handles window and menu concerns. No UI of its own.

**Tests first:**
- The application builds and launches on macOS and Linux.
- `TauriClient` passes the shared transport contract suite from P9.1.
- The bundled frontend is the same `frontend/` build the server serves — no
  fork, verified by build configuration.

**Why S:** Tauri scaffolding over an interface that already exists.

**Hand over:** P9.1's `BamClient` contract, "no UI in this crate", the three
tests.

**Done when:** all three pass.

---

### P9.4 — Package list and detail views · **S**

Vue components in `frontend/`, so both variants get them at once. Virtualized
list, detail pane, query input with the same inline error display as P3.5.

**Tests first:**
- Component tests against a mock client for list, detail, and query input.
- Virtualization: rendering an 84,000-row result mounts a bounded number of row
  components.
- Query errors render with the span highlighted, matching P3.5's behaviour.
- Selections (I7) toggle from the list and persist through the API.

**Why S:** conventional component work against a mocked client.

**Hand over:** P9.1's client interface, P3.5's error-display behaviour as the
reference, the four tests.

**Done when:** all four pass.

---

### P9.5 — Timeline visualization · **S**

Uploads over time across the ingested archive, filterable by the active query.

Respect `date_precision` (P1.2): `week`-precision points must not be drawn as
though they were exact. A visualization that silently implies precision the
data does not have is worse than no visualization.

**Tests first:**
- Bucketing is correct for a fixture spanning several years.
- `week`-precision entries are rendered distinguishably from `exact` ones.
- The timeline reflects the active query rather than the whole archive.

**Why S:** a chart over data that already exists, with the one non-obvious
constraint stated.

**Hand over:** the `package` schema including `date_precision`, "week-precision
must be visually distinguishable", the three tests.

**Done when:** all three pass.

---

### P9.6 — Per-archive content visualization · **S**

Visualize an archive's inventory (P5.8) — file types, sizes, directory
structure.

**Tests first:**
- Renders from an inventory fixture without needing the blob present (P5.2's
  invariant: enrichment outlives the archive).
- An archive with no inventory yet shows a clear "not analyzed" state rather
  than an empty chart.

**Why S:** a chart over an existing enrichment payload.

**Hand over:** the inventory payload shape from P5.8, the two tests.

**Done when:** both pass.

---

### P9.7 — AmigaGuide parser · **S**

`nom` into a custom AST. No maintained Rust library exists; the format is
modest — line commands (`@node`, `@endnode`, `@title`) and inline attributes
(`@{b}`, `@{"Text" link Node}`) — a few hundred lines (§13).

`nom` genuinely earns its place here, unlike in P2.4: this is line-command
heavy with many small similar productions, which is what combinators are good
at.

**Tests first:**
- A real AmigaGuide fixture from Aminet's `text/hyper/` parses to the expected
  AST.
- Node links resolve to node names.
- Inline attributes nest correctly.
- Malformed markup degrades to plain text rather than failing the document —
  these are thirty-year-old files and some are simply broken.
- Encoding is handled through P1.5, not assumed UTF-8.

**Why S:** a well-understood parser with a documented format and fixtures
available.

**Hand over:** §13's AmigaGuide paragraph, the `ag2html`/`guide2html`
converters as references, P1.5's decode helper, the five tests.

**Done when:** all five pass.

---

**Phase 9 exit:** one Vue frontend, running both as a desktop application and
in a browser, over a core that never learned which one it was talking to.
