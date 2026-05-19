//! `commands::git` per spec §11 *Versioning*.
//!
//! Three IPC commands wired against `git::repo` + `git::history` +
//! `git::diff` + `git::revert`. The revert path validates the
//! reverted blob parses as a Promptibrary prompt before writing —
//! refuses to overwrite the live file with un-parseable data.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_state::ManagedState;
use crate::commands::vault::current_vault_db;
use crate::domain::git::{PromptDiff, PromptHistoryEntry};
use crate::error::{AppError, AppErrorKind, Result};
use crate::git::{diff, history, repo, revert};
use crate::ids::PromptId;
use crate::vault::frontmatter::parse_prompt_frontmatter;
use crate::vault::markdown::parse_markdown_document;

const HISTORY_WINDOW_DEFAULT: usize = 200;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptHistoryInput {
    pub vault_relative_path: String,
    #[serde(default)]
    pub window_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptHistoryOutput {
    pub history: Vec<PromptHistoryEntry>,
}

#[tauri::command]
pub async fn get_prompt_history(
    input: GetPromptHistoryInput,
    services: State<'_, ManagedState>,
) -> Result<GetPromptHistoryOutput> {
    let (vault, _db) = current_vault_db(&services).await?;
    let repo = repo::open_repo(&vault.vault_root)?;
    let window = input
        .window_size
        .map(|v| v as usize)
        .unwrap_or(HISTORY_WINDOW_DEFAULT);
    let entries = history::get_file_history(
        &repo,
        std::path::Path::new(&input.vault_relative_path),
        window,
    )?;
    Ok(GetPromptHistoryOutput { history: entries })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptDiffInput {
    pub prompt_id: PromptId,
    pub vault_relative_path: String,
    #[serde(default)]
    pub from_sha: Option<String>,
    pub to_sha: String,
}

#[tauri::command]
pub async fn get_prompt_diff(
    input: GetPromptDiffInput,
    services: State<'_, ManagedState>,
) -> Result<PromptDiff> {
    let (vault, _db) = current_vault_db(&services).await?;
    let repo = repo::open_repo(&vault.vault_root)?;
    diff::get_file_diff(
        &repo,
        &input.prompt_id,
        input.from_sha.as_deref(),
        &input.to_sha,
        std::path::Path::new(&input.vault_relative_path),
    )
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertPromptToCommitInput {
    pub prompt_id: PromptId,
    pub vault_relative_path: String,
    pub commit_sha: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertPromptToCommitOutput {
    pub prompt_id: PromptId,
    pub commit_sha: String,
    pub bytes_written: usize,
}

#[tauri::command]
pub async fn revert_prompt_to_commit(
    input: RevertPromptToCommitInput,
    services: State<'_, ManagedState>,
) -> Result<RevertPromptToCommitOutput> {
    let (vault, _db) = current_vault_db(&services).await?;
    let repo = repo::open_repo(&vault.vault_root)?;
    let rel = std::path::PathBuf::from(&input.vault_relative_path);
    let target = vault.vault_root.join(&rel);

    let bytes = revert::revert_file_to_commit(
        &repo,
        &input.commit_sha,
        &rel,
        &target,
        |blob_bytes| -> Result<()> {
            // Validate: must be UTF-8, parse as Markdown document
            // with frontmatter, frontmatter must parse as a
            // PromptFrontmatter. We don't require the body to be
            // non-empty — an empty-body prompt is still a valid
            // prompt.
            let text = std::str::from_utf8(blob_bytes).map_err(|_| {
                AppError::new(
                    AppErrorKind::PromptMalformed,
                    "blob is not valid UTF-8",
                )
            })?;
            let doc = parse_markdown_document(text)?;
            if doc.frontmatter_yaml.trim().is_empty() {
                return Err(AppError::new(
                    AppErrorKind::PromptMalformed,
                    "blob has no frontmatter — not a Promptibrary prompt",
                ));
            }
            parse_prompt_frontmatter(&doc.frontmatter_yaml)?;
            Ok(())
        },
    )?;

    Ok(RevertPromptToCommitOutput {
        prompt_id: input.prompt_id,
        commit_sha: input.commit_sha,
        bytes_written: bytes.len(),
    })
}
