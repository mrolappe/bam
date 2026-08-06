-- Landing: append-only, exactly what the origin said. `raw` is BLOB, not
-- TEXT — encoding is detected later (see bam-handoff.md §13) and must stay
-- correctable without re-fetching.
CREATE TABLE landing_index_line (
  id         INTEGER PRIMARY KEY,
  fetched_at TEXT NOT NULL,
  source_url TEXT NOT NULL,
  line_no    INTEGER NOT NULL,
  raw        BLOB NOT NULL
);

-- Normalized: derived, droppable, rebuildable from landing with no network.
-- date_precision distinguishes Aminet's +/-1-week INDEX-derived dates from
-- exact ones; it may be upgraded 'week' -> 'exact', never the reverse.
CREATE TABLE package (
  id             INTEGER PRIMARY KEY,
  dir            TEXT NOT NULL,
  file           TEXT NOT NULL,
  name           TEXT NOT NULL,
  version        TEXT,
  size_bytes     INTEGER,
  uploaded_on    TEXT,
  date_precision TEXT NOT NULL,
  description    TEXT,
  landing_id     INTEGER NOT NULL REFERENCES landing_index_line(id),
  UNIQUE(dir, file)
);

-- Enrichment: cascades on package delete, but survives re-derivation of
-- `package` from landing (re-normalizing must not discard e.g. LLM summaries).
CREATE TABLE enrichment (
  package_id       INTEGER NOT NULL REFERENCES package(id) ON DELETE CASCADE,
  kind             TEXT NOT NULL,
  producer_version INTEGER NOT NULL,
  produced_at      TEXT NOT NULL,
  payload          TEXT NOT NULL,
  PRIMARY KEY (package_id, kind)
);

-- Selections: a core concept (invariant I7), not TUI state.
CREATE TABLE selection (
  id         INTEGER PRIMARY KEY,
  name       TEXT UNIQUE,
  created_at TEXT NOT NULL,
  ephemeral  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE selection_member (
  selection_id INTEGER NOT NULL REFERENCES selection(id) ON DELETE CASCADE,
  package_id   INTEGER NOT NULL REFERENCES package(id)   ON DELETE CASCADE,
  PRIMARY KEY (selection_id, package_id)
);
