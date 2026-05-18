-- 0002_fts — FTS5 virtual table for full-text search across prompts.
--
-- Spec §4 SQLite schema specifies `tokenize = 'unicode61 remove_diacritics 2'`.
-- The triggers keep `prompts_fts` mirrored to the canonical `prompts` rows.
-- Tag concatenation is handled at write time by the repo layer (L5), so the
-- `tags` column receives an empty string by default in these triggers.

CREATE VIRTUAL TABLE prompts_fts USING fts5(
  prompt_id UNINDEXED,
  title,
  summary,
  body,
  tags,
  tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER prompts_fts_ai AFTER INSERT ON prompts BEGIN
  INSERT INTO prompts_fts(prompt_id, title, summary, body, tags)
  VALUES (new.id, new.title, new.summary, new.body, '');
END;

CREATE TRIGGER prompts_fts_au AFTER UPDATE ON prompts BEGIN
  UPDATE prompts_fts
    SET title = new.title,
        summary = new.summary,
        body = new.body
   WHERE prompt_id = new.id;
END;

CREATE TRIGGER prompts_fts_ad AFTER DELETE ON prompts BEGIN
  DELETE FROM prompts_fts WHERE prompt_id = old.id;
END;
