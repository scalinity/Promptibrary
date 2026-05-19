//! Telemetry event log + per-prompt aggregate computation per spec §10.
//!
//! Two responsibilities:
//!
//! 1. **Event log** — append-only `telemetry_events` rows, one per
//!    launch lifecycle moment. The seven event types defined in spec
//!    §10 are encoded as the `LaunchTelemetryEvent` enum below; the
//!    payload travels as JSON in the `payload_json` column.
//!
//! 2. **Per-prompt aggregates** — `prompt_aggregates(db)` computes the
//!    per-prompt `PromptTelemetrySummary` (launch_count, last_used_at,
//!    success_rate, avg_run_seconds, avg_token_count) directly from the
//!    `runs` table. The §10 spec mentions a cached `prompt_stats` table
//!    for future optimisation; this implementation computes on demand
//!    because the dataset for V1 is small (~hundreds of runs) and the
//!    aggregate SQL is bounded by a single GROUP BY.
//!
//! ## L3 gap (surfaced — see `docs/notes/L5-observations.md`)
//!
//! The launch pipeline does not currently persist run rows at lifecycle
//! transitions. The repo + aggregate functions here are wired and
//! tested against synthetic `runs` data; the production-path
//! end-to-end acceptance criterion (launch 3× → library row shows
//! `launch_count: 3`) cannot pass until L3 is reconciled. The
//! reviewer's call.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::{AppError, Result};

// ─── Event types ──────────────────────────────────────────────────────

/// The seven launch lifecycle event types from spec §10 *Telemetry
/// fields per launch*. Each variant carries a typed payload that
/// serializes to the `payload_json` column.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LaunchTelemetryEvent {
    LaunchCreated {
        profile_id: String,
        model: String,
    },
    LaunchStarted {
        process_id: i64,
    },
    /// First byte of PTY output observed. `latency_ms` measures the
    /// time from `LaunchStarted` and is what the L5 hybrid-ranking
    /// recency boost (and the diagnostics card) consume.
    FirstOutput {
        latency_ms: i64,
    },
    LaunchStopped {
        signal: String,
        graceful: bool,
    },
    LaunchFinished {
        exit_code: i32,
        wall_clock_ms: i64,
        token_count: Option<i64>,
        cost_usd: Option<f64>,
    },
    TranscriptSpooled {
        path: String,
        bytes: i64,
    },
    Error {
        kind: String,
        message: String,
    },
}

impl LaunchTelemetryEvent {
    /// Wire-event-type string written to `telemetry_events.event_type`.
    /// These literals are also what the §10 query examples join on.
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::LaunchCreated { .. } => "launch_created",
            Self::LaunchStarted { .. } => "launch_started",
            Self::FirstOutput { .. } => "first_output",
            Self::LaunchStopped { .. } => "launch_stopped",
            Self::LaunchFinished { .. } => "launch_finished",
            Self::TranscriptSpooled { .. } => "transcript_spooled",
            Self::Error { .. } => "error",
        }
    }
}

/// Append a typed event to the telemetry log. The id is a fresh ULID
/// generated inside this function; the caller supplies the run/prompt
/// linkage and the typed payload.
pub async fn record_event(
    db: &SqlitePool,
    run_id: &str,
    prompt_id: &str,
    event: &LaunchTelemetryEvent,
) -> Result<()> {
    let payload_json = serde_json::to_string(event).map_err(AppError::from)?;
    let id = crate::ids::new_ulid();
    let created_at = crate::time::now_utc().to_rfc3339();
    sqlx::query(
        "INSERT INTO telemetry_events (id, run_id, prompt_id, event_type, created_at, payload_json)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(run_id)
    .bind(prompt_id)
    .bind(event.event_type())
    .bind(created_at)
    .bind(payload_json)
    .execute(db)
    .await
    .map_err(AppError::from)?;
    Ok(())
}

// ─── Per-prompt aggregates ────────────────────────────────────────────

/// Aggregate summary for one prompt — what the library row displays.
/// Mirrors `domain::prompt::PromptTelemetrySummary` so the values can
/// be hydrated directly onto the `Prompt` struct served by the index.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptAggregateRow {
    pub prompt_id: String,
    pub launch_count: i64,
    pub last_used_at: Option<DateTime<Utc>>,
    pub success_rate: Option<f64>,
    pub avg_run_seconds: Option<f64>,
    pub avg_token_count: Option<i64>,
}

