-- HTTP conditional-GET cache: one ETag per URL, so a repeat fetch of an
-- unchanged INDEX/RECENT costs a single 304.
CREATE TABLE http_cache (
  url  TEXT PRIMARY KEY,
  etag TEXT NOT NULL
);
