//! `commands::search` per spec §11.
//!
//! L5 lands FTS5 text search and a basic tag-suggestion endpoint. The
//! semantic + hybrid path (`commands::search::cmdk_search`, semantic
//! toggle on `search_prompts`) lands in a later L5 ticket and remains a
//! stub here so the dispatcher does not break.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;

use crate::app_state::ManagedState;
use crate::commands::not_yet_implemented_stub;
use crate::commands::vault::current_vault_db;
use crate::error::{AppError, Result};
use crate::index::fts::search_fts;

/// Input contract for `search_prompts`. Matches the frontend
/// `SearchPromptsArgs` in `src/shared/api/ipc.ts`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPromptsInput {
    pub query: String,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Output row contract — matches `PromptSearchResult` in TS.
///
/// `score` is the model-agnostic relevance number we hand to the
/// frontend: higher = more relevant. For FTS-only results it is the
/// negation of the bm25 rank (FTS5 returns negative ranks where smaller
/// is better, so `-rank` ≥ 0 with larger = more relevant). The hybrid
/// search ticket will replace this with the full §9 combined score.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptSearchResult {
    pub prompt_id: String,
    pub title: String,
    pub snippet: Option<String>,
    pub score: f64,
}

const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 200;

fn effective_limit(input: Option<u32>) -> usize {
    input.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT) as usize
}

/// FTS5-ranked search. Empty query falls back to recent prompts.
/// Archived prompts are always excluded. Tag, when supplied, filters
/// hits to those tagged with the literal value (case-sensitive — tags
/// are normalized on write).
#[tauri::command]
pub async fn search_prompts(
    input: SearchPromptsInput,
    services: State<'_, ManagedState>,
) -> Result<Vec<PromptSearchResult>> {
    let (_vault, db) = current_vault_db(&services).await?;
    search_prompts_inner(&db, &input).await
}

async fn search_prompts_inner(
    db: &SqlitePool,
    input: &SearchPromptsInput,
) -> Result<Vec<PromptSearchResult>> {
    let limit = effective_limit(input.limit);
    let trimmed = input.query.trim();
    if trimmed.is_empty() {
        return recent_prompts(db, input.tag.as_deref(), limit).await;
    }
    fts_results(db, trimmed, input.tag.as_deref(), limit).await
}

async fn fts_results(
    db: &SqlitePool,
    query: &str,
    tag: Option<&str>,
    limit: usize,
) -> Result<Vec<PromptSearchResult>> {
    // Pull a wider window from FTS than we need so the post-tag-filter
    // result set is still likely to fill the requested limit.
    let raw_limit = if tag.is_some() { limit * 4 } else { limit };
    let hits = search_fts(db, query, raw_limit).await?;
    if hits.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<String> = hits.iter().map(|h| h.prompt_id.clone()).collect();
    let titles = fetch_titles(db, &ids, tag).await?;

    let mut out = Vec::with_capacity(hits.len());
    for hit in hits {
        let Some(title) = titles.get(&hit.prompt_id) else {
            // Either the prompt no longer exists (race against delete)
            // or the tag filter excluded it.
            continue;
        };
        out.push(PromptSearchResult {
            prompt_id: hit.prompt_id.clone(),
            title: title.clone(),
            snippet: hit.snippet,
            // FTS5 bm25 returns negative ranks (smaller = better).
            // Flip the sign so the frontend's "higher = better"
            // convention holds.
            score: -hit.bm25_rank,
        });
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

async fn recent_prompts(
    db: &SqlitePool,
    tag: Option<&str>,
    limit: usize,
) -> Result<Vec<PromptSearchResult>> {
    let limit_i64 = limit as i64;
    let rows: Vec<(String, String)> = if let Some(tag_value) = tag {
        sqlx::query_as(
            "SELECT p.id, p.title
               FROM prompts p
               JOIN prompt_tags pt ON pt.prompt_id = p.id
              WHERE p.archived_at IS NULL
                AND pt.tag_name = ?
              ORDER BY p.updated_at DESC, p.id DESC
              LIMIT ?",
        )
        .bind(tag_value)
        .bind(limit_i64)
        .fetch_all(db)
        .await
        .map_err(AppError::from)?
    } else {
        sqlx::query_as(
            "SELECT id, title
               FROM prompts
              WHERE archived_at IS NULL
              ORDER BY updated_at DESC, id DESC
              LIMIT ?",
        )
        .bind(limit_i64)
        .fetch_all(db)
        .await
        .map_err(AppError::from)?
    };

    // No FTS rank for empty-query results — assign a uniform 0.0 score
    // so the frontend sorts by the SQL ORDER BY semantics (most recent
    // first) without trying to re-sort by score.
    Ok(rows
        .into_iter()
        .map(|(prompt_id, title)| PromptSearchResult {
            prompt_id,
            title,
            snippet: None,
            score: 0.0,
        })
        .collect())
}

/// Look up titles for the given prompt ids in one round-trip, applying
/// the tag filter as a JOIN when supplied. Returns id → title map; ids
/// that have been deleted or that fail the tag filter are simply absent.
async fn fetch_titles(
    db: &SqlitePool,
    ids: &[String],
    tag: Option<&str>,
) -> Result<std::collections::HashMap<String, String>> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let placeholders = vec!["?"; ids.len()].join(",");

    let sql = if tag.is_some() {
        format!(
            "SELECT p.id, p.title
               FROM prompts p
               JOIN prompt_tags pt ON pt.prompt_id = p.id
              WHERE p.id IN ({placeholders}) AND pt.tag_name = ?"
        )
    } else {
        format!(
            "SELECT id, title FROM prompts WHERE id IN ({placeholders})"
        )
    };

    let mut q = sqlx::query_as::<_, (String, String)>(&sql);
    for id in ids {
        q = q.bind(id);
    }
    if let Some(tag_value) = tag {
        q = q.bind(tag_value);
    }
    let rows = q.fetch_all(db).await.map_err(AppError::from)?;
    Ok(rows.into_iter().collect())
}

// --- suggest_tags ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestTagsInput {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSuggestion {
    pub name: String,
    pub usage_count: i64,
}