/// Compute per-prompt aggregates across the entire `runs` table.
///
/// TODO(scale): full-table scan with `GROUP BY prompt_id` every call.
/// Acceptable at V1 (~hundreds of runs); V2 should land the spec §10
/// `prompt_stats` cache table per `docs/V2-CANDIDATES.md`.
///
/// Per §10:
/// - `launch_count` — total runs for this prompt.
/// - `last_used_at` — `MAX(started_at)`.
/// - `success_rate` — fraction of *finished* runs that had exit_code=0;
///   NULL when no run has reached `finished` status.
/// - `avg_run_seconds` — average of `ended_at - started_at` over
///   finished runs. NULL when no runs have ended.
/// - `avg_token_count` — average of the integer value at
///   `json_extract(token_count_json, '$.total')` across runs that
///   reported one. NULL when none reported.
pub async fn prompt_aggregates(db: &SqlitePool) -> Result<Vec<PromptAggregateRow>> {
    let rows: Vec<(
        String,
        i64,
        Option<String>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
    )> = sqlx::query_as(
        "SELECT prompt_id,
                COUNT(*) AS launch_count,
                MAX(started_at) AS last_used_at,
                AVG(CASE WHEN status = 'finished' AND exit_code = 0 THEN 1.0
                         WHEN status = 'finished' THEN 0.0
                         ELSE NULL END) AS success_rate,
                AVG(CASE WHEN ended_at IS NOT NULL
                         THEN (julianday(ended_at) - julianday(started_at)) * 86400.0
                         ELSE NULL END) AS avg_run_seconds,
                AVG(CASE WHEN token_count_json IS NOT NULL
                              AND json_valid(token_count_json)
                         THEN CAST(json_extract(token_count_json, '$.total') AS REAL)
                         ELSE NULL END) AS avg_token_count
           FROM runs
       GROUP BY prompt_id",
    )
    .fetch_all(db)
    .await
    .map_err(AppError::from)?;

    Ok(rows
        .into_iter()
        .map(
            |(prompt_id, launch_count, last_used_at, success_rate, avg_run_seconds, avg_token_count)| {
                PromptAggregateRow {
                    prompt_id,
                    launch_count,
                    last_used_at: last_used_at
                        .as_deref()
                        .and_then(crate::time::parse_db_timestamp),
                    success_rate,
                    avg_run_seconds,
                    avg_token_count: avg_token_count.map(|v| v.round() as i64),
                }
            },
        )
        .collect())
}

/// Drop every row from `telemetry_events`. Used by the patched §13
/// *Clear telemetry cache* destructive action. `runs` rows are
/// preserved by design — clearing the event log purges the
/// per-event detail without removing the run-level summary the
/// library row reads.
pub async fn clear_event_log(db: &SqlitePool) -> Result<u64> {
    let res = sqlx::query("DELETE FROM telemetry_events")
        .execute(db)
        .await
        .map_err(AppError::from)?;
    Ok(res.rows_affected())
}

