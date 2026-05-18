-- 0003_embeddings — local embedding storage for hybrid search per spec §4 + §9.
--
-- The plain `prompt_embeddings` table stores the canonical vector blobs.
-- A companion `sqlite-vec` `vec0` virtual table (declared at runtime once the
-- extension is loaded — see `index::embeddings` in L5) provides the kNN search
-- surface. We don't declare `vec0` here because the extension isn't available
-- in `sqlx::migrate!`'s default connection; the runtime path attaches it after
-- `PRAGMA load_extension` is permitted.

CREATE TABLE prompt_embeddings (
  prompt_id TEXT PRIMARY KEY,
  model TEXT NOT NULL,
  dim INTEGER NOT NULL,
  embedding BLOB NOT NULL,
  indexed_checksum_sha256 TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (prompt_id) REFERENCES prompts(id) ON DELETE CASCADE
);

CREATE INDEX prompt_embeddings_model_idx ON prompt_embeddings(model);
