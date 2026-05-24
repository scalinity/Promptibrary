//! `commands::extraction` — wires source detection, fetching, LLM extraction,
//! and candidate-saving into the IPC surface per spec §11.
//!
//! Each command emits `extract://progress` events at phase boundaries
//! (`detecting | fetching | normalizing | calling_model | validating |
//! complete | failed`) so the frontend state machine can advance visibly.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::app_state::ManagedState;
use crate::commands::prompts::unique_slug_public;
use crate::commands::vault::{current_vault_db, current_vault_db_inner};
use crate::domain::prompt::{LaunchDefaults, Prompt};
use crate::domain::source::Source;
use crate::error::{AppError, AppErrorKind, Result};
use crate::extraction::anthropic::AnthropicClient;
use crate::extraction::cache;
use crate::extraction::fetchers::{article, x_twitter, youtube};
use crate::extraction::normalize::normalize_for_extraction;
use crate::extraction::rate_limit::Provider;
use crate::extraction::types::{
    CandidatePrompt, ExtractionFailure, ExtractionMode, ExtractionResponse, FetchedSourceContent,
    LaunchDefaultsPatch, SourceDetection,
};

const PROGRESS_EVENT: &str = "extract://progress";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractProgressEvent {
    pub phase: ExtractPhase,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractPhase {
    Detecting,
    Fetching,
    Normalizing,
    CallingModel,
    Validating,
    Complete,
    Failed,
}

fn emit_progress(app: &AppHandle, phase: ExtractPhase, message: impl Into<String>) {
    let _ = app.emit(
        PROGRESS_EVENT,
        ExtractProgressEvent {
            phase,
            message: message.into(),
        },
    );
}

// ─── detect_source ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectSourceInput {
    pub url: String,
}

#[tauri::command]
pub async fn detect_source(input: DetectSourceInput) -> Result<SourceDetection> {
    Ok(crate::extraction::detect::detect_source(&input.url))
}

// ─── fetch_source_preview ────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchSourcePreviewInput {
    pub url: String,
    #[serde(default)]
    pub force_refresh: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum FetchSourcePreviewResult {
    Ok { content: FetchedSourceContent },
    Failed { failure: ExtractionFailure },
}

#[tauri::command]
pub async fn fetch_source_preview(
    app: AppHandle,
    input: FetchSourcePreviewInput,
    services: State<'_, ManagedState>,
) -> Result<FetchSourcePreviewResult> {
    emit_progress(&app, ExtractPhase::Detecting, "detecting source kind");
    let detected = crate::extraction::detect::detect_source(&input.url);

    let (kind, canonical_url) = match &detected {
        SourceDetection::Youtube { canonical_url, .. } => ("youtube", canonical_url.clone()),
        SourceDetection::XTwitter { canonical_url, .. } => ("x_twitter", canonical_url.clone()),
        SourceDetection::Article { canonical_url, .. } => ("article", canonical_url.clone()),
        SourceDetection::Unsupported { reason } => {
            emit_progress(&app, ExtractPhase::Failed, "unsupported source");
            return Ok(FetchSourcePreviewResult::Failed {
                failure: ExtractionFailure::UnsupportedSource { reason: *reason },
            });
        }
    };

    let (_, db) = current_vault_db(&services).await?;
    let cache_key = cache::compute_source_cache_key(kind, &canonical_url);
    if !input.force_refresh {
        if let Some(content) = cache::get_fetched_source(&db, &cache_key).await? {
            emit_progress(&app, ExtractPhase::Complete, "served from cache");
            return Ok(FetchSourcePreviewResult::Ok { content });
        }
    }

    emit_progress(&app, ExtractPhase::Fetching, format!("fetching {kind}"));
    let outcome = match &detected {
        SourceDetection::Youtube {
            canonical_url,
            video_id,
        } => {
            let _permit = services.rate_limiter.acquire(Provider::Youtube).await;
            youtube::fetch_youtube(
                canonical_url,
                video_id,
                services.yt_dlp.as_ref(),
                &services.extraction_temp_dir,
            )
            .await?
        }
        SourceDetection::XTwitter {
            canonical_url,
            post_id,
            ..
        } => {
            let _permit = services.rate_limiter.acquire(Provider::XTwitter).await;
            let mut outcome =
                x_twitter::fetch_oembed(canonical_url, post_id, &services.http).await?;
            // SCA-967 — best-effort vision augmentation. oEmbed gives us
            // the post text + author but never carries media URLs;
            // scraping OG meta tags on the canonical page is what
            // surfaces image-embedded prompts.
            if let Ok(ref mut content) = outcome {
                x_twitter::augment_with_og_images(content, &services.http).await;
            }
            outcome
        }
        SourceDetection::Article { canonical_url, .. } => {
            let _permit = services.rate_limiter.acquire(Provider::Article).await;
            article::fetch_article(&input.url, canonical_url, &services.http).await?
        }
        SourceDetection::Unsupported { .. } => unreachable!("handled above"),
    };

    match outcome {
        Ok(content) => {
            cache::put_fetched_source(&db, &cache_key, kind, &canonical_url, &content).await?;
            emit_progress(&app, ExtractPhase::Complete, "preview ready");
            Ok(FetchSourcePreviewResult::Ok { content })
        }
        Err(failure) => {
            emit_progress(&app, ExtractPhase::Failed, "fetch failed");
            Ok(FetchSourcePreviewResult::Failed { failure })
        }
    }
}

