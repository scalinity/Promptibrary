-- 0001_init — base prompt table + tag join table per spec §4 SQLite schema.

CREATE TABLE prompts (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  slug TEXT NOT NULL,
  summary TEXT NOT NULL,
  body TEXT NOT NULL,
  vault_path TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  source_kind TEXT NOT NULL,
  source_json TEXT NOT NULL,
  variables_json TEXT NOT NULL,
  launch_defaults_json TEXT NOT NULL,
  checksum_sha256 TEXT NOT NULL
);

CREATE INDEX prompts_archived_idx ON prompts(archived_at);
CREATE INDEX prompts_updated_at_idx ON prompts(updated_at DESC);

CREATE TABLE prompt_tags (
  prompt_id TEXT NOT NULL,
  tag_name TEXT NOT NULL,
  PRIMARY KEY (prompt_id, tag_name),
  FOREIGN KEY (prompt_id) REFERENCES prompts(id) ON DELETE CASCADE
);

CREATE INDEX prompt_tags_name_idx ON prompt_tags(tag_name);
