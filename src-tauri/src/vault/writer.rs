//! Atomic writes to the vault: prompts, run transcripts, archived files.
//!
//! Spec §3 module contract. Every write goes through `util::atomic_write`,
//! and frontmatter is built from typed structs via `serde_yaml`. The body
//! is appended verbatim — Obsidian/Markdown tolerates a wide range of body
//! shapes, so we don't normalize.
//!
//! `archive_prompt` reuses `write_prompt` after stamping `archived_at`.
//! `write_run_transcript` writes the initial header per spec §7.

use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::domain::prompt::Prompt;
use crate::error::Result;
use crate::time::now_utc;
use crate::util::atomic_write::atomic_write_string;
use crate::vault::markdown::{serialize_markdown_document, MarkdownDocument};
use crate::vault::paths::VaultPaths;

/// Serialize a `Prompt` into the canonical Markdown + frontmatter shape that
/// `write_prompt` would write to disk. Pure function — exposed so callers
/// like `export_prompt` can produce the same bytes without touching the
/// vault filesystem.
pub fn render_prompt_markdown(prompt: &Prompt) -> Result<String> {
    let frontmatter_yaml = serde_yaml::to_string(&PromptFile {
        promptibrary_schema: 1,
        id: prompt.id.0.clone(),
        title: prompt.title.clone(),
        slug: prompt.slug.clone(),
        summary: prompt.summary.clone(),
        created_at: prompt.created_at.to_rfc3339(),
        updated_at: prompt.updated_at.to_rfc3339(),
        archived_at: prompt.archived_at.map(|d| d.to_rfc3339()),
        tags: prompt.tags.clone(),
        source: serde_yaml::to_value(&prompt.source).map_err(crate::error::AppError::from)?,
        variables: serde_yaml::to_value(&prompt.variables).map_err(crate::error::AppError::from)?,
        launch_defaults: serde_yaml::to_value(&prompt.launch_defaults)
            .map_err(crate::error::AppError::from)?,
    })
    .map_err(crate::error::AppError::from)?;

    let doc = MarkdownDocument {
        frontmatter_yaml,
        body: prompt.body.clone(),
    };
    Ok(serialize_markdown_document(&doc))
}

/// Serialize a `Prompt` into the canonical Markdown+frontmatter shape and
/// atomically write it to the vault at `prompt.vault_path`.
///
/// The on-disk frontmatter is derived from the `Prompt`. `checksum_sha256`
/// is recomputed from the rendered document so callers don't have to.
pub fn write_prompt(vault: &VaultPaths, prompt: &mut Prompt) -> Result<()> {
    let serialized = render_prompt_markdown(prompt)?;

    // Stamp the checksum into the in-memory record AFTER serialization, so
    // the on-disk content is the canonical input to the hash.
    let mut h = Sha256::new();
    h.update(serialized.as_bytes());
    prompt.checksum_sha256 = format!("sha256:{}", hex_lower(&h.finalize()));

    let abs = vault.absolute(&prompt.vault_path).ok_or_else(|| {
        crate::error::AppError::new(
            crate::error::AppErrorKind::PromptMalformed,
            "vault_path failed traversal check",
        )
        .with_detail("vault_path", prompt.vault_path.clone())
    })?;
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent).map_err(crate::error::AppError::from)?;
    }
    atomic_write_string(&abs, &serialized)
}

pub fn archive_prompt(vault: &VaultPaths, prompt: &mut Prompt) -> Result<()> {
    prompt.archived_at = Some(now_utc());
    prompt.updated_at = now_utc();
    write_prompt(vault, prompt)
}

/// Write the initial transcript header for a run. Spec §7 describes the
/// format; we keep it minimal here and let `launch::transcript_writer`
/// stream content into the body in L3.
pub fn write_run_transcript_header(
    vault: &VaultPaths,
    relative_vault_path: &str,
    header: &TranscriptHeader,
) -> Result<()> {
    let frontmatter_yaml = serde_yaml::to_string(header).map_err(crate::error::AppError::from)?;
    let doc = MarkdownDocument {
        frontmatter_yaml,
        body: format!("# Promptibrary Run {}\n\n", header.id),
    };
    let serialized = serialize_markdown_document(&doc);
    let abs = vault.absolute(relative_vault_path).ok_or_else(|| {
        crate::error::AppError::new(
            crate::error::AppErrorKind::PromptMalformed,
            "transcript vault_path failed traversal check",
        )
        .with_detail("vault_path", relative_vault_path.to_string())
    })?;
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent).map_err(crate::error::AppError::from)?;
    }
    atomic_write_string(&abs, &serialized)
}

