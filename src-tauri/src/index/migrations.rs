//! Migration runner.
//!
//! Migrations live under `src-tauri/migrations/` and apply in numeric order:
//! `0001_init` → `0002_fts` → `0003_embeddings` → `0004_telemetry`. The runner
//! is invoked by `AppServices` on startup (L1+).

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
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    async fn temp_pool() -> sqlx::SqlitePool {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("test.sqlite");
        // Leak the tempdir for the duration of the test — the pool keeps the
        // file open and the dir drops when the test scope ends.
        std::mem::forget(dir);
        let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
            .unwrap()
            .create_if_missing(true)
            .foreign_keys(true);
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
    async fn fts_triggers_mirror_inserts() {
        let pool = temp_pool().await;
        run_migrations(&pool).await.expect("migrations ok");

        sqlx::query(
            "INSERT INTO prompts (id, title, slug, summary, body, vault_path,
             created_at, updated_at, source_kind, source_json, variables_json,
             launch_defaults_json, checksum_sha256)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("01ABC")
        .bind("Hello world prompt")
        .bind("hello-world")
        .bind("A summary")
        .bind("body with searchable text")
        .bind("promptibrary/prompts/hello.md")
        .bind("2026-05-18T14:00:00Z")
        .bind("2026-05-18T14:00:00Z")
        .bind("manual")
        .bind("{}")
        .bind("[]")
        .bind("{}")
        .bind("sha256:0")
        .execute(&pool)
        .await
        .expect("insert prompt");

        let hits: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM prompts_fts WHERE prompts_fts MATCH 'searchable'",
        )
        .fetch_one(&pool)
        .await
        .expect("fts query");
        assert_eq!(hits, 1, "FTS trigger should mirror inserted row");
    }
}
