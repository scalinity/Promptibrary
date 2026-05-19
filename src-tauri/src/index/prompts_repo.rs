//! Prompt index row CRUD.
//!
//! Spec §3 module contract: `upsert_prompt`, `delete_prompt`,
//! `get_prompt_index`, `list_prompts_for_library`. The schema is in
//! `migrations/0001_init.sql`; embedded JSON columns hold typed sub-trees.
//!
//! Tag handling: we keep `prompt_tags` in lockstep with the canonical
//! `prompt.tags` list — on upsert we delete and re-insert the tag rows
//! inside the same transaction. The FTS triggers (migration 0006) re-derive
//! the FTS `tags` column when those rows change.

use sqlx::SqlitePool;

use crate::domain::prompt::Prompt;
use crate::error::{AppError, Result};

/// Insert-or-replace the canonical row for this prompt. Tags are rewritten
/// in the same transaction.
pub async fn upsert_prompt(db: &SqlitePool, prompt: &Prompt) -> Result<()> {
    let source_kind = source_kind_for(&prompt.source);
    let source_json = serde_json::to_string(&prompt.source).map_err(AppError::from)?;
    let variables_json = serde_json::to_string(&prompt.variables).map_err(AppError::from)?;
    let launch_defaults_json =
        serde_json::to_string(&prompt.launch_defaults).map_err(AppError::from)?;

    let mut tx = db.begin().await.map_err(AppError::from)?;
    sqlx::query(
        "INSERT INTO prompts (id, title, slug, summary, body, vault_path,
            created_at, updated_at, archived_at, source_kind, source_json,
            variables_json, launch_defaults_json, checksum_sha256)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            slug = excluded.slug,
            summary = excluded.summary,
            body = excluded.body,
            vault_path = excluded.vault_path,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at,
            archived_at = excluded.archived_at,
            source_kind = excluded.source_kind,
            source_json = excluded.source_json,
            variables_json = excluded.variables_json,
            launch_defaults_json = excluded.launch_defaults_json,
            checksum_sha256 = excluded.checksum_sha256",
    )
    .bind(&prompt.id.0)
    .bind(&prompt.title)
    .bind(&prompt.slug)
    .bind(&prompt.summary)
    .bind(&prompt.body)
    .bind(&prompt.vault_path)
    .bind(prompt.created_at.to_rfc3339())
    .bind(prompt.updated_at.to_rfc3339())
    .bind(prompt.archived_at.map(|d| d.to_rfc3339()))
    .bind(&source_kind)
    .bind(&source_json)
    .bind(&variables_json)
    .bind(&launch_defaults_json)
    .bind(&prompt.checksum_sha256)
    .execute(&mut *tx)
    .await
    .map_err(AppError::from)?;

    sqlx::query("DELETE FROM prompt_tags WHERE prompt_id = ?")
        .bind(&prompt.id.0)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;
    for tag in &prompt.tags {
        sqlx::query("INSERT INTO prompt_tags (prompt_id, tag_name) VALUES (?, ?)")
            .bind(&prompt.id.0)
            .bind(tag)
            .execute(&mut *tx)
            .await
            .map_err(AppError::from)?;
    }
    tx.commit().await.map_err(AppError::from)?;
    Ok(())
}

