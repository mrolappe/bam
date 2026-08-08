-- One embedding vector per package (P7.4), packed as raw little-endian
-- float32 bytes — `vec_distance_cosine` (sqlite-vec) reads that layout
-- directly, no further packing needed at query time. `model`/`dim` are
-- stored alongside the vector so a model switch (different `dim`) is
-- detectable rather than silently comparing incompatible vectors.
CREATE TABLE package_embedding (
  package_id INTEGER PRIMARY KEY REFERENCES package(id) ON DELETE CASCADE,
  model      TEXT NOT NULL,
  dim        INTEGER NOT NULL,
  vector     BLOB NOT NULL
);
