//! FTS5 read surface — bm25-ranked text search across the `prompts_fts`
//! virtual table.
//!
//! Per spec §9 *Text search*:
//!
//! - The FTS schema and triggers live in `migrations/0002_fts.sql` and
//!   `migrations/0006_fts_tags_sync.sql`. The triggers keep `prompts_fts`
//!   in lockstep with `prompts` + `prompt_tags`, so this module only owns
//!   the **read** path.
//! - bm25 column weights are `title=5.0, summary=2.0, body=1.0, tags=4.0`
//!   per the spec's *FTS5 weights* table.
//! - Archived prompts (`prompts.archived_at IS NOT NULL`) are excluded.
//!
//! The hybrid ranking layer (`commands::search`) calls `search_fts` and
//! folds its rank into the combined score.

use sqlx::SqlitePool;

use crate::error::{AppError, Result};

/// One result row from an FTS5 search — the prompt id plus the bm25 rank
/// (lower is better) and the FTS5-generated snippet for the body column.
#[derive(Debug, Clone)]
pub struct FtsHit {
    pub prompt_id: String,
    pub bm25_rank: f64,
    pub snippet: Option<String>,
}

/// Escape a user-supplied query for the FTS5 MATCH operator. FTS5 treats
/// any token containing one of `* " ( ) :` as an operator, and bare
/// hyphens become NOT operators when leading. The simplest safe approach
/// is to split into whitespace tokens, quote each one with embedded `"`
/// doubled per the FTS5 grammar, and AND-join them. Empty input yields
/// an empty string so the caller can take the empty-query branch.
///
/// SCA-761 trade-off: blanket-quoting every token also disables the
/// FTS5 column-filter operator (`title:foo`). That's intentional for
/// V1 — the Cmd-K palette doesn't expose column-filter syntax, and
/// allowing untrusted input to pass through unquoted would re-open
/// the operator-injection surface this function exists to close. If
/// a future surface (advanced search panel?) wants column filters,
/// it needs a smarter parser that distinguishes user-typed operators
/// from accidentally-typed metachars — do NOT just relax this escape.
fn escape_fts_query(query: &str) -> String {
    let mut tokens = Vec::new();
    for raw in query.split_whitespace() {
        if raw.is_empty() {
            continue;
        }
        let escaped = raw.replace('"', "\"\"");
        tokens.push(format!("\"{escaped}\""));
    }
    tokens.join(" ")
}

