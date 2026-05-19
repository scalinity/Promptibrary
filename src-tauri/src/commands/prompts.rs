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
use crate::vault::writer::{archive_prompt, write_prompt};

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
        launch_defaults: default_launch_defaults(),
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
    input: GetPromptInput,
    services: State<'_, ManagedState>,
) -> Result<String> {
    let prompt = get_prompt(input, services).await?;
    serde_json::to_string_pretty(&prompt).map_err(AppError::from)
}

// ---------- Helpers --------------------------------------------------------

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

fn prompt_from_file(vault_path: &str, content: &str) -> Result<Prompt> {
    let doc = parse_markdown_document(content)?;
    let fm = parse_prompt_frontmatter(&doc.frontmatter_yaml)?;
    let body = doc.body;
    let checksum = format!("sha256:{}", hex_sha256(content));
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
        launch_defaults: fm.launch_defaults.unwrap_or_else(default_launch_defaults),
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

fn hex_sha256(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    let mut out = String::with_capacity(64);
    for b in h.finalize() {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn default_launch_defaults() -> crate::domain::prompt::LaunchDefaults {
    use crate::domain::prompt::*;
    LaunchDefaults {
        destination: LaunchDestination::ClaudeCodeCli,
        model: ClaudeModelId::ClaudeSonnet46,
        verifier_mode: VerifierMode::Off,
        working_directory: None,
        additional_directories: vec![],
        permission_mode: ClaudePermissionMode::Default,
        allowed_tools: vec![],
        disallowed_tools: vec![],
        mcp_config_paths: vec![],
        strict_mcp_config: false,
        append_system_prompt: None,
        max_turns: None,
    }
}
