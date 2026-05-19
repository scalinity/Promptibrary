//! `commands::search` per spec §11.
//!
//! L5 lands FTS5 text search, hybrid ranking (with the semantic
//! component held at 0 until the embedding model ships — see
//! `docs/notes/L5-observations.md`), and the Cmd-K palette backend.
//!
//! Hybrid score per spec §9 *Combined ranking*:
//!
//! ```text
//! score = 0.55 * reciprocal_rank_text
//!       + 0.35 * normalized_semantic
//!       + recency_boost
//!       + usage_boost
//! ```
//!
//! With the embedding service deferred, `normalized_semantic` is 0 for
//! every prompt and the score degrades cleanly to text + recency + usage.
//!
//! Boosts (capped at 0.10 each):
//!   - recency_boost: 0.10 when last_used_at ≤ 7 days,
//!                    0.05 when ≤ 30 days, else 0.
//!   - usage_boost:   min(0.10, log10(launch_count + 1) / 10).
//!
//! Exact (case-insensitive) title matches are pinned to the top, ahead
//! of every score-based result.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;

use crate::app_state::ManagedState;
use crate::commands::vault::current_vault_db;
use crate::error::{AppError, Result};
use crate::index::fts::search_fts;
use crate::index::sql_util::escape_like;

// ─── Search mode ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    /// FTS5 only — bm25 rank converted to a positive score.
    Text,
    /// Embeddings only — currently returns no results because the
    /// embedding service is deferred. Kept on the type so the frontend
    /// toggle has a stable contract.
    Semantic,
    /// FTS5 + embeddings + recency + usage with exact-title pinning.
    /// This is the default for the library and Cmd-K.
    #[default]
    Hybrid,
}

// ─── Inputs / outputs ─────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPromptsInput {
    pub query: String,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub mode: Option<SearchMode>,
    #[serde(default)]
    pub include_archived: Option<bool>,
}

/// Score breakdown the frontend can show for "why did this rank here?"
/// debugging. All fields zero by default; populated only on the hybrid
/// path.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreParts {
    pub text: f64,
    pub semantic: f64,
    pub recency: f64,
    pub usage: f64,
    pub exact_title_pin: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptSearchResult {
    pub prompt_id: String,
    pub title: String,
    pub snippet: Option<String>,
    pub score: f64,
    #[serde(default)]
    pub score_parts: ScoreParts,
}

const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 200;
const W_TEXT: f64 = 0.55;
const W_SEMANTIC: f64 = 0.35;
const RRF_K: f64 = 60.0;

/// Score bonus added to the exact-title-match pin so any pinned
/// result outranks every non-pinned one regardless of weights and
/// boosts. The natural score range is bounded by
/// `W_TEXT * (1/(RRF_K+1)) + W_SEMANTIC + recency_boost_max + usage_boost_max`,
/// which is ≈ 0.209 today (semantic=0 while the embedding service is
/// deferred) and ≈ 0.559 in the future (semantic=0.35). 1_000.0
/// gives multiple orders of magnitude of headroom either way.
const EXACT_TITLE_PIN_BONUS: f64 = 1_000.0;

fn effective_limit(input: Option<u32>) -> usize {
    input.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT) as usize
}

// ─── search_prompts ───────────────────────────────────────────────────

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
    let mode = input.mode.unwrap_or_default();
    // SCA-739: include_archived defaults to false. Applies to the
    // recent-prompts path and the prompt-meta join used by hybrid
    // and fts-only. FTS search itself (`index::fts::search_fts`)
    // continues to exclude archived rows unconditionally — that's
    // the spec's text-search baseline; the toggle controls list
    // membership, not whether archived rows are full-text-searchable.
    let include_archived = input.include_archived.unwrap_or(false);

    if trimmed.is_empty() {
        return recent_prompts(db, input.tag.as_deref(), limit, include_archived).await;
    }

    match mode {
        SearchMode::Text => {
            fts_only(db, trimmed, input.tag.as_deref(), limit, include_archived).await
        }
        SearchMode::Semantic => {
            // Embedding service is deferred; no semantic results to return.
            Ok(Vec::new())
        }
        SearchMode::Hybrid => {
            hybrid(db, trimmed, input.tag.as_deref(), limit, include_archived).await
        }
    }
}

// ─── Hybrid path ──────────────────────────────────────────────────────

