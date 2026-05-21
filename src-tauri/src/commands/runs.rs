//! `commands::runs` per spec §11.
//!
//! SCA-917: read-path implementations for the 5 IPC handlers. Writes
//! land via `commands::launches` (start/finish lifecycle); this
//! module reads back what the launch pipeline persisted to the `runs`
//! table + transcript spool.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_state::ManagedState;
use crate::commands::vault::current_vault_db;
use crate::domain::run::Run;
use crate::error::{AppError, AppErrorKind, Result};
use crate::ids::{PromptId, RunId};
use crate::index::runs_repo;

const DEFAULT_LIST_LIMIT: i64 = 50;
const MAX_LIST_LIMIT: i64 = 500;

fn clamp_limit(raw: Option<i64>, default: i64) -> i64 {
    raw.unwrap_or(default).clamp(1, MAX_LIST_LIMIT)
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListRunsInput {
    pub limit: Option<i64>,
}

#[tauri::command]
pub async fn list_runs(
    input: ListRunsInput,
    services: State<'_, ManagedState>,
) -> Result<Vec<Run>> {
    let (_vault, db) = current_vault_db(&services).await?;
    let limit = clamp_limit(input.limit, DEFAULT_LIST_LIMIT);
    let rows = runs_repo::list_runs(&db, limit).await?;
    rows.into_iter().map(runs_repo::to_domain_run).collect()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetRunInput {
    pub run_id: RunId,
}

#[tauri::command]
pub async fn get_run(
    input: GetRunInput,
    services: State<'_, ManagedState>,
) -> Result<Run> {
    let (_vault, db) = current_vault_db(&services).await?;
    let row = runs_repo::get_run(&db, input.run_id.0.as_str())
        .await?
        .ok_or_else(|| {
            AppError::new(
                AppErrorKind::RunNotFound,
                format!("run {} not found", input.run_id.0),
            )
        })?;
    runs_repo::to_domain_run(row)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptRunsInput {
    pub prompt_id: PromptId,
    pub limit: Option<i64>,
}

#[tauri::command]
pub async fn get_prompt_runs(
    input: GetPromptRunsInput,
    services: State<'_, ManagedState>,
) -> Result<Vec<Run>> {
    let (_vault, db) = current_vault_db(&services).await?;
    let limit = clamp_limit(input.limit, 25);
    let rows = runs_repo::get_prompt_runs(&db, input.prompt_id.0.as_str(), limit).await?;
    rows.into_iter().map(runs_repo::to_domain_run).collect()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchTranscriptInput {
    pub run_id: RunId,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchTranscriptOutput {
    pub run_id: RunId,
    pub source: TranscriptSource,
    /// Absolute path the bytes were read from (vault file or spool).
    pub path: PathBuf,
    /// Raw transcript bytes (ANSI-preserving). Frontend renders via
    /// xterm.js or the AnsiTranscript viewer.
    pub content: String,
    pub byte_len: i64,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptSource {
    Vault,
    Spool,
}

#[tauri::command]
pub async fn fetch_transcript(
    input: FetchTranscriptInput,
    services: State<'_, ManagedState>,
) -> Result<FetchTranscriptOutput> {
    let (vault, db) = current_vault_db(&services).await?;
    let row = runs_repo::get_run(&db, input.run_id.0.as_str())
        .await?
        .ok_or_else(|| {
            AppError::new(
                AppErrorKind::RunNotFound,
                format!("run {} not found", input.run_id.0),
            )
        })?;

    // Prefer the vault-resident transcript (canonical, version-controlled);
    // fall back to the spool for runs still in-flight or for which the
    // sweep has not yet copied to the vault.
    if let Some(vault_path) = row.transcript_vault_path.as_ref() {
        let absolute = vault.absolute(vault_path).ok_or_else(|| {
            AppError::new(
                AppErrorKind::TranscriptUnavailable,
                "transcript vault path resolves outside the vault root",
            )
        })?;
        match std::fs::read_to_string(&absolute) {
            Ok(content) => {
                let byte_len = content.len() as i64;
                return Ok(FetchTranscriptOutput {
                    run_id: input.run_id,
                    source: TranscriptSource::Vault,
                    path: absolute,
                    content,
                    byte_len,
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // fall through to spool
            }
            Err(e) => return Err(AppError::from(e)),
        }
    }

    if let Some(spool) = row.transcript_spool_path.as_ref() {
        let spool_path = PathBuf::from(spool);
        match std::fs::read_to_string(&spool_path) {
            Ok(content) => {
                let byte_len = content.len() as i64;
                return Ok(FetchTranscriptOutput {
                    run_id: input.run_id,
                    source: TranscriptSource::Spool,
                    path: spool_path,
                    content,
                    byte_len,
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(AppError::from(e)),
        }
    }

    Err(AppError::new(
        AppErrorKind::TranscriptUnavailable,
        "no transcript on vault or spool for this run",
    ))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairOrphanedTranscriptsOutput {
    /// Number of vault transcript files whose run row was missing or
    /// whose `transcript_vault_path` was NULL; they were re-attached.
    pub recovered: i64,
    /// Number of `runs.transcript_vault_path` entries that point to
    /// files no longer present on disk; the column was cleared.
    pub lost: i64,
}

#[tauri::command]
pub async fn repair_orphaned_transcripts(
    services: State<'_, ManagedState>,
) -> Result<RepairOrphanedTranscriptsOutput> {
    let (vault, db) = current_vault_db(&services).await?;

    // 1. Sweep rows whose vault_path points to a missing file → clear column.
    let mut lost: i64 = 0;
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT id, transcript_vault_path FROM runs \
         WHERE transcript_vault_path IS NOT NULL",
    )
    .fetch_all(&db)
    .await
    .map_err(AppError::from)?;

    for (id, rel) in rows {
        let absolute = match vault.absolute(&rel) {
            Some(p) => p,
            None => continue,
        };
        if !absolute.exists() {
            sqlx::query("UPDATE runs SET transcript_vault_path = NULL WHERE id = ?")
                .bind(&id)
                .execute(&db)
                .await
                .map_err(AppError::from)?;
            lost += 1;
        }
    }

    // 2. (Future) walk the vault's runs/ directory looking for *.md
    //    transcripts whose run_id has no row — re-attach. For V1 the
    //    sweep is one-directional (cleanup only); a backfill scan is
    //    a follow-up V2 item per docs/V2-CANDIDATES.md.
    let recovered: i64 = 0;
    Ok(RepairOrphanedTranscriptsOutput { recovered, lost })
}
