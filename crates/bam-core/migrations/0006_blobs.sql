-- Blob cache (§6): BLAKE3-addressed archive bytes live on disk via
-- BlobStore; this table tracks them for LRU eviction. `package.archive_hash`
-- maps a package to its cached blob; NULL means not cached (or evicted).
CREATE TABLE blobs (
  hash      TEXT PRIMARY KEY,
  size      INTEGER NOT NULL,
  last_used TEXT NOT NULL,
  pinned    INTEGER NOT NULL DEFAULT 0
);

ALTER TABLE package ADD COLUMN archive_hash TEXT REFERENCES blobs(hash);