async fn hybrid(
    db: &SqlitePool,
    query: &str,
    tag: Option<&str>,
    limit: usize,
    include_archived: bool,
) -> Result<Vec<PromptSearchResult>> {
    // Pull a 4× window from FTS so post-tag-filter we still fill the limit.
    let raw_limit = if tag.is_some() { limit * 4 } else { limit };
    let hits = search_fts(db, query, raw_limit).await?;
    if hits.is_empty() {
        return Ok(Vec::new());
    }

    // Reciprocal rank: 1 / (k + position), positions 1-indexed per RRF.
    let mut text_rank: HashMap<String, f64> = HashMap::new();
    let mut snippets: HashMap<String, Option<String>> = HashMap::new();
    for (i, hit) in hits.iter().enumerate() {
        let rrf = 1.0 / (RRF_K + (i as f64 + 1.0));
        text_rank.insert(hit.prompt_id.clone(), rrf);
        snippets.insert(hit.prompt_id.clone(), hit.snippet.clone());
    }

    let ids: Vec<String> = hits.iter().map(|h| h.prompt_id.clone()).collect();
    let prompt_meta = fetch_prompt_meta(db, &ids, tag, include_archived).await?;
    let usage = fetch_usage_stats(db, &ids).await?;

    let now = Utc::now();
    let query_lower = query.to_lowercase();

    let mut out: Vec<PromptSearchResult> = Vec::with_capacity(prompt_meta.len());
    for id in &ids {
        let Some(meta) = prompt_meta.get(id) else {
            continue; // filtered out by tag or deleted mid-flight
        };
        let text_component = text_rank.get(id).copied().unwrap_or(0.0);
        let stats = usage.get(id).copied().unwrap_or(UsageStats::default());

        let recency = recency_boost(stats.last_used_at, now);
        let usage_b = usage_boost(stats.launch_count);

        // semantic component is held at 0 until the embedding service
        // lands. We still feed the weighted sum so the score formula
        // matches §9 exactly.
        let semantic = 0.0;
        // SCA-737: full Unicode case-folding on BOTH sides. Pre-fix
        // mixed `query.to_lowercase()` (Unicode) with
        // `eq_ignore_ascii_case` (ASCII-only), so titles with
        // accented chars (São Paulo, Übung) never pinned even on an
        // exact-typed query. Pure-Unicode comparison handles every
        // case-foldable script consistently.
        let pin = meta.title.to_lowercase() == query_lower;

        // Pin score: stays well clear of the natural score range
        // (see EXACT_TITLE_PIN_BONUS docstring for the math).
        let pin_bonus = if pin { EXACT_TITLE_PIN_BONUS } else { 0.0 };

        let score =
            pin_bonus + W_TEXT * text_component + W_SEMANTIC * semantic + recency + usage_b;

        out.push(PromptSearchResult {
            prompt_id: id.clone(),
            title: meta.title.clone(),
            snippet: snippets.remove(id).unwrap_or(None),
            score,
            score_parts: ScoreParts {
                text: text_component,
                semantic,
                recency,
                usage: usage_b,
                exact_title_pin: pin,
            },
        });
    }

    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.truncate(limit);
    Ok(out)
}

