//! Run row CRUD per spec §10 *Run record lifecycle*.
//!
//! Three lifecycle moments produce writes:
//!
//! 1. **`insert_run`** — at PTY spawn. `status = "started"`, `started_at`
//!    set, `ended_at` / `exit_code` / `signal` all NULL.
//! 2. **`update_run_status`** — when the status transitions without a
//!    terminal exit (e.g. "running" → "stopping" in mid-flight on a
//!    user-requested stop). Just flips the column.
//! 3. **`complete_run`** — at process exit. Sets `ended_at`,
//!    `exit_code`, optional `signal`, optional `token_count_json`,
//!    optional `cost_usd`, optional transcript paths. Atomic, no
//!    intermediate states.
//!
//! ## Scope clarification (SCA-785)
//!
//! This module is the storage surface only. The L3 launch pipeline
//! that *calls* these functions is currently stubbed (every file under
//! `src/launch/` is 3 lines; `portable-pty` is commented out in
//! `Cargo.toml`). L5 telemetry's `prompt_aggregates` reads `runs` via
//! `MAX(started_at)` / `COUNT(*)` / etc., so it's correct against
//! synthetic data inserted via this surface but the production-path
//! end-to-end acceptance criterion ("launch 3× → library row shows
//! launch_count: 3") still depends on the L3 launch pipeline landing.

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::error::{AppError, Result};

// SCA-910: canonical RunStatus lives in `domain::run`. We re-export it
// here so the storage-layer callsites (insert_run, update_run_status,
// complete_run, plus tests) keep their existing `runs_repo::RunStatus`
// imports while the type is unified across the wire.
pub use crate::domain::run::RunStatus;

/// Input bundle for `insert_run`. Mirrors the §10 *Run record fields*
/// schema for the values known at PTY-spawn time.
#[derive(Debug, Clone)]
pub struct InsertRunInput<'a> {
    pub run_id: &'a str,
    pub prompt_id: &'a str,
    pub prompt_title: &'a str,
    /// Serialised `LaunchProfile` snapshot — captures the resolved
    /// variable values, working directory, launch defaults overrides.
    pub profile_json: &'a str,
    pub started_at: DateTime<Utc>,
}

/// Insert a fresh run row in the `started` state. Idempotent against
/// the primary key — re-running with the same `run_id` returns Err
/// rather than silently double-inserting; the launch pipeline is
/// expected to generate a new ULID per launch.
pub async fn insert_run(db: &SqlitePool, input: &InsertRunInput<'_>) -> Result<()> {
    sqlx::query(
        "INSERT INTO runs (id, prompt_id, prompt_title, status, profile_json,
            started_at, stdout_bytes, stderr_bytes)
         VALUES (?, ?, ?, ?, ?, ?, 0, 0)",
    )
    .bind(input.run_id)
    .bind(input.prompt_id)
    .bind(input.prompt_title)
    .bind(RunStatus::Started.as_str())
    .bind(input.profile_json)
    .bind(input.started_at.to_rfc3339())
    .execute(db)
    .await
    .map_err(AppError::from)?;
    Ok(())
}

/// Flip the status column on an existing row. Used for mid-flight
/// transitions (`Started → FirstOutput → Running → Stopping`) that
/// don't yet have an exit code.
pub async fn update_run_status(
    db: &SqlitePool,
    run_id: &str,
    status: RunStatus,
) -> Result<()> {
    let res = sqlx::query("UPDATE runs SET status = ? WHERE id = ?")
        .bind(status.as_str())
        .bind(run_id)
        .execute(db)
        .await
        .map_err(AppError::from)?;
    if res.rows_affected() == 0 {
        return Err(AppError::new(
            crate::error::AppErrorKind::RunNotFound,
            format!("update_run_status: run {run_id} not found"),
        ));
    }
    Ok(())
}

/// Input bundle for `complete_run`. Captures the terminal-state
/// columns from spec §10.
#[derive(Debug, Clone)]
pub struct CompleteRunInput<'a> {
    pub run_id: &'a str,
    pub status: RunStatus,
    pub ended_at: DateTime<Utc>,
    pub exit_code: Option<i32>,
    pub signal: Option<&'a str>,
    pub transcript_vault_path: Option<&'a str>,
    pub transcript_spool_path: Option<&'a str>,
    pub stdout_bytes: i64,
    pub stderr_bytes: i64,
    pub token_count_json: Option<&'a str>,
    pub cost_usd: Option<f64>,
    pub error_json: Option<&'a str>,
}