/// Tag-autocomplete endpoint. Returns tags ranked by usage_count
/// descending. When a prefix query is supplied, hits are restricted to
/// `tag_name LIKE prefix%`. Archived prompts are excluded from counts.
#[tauri::command]
pub async fn suggest_tags(
    input: SuggestTagsInput,
    services: State<'_, ManagedState>,
) -> Result<Vec<TagSuggestion>> {
    let (_vault, db) = current_vault_db(&services).await?;
    suggest_tags_inner(&db, &input).await
}

async fn suggest_tags_inner(
    db: &SqlitePool,
    input: &SuggestTagsInput,
) -> Result<Vec<TagSuggestion>> {
    let limit_i64 = input.limit.unwrap_or(20).clamp(1, 100) as i64;
    let prefix = input
        .query
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let rows: Vec<(String, i64)> = if let Some(p) = prefix {
        sqlx::query_as(
            "SELECT pt.tag_name, COUNT(*) AS usage
               FROM prompt_tags pt
               JOIN prompts p ON p.id = pt.prompt_id
              WHERE p.archived_at IS NULL
                AND pt.tag_name LIKE ? ESCAPE '\\'
           GROUP BY pt.tag_name
           ORDER BY usage DESC, pt.tag_name ASC
              LIMIT ?",
        )
        .bind(format!("{}%", escape_like(p)))
        .bind(limit_i64)
        .fetch_all(db)
        .await
        .map_err(AppError::from)?
    } else {
        sqlx::query_as(
            "SELECT pt.tag_name, COUNT(*) AS usage
               FROM prompt_tags pt
               JOIN prompts p ON p.id = pt.prompt_id
              WHERE p.archived_at IS NULL
           GROUP BY pt.tag_name
           ORDER BY usage DESC, pt.tag_name ASC
              LIMIT ?",
        )
        .bind(limit_i64)
        .fetch_all(db)
        .await
        .map_err(AppError::from)?
    };
    Ok(rows
        .into_iter()
        .map(|(name, usage_count)| TagSuggestion { name, usage_count })
        .collect())
}