/// bm25-ranked text search across non-archived prompts. Returns up to
/// `limit` hits ordered by ascending rank. Empty queries return an empty
/// vector — the caller is responsible for the recent-prompts fallback.
pub async fn search_fts(db: &SqlitePool, query: &str, limit: usize) -> Result<Vec<FtsHit>> {
    let escaped = escape_fts_query(query);
    if escaped.is_empty() {
        return Ok(Vec::new());
    }
    let limit_i64: i64 = limit.try_into().unwrap_or(i64::MAX);
    // bm25 weights are positional across ALL columns in the FTS5
    // schema, including UNINDEXED ones. The prompts_fts schema is
    //   (prompt_id UNINDEXED, title, summary, body, tags)
    // so weight slot 0 is consumed by the UNINDEXED prompt_id and slots
    // 1..=4 are the indexed columns. Per spec §9, indexed weights are
    // title=5.0, summary=2.0, body=1.0, tags=4.0; the UNINDEXED slot
    // receives an inert 0.0 placeholder.
    //
    // snippet() likewise uses positional column indices — body is
    // column 3 in the schema (0=prompt_id, 1=title, 2=summary, 3=body).
    let rows: Vec<(String, f64, Option<String>)> = sqlx::query_as(
        "SELECT prompts_fts.prompt_id,
                bm25(prompts_fts, 0.0, 5.0, 2.0, 1.0, 4.0) AS rank,
                snippet(prompts_fts, 3, '<mark>', '</mark>', '…', 16) AS snippet
           FROM prompts_fts
           JOIN prompts ON prompts.id = prompts_fts.prompt_id
          WHERE prompts_fts MATCH ?
            AND prompts.archived_at IS NULL
          ORDER BY rank ASC
          LIMIT ?",
    )
    .bind(&escaped)
    .bind(limit_i64)
    .fetch_all(db)
    .await
    .map_err(AppError::from)?;

    Ok(rows
        .into_iter()
        .map(|(prompt_id, bm25_rank, snippet)| FtsHit {
            prompt_id,
            bm25_rank,
            snippet,
        })
        .collect())
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
            .expect("connect");
        run_migrations(&pool).await.expect("migrations");
        pool
    }

    fn sample_prompt(
        id: &str,
        title: &str,
        summary: &str,
        body: &str,
        tags: &[&str],
        archived: bool,
    ) -> Prompt {
        Prompt {
            id: PromptId(id.into()),
            title: title.into(),
            slug: id.into(),
            summary: summary.into(),
            body: body.into(),
            vault_path: format!("promptibrary/prompts/{id}.md"),
            created_at: crate::time::now_utc(),
            updated_at: crate::time::now_utc(),
            archived_at: archived.then(crate::time::now_utc),
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
    async fn empty_query_returns_no_hits() {
        let db = temp_pool().await;
        let hits = search_fts(&db, "   ", 10).await.unwrap();
        assert!(hits.is_empty());
    }

    /// Acceptance criterion: a query matching a title outranks the same
    /// query when it only matches the body. bm25 with title-weight=5.0
    /// vs body-weight=1.0 should produce a lower (better) rank for the
    /// title-match row.
    #[tokio::test]
    async fn title_match_outranks_body_match() {
        let db = temp_pool().await;
        upsert_prompt(
            &db,
            &sample_prompt(
                "01title",
                "kubernetes deployment",
                "summary unrelated",
                "body unrelated",
                &[],
                false,
            ),
        )
        .await
        .unwrap();
        upsert_prompt(
            &db,
            &sample_prompt(
                "02body",
                "irrelevant title here",
                "summary unrelated",
                "this body mentions kubernetes deployment once",
                &[],
                false,
            ),
        )
        .await
        .unwrap();

        let hits = search_fts(&db, "kubernetes deployment", 10).await.unwrap();
        assert_eq!(hits.len(), 2, "both prompts should match");
        assert_eq!(hits[0].prompt_id, "01title", "title-match must come first");
        assert!(
            hits[0].bm25_rank < hits[1].bm25_rank,
            "title row {} should have lower bm25 rank than body row {}",
            hits[0].bm25_rank,
            hits[1].bm25_rank
        );
    }

    /// Tag matches must contribute via weight=4.0. A pure-tag match
    /// should beat a pure-body match for the same query.
    #[tokio::test]
    async fn tag_match_outranks_body_match() {
        let db = temp_pool().await;
        upsert_prompt(
            &db,
            &sample_prompt("03tag", "alpha", "beta", "gamma", &["kubernetes"], false),
        )
        .await
        .unwrap();
        upsert_prompt(
            &db,
            &sample_prompt(
                "04body",
                "alpha title",
                "beta summary",
                "this body mentions kubernetes once",
                &[],
                false,
            ),
        )
        .await
        .unwrap();

        let hits = search_fts(&db, "kubernetes", 10).await.unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(
            hits[0].prompt_id, "03tag",
            "tag-match must outrank body-match"
        );
    }

    #[tokio::test]
    async fn archived_prompts_are_excluded() {
        let db = temp_pool().await;
        upsert_prompt(
            &db,
            &sample_prompt("05arch", "kubernetes deployment", "", "", &[], true),
        )
        .await
        .unwrap();
        upsert_prompt(
            &db,
            &sample_prompt("06live", "kubernetes deployment", "", "", &[], false),
        )
        .await
        .unwrap();

        let hits = search_fts(&db, "kubernetes", 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].prompt_id, "06live");
    }

    #[tokio::test]
    async fn quotes_in_query_are_safely_escaped() {
        // Reaches the FTS5 layer without a parse error.
        let db = temp_pool().await;
        let _ = search_fts(&db, "kubernetes \"deployment\"", 10).await.unwrap();
    }
}