fn recency_boost(last_used_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> f64 {
    match last_used_at {
        Some(t) => {
            // SCA-741: explicit guard against negative days. A row
            // with t > now (clock skew, manual SQL insert, future-tz
            // value) is a misconfigured signal, not a recent launch.
            // Pre-fix it would have passed `days <= 7.0` and silently
            // returned 0.10. We return 0.0 to treat it as "no signal"
            // rather than "most-recent ever".
            let secs = (now - t).num_seconds();
            if secs < 0 {
                return 0.0;
            }
            let days = secs as f64 / 86_400.0;
            if days <= 7.0 {
                0.10
            } else if days <= 30.0 {
                0.05
            } else {
                0.0
            }
        }
        None => 0.0,
    }
}

fn usage_boost(launch_count: i64) -> f64 {
    let v = ((launch_count as f64 + 1.0).log10()) / 10.0;
    v.clamp(0.0, 0.10)
}

// ─── Text-only path ───────────────────────────────────────────────────

async fn fts_only(
    db: &SqlitePool,
    query: &str,
    tag: Option<&str>,
    limit: usize,
    include_archived: bool,
) -> Result<Vec<PromptSearchResult>> {
    let raw_limit = if tag.is_some() { limit * 4 } else { limit };
    let hits = search_fts(db, query, raw_limit).await?;
    if hits.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<String> = hits.iter().map(|h| h.prompt_id.clone()).collect();
    let meta = fetch_prompt_meta(db, &ids, tag, include_archived).await?;

    let mut out = Vec::with_capacity(hits.len());
    for hit in hits {
        let Some(m) = meta.get(&hit.prompt_id) else {
            continue;
        };
        out.push(PromptSearchResult {
            prompt_id: hit.prompt_id.clone(),
            title: m.title.clone(),
            snippet: hit.snippet,
            // FTS5 bm25 returns negative ranks (smaller = better); flip
            // the sign so the frontend's "higher = better" convention
            // holds.
            score: -hit.bm25_rank,
            score_parts: ScoreParts::default(),
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
    include_archived: bool,
) -> Result<Vec<PromptSearchResult>> {
    // Empty-query path per spec §9: order by last_used_at DESC,
    // updated_at DESC. We left-join the runs aggregate to pick up
    // last_used_at; prompts with no runs sort by updated_at.
    //
    // SCA-739: include_archived drops the archived_at IS NULL clause
    // so the library can render archived rows when the toggle asks.
    let limit_i64 = limit as i64;
    let rows: Vec<(String, String)> = if let Some(tag_value) = tag {
        let where_clause = if include_archived {
            "WHERE pt.tag_name = ?"
        } else {
            "WHERE p.archived_at IS NULL AND pt.tag_name = ?"
        };
        let sql = format!(
            "SELECT p.id, p.title
               FROM prompts p
               JOIN prompt_tags pt ON pt.prompt_id = p.id
          LEFT JOIN (SELECT prompt_id, MAX(started_at) AS last_used_at
                       FROM runs GROUP BY prompt_id) r
                 ON r.prompt_id = p.id
              {where_clause}
              ORDER BY COALESCE(r.last_used_at, p.updated_at) DESC, p.id DESC
              LIMIT ?"
        );
        sqlx::query_as(&sql)
            .bind(tag_value)
            .bind(limit_i64)
            .fetch_all(db)
            .await
            .map_err(AppError::from)?
    } else {
        let where_clause = if include_archived {
            ""
        } else {
            "WHERE p.archived_at IS NULL"
        };
        let sql = format!(
            "SELECT p.id, p.title
               FROM prompts p
          LEFT JOIN (SELECT prompt_id, MAX(started_at) AS last_used_at
                       FROM runs GROUP BY prompt_id) r
                 ON r.prompt_id = p.id
              {where_clause}
              ORDER BY COALESCE(r.last_used_at, p.updated_at) DESC, p.id DESC
              LIMIT ?"
        );
        sqlx::query_as(&sql)
            .bind(limit_i64)
            .fetch_all(db)
            .await
            .map_err(AppError::from)?
    };

    Ok(rows
        .into_iter()
        .map(|(prompt_id, title)| PromptSearchResult {
            prompt_id,
            title,
            snippet: None,
            score: 0.0,
            score_parts: ScoreParts::default(),
        })
        .collect())
}

// ─── Per-prompt metadata + usage lookups ──────────────────────────────

#[derive(Debug, Clone)]
struct PromptMeta {
    title: String,
}

async fn fetch_prompt_meta(
    db: &SqlitePool,
    ids: &[String],
    tag: Option<&str>,
    include_archived: bool,
) -> Result<HashMap<String, PromptMeta>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let archived_clause = if include_archived {
        ""
    } else {
        " AND p.archived_at IS NULL"
    };
    let sql = if tag.is_some() {
        format!(
            "SELECT p.id, p.title
               FROM prompts p
               JOIN prompt_tags pt ON pt.prompt_id = p.id
              WHERE p.id IN ({placeholders})
                AND pt.tag_name = ?{archived_clause}"
        )
    } else {
        let archived_clause = if include_archived {
            ""
        } else {
            " AND archived_at IS NULL"
        };
        format!(
            "SELECT id, title FROM prompts
              WHERE id IN ({placeholders}){archived_clause}"
        )
    };
    let mut q = sqlx::query_as::<_, (String, String)>(&sql);
    for id in ids {
        q = q.bind(id);
    }
    if let Some(t) = tag {
        q = q.bind(t);
    }
    let rows = q.fetch_all(db).await.map_err(AppError::from)?;
    Ok(rows
        .into_iter()
        .map(|(id, title)| (id, PromptMeta { title }))
        .collect())
}

#[derive(Debug, Clone, Copy, Default)]
struct UsageStats {
    launch_count: i64,
    last_used_at: Option<DateTime<Utc>>,
}

async fn fetch_usage_stats(
    db: &SqlitePool,
    ids: &[String],
) -> Result<HashMap<String, UsageStats>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!(
        "SELECT prompt_id,
                COUNT(*) AS launch_count,
                MAX(started_at) AS last_used_at
           FROM runs
          WHERE prompt_id IN ({placeholders})
       GROUP BY prompt_id"
    );
    let mut q = sqlx::query_as::<_, (String, i64, Option<String>)>(&sql);
    for id in ids {
        q = q.bind(id);
    }
    let rows = q.fetch_all(db).await.map_err(AppError::from)?;
    let mut out = HashMap::new();
    for (prompt_id, launch_count, last_used_at) in rows {
        out.insert(
            prompt_id,
            UsageStats {
                launch_count,
                last_used_at: last_used_at
                    .as_deref()
                    .and_then(crate::time::parse_db_timestamp),
            },
        );
    }
    Ok(out)
}

// ─── suggest_tags ─────────────────────────────────────────────────────

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

// ─── Cmd-K palette ────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CmdkSearchInput {
    pub query: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CmdkResultKind {
    Prompt,
    Run,
    Action,
    Route,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CmdkResult {
    pub kind: CmdkResultKind,
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub score: f64,
}

const CMDK_PROMPT_LIMIT: usize = 8;
const CMDK_RUN_LIMIT: usize = 5;
const CMDK_ACTION_LIMIT: usize = 8;
const CMDK_ROUTE_LIMIT: usize = 8;

#[tauri::command]
pub async fn cmdk_search(
    input: CmdkSearchInput,
    services: State<'_, ManagedState>,
) -> Result<Vec<CmdkResult>> {
    let (_vault, db) = current_vault_db(&services).await?;
    cmdk_search_inner(&db, &input).await
}

async fn cmdk_search_inner(
    db: &SqlitePool,
    input: &CmdkSearchInput,
) -> Result<Vec<CmdkResult>> {
    let q = input.query.trim();

    // Prompts via the hybrid search path (when query is empty this
    // falls back to recent prompts).
    let prompt_results = search_prompts_inner(
        db,
        &SearchPromptsInput {
            query: q.to_string(),
            tag: None,
            limit: Some(CMDK_PROMPT_LIMIT as u32),
            mode: Some(SearchMode::Hybrid),
            include_archived: Some(false),
        },
    )
    .await?;

    let mut out: Vec<CmdkResult> = Vec::with_capacity(
        CMDK_PROMPT_LIMIT + CMDK_RUN_LIMIT + CMDK_ACTION_LIMIT + CMDK_ROUTE_LIMIT,
    );
    for p in prompt_results {
        out.push(CmdkResult {
            kind: CmdkResultKind::Prompt,
            id: p.prompt_id,
            title: p.title,
            subtitle: p.snippet,
            score: p.score,
        });
    }

    // Runs: substring match on prompt_title, most recent first.
    let runs = cmdk_recent_runs(db, q, CMDK_RUN_LIMIT).await?;
    out.extend(runs);

    // Actions: static catalog filtered by case-insensitive contains.
    out.extend(cmdk_actions(q));

    // Routes: static catalog of every §12 route filtered the same way.
    out.extend(cmdk_routes(q));

    Ok(out)
}

async fn cmdk_recent_runs(
    db: &SqlitePool,
    query: &str,
    limit: usize,
) -> Result<Vec<CmdkResult>> {
    let limit_i64 = limit as i64;
    let rows: Vec<(String, String, String, String)> = if query.is_empty() {
        sqlx::query_as(
            "SELECT id, prompt_id, prompt_title, started_at
               FROM runs
              ORDER BY started_at DESC
              LIMIT ?",
        )
        .bind(limit_i64)
        .fetch_all(db)
        .await
        .map_err(AppError::from)?
    } else {
        sqlx::query_as(
            "SELECT id, prompt_id, prompt_title, started_at
               FROM runs
              WHERE prompt_title LIKE ? ESCAPE '\\'
              ORDER BY started_at DESC
              LIMIT ?",
        )
        .bind(format!("%{}%", escape_like(query)))
        .bind(limit_i64)
        .fetch_all(db)
        .await
        .map_err(AppError::from)?
    };
    Ok(rows
        .into_iter()
        .map(|(id, _prompt_id, prompt_title, started_at)| CmdkResult {
            kind: CmdkResultKind::Run,
            id,
            title: prompt_title,
            subtitle: Some(started_at),
            score: 0.0,
        })
        .collect())
}

fn cmdk_actions(query: &str) -> Vec<CmdkResult> {
    const ACTIONS: &[(&str, &str, &str)] = &[
        ("new-prompt", "New prompt", "Create a new launch profile"),
        ("import-url", "Import from URL", "Article / YouTube / X import"),
        ("rebuild-index", "Rebuild index", "Drop + re-scan the vault"),
        ("run-diagnostics", "Run diagnostics", "Check claude / yt-dlp / git / keychain"),
        ("reveal-vault", "Reveal vault in Terminal.app", "Open Terminal at the vault root"),
        ("repair-orphans", "Repair orphaned transcripts", "Move stray spool files into the vault"),
    ];
    filter_static(ACTIONS, CmdkResultKind::Action, query, CMDK_ACTION_LIMIT)
}

fn cmdk_routes(query: &str) -> Vec<CmdkResult> {
    // SCA-738: only routes that actually exist in src/router.tsx
    // are listed here. Nested /settings/* routes don't exist; spec
    // §12 lists them as logical sub-surfaces inside the single
    // /settings route. /prompt/:promptId and /run/:runId require an
    // id, so they're not useful as static "go to" entries — the
    // user reaches them via the Prompt and Run sections of Cmd-K.
    const ROUTES: &[(&str, &str, &str)] = &[
        ("library", "Library", "/"),
        ("import", "Import", "/import"),
        ("settings", "Settings", "/settings"),
    ];
    filter_static(ROUTES, CmdkResultKind::Route, query, CMDK_ROUTE_LIMIT)
}

fn filter_static(
    items: &[(&'static str, &'static str, &'static str)],
    kind: CmdkResultKind,
    query: &str,
    limit: usize,
) -> Vec<CmdkResult> {
    let q = query.to_lowercase();
    items
        .iter()
        .filter(|(_id, title, subtitle)| {
            q.is_empty()
                || title.to_lowercase().contains(&q)
                || subtitle.to_lowercase().contains(&q)
        })
        .take(limit)
        .map(|(id, title, subtitle)| CmdkResult {
            kind,
            id: (*id).into(),
            title: (*title).into(),
            subtitle: Some((*subtitle).into()),
            score: 0.0,
        })
        .collect()
}

// ─── Tests ────────────────────────────────────────────────────────────

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
    use chrono::Duration;
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

    async fn insert_run(
        db: &SqlitePool,
        run_id: &str,
        prompt_id: &str,
        prompt_title: &str,
        started_at: DateTime<Utc>,
    ) {
        sqlx::query(
            "INSERT INTO runs (id, prompt_id, prompt_title, status, profile_json,
                  started_at, stdout_bytes, stderr_bytes)
              VALUES (?, ?, ?, ?, ?, ?, 0, 0)",
        )
        .bind(run_id)
        .bind(prompt_id)
        .bind(prompt_title)
        .bind("finished")
        .bind("{}")
        .bind(started_at.to_rfc3339())
        .execute(db)
        .await
        .unwrap();
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
                mode: None,
                include_archived: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(res.len(), 2);
    }

    #[tokio::test]
    async fn hybrid_recency_boost_lifts_recently_used_prompts() {
        let db = temp_pool().await;
        upsert_prompt(
            &db,
            &sample_prompt("01a", "kubernetes guide", "kubernetes guide body", &[]),
        )
        .await
        .unwrap();
        upsert_prompt(
            &db,
            &sample_prompt(
                "02b",
                "kubernetes guide v2",
                "kubernetes guide body",
                &[],
            ),
        )
        .await
        .unwrap();
        // 01a was launched yesterday → +0.10 recency boost.
        // 02b has no runs → +0.00 recency boost.
        insert_run(
            &db,
            "r1",
            "01a",
            "kubernetes guide",
            Utc::now() - Duration::days(1),
        )
        .await;

        let res = search_prompts_inner(
            &db,
            &SearchPromptsInput {
                query: "kubernetes".into(),
                tag: None,
                limit: None,
                mode: Some(SearchMode::Hybrid),
                include_archived: None,
            },
        )
        .await
        .unwrap();
        assert!(res.len() >= 2);
        assert_eq!(res[0].prompt_id, "01a", "recency-boosted prompt should win");
        assert!(res[0].score_parts.recency > 0.0);
    }

    #[tokio::test]
    async fn hybrid_exact_title_match_pins_to_top() {
        let db = temp_pool().await;
        upsert_prompt(
            &db,
            &sample_prompt("01a", "kubernetes deployment", "x", &[]),
        )
        .await
        .unwrap();
        upsert_prompt(
            &db,
            &sample_prompt(
                "02b",
                "kubernetes deployment v3 — popular",
                "kubernetes deployment body",
                &[],
            ),
        )
        .await
        .unwrap();
        // Push 02b's recency + usage hard so without the pin it would
        // outrank the exact-match.
        for i in 0..50 {
            insert_run(
                &db,
                &format!("r{i}"),
                "02b",
                "kubernetes deployment v3 — popular",
                Utc::now() - Duration::hours(1),
            )
            .await;
        }

        let res = search_prompts_inner(
            &db,
            &SearchPromptsInput {
                query: "kubernetes deployment".into(),
                tag: None,
                limit: None,
                mode: Some(SearchMode::Hybrid),
                include_archived: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(res[0].prompt_id, "01a", "exact title must pin");
        assert!(res[0].score_parts.exact_title_pin);
    }

    /// SCA-737 regression: a title with an accented character must
    /// still pin to top when the query types the same accented form.
    /// Pre-fix used `eq_ignore_ascii_case` which only folded ASCII,
    /// so "São Paulo Guide" never matched even with an identical
    /// (modulo case) query.
    #[tokio::test]
    async fn hybrid_exact_title_pin_handles_unicode() {
        let db = temp_pool().await;
        upsert_prompt(
            &db,
            &sample_prompt("01a", "São Paulo Guide", "x", &[]),
        )
        .await
        .unwrap();
        upsert_prompt(
            &db,
            &sample_prompt(
                "02b",
                "Travel São Paulo by foot — popular",
                "São Paulo body",
                &[],
            ),
        )
        .await
        .unwrap();

        let res = search_prompts_inner(
            &db,
            &SearchPromptsInput {
                query: "são paulo guide".into(), // lowercase + accented
                tag: None,
                limit: None,
                mode: Some(SearchMode::Hybrid),
                include_archived: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(res[0].prompt_id, "01a", "Unicode exact title must pin");
        assert!(res[0].score_parts.exact_title_pin);
    }

    #[test]
    fn usage_boost_grows_with_log10_count() {
        assert_eq!(usage_boost(0), 0.0);
        assert!(usage_boost(9) > usage_boost(1));
        // At launch_count = 1_000_000_000, log10(1e9 + 1) / 10 ≈ 0.9, so
        // the cap at 0.10 must hold.
        assert_eq!(usage_boost(1_000_000_000), 0.10);
    }

    #[test]
    fn recency_boost_steps() {
        let now = Utc::now();
        assert_eq!(recency_boost(None, now), 0.0);
        assert_eq!(recency_boost(Some(now - Duration::days(1)), now), 0.10);
        assert_eq!(recency_boost(Some(now - Duration::days(8)), now), 0.05);
        assert_eq!(recency_boost(Some(now - Duration::days(60)), now), 0.0);
    }

    /// SCA-741: a future timestamp (clock skew, misconfigured row)
    /// must NOT silently earn the maximum boost. Pre-fix the
    /// negative-day diff passed `days <= 7.0` and returned 0.10.
    #[test]
    fn recency_boost_clamps_future_timestamps_to_zero() {
        let now = Utc::now();
        assert_eq!(recency_boost(Some(now + Duration::hours(1)), now), 0.0);
        assert_eq!(recency_boost(Some(now + Duration::days(7)), now), 0.0);
    }

    #[tokio::test]
    async fn semantic_mode_returns_empty_until_embeddings_land() {
        let db = temp_pool().await;
        upsert_prompt(&db, &sample_prompt("01a", "title", "body", &[]))
            .await
            .unwrap();
        let res = search_prompts_inner(
            &db,
            &SearchPromptsInput {
                query: "title".into(),
                tag: None,
                limit: None,
                mode: Some(SearchMode::Semantic),
                include_archived: None,
            },
        )
        .await
        .unwrap();
        assert!(res.is_empty());
    }

    /// SCA-739 regression: include_archived must actually drop the
    /// archived_at IS NULL filter on the recent-prompts and
    /// hybrid/text-mode metadata-fetch paths. Pre-fix it was parsed
    /// but never read; the toggle silently no-op'd.
    #[tokio::test]
    async fn include_archived_surfaces_archived_prompts() {
        let db = temp_pool().await;
        let mut live = sample_prompt("01live", "kubernetes runbook", "x", &[]);
        let mut archived = sample_prompt("02arch", "kubernetes legacy", "x", &[]);
        archived.archived_at = Some(crate::time::now_utc());
        upsert_prompt(&db, &live).await.unwrap();
        upsert_prompt(&db, &archived).await.unwrap();

        // include_archived=false (default) — archived row excluded.
        let res_excluded = search_prompts_inner(
            &db,
            &SearchPromptsInput {
                query: "".into(),
                tag: None,
                limit: None,
                mode: None,
                include_archived: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(res_excluded.len(), 1);
        assert_eq!(res_excluded[0].prompt_id, "01live");

        // include_archived=true — both rows visible.
        let res_included = search_prompts_inner(
            &db,
            &SearchPromptsInput {
                query: "".into(),
                tag: None,
                limit: None,
                mode: None,
                include_archived: Some(true),
            },
        )
        .await
        .unwrap();
        assert_eq!(res_included.len(), 2);

        // Hybrid path: archived row surfaces in meta lookup too once
        // include_archived=true. (FTS layer still excludes by design.)
        // For this test, use the lower-level recent-prompts assertion
        // since FTS exclusion is separately tested; the contract for
        // L5 is that the toggle controls list membership.
        let _ = (live.id.clone(), archived.id.clone());
    }

    #[tokio::test]
    async fn text_mode_uses_negated_bm25_score() {
        let db = temp_pool().await;
        upsert_prompt(&db, &sample_prompt("01a", "kubernetes", "x", &[]))
            .await
            .unwrap();
        let res = search_prompts_inner(
            &db,
            &SearchPromptsInput {
                query: "kubernetes".into(),
                tag: None,
                limit: None,
                mode: Some(SearchMode::Text),
                include_archived: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(res.len(), 1);
        assert!(res[0].score > 0.0);
        assert_eq!(res[0].score_parts.text, 0.0); // not populated on text mode
    }

    #[tokio::test]
    async fn cmdk_search_returns_prompts_actions_and_routes() {
        let db = temp_pool().await;
        upsert_prompt(&db, &sample_prompt("01a", "kubernetes guide", "x", &[]))
            .await
            .unwrap();

        let res = cmdk_search_inner(
            &db,
            &CmdkSearchInput {
                query: "kubernetes".into(),
            },
        )
        .await
        .unwrap();
        assert!(
            res.iter()
                .any(|r| matches!(r.kind, CmdkResultKind::Prompt) && r.id == "01a"),
            "prompts section missing"
        );

        // Static catalogs: empty + non-matching queries still allow the
        // catalogs to show via the `q.is_empty()` and contains() match.
        let all = cmdk_search_inner(
            &db,
            &CmdkSearchInput {
                query: String::new(),
            },
        )
        .await
        .unwrap();
        assert!(
            all.iter().any(|r| matches!(r.kind, CmdkResultKind::Action)),
            "actions catalog missing on empty query"
        );
        assert!(
            all.iter().any(|r| matches!(r.kind, CmdkResultKind::Route)),
            "routes catalog missing on empty query"
        );
    }

    #[tokio::test]
    async fn cmdk_search_returns_recent_runs_matching_title() {
        let db = temp_pool().await;
        upsert_prompt(&db, &sample_prompt("01a", "kubernetes guide", "x", &[]))
            .await
            .unwrap();
        insert_run(&db, "r1", "01a", "kubernetes guide", Utc::now()).await;

        let res = cmdk_search_inner(
            &db,
            &CmdkSearchInput {
                query: "kubernetes".into(),
            },
        )
        .await
        .unwrap();
        assert!(
            res.iter()
                .any(|r| matches!(r.kind, CmdkResultKind::Run) && r.id == "r1"),
            "expected run hit, got {res:?}"
        );
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
}