// ─── extract_prompt_candidates ───────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractCandidatesInput {
    pub content: FetchedSourceContent,
    #[serde(default)]
    pub extraction_mode: ExtractionMode,
    #[serde(default)]
    pub force_refresh: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ExtractCandidatesResult {
    Ok { response: ExtractionResponse },
    Failed { failure: ExtractionFailure },
}

#[tauri::command]
pub async fn extract_prompt_candidates(
    app: AppHandle,
    input: ExtractCandidatesInput,
    services: State<'_, ManagedState>,
) -> Result<ExtractCandidatesResult> {
    emit_progress(&app, ExtractPhase::Normalizing, "normalizing source");
    // SCA-906 — resolve extraction model + source cap from settings so
    // the user-tunable defaults flow into the LLM call. Loading local
    // settings is a small disk read; doing it inline here keeps the
    // hot path readable.
    let local = crate::commands::settings::load_persisted_local(&services.app_data_dir);
    let (model_id, cap) = match input.extraction_mode {
        crate::extraction::types::ExtractionMode::Standard => (
            local.extraction_model.as_wire().to_string(),
            local.source_cap_standard as usize,
        ),
        crate::extraction::types::ExtractionMode::Deep => (
            local.deep_extraction_model.as_wire().to_string(),
            local.source_cap_deep as usize,
        ),
    };
    let extraction_input = normalize_for_extraction(
        input.content,
        input.extraction_mode,
        cap,
        model_id,
    );

    emit_progress(&app, ExtractPhase::CallingModel, "calling anthropic");
    let (_, db) = current_vault_db(&services).await?;
    let _permit = services.rate_limiter.acquire(Provider::Anthropic).await;
    let client = AnthropicClient::new(Arc::clone(&services.anthropic_transport));
    let outcome = client
        .extract_candidates(extraction_input, input.force_refresh, Some(&db))
        .await?;

    emit_progress(&app, ExtractPhase::Validating, "validating model output");
    match outcome {
        Ok(response) => {
            emit_progress(&app, ExtractPhase::Complete, "candidates ready");
            Ok(ExtractCandidatesResult::Ok { response })
        }
        Err(failure) => {
            emit_progress(&app, ExtractPhase::Failed, "extraction failed");
            Ok(ExtractCandidatesResult::Failed { failure })
        }
    }
}

// ─── save_extracted_prompt ───────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveExtractedPromptInput {
    pub candidate: CandidatePrompt,
    /// The fetched source the candidate came from — attaches as the new
    /// prompt's `source` field so future Git history + library views
    /// know its origin.
    pub source: Source,
}

#[tauri::command]
pub async fn save_extracted_prompt(
    input: SaveExtractedPromptInput,
    services: State<'_, ManagedState>,
) -> Result<Prompt> {
    let SaveExtractedPromptInput { candidate, source } = input;

    // Defense-in-depth: re-validate the candidate. Already gated by the
    // model + Anthropic client, but the user may have edited before save.
    let mini = ExtractionResponse {
        schema_version: 1,
        candidates: vec![candidate.clone()],
    };
    let raw = serde_json::to_string(&mini)?;
    if let Err(errors) = crate::extraction::response::parse_and_validate(&raw) {
        return Err(AppError::new(
            AppErrorKind::ExtractionFailed,
            format!("candidate validation failed: {errors:?}"),
        ));
    }

    create_prompt_from_candidate(candidate, source, &services).await
}

