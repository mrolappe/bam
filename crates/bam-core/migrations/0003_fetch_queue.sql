CREATE TABLE fetch_queue (
  url             TEXT PRIMARY KEY,
  kind            TEXT NOT NULL,
  priority        INTEGER NOT NULL DEFAULT 0,
  attempts        INTEGER NOT NULL DEFAULT 0,
  next_attempt_at TEXT,
  etag            TEXT,
  last_status     INTEGER,
  claimed_at      TEXT
);