pub async fn delete_prompt(db: &SqlitePool, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM prompts WHERE id = ?")
        .bind(id)
        .execute(db)
        .await
        .map_err(AppError::from)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct PromptIndexRow {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub summary: String,
    pub vault_path: String,
    pub archived_at: Option<String>,
    pub tags: Vec<String>,
}

pub async fn get_prompt_index(db: &SqlitePool, id: &str) -> Result<Option<PromptIndexRow>> {
    let row: Option<(String, String, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, title, slug, summary, vault_path, archived_at FROM prompts WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await
    .map_err(AppError::from)?;
    let Some((id, title, slug, summary, vault_path, archived_at)) = row else {
        return Ok(None);
    };
    let tags: Vec<String> =
        sqlx::query_scalar("SELECT tag_name FROM prompt_tags WHERE prompt_id = ?")
            .bind(&id)
            .fetch_all(db)
            .await
            .map_err(AppError::from)?;
    Ok(Some(PromptIndexRow {
        id,
        title,
        slug,
        summary,
        vault_path,
        archived_at,
        tags,
    }))
}

#[derive(Debug, Clone, Default)]
pub struct LibraryFilters {
    pub include_archived: bool,
    pub tag: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub async fn list_prompts_for_library(
    db: &SqlitePool,
    filters: LibraryFilters,
) -> Result<Vec<PromptIndexRow>> {
    let mut sql = String::from(
        "SELECT id, title, slug, summary, vault_path, archived_at FROM prompts",
    );
    let mut clauses: Vec<&'static str> = Vec::new();
    if !filters.include_archived {
        clauses.push("archived_at IS NULL");
    }
    if filters.tag.is_some() {
        clauses.push("id IN (SELECT prompt_id FROM prompt_tags WHERE tag_name = ?)");
    }
    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    sql.push_str(" ORDER BY updated_at DESC");
    if let Some(limit) = filters.limit {
        sql.push_str(&format!(" LIMIT {limit}"));
        if let Some(offset) = filters.offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }
    }

    let mut q = sqlx::query_as::<_, (String, String, String, String, String, Option<String>)>(&sql);
    if let Some(tag) = filters.tag {
        q = q.bind(tag);
    }
    let rows = q.fetch_all(db).await.map_err(AppError::from)?;
    let mut out = Vec::with_capacity(rows.len());
    for (id, title, slug, summary, vault_path, archived_at) in rows {
        let tags: Vec<String> =
            sqlx::query_scalar("SELECT tag_name FROM prompt_tags WHERE prompt_id = ?")
                .bind(&id)
                .fetch_all(db)
                .await
                .map_err(AppError::from)?;
        out.push(PromptIndexRow {
            id,
            title,
            slug,
            summary,
            vault_path,
            archived_at,
            tags,
        });
    }
    Ok(out)
}

/// Slug uniqueness probe used at create-time to compute the next available
/// `<slug>-<N>` suffix per spec §5. Walks `prompt_tags`-aware result set —
/// we only check the canonical `slug` column.
pub async fn slug_in_use(db: &SqlitePool, slug: &str) -> Result<bool> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM prompts WHERE slug = ?")
        .bind(slug)
        .fetch_one(db)
        .await
        .map_err(AppError::from)?;
    Ok(n > 0)
}

fn source_kind_for(s: &crate::domain::source::Source) -> String {
    match s {
        crate::domain::source::Source::Manual(_) => "manual".into(),
        crate::domain::source::Source::Youtube(_) => "youtube".into(),
        crate::domain::source::Source::XTwitter(_) => "x_twitter".into(),
        crate::domain::source::Source::Article(_) => "article".into(),
    }
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
    use crate::index::db::connect_options;
    use crate::index::migrations::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::str::FromStr;

    async fn temp_pool() -> SqlitePool {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("test.sqlite");
        std::mem::forget(dir);
        let opts = connect_options(&path);
        let _ = path;
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .expect("connect")
    }

    fn make_prompt(id: &str, slug: &str, archived: bool, tags: Vec<&str>) -> Prompt {
        Prompt {
            id: PromptId(id.into()),
            title: "T".into(),
            slug: slug.into(),
            summary: "s".into(),
            body: "b".into(),
            vault_path: format!("promptibrary/prompts/{slug}.md"),
            created_at: crate::time::now_utc(),
            updated_at: crate::time::now_utc(),
            archived_at: archived.then(crate::time::now_utc),
            tags: tags.into_iter().map(|t| t.to_string()).collect(),
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
    async fn upsert_and_list_returns_rows() {
        let pool = temp_pool().await;
        run_migrations(&pool).await.unwrap();

        let a = make_prompt("01ALPHA", "alpha", false, vec!["x", "y"]);
        let b = make_prompt("01BETA", "beta", false, vec!["x"]);
        let c = make_prompt("01CHARLIE", "charlie", true, vec!["x"]);
        upsert_prompt(&pool, &a).await.unwrap();
        upsert_prompt(&pool, &b).await.unwrap();
        upsert_prompt(&pool, &c).await.unwrap();

        let active = list_prompts_for_library(&pool, LibraryFilters::default())
            .await
            .unwrap();
        assert_eq!(active.len(), 2);
        let with_archived = list_prompts_for_library(
            &pool,
            LibraryFilters {
                include_archived: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(with_archived.len(), 3);

        let filtered = list_prompts_for_library(
            &pool,
            LibraryFilters {
                tag: Some("y".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "01ALPHA");
    }

    #[tokio::test]
    async fn upsert_replaces_tags() {
        let pool = temp_pool().await;
        run_migrations(&pool).await.unwrap();
        let mut p = make_prompt("01ABC", "abc", false, vec!["one", "two"]);
        upsert_prompt(&pool, &p).await.unwrap();
        p.tags = vec!["only-this".into()];
        upsert_prompt(&pool, &p).await.unwrap();
        let row = get_prompt_index(&pool, "01ABC").await.unwrap().unwrap();
        assert_eq!(row.tags, vec!["only-this".to_string()]);
    }

    #[tokio::test]
    async fn delete_prompt_removes_row() {
        let pool = temp_pool().await;
        run_migrations(&pool).await.unwrap();
        let p = make_prompt("01ABC", "abc", false, vec!["t"]);
        upsert_prompt(&pool, &p).await.unwrap();
        delete_prompt(&pool, "01ABC").await.unwrap();
        let row = get_prompt_index(&pool, "01ABC").await.unwrap();
        assert!(row.is_none());
    }

    #[tokio::test]
    async fn slug_in_use_detects_existing() {
        let pool = temp_pool().await;
        run_migrations(&pool).await.unwrap();
        let p = make_prompt("01ABC", "abc", false, vec![]);
        upsert_prompt(&pool, &p).await.unwrap();
        assert!(slug_in_use(&pool, "abc").await.unwrap());
        assert!(!slug_in_use(&pool, "abc-2").await.unwrap());
    }
}