async fn create_prompt_from_candidate(
    candidate: CandidatePrompt,
    source: Source,
    services: &ManagedState,
) -> Result<Prompt> {
    use crate::domain::prompt::PromptTelemetrySummary;
    use crate::ids::PromptId;
    use crate::index::prompts_repo::upsert_prompt as repo_upsert;
    use crate::time::now_utc;
    use crate::util::slug::slugify;
    use crate::vault::paths::prompt_path_for_slug;
    use crate::vault::writer::write_prompt;

    let _guard = services.create_prompt_lock.lock().await;
    let (vault, db) = current_vault_db_inner(services).await?;
    let now = now_utc();
    let base_slug = slugify(&candidate.title);
    let slug = unique_slug_public(&db, &base_slug).await?;
    let vault_path = prompt_path_for_slug(&slug);
    let id = PromptId(crate::ids::new_ulid());

    let mut launch_defaults = LaunchDefaults::default();
    candidate.launch_defaults_patch.apply(&mut launch_defaults);

    let mut prompt = Prompt {
        id,
        title: candidate.title,
        slug,
        summary: candidate.summary,
        body: candidate.body,
        vault_path,
        created_at: now,
        updated_at: now,
        archived_at: None,
        // Imported prompts get the `imported` tag plus whatever the
        // candidate chose. Dedup in case the model already included it.
        tags: ensure_imported_tag(candidate.tags),
        source,
        variables: candidate.variables,
        launch_defaults,
        telemetry: PromptTelemetrySummary {
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

fn ensure_imported_tag(mut tags: Vec<String>) -> Vec<String> {
    if !tags.iter().any(|t| t == "imported") {
        tags.push("imported".into());
    }
    tags
}

#[allow(dead_code, unused_imports)]
mod _silence_unused {
    use super::LaunchDefaultsPatch;
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use sqlx::sqlite::SqlitePoolOptions;

    use crate::app_state::AppServices;
    use crate::domain::source::{ManualSource, Source};
    use crate::extraction::types::{
        CandidateConfidence, CandidatePrompt, LaunchDefaultsPatch,
    };
    use crate::index::db::in_memory_connect_options;
    use crate::index::migrations::run_migrations;
    use crate::settings::secret_store::InMemorySecretStore;
    use crate::vault::paths::VaultPaths;

    /// SCA-711 regression: two concurrent `create_prompt_from_candidate`
    /// calls with the same title must each get a distinct slug, mirroring
    /// the SCA-589 invariant for `create_prompt`. Without
    /// `create_prompt_lock` (held inside the helper), both calls would
    /// observe the same slug as free and race to write the same vault
    /// path, producing duplicate-vault_path rows + corrupted YAML.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_save_with_same_title_yields_distinct_slugs() {
        let vault_dir = tempfile::tempdir().unwrap();
        let vault = VaultPaths::new(vault_dir.path().to_path_buf());
        crate::vault::repair::repair_missing_dirs(&vault).unwrap();

        // SQLite in-memory DBs are per-connection unless you opt into a
        // shared cache; keep the pool single-connection so every
        // operation sees the migrated schema. The test still exercises
        // the SCA-589 lock — concurrency is at the tokio task level, not
        // the DB connection level.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(in_memory_connect_options())
            .await
            .unwrap();
        run_migrations(&pool).await.unwrap();

        let services: Arc<AppServices> = Arc::new(AppServices::with_secret_store(Arc::new(
            InMemorySecretStore::new(),
        )));
        {
            let mut state = services.state.write().await;
            state.vault = Some(vault);
            state.db = Some(pool);
        }

        let candidate = CandidatePrompt {
            title: "Refactor streaming response handler".into(),
            summary: "Concurrent regression test".into(),
            body: "x".repeat(300),
            tags: vec!["refactor".into()],
            variables: vec![],
            launch_defaults_patch: LaunchDefaultsPatch::default(),
            confidence: CandidateConfidence::Medium,
            rationale: "concurrency regression".into(),
            source_anchors: vec![],
        };
        let source = Source::Manual(ManualSource {
            title: None,
            author: None,
            fetched_at: None,
            content_hash: None,
        });

        let s1 = services.clone();
        let c1 = candidate.clone();
        let src1 = source.clone();
        let s2 = services.clone();
        let c2 = candidate.clone();
        let src2 = source.clone();

        let (p1, p2) = tokio::join!(
            tokio::spawn(async move { create_prompt_from_candidate(c1, src1, &s1).await }),
            tokio::spawn(async move { create_prompt_from_candidate(c2, src2, &s2).await }),
        );
        let p1 = p1.unwrap().expect("first save succeeds");
        let p2 = p2.unwrap().expect("second save succeeds");

        assert_ne!(
            p1.slug, p2.slug,
            "concurrent saves with identical titles must yield distinct slugs"
        );
        assert_ne!(
            p1.vault_path, p2.vault_path,
            "concurrent saves must write to distinct vault paths"
        );
    }
}