/// Count the events recorded for a single run. Used by the run-detail
/// view and the integration test for the launch-lifecycle hookup.
pub async fn event_count_for_run(db: &SqlitePool, run_id: &str) -> Result<i64> {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM telemetry_events WHERE run_id = ?")
            .bind(run_id)
            .fetch_one(db)
            .await
            .map_err(AppError::from)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
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

    /// Insert a minimal prompt so the FK on telemetry_events (via runs)
    /// resolves. Returns the prompt id we used.
    async fn seed_prompt(db: &SqlitePool, id: &str) -> String {
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
        id.into()
    }

    async fn insert_run(
        db: &SqlitePool,
        run_id: &str,
        prompt_id: &str,
        status: &str,
        exit_code: Option<i32>,
        started_at: &str,
        ended_at: Option<&str>,
        token_total: Option<i64>,
    ) {
        sqlx::query(
            "INSERT INTO runs (id, prompt_id, prompt_title, status, profile_json,
                started_at, ended_at, exit_code, stdout_bytes, stderr_bytes,
                token_count_json)
             VALUES (?, ?, 'T', ?, '{}', ?, ?, ?, 0, 0, ?)",
        )
        .bind(run_id)
        .bind(prompt_id)
        .bind(status)
        .bind(started_at)
        .bind(ended_at)
        .bind(exit_code)
        .bind(token_total.map(|t| format!("{{\"total\":{t}}}")))
        .execute(db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn record_event_inserts_typed_payload() {
        let db = temp_pool().await;
        let prompt_id = seed_prompt(&db, "p1").await;
        insert_run(
            &db,
            "r1",
            &prompt_id,
            "started",
            None,
            "2026-05-19T00:00:00Z",
            None,
            None,
        )
        .await;

        record_event(
            &db,
            "r1",
            &prompt_id,
            &LaunchTelemetryEvent::FirstOutput { latency_ms: 327 },
        )
        .await
        .unwrap();

        let count = event_count_for_run(&db, "r1").await.unwrap();
        assert_eq!(count, 1);

        let (event_type, payload_json): (String, String) = sqlx::query_as(
            "SELECT event_type, payload_json FROM telemetry_events WHERE run_id = ?",
        )
        .bind("r1")
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(event_type, "first_output");
        let payload: Value = serde_json::from_str(&payload_json).unwrap();
        assert_eq!(payload["latency_ms"], 327);
    }

    #[tokio::test]
    async fn prompt_aggregates_returns_launch_count_and_last_used() {
        let db = temp_pool().await;
        let p1 = seed_prompt(&db, "p1").await;
        let p2 = seed_prompt(&db, "p2").await;
        insert_run(
            &db, "r1", &p1, "finished", Some(0),
            "2026-05-19T00:00:00Z", Some("2026-05-19T00:00:30Z"), Some(100),
        )
        .await;
        insert_run(
            &db, "r2", &p1, "finished", Some(0),
            "2026-05-19T01:00:00Z", Some("2026-05-19T01:00:10Z"), Some(200),
        )
        .await;
        insert_run(
            &db, "r3", &p1, "finished", Some(1),
            "2026-05-19T02:00:00Z", Some("2026-05-19T02:00:50Z"), None,
        )
        .await;
        insert_run(
            &db, "r4", &p2, "running", None,
            "2026-05-19T03:00:00Z", None, None,
        )
        .await;

        let mut aggs = prompt_aggregates(&db).await.unwrap();
        aggs.sort_by(|a, b| a.prompt_id.cmp(&b.prompt_id));
        assert_eq!(aggs.len(), 2);

        let p1_row = &aggs[0];
        assert_eq!(p1_row.prompt_id, "p1");
        assert_eq!(p1_row.launch_count, 3);
        assert!(p1_row.last_used_at.is_some());
        // Two of three runs finished with exit 0 → 2/3 ≈ 0.667.
        assert!((p1_row.success_rate.unwrap() - (2.0 / 3.0)).abs() < 1e-6);
        // Wall clock: 30, 10, 50 seconds → average 30.
        assert!((p1_row.avg_run_seconds.unwrap() - 30.0).abs() < 0.01);
        // Token totals: 100, 200 (third run reported none) → avg 150.
        assert_eq!(p1_row.avg_token_count, Some(150));

        let p2_row = &aggs[1];
        assert_eq!(p2_row.launch_count, 1);
        // p2 has no finished runs ⇒ success_rate NULL.
        assert!(p2_row.success_rate.is_none());
        assert!(p2_row.avg_run_seconds.is_none());
        assert!(p2_row.avg_token_count.is_none());
    }

    #[tokio::test]
    async fn clear_event_log_drops_all_events_but_preserves_runs() {
        let db = temp_pool().await;
        let p1 = seed_prompt(&db, "p1").await;
        insert_run(
            &db, "r1", &p1, "finished", Some(0),
            "2026-05-19T00:00:00Z", Some("2026-05-19T00:00:10Z"), None,
        )
        .await;
        record_event(
            &db, "r1", &p1,
            &LaunchTelemetryEvent::LaunchCreated {
                profile_id: "pf1".into(),
                model: "claude-sonnet-4-6".into(),
            },
        )
        .await
        .unwrap();
        record_event(
            &db, "r1", &p1,
            &LaunchTelemetryEvent::LaunchFinished {
                exit_code: 0,
                wall_clock_ms: 10_000,
                token_count: Some(100),
                cost_usd: Some(0.01),
            },
        )
        .await
        .unwrap();

        let cleared = clear_event_log(&db).await.unwrap();
        assert_eq!(cleared, 2);
        let after = event_count_for_run(&db, "r1").await.unwrap();
        assert_eq!(after, 0);

        // Runs row is still there.
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM runs")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    /// SCA-746 regression: a row with a non-JSON token_count_json
    /// value must NOT error the entire prompt_aggregates SELECT.
    /// Pre-fix json_extract raised at runtime; the json_valid guard
    /// treats the row as if it didn't report a token count.
    #[tokio::test]
    async fn prompt_aggregates_tolerates_malformed_token_count_json() {
        let db = temp_pool().await;
        let p1 = seed_prompt(&db, "p1").await;
        // Good row.
        insert_run(
            &db, "r1", &p1, "finished", Some(0),
            "2026-05-19T00:00:00Z", Some("2026-05-19T00:00:30Z"), Some(100),
        )
        .await;
        // Malformed JSON row.
        sqlx::query(
            "INSERT INTO runs (id, prompt_id, prompt_title, status, profile_json,
                started_at, ended_at, exit_code, stdout_bytes, stderr_bytes,
                token_count_json)
             VALUES (?, ?, 'T', 'finished', '{}',
                '2026-05-19T01:00:00Z', '2026-05-19T01:00:10Z', 0, 0, 0,
                'not-json')",
        )
        .bind("r2")
        .bind(&p1)
        .execute(&db)
        .await
        .unwrap();

        let aggs = prompt_aggregates(&db).await.unwrap();
        assert_eq!(aggs.len(), 1);
        assert_eq!(aggs[0].launch_count, 2);
        // Avg only counts the good row's 100 — the malformed row
        // contributes NULL to the AVG, which SQLite drops.
        assert_eq!(aggs[0].avg_token_count, Some(100));
    }

    #[test]
    fn event_type_string_matches_spec_grammar() {
        for (event, expected) in [
            (
                LaunchTelemetryEvent::LaunchCreated {
                    profile_id: "p".into(),
                    model: "m".into(),
                },
                "launch_created",
            ),
            (LaunchTelemetryEvent::LaunchStarted { process_id: 1 }, "launch_started"),
            (LaunchTelemetryEvent::FirstOutput { latency_ms: 1 }, "first_output"),
            (
                LaunchTelemetryEvent::LaunchStopped {
                    signal: "SIGINT".into(),
                    graceful: true,
                },
                "launch_stopped",
            ),
            (
                LaunchTelemetryEvent::LaunchFinished {
                    exit_code: 0,
                    wall_clock_ms: 1,
                    token_count: None,
                    cost_usd: None,
                },
                "launch_finished",
            ),
            (
                LaunchTelemetryEvent::TranscriptSpooled {
                    path: "/tmp".into(),
                    bytes: 1,
                },
                "transcript_spooled",
            ),
            (
                LaunchTelemetryEvent::Error {
                    kind: "x".into(),
                    message: "y".into(),
                },
                "error",
            ),
        ] {
            assert_eq!(event.event_type(), expected);
        }
    }
}