fn escape_like(input: &str) -> String {
    // SQLite LIKE wildcards: %, _, and the escape char itself. Escape
    // with backslash to match the `ESCAPE '\'` clause above.
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if matches!(ch, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

// --- cmdk_search — stub, lands in the hybrid-ranking ticket ---

#[tauri::command]
pub async fn cmdk_search(_input: serde_json::Value) -> Result<serde_json::Value> {
    not_yet_implemented_stub("commands::search::cmdk_search")
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
    use crate::index::db::in_memory_connect_options;
    use crate::index::migrations::run_migrations;
    use crate::index::prompts_repo::upsert_prompt;
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

    fn sample_prompt(id: &str, title: &str, body: &str, tags: &[&str]) -> Prompt {
        Prompt {
            id: PromptId(id.into()),
            title: title.into(),
            slug: id.into(),
            summary: String::new(),
            body: body.into(),
            vault_path: format!("promptibrary/prompts/{id}.md"),
            created_at: crate::time::now_utc(),
            updated_at: crate::time::now_utc(),
            archived_at: None,
            tags: tags.iter().map(|t| (*t).to_string()).collect(),
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
            checksum_sha256: "sha256:0".into(),
        }
    }

    #[tokio::test]
    async fn empty_query_returns_recent_prompts() {
        let db = temp_pool().await;
        upsert_prompt(&db, &sample_prompt("01a", "First prompt", "x", &[]))
            .await
            .unwrap();
        upsert_prompt(&db, &sample_prompt("02b", "Second prompt", "x", &[]))
            .await
            .unwrap();

        let res = search_prompts_inner(
            &db,
            &SearchPromptsInput {
                query: "   ".into(),
                tag: None,
                limit: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(res.len(), 2);
        assert!(res.iter().any(|r| r.prompt_id == "01a"));
        assert!(res.iter().any(|r| r.prompt_id == "02b"));
    }

    #[tokio::test]
    async fn fts_query_returns_ranked_results_with_positive_score() {
        let db = temp_pool().await;
        upsert_prompt(
            &db,
            &sample_prompt("01t", "kubernetes deployment", "body unrelated", &[]),
        )
        .await
        .unwrap();
        upsert_prompt(
            &db,
            &sample_prompt(
                "02b",
                "irrelevant title",
                "this body mentions kubernetes deployment once",
                &[],
            ),
        )
        .await
        .unwrap();

        let res = search_prompts_inner(
            &db,
            &SearchPromptsInput {
                query: "kubernetes deployment".into(),
                tag: None,
                limit: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(res.len(), 2);
        assert_eq!(res[0].prompt_id, "01t");
        // Score is -bm25_rank, and bm25_rank is negative ⇒ score is positive.
        assert!(res[0].score > 0.0, "expected positive score, got {}", res[0].score);
        assert!(res[0].score >= res[1].score);
    }

    #[tokio::test]
    async fn tag_filter_excludes_non_matching_prompts() {
        let db = temp_pool().await;
        upsert_prompt(
            &db,
            &sample_prompt("01a", "kubernetes guide", "x", &["ops"]),
        )
        .await
        .unwrap();
        upsert_prompt(
            &db,
            &sample_prompt("02a", "kubernetes overview", "x", &["docs"]),
        )
        .await
        .unwrap();

        let res = search_prompts_inner(
            &db,
            &SearchPromptsInput {
                query: "kubernetes".into(),
                tag: Some("ops".into()),
                limit: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].prompt_id, "01a");
    }

    #[tokio::test]
    async fn suggest_tags_returns_usage_counts_descending() {
        let db = temp_pool().await;
        upsert_prompt(&db, &sample_prompt("01a", "t1", "x", &["ops", "shared"]))
            .await
            .unwrap();
        upsert_prompt(&db, &sample_prompt("02b", "t2", "x", &["ops"]))
            .await
            .unwrap();
        upsert_prompt(&db, &sample_prompt("03c", "t3", "x", &["other"]))
            .await
            .unwrap();

        let res = suggest_tags_inner(
            &db,
            &SuggestTagsInput {
                query: None,
                limit: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(res[0].name, "ops");
        assert_eq!(res[0].usage_count, 2);
    }

    #[tokio::test]
    async fn suggest_tags_prefix_filter() {
        let db = temp_pool().await;
        upsert_prompt(&db, &sample_prompt("01a", "t", "x", &["ops", "docs"]))
            .await
            .unwrap();
        upsert_prompt(&db, &sample_prompt("02b", "t", "x", &["ops"]))
            .await
            .unwrap();

        let res = suggest_tags_inner(
            &db,
            &SuggestTagsInput {
                query: Some("o".into()),
                limit: None,
            },
        )
        .await
        .unwrap();
        // 'o' prefix matches only 'ops'; 'docs' starts with 'd'.
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].name, "ops");
    }
}