#[derive(serde::Serialize)]
struct PromptFile {
    promptibrary_schema: u32,
    id: String,
    title: String,
    slug: String,
    summary: String,
    created_at: String,
    updated_at: String,
    archived_at: Option<String>,
    tags: Vec<String>,
    source: serde_yaml::Value,
    variables: serde_yaml::Value,
    launch_defaults: serde_yaml::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranscriptHeader {
    pub promptibrary_schema: u32,
    pub id: String,
    pub prompt_id: String,
    pub started_at: chrono::DateTime<Utc>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_prompt_sha256: Option<String>,
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::prompt::{
        ClaudeModelId, ClaudePermissionMode, LaunchDefaults, LaunchDestination, Prompt,
        PromptTelemetrySummary, VerifierMode,
    };
    use crate::domain::source::{ManualSource, Source};
    use crate::ids::PromptId;
    use crate::vault::paths::VaultPaths;

    fn sample_prompt(vault_path: &str) -> Prompt {
        Prompt {
            id: PromptId("01JZ7M1K6M8D4E9SZ7P1Q9KT4A".into()),
            title: "Hello".into(),
            slug: "hello".into(),
            summary: "a test".into(),
            body: "Body content with {{text:name}}.\n".into(),
            vault_path: vault_path.into(),
            created_at: now_utc(),
            updated_at: now_utc(),
            archived_at: None,
            tags: vec!["test".into()],
            source: Source::Manual(ManualSource {
                title: None,
                author: None,
                fetched_at: None,
                content_hash: None,
            }),
            variables: vec![],
            launch_defaults: LaunchDefaults {
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
            },
            telemetry: PromptTelemetrySummary {
                launch_count: 0,
                last_used_at: None,
                success_rate: None,
                avg_run_seconds: None,
                avg_token_count: None,
            },
            checksum_sha256: String::new(),
        }
    }

    #[test]
    fn write_prompt_creates_file_and_computes_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let v = VaultPaths::new(dir.path());
        let mut p = sample_prompt("promptibrary/prompts/hello.md");
        write_prompt(&v, &mut p).unwrap();
        assert!(v.absolute(&p.vault_path).unwrap().exists());
        assert!(p.checksum_sha256.starts_with("sha256:"));
        // Round-trip: read the file and find the frontmatter id.
        let read = std::fs::read_to_string(v.absolute(&p.vault_path).unwrap()).unwrap();
        assert!(read.contains("id: 01JZ7M1K6M8D4E9SZ7P1Q9KT4A"));
        assert!(read.contains("Body content"));
    }

    #[test]
    fn archive_prompt_stamps_archived_at() {
        let dir = tempfile::tempdir().unwrap();
        let v = VaultPaths::new(dir.path());
        let mut p = sample_prompt("promptibrary/prompts/hello.md");
        write_prompt(&v, &mut p).unwrap();
        archive_prompt(&v, &mut p).unwrap();
        assert!(p.archived_at.is_some());
        let read = std::fs::read_to_string(v.absolute(&p.vault_path).unwrap()).unwrap();
        assert!(read.contains("archived_at: "));
    }

    #[test]
    fn write_run_transcript_creates_dated_path() {
        let dir = tempfile::tempdir().unwrap();
        let v = VaultPaths::new(dir.path());
        let header = TranscriptHeader {
            promptibrary_schema: 1,
            id: "01RUN".into(),
            prompt_id: "01PROMPT".into(),
            started_at: now_utc(),
            status: "running".into(),
            resolved_prompt_sha256: Some("sha256:abc".into()),
        };
        write_run_transcript_header(&v, "promptibrary/runs/2026/05/18/01RUN.md", &header).unwrap();
        let abs = v.absolute("promptibrary/runs/2026/05/18/01RUN.md").unwrap();
        assert!(abs.exists());
    }
}
