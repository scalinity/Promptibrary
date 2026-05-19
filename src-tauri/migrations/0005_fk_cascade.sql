-- 0005_fk_cascade — add foreign-key + ON DELETE CASCADE to runs and
-- telemetry_events so deleting a prompt no longer leaves orphaned rows
-- (review report 🟡 #2 / SCA-565).
--
-- SQLite has no `ALTER TABLE … ADD CONSTRAINT`, so we follow the standard
-- table-swap pattern documented in https://sqlite.org/lang_altertable.html
-- §7 ("Making Other Kinds Of Table Schema Changes"). Existing rows (if any —
-- L0 ships before anyone has data) copy across via `INSERT … SELECT`.
--
-- The migration is wrapped in sqlx's per-file transaction. `PRAGMA
-- foreign_keys = OFF` is set explicitly at the top because the table swap
-- temporarily violates the FK we are adding (the old `runs` table holds
-- nominal references); SQLite requires FKs off during the dance.

PRAGMA foreign_keys = OFF;

-- --- runs ---
CREATE TABLE runs_new (
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
  error_json TEXT,
  FOREIGN KEY (prompt_id) REFERENCES prompts(id) ON DELETE CASCADE
);

INSERT INTO runs_new SELECT * FROM runs;
DROP TABLE runs;
ALTER TABLE runs_new RENAME TO runs;

CREATE INDEX runs_prompt_id_idx ON runs(prompt_id);
CREATE INDEX runs_status_idx ON runs(status);
CREATE INDEX runs_started_at_idx ON runs(started_at DESC);

-- --- telemetry_events ---
-- Old definition has FK only on run_id (added in 0004). We expand to include
-- prompt_id as well so prompt-cascade works without going through runs.
CREATE TABLE telemetry_events_new (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  prompt_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  created_at TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE,
  FOREIGN KEY (prompt_id) REFERENCES prompts(id) ON DELETE CASCADE
);

INSERT INTO telemetry_events_new SELECT * FROM telemetry_events;
DROP TABLE telemetry_events;
ALTER TABLE telemetry_events_new RENAME TO telemetry_events;

CREATE INDEX telemetry_events_prompt_id_idx ON telemetry_events(prompt_id);
CREATE INDEX telemetry_events_event_type_idx ON telemetry_events(event_type);

PRAGMA foreign_keys = ON;
