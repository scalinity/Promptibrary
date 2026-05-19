-- 0007_extraction_candidates_cache — separate cache table for LLM extraction
-- output. The fetched-content side already exists as `extraction_cache` from
-- 0004; per spec §6, fetched content and candidates have different TTLs
-- (7 days / 30 days) and different invalidation triggers (fetched content
-- on origin URL change, candidates on prompt_version + model + mode change).

CREATE TABLE extraction_candidates_cache (
  cache_key TEXT PRIMARY KEY,
  source_kind TEXT NOT NULL,
  canonical_url TEXT NOT NULL,
  extraction_mode TEXT NOT NULL,
  model TEXT NOT NULL,
  prompt_version INTEGER NOT NULL,
  response_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL
);

CREATE INDEX extraction_candidates_cache_expires_idx
  ON extraction_candidates_cache(expires_at);
CREATE INDEX extraction_candidates_cache_url_idx
  ON extraction_candidates_cache(canonical_url);
