//! `commands::prompts` per spec §11.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_state::ManagedState;
use crate::commands::vault::current_vault_db;
use crate::domain::prompt::Prompt;
use crate::domain::source::{ManualSource, Source};
use crate::domain::variable::Variable;
use crate::error::{AppError, AppErrorKind, Result};
use crate::ids::PromptId;
use crate::index::prompts_repo::{
    self, delete_prompt as repo_delete, get_prompt_index, list_prompts_for_library,
    upsert_prompt as repo_upsert, LibraryFilters, PromptIndexRow,
};
use crate::time::now_utc;
use crate::util::slug::slugify;
use crate::vault::frontmatter::parse_prompt_frontmatter;
use crate::vault::markdown::parse_markdown_document;
use crate::vault::paths::prompt_path_for_slug;
use crate::vault::writer::{archive_prompt, render_prompt_markdown, write_prompt};
use crate::util::atomic_write::atomic_write_bytes;
use std::path::PathBuf;

// SCA-966 — defense-in-depth length caps applied at the IPC boundary.
// body matches the assistant tool registry's 200KB cap (SCA-944); title
// and summary track the spec §6 candidate-schema upper bounds. Any
// caller — assistant tool, extraction's save path, manual frontend
// forms, or a future IPC — that goes through create_prompt or
// update_prompt is bounded here so the vault file never holds a body
// that wouldn't fit in memory comfortably.
const MAX_PROMPT_BODY_CHARS: usize = 200_000;
const MAX_PROMPT_TITLE_CHARS: usize = 200;
const MAX_PROMPT_SUMMARY_CHARS: usize = 280;

fn validate_body(body: &str) -> Result<()> {
    if body.len() > MAX_PROMPT_BODY_CHARS {
        return Err(AppError::new(
            AppErrorKind::Internal,
            format!(
                "prompt body too large ({} > {} chars)",
                body.len(),
                MAX_PROMPT_BODY_CHARS
            ),
        ));
    }
    Ok(())
}

fn validate_title(title: &str) -> Result<()> {
    if title.chars().count() > MAX_PROMPT_TITLE_CHARS {
        return Err(AppError::new(
            AppErrorKind::Internal,
            format!(
                "prompt title too long ({} > {} chars)",
                title.chars().count(),
                MAX_PROMPT_TITLE_CHARS
            ),
        ));
    }
    Ok(())
}

