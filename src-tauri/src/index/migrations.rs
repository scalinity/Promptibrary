//! Migration runner.
//!
//! Migrations live under `src-tauri/migrations/` and apply in numeric order:
//! `0001_init` → `0002_fts` → `0003_embeddings` → `0004_telemetry` → `0005_fk_cascade`
//! → `0006_fts_tags_sync`.
//!
//! Callers MUST use [`crate::index::db::connect`] (or `connect_options`) to
//! build the pool. That factory applies the three CLAUDE.md PRAGMAs
//! (`foreign_keys=ON`, `journal_mode=WAL`, `busy_timeout=5000`) on every
//! connection — without them, every `ON DELETE CASCADE` declared in the schema
//! is silently inert.

#[allow(dead_code)] // wired up by AppServices in L1
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[allow(dead_code)] // called by AppServices on startup in L1
pub async fn run_migrations(pool: &sqlx::SqlitePool) -> crate::error::Result<()> {
    MIGRATOR
        .run(pool)
        .await
        .map_err(crate::error::AppError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::db::connect_options;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::SqlitePool;

    /// Build a pool against a fresh on-disk SQLite file using the canonical
    /// `connect_options`. The `tempfile::TempDir` is leaked intentionally —
    /// the pool keeps the file open for the test duration and the OS reclaims
    /// the temp directory on process exit. Switching to `:memory:` is tracked
    /// as SCA-575 (F12); doing it here would couple F1 + F12 in one commit.
    async fn temp_pool() -> SqlitePool {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("test.sqlite");
        std::mem::forget(dir);
        let opts = connect_options(&path);
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .expect("connect to temp sqlite")
    }

    #[tokio::test]
    async fn run_migrations_applies_all_four() {
        let pool = temp_pool().await;
        run_migrations(&pool).await.expect("migrations ok");

        // Spot-check that each migration's primary objects exist.
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type IN ('table','view') ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .expect("query tables");

        for expected in [
            "prompts",
            "prompt_tags",
            "prompts_fts",
            "prompt_embeddings",
            "runs",
            "telemetry_events",
            "extraction_cache",
        ] {
            assert!(
                tables.iter().any(|t| t == expected),
                "expected table {expected} missing from {tables:?}"
            );
        }
    }

    #[tokio::test]
    async fn pragmas_are_set_on_pool_connections() {
        // Regression test for SCA-564 — without these PRAGMAs every
        // ON DELETE CASCADE in the schema is silently inert.
        let pool = temp_pool().await;
        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .expect("read foreign_keys");
        assert_eq!(foreign_keys, 1, "foreign_keys must be ON");

        let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await
            .expect("read journal_mode");
        assert_eq!(journal_mode.to_lowercase(), "wal", "journal_mode must be WAL");

        let busy_timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
            .fetch_one(&pool)
            .await
            .expect("read busy_timeout");
        assert_eq!(busy_timeout, 5000, "busy_timeout must be 5000ms");
    }

    #[tokio::test]
    async fn fts_triggers_mirror_inserts() {
        let pool = temp_pool().await;
        run_migrations(&pool).await.expect("migrations ok");

        insert_minimal_prompt(&pool, "01ABC", "promptibrary/prompts/hello.md").await;

        let hits: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM prompts_fts WHERE prompts_fts MATCH 'searchable'",
        )
        .fetch_one(&pool)
        .await
        .expect("fts query");
        assert_eq!(hits, 1, "FTS trigger should mirror inserted row");
    }

    #[tokio::test]
    async fn prompt_delete_cascades_to_prompt_tags() {
        // Regression test for SCA-564 — proves PRAGMA foreign_keys=ON makes
        // the ON DELETE CASCADE on prompt_tags actually fire.
        let pool = temp_pool().await;
        run_migrations(&pool).await.expect("migrations ok");

        insert_minimal_prompt(&pool, "01XYZ", "promptibrary/prompts/x.md").await;
        sqlx::query("INSERT INTO prompt_tags (prompt_id, tag_name) VALUES (?, ?)")
            .bind("01XYZ")
            .bind("agentic")
            .execute(&pool)
            .await
            .expect("insert tag");

        let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM prompt_tags")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(before, 1);

        sqlx::query("DELETE FROM prompts WHERE id = ?")
            .bind("01XYZ")
            .execute(&pool)
            .await
            .expect("delete prompt");

        let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM prompt_tags")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(after, 0, "prompt_tags row must cascade-delete with its prompt");
    }

    async fn insert_minimal_prompt(pool: &SqlitePool, id: &str, vault_path: &str) {
        sqlx::query(
            "INSERT INTO prompts (id, title, slug, summary, body, vault_path,
             created_at, updated_at, source_kind, source_json, variables_json,
             launch_defaults_json, checksum_sha256)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind("Hello world prompt")
        .bind("hello-world")
        .bind("A summary")
        .bind("body with searchable text")
        .bind(vault_path)
        .bind("2026-05-18T14:00:00Z")
        .bind("2026-05-18T14:00:00Z")
        .bind("manual")
        .bind("{}")
        .bind("[]")
        .bind("{}")
        .bind("sha256:0")
        .execute(pool)
        .await
        .expect("insert prompt");
    }
}
