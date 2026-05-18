-- 0004_telemetry — run history, telemetry event log, extraction cache per spec §4 + §10.

CREATE TABLE runs (
  id TEXT PRIMARY KEY,
  prompt_id TEXT NOT NULL,
  prompt_title TEXT NOT NULL,
  status TEXT NOT NULL,
  profile_json TEXT NOT NULL,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  exit_code INTEGER,
  signal TEXT,
  transcript_vault_path TEXT,
  transcript_spool_path TEXT,
  stdout_bytes INTEGER NOT NULL DEFAULT 0,
  stderr_bytes INTEGER NOT NULL DEFAULT 0,
  token_count_json TEXT,
  cost_usd REAL,
  error_json TEXT
);

CREATE INDEX runs_prompt_id_idx ON runs(prompt_id);
CREATE INDEX runs_status_idx ON runs(status);
CREATE INDEX runs_started_at_idx ON runs(started_at DESC);

CREATE TABLE telemetry_events (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  prompt_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  created_at TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
);

CREATE INDEX telemetry_events_prompt_id_idx ON telemetry_events(prompt_id);
CREATE INDEX telemetry_events_event_type_idx ON telemetry_events(event_type);

CREATE TABLE extraction_cache (
  cache_key TEXT PRIMARY KEY,
  source_kind TEXT NOT NULL,
  origin_url TEXT NOT NULL,
  fetched_content_json TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  expires_at TEXT NOT NULL
);

CREATE INDEX extraction_cache_expires_idx ON extraction_cache(expires_at);
