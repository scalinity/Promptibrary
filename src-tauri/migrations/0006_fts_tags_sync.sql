-- 0006_fts_tags_sync — keep `prompts_fts.tags` in sync without relying on
-- the repo layer to recompute it on every write (review report 🟡 #4 /
-- SCA-566).
--
-- The original 0002 trigger set `tags = ''` on INSERT and never touched the
-- column on UPDATE. With L5's tag weighting (FTS weight 4.0 — the highest),
-- a stale `tags` column would silently degrade ranking. We replace the
-- prompt-side triggers with subquery-driven versions, and add prompt_tags
-- triggers so tag inserts/deletes immediately reflect in FTS.
--
-- All four triggers source `tags` from a single GROUP_CONCAT against
-- prompt_tags, so the repo layer has nothing extra to remember.

DROP TRIGGER IF EXISTS prompts_fts_ai;
DROP TRIGGER IF EXISTS prompts_fts_au;
DROP TRIGGER IF EXISTS prompts_fts_ad;

CREATE TRIGGER prompts_fts_ai AFTER INSERT ON prompts BEGIN
  INSERT INTO prompts_fts(prompt_id, title, summary, body, tags)
  VALUES (
    new.id,
    new.title,
    new.summary,
    new.body,
    COALESCE(
      (SELECT GROUP_CONCAT(tag_name, ' ') FROM prompt_tags WHERE prompt_id = new.id),
      ''
    )
  );
END;

CREATE TRIGGER prompts_fts_au AFTER UPDATE ON prompts BEGIN
  UPDATE prompts_fts
    SET title = new.title,
        summary = new.summary,
        body = new.body,
        tags = COALESCE(
          (SELECT GROUP_CONCAT(tag_name, ' ') FROM prompt_tags WHERE prompt_id = new.id),
          ''
        )
   WHERE prompt_id = new.id;
END;

CREATE TRIGGER prompts_fts_ad AFTER DELETE ON prompts BEGIN
  DELETE FROM prompts_fts WHERE prompt_id = old.id;
END;

-- prompt_tags triggers: any tag mutation re-derives the tags column for that
-- prompt's FTS row. Uses the same GROUP_CONCAT subquery to stay consistent.
CREATE TRIGGER prompt_tags_fts_ai AFTER INSERT ON prompt_tags BEGIN
  UPDATE prompts_fts
    SET tags = COALESCE(
      (SELECT GROUP_CONCAT(tag_name, ' ') FROM prompt_tags WHERE prompt_id = new.prompt_id),
      ''
    )
   WHERE prompt_id = new.prompt_id;
END;

CREATE TRIGGER prompt_tags_fts_ad AFTER DELETE ON prompt_tags BEGIN
  UPDATE prompts_fts
    SET tags = COALESCE(
      (SELECT GROUP_CONCAT(tag_name, ' ') FROM prompt_tags WHERE prompt_id = old.prompt_id),
      ''
    )
   WHERE prompt_id = old.prompt_id;
END;