/// Mark a run terminal. One atomic UPDATE sets every terminal column.
pub async fn complete_run(db: &SqlitePool, input: &CompleteRunInput<'_>) -> Result<()> {
    let res = sqlx::query(
        "UPDATE runs SET
            status = ?,
            ended_at = ?,
            exit_code = ?,
            signal = ?,
            transcript_vault_path = ?,
            transcript_spool_path = ?,
            stdout_bytes = ?,
            stderr_bytes = ?,
            token_count_json = ?,
            cost_usd = ?,
            error_json = ?
          WHERE id = ?",
    )
    .bind(input.status.as_str())
    .bind(input.ended_at.to_rfc3339())
    .bind(input.exit_code)
    .bind(input.signal)
    .bind(input.transcript_vault_path)
    .bind(input.transcript_spool_path)
    .bind(input.stdout_bytes)
    .bind(input.stderr_bytes)
    .bind(input.token_count_json)
    .bind(input.cost_usd)
    .bind(input.error_json)
    .bind(input.run_id)
    .execute(db)
    .await
    .map_err(AppError::from)?;
    if res.rows_affected() == 0 {
        return Err(AppError::new(
            crate::error::AppErrorKind::RunNotFound,
            format!("complete_run: run {} not found", input.run_id),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::db::in_memory_connect_options;
    use crate::index::migrations::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn temp_pool() -> SqlitePool {
        let opts = in_memory_connect_options();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        run_migrations(&pool).await.unwrap();
        pool
    }

    async fn seed_prompt(db: &SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO prompts (id, title, slug, summary, body, vault_path,
                created_at, updated_at, source_kind, source_json,
                variables_json, launch_defaults_json, checksum_sha256)
             VALUES (?, 'T', ?, '', '', ?, ?, ?, 'manual', '{}', '[]', '{}', 'sha256:0')",
        )
        .bind(id)
        .bind(id)
        .bind(format!("promptibrary/prompts/{id}.md"))
        .bind("2026-05-19T00:00:00Z")
        .bind("2026-05-19T00:00:00Z")
        .execute(db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn insert_then_complete_round_trip() {
        let db = temp_pool().await;
        seed_prompt(&db, "p1").await;
        let started = "2026-05-19T01:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let ended = "2026-05-19T01:00:30Z".parse::<DateTime<Utc>>().unwrap();

        insert_run(
            &db,
            &InsertRunInput {
                run_id: "r1",
                prompt_id: "p1",
                prompt_title: "Test",
                profile_json: "{}",
                started_at: started,
            },
        )
        .await
        .unwrap();

        let (status,): (String,) = sqlx::query_as("SELECT status FROM runs WHERE id = 'r1'")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(status, "started");

        complete_run(
            &db,
            &CompleteRunInput {
                run_id: "r1",
                status: RunStatus::Finished,
                ended_at: ended,
                exit_code: Some(0),
                signal: None,
                transcript_vault_path: Some("promptibrary/runs/2026/05/r1.md"),
                transcript_spool_path: None,
                stdout_bytes: 1024,
                stderr_bytes: 0,
                token_count_json: Some(r#"{"total":150}"#),
                cost_usd: Some(0.01),
                error_json: None,
            },
        )
        .await
        .unwrap();

        let (status, exit_code, token_count_json): (String, Option<i32>, Option<String>) =
            sqlx::query_as(
                "SELECT status, exit_code, token_count_json FROM runs WHERE id = 'r1'",
            )
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(status, "finished");
        assert_eq!(exit_code, Some(0));
        assert_eq!(token_count_json.as_deref(), Some(r#"{"total":150}"#));
    }

    #[tokio::test]
    async fn update_run_status_intermediate_transitions() {
        let db = temp_pool().await;
        seed_prompt(&db, "p1").await;
        insert_run(
            &db,
            &InsertRunInput {
                run_id: "r1",
                prompt_id: "p1",
                prompt_title: "T",
                profile_json: "{}",
                started_at: Utc::now(),
            },
        )
        .await
        .unwrap();

        for s in [RunStatus::FirstOutput, RunStatus::Stopping] {
            update_run_status(&db, "r1", s).await.unwrap();
            let (got,): (String,) = sqlx::query_as("SELECT status FROM runs WHERE id = 'r1'")
                .fetch_one(&db)
                .await
                .unwrap();
            assert_eq!(got, s.as_str());
        }
    }

    #[tokio::test]
    async fn update_run_status_unknown_run_errors() {
        let db = temp_pool().await;
        let err = update_run_status(&db, "nope", RunStatus::FirstOutput)
            .await
            .unwrap_err();
        assert_eq!(err.kind, crate::error::AppErrorKind::RunNotFound);
    }

    #[tokio::test]
    async fn complete_run_unknown_run_errors() {
        let db = temp_pool().await;
        let err = complete_run(
            &db,
            &CompleteRunInput {
                run_id: "nope",
                status: RunStatus::Finished,
                ended_at: Utc::now(),
                exit_code: Some(0),
                signal: None,
                transcript_vault_path: None,
                transcript_spool_path: None,
                stdout_bytes: 0,
                stderr_bytes: 0,
                token_count_json: None,
                cost_usd: None,
                error_json: None,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind, crate::error::AppErrorKind::RunNotFound);
    }

    #[tokio::test]
    async fn duplicate_insert_run_errors() {
        let db = temp_pool().await;
        seed_prompt(&db, "p1").await;
        let input = InsertRunInput {
            run_id: "r1",
            prompt_id: "p1",
            prompt_title: "T",
            profile_json: "{}",
            started_at: Utc::now(),
        };
        insert_run(&db, &input).await.unwrap();
        // Second insert hits the PRIMARY KEY uniqueness; the launch
        // pipeline must not retry with the same ULID.
        let err = insert_run(&db, &input).await.unwrap_err();
        // PRIMARY KEY violation surfaces as a generic SQL error rather
        // than ForeignKeyViolation; we just assert it errored.
        assert!(!err.message.is_empty());
    }
}