fn validate_summary(summary: &str) -> Result<()> {
    if summary.chars().count() > MAX_PROMPT_SUMMARY_CHARS {
        return Err(AppError::new(
            AppErrorKind::Internal,
            format!(
                "prompt summary too long ({} > {} chars)",
                summary.chars().count(),
                MAX_PROMPT_SUMMARY_CHARS
            ),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsInput {
    #[serde(default)]
    pub include_archived: bool,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptListItem {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub summary: String,
    pub vault_path: String,
    pub archived_at: Option<String>,
    pub tags: Vec<String>,
}

impl From<PromptIndexRow> for PromptListItem {
    fn from(r: PromptIndexRow) -> Self {
        Self {
            id: r.id,
            title: r.title,
            slug: r.slug,
            summary: r.summary,
            vault_path: r.vault_path,
            archived_at: r.archived_at,
            tags: r.tags,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptInput {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePromptInput {
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub variables: Vec<Variable>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePromptInput {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub variables: Option<Vec<Variable>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Markdown,
    Json,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPromptInput {
    pub id: String,
    pub format: ExportFormat,
    pub destination: PathBuf,
}

#[tauri::command]
pub async fn list_prompts(
    input: ListPromptsInput,
    services: State<'_, ManagedState>,
) -> Result<Vec<PromptListItem>> {
    let (_vault, db) = current_vault_db(&services).await?;
    let rows = list_prompts_for_library(
        &db,
        LibraryFilters {
            include_archived: input.include_archived,
            tag: input.tag,
            limit: input.limit,
            offset: input.offset,
        },
    )
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn get_prompt(
    input: GetPromptInput,
    services: State<'_, ManagedState>,
) -> Result<Prompt> {
    let (vault, db) = current_vault_db(&services).await?;
    let row = get_prompt_index(&db, &input.id)
        .await?
        .ok_or_else(|| AppError::new(AppErrorKind::PromptNotFound, "prompt not found"))?;
    let abs = vault
        .absolute(&row.vault_path)
        .ok_or_else(|| {
            AppError::new(
                AppErrorKind::PromptMalformed,
                "vault_path failed traversal check",
            )
            .with_detail("vault_path", row.vault_path.clone())
        })?;
    let content = std::fs::read_to_string(&abs).map_err(|e| {
        // SCA-597: don't echo the vault_path into the wire — if the path
        // was attacker-influenced (despite the C-1 guard) it would leak
        // the target. Log full detail via tracing and surface a generic
        // PromptNotFound on the wire.
        tracing::warn!(vault_path = %row.vault_path, error = ?e, "get_prompt: file read failed");
        AppError::new(AppErrorKind::PromptNotFound, "vault file missing")
    })?;
    prompt_from_file(&row.vault_path, &content)
}

#[tauri::command]
pub async fn create_prompt(
    input: CreatePromptInput,
    services: State<'_, ManagedState>,
) -> Result<Prompt> {
    // SCA-966 — defense-in-depth length caps. Catches multi-MB writes
    // from any frontend or future IPC caller before they reach the
    // atomic-write path.
    validate_title(&input.title)?;
    validate_summary(&input.summary)?;
    validate_body(&input.body)?;
    // Serialize create_prompt globally to close SCA-589: without this,
    // two concurrent IPC calls observe the same slug as free, race the
    // atomic_write rename, and produce duplicate vault_path rows.
    let _guard = services.create_prompt_lock.lock().await;
    let (vault, db) = current_vault_db(&services).await?;
    let now = now_utc();
    let base_slug = slugify(&input.title);
    let slug = unique_slug(&db, &base_slug).await?;
    let vault_path = prompt_path_for_slug(&slug);
    let id = PromptId(crate::ids::new_ulid());

    let mut prompt = Prompt {
        id,
        title: input.title,
        slug,
        summary: input.summary,
        body: input.body,
        vault_path,
        created_at: now,
        updated_at: now,
        archived_at: None,
        tags: input.tags,
        source: Source::Manual(ManualSource {
            title: None,
            author: None,
            fetched_at: None,
            content_hash: None,
        }),
        variables: input.variables,
        launch_defaults: crate::domain::prompt::LaunchDefaults::default(),
        telemetry: crate::domain::prompt::PromptTelemetrySummary {
            launch_count: 0,
            last_used_at: None,
            success_rate: None,
            avg_run_seconds: None,
            avg_token_count: None,
        },
        checksum_sha256: String::new(),
    };
    write_prompt(&vault, &mut prompt)?;
    repo_upsert(&db, &prompt).await?;
    Ok(prompt)
}

#[tauri::command]
pub async fn update_prompt(
    input: UpdatePromptInput,
    services: State<'_, ManagedState>,
) -> Result<Prompt> {
    // SCA-966 — same caps as create_prompt; applied to whichever
    // partial fields the caller supplied.
    if let Some(ref t) = input.title {
        validate_title(t)?;
    }
    if let Some(ref s) = input.summary {
        validate_summary(s)?;
    }
    if let Some(ref b) = input.body {
        validate_body(b)?;
    }
    let (vault, db) = current_vault_db(&services).await?;
    let row = get_prompt_index(&db, &input.id)
        .await?
        .ok_or_else(|| AppError::new(AppErrorKind::PromptNotFound, "prompt not found"))?;
    let abs = vault
        .absolute(&row.vault_path)
        .ok_or_else(|| {
            AppError::new(
                AppErrorKind::PromptMalformed,
                "vault_path failed traversal check",
            )
            .with_detail("vault_path", row.vault_path.clone())
        })?;
    let content = std::fs::read_to_string(&abs).map_err(AppError::from)?;
    let mut prompt = prompt_from_file(&row.vault_path, &content)?;
    if let Some(t) = input.title {
        prompt.title = t;
    }
    if let Some(s) = input.summary {
        prompt.summary = s;
    }
    if let Some(b) = input.body {
        prompt.body = b;
    }
    if let Some(t) = input.tags {
        prompt.tags = t;
    }
    if let Some(v) = input.variables {
        prompt.variables = v;
    }
    prompt.updated_at = now_utc();
    write_prompt(&vault, &mut prompt)?;
    repo_upsert(&db, &prompt).await?;
    Ok(prompt)
}

#[tauri::command]
pub async fn archive_prompt_cmd(
    input: GetPromptInput,
    services: State<'_, ManagedState>,
) -> Result<Prompt> {
    let (vault, db) = current_vault_db(&services).await?;
    let row = get_prompt_index(&db, &input.id)
        .await?
        .ok_or_else(|| AppError::new(AppErrorKind::PromptNotFound, "prompt not found"))?;
    let abs = vault
        .absolute(&row.vault_path)
        .ok_or_else(|| {
            AppError::new(
                AppErrorKind::PromptMalformed,
                "vault_path failed traversal check",
            )
            .with_detail("vault_path", row.vault_path.clone())
        })?;
    let content = std::fs::read_to_string(&abs).map_err(AppError::from)?;
    let mut prompt = prompt_from_file(&row.vault_path, &content)?;
    archive_prompt(&vault, &mut prompt)?;
    repo_upsert(&db, &prompt).await?;
    Ok(prompt)
}

#[tauri::command]
pub async fn delete_prompt(
    input: GetPromptInput,
    services: State<'_, ManagedState>,
) -> Result<()> {
    let (vault, db) = current_vault_db(&services).await?;
    let row = get_prompt_index(&db, &input.id)
        .await?
        .ok_or_else(|| AppError::new(AppErrorKind::PromptNotFound, "prompt not found"))?;
    let abs = vault
        .absolute(&row.vault_path)
        .ok_or_else(|| {
            AppError::new(
                AppErrorKind::PromptMalformed,
                "vault_path failed traversal check",
            )
            .with_detail("vault_path", row.vault_path.clone())
        })?;
    if abs.exists() {
        std::fs::remove_file(&abs).map_err(AppError::from)?;
    }
    repo_delete(&db, &input.id).await?;
    Ok(())
}

#[tauri::command]
pub async fn export_prompt(
    input: ExportPromptInput,
    services: State<'_, ManagedState>,
) -> Result<PathBuf> {
    // Reuse get_prompt for the canonical fetch — keeps the on-disk shape
    // and the exported shape identical for markdown exports.
    let prompt = get_prompt(GetPromptInput { id: input.id.clone() }, services).await?;

    let bytes = match input.format {
        ExportFormat::Markdown => render_prompt_markdown(&prompt)?.into_bytes(),
        ExportFormat::Json => serde_json::to_vec_pretty(&prompt).map_err(AppError::from)?,
    };

    // Caller picks the destination via the OS save dialog; require it to be
    // absolute so we don't silently create files under the cwd.
    if !input.destination.is_absolute() {
        return Err(AppError::new(
            AppErrorKind::Internal,
            "export destination must be an absolute path",
        ));
    }

    atomic_write_bytes(&input.destination, &bytes)?;
    Ok(input.destination)
}

// ---------- Helpers --------------------------------------------------------

/// Public re-export of `unique_slug` so other command modules
/// (`commands::extraction::save_extracted_prompt`) can share the
/// collision-avoiding slug logic without duplicating it.
pub async fn unique_slug_public(db: &sqlx::SqlitePool, base: &str) -> Result<String> {
    unique_slug(db, base).await
}

async fn unique_slug(db: &sqlx::SqlitePool, base: &str) -> Result<String> {
    if !prompts_repo::slug_in_use(db, base).await? {
        return Ok(base.to_string());
    }
    let mut n: u32 = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !prompts_repo::slug_in_use(db, &candidate).await? {
            return Ok(candidate);
        }
        n += 1;
        if n > 9999 {
            return Err(AppError::new(
                AppErrorKind::Internal,
                "could not find unique slug",
            ));
        }
    }
}

pub(crate) fn prompt_from_file(vault_path: &str, content: &str) -> Result<Prompt> {
    let doc = parse_markdown_document(content)?;
    let fm = parse_prompt_frontmatter(&doc.frontmatter_yaml)?;
    let body = doc.body;
    let checksum = format!("sha256:{}", crate::util::hash::sha256_hex(content));
    Ok(Prompt {
        id: PromptId(fm.id),
        title: fm.title,
        slug: fm.slug,
        summary: fm.summary,
        body,
        vault_path: vault_path.to_string(),
        created_at: fm.created_at,
        updated_at: fm.updated_at,
        archived_at: fm.archived_at,
        tags: fm.tags,
        source: fm.source,
        variables: fm.variables,
        launch_defaults: fm.launch_defaults.unwrap_or_default(),
        telemetry: crate::domain::prompt::PromptTelemetrySummary {
            launch_count: 0,
            last_used_at: None,
            success_rate: None,
            avg_run_seconds: None,
            avg_token_count: None,
        },
        checksum_sha256: checksum,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_body_accepts_empty_and_normal_lengths() {
        validate_body("").unwrap();
        validate_body("a normal-length prompt").unwrap();
        // Exactly at the cap is OK.
        validate_body(&"x".repeat(MAX_PROMPT_BODY_CHARS)).unwrap();
    }

    #[test]
    fn validate_body_rejects_oversized() {
        let too_big = "x".repeat(MAX_PROMPT_BODY_CHARS + 1);
        let err = validate_body(&too_big).unwrap_err();
        assert_eq!(err.kind, AppErrorKind::Internal);
        assert!(err.message.contains("body too large"));
    }

    #[test]
    fn validate_title_accepts_normal_and_caps() {
        validate_title("").unwrap();
        validate_title("My prompt").unwrap();
        validate_title(&"a".repeat(MAX_PROMPT_TITLE_CHARS)).unwrap();
    }

    #[test]
    fn validate_title_rejects_oversized() {
        let too_long = "a".repeat(MAX_PROMPT_TITLE_CHARS + 1);
        let err = validate_title(&too_long).unwrap_err();
        assert_eq!(err.kind, AppErrorKind::Internal);
        assert!(err.message.contains("title too long"));
    }

    #[test]
    fn validate_summary_uses_grapheme_count_not_byte_count() {
        // Each em-dash is 3 bytes but 1 char. Ensure the cap is by
        // chars (so multibyte content isn't unfairly truncated).
        let s = "—".repeat(MAX_PROMPT_SUMMARY_CHARS);
        validate_summary(&s).unwrap();
        let s2 = "—".repeat(MAX_PROMPT_SUMMARY_CHARS + 1);
        assert!(validate_summary(&s2).is_err());
    }
}
