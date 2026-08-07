-- Landing for fetched READMEs. `raw` is BLOB, not TEXT — same reasoning as
-- landing_index_line (P1.2): encoding is detected later and must stay
-- correctable without re-fetching. Unlike landing_index_line, this table is
-- keyed by url and upserted, not append-only: a re-fetch updates the same row.
CREATE TABLE landing_readme (
  id                INTEGER PRIMARY KEY,
  package_id        INTEGER NOT NULL REFERENCES package(id) ON DELETE CASCADE,
  url               TEXT NOT NULL UNIQUE,
  fetched_at        TEXT NOT NULL,
  raw               BLOB NOT NULL,
  detected_encoding TEXT NOT NULL
);
