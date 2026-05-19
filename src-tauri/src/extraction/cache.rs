//! Extraction cache — fetched content (7d) + LLM candidates (30d) per spec §6.
//!
//! Two SQLite tables:
//! - `extraction_cache` (migration 0004): fetched-source side, key = sha256
//!   over (source_kind + canonical_url). Holds normalized
//!   `FetchedSourceContent` JSON.
//! - `extraction_candidates_cache` (migration 0007): LLM-candidate side,
//!   key = sha256 over (kind + canonical_url + mode + model +
//!   prompt_version). Bumping `EXTRACTION_PROMPT_VERSION` invalidates rows
//!   transparently because no future key matches them.
//!
//! Force-refresh skips the get-side lookup; the next put overwrites.

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use super::types::{ExtractionMode, ExtractionResponse, FetchedSourceContent};
use crate::error::Result;

const FETCHED_TTL_DAYS: i64 = 7;
const CANDIDATES_TTL_DAYS: i64 = 30;

pub fn compute_source_cache_key(source_kind: &str, canonical_url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source_kind.as_bytes());
    hasher.update(b"\x1f");
    hasher.update(canonical_url.as_bytes());
    hex(hasher.finalize())
}

pub fn compute_candidates_cache_key(
    source_kind: &str,
    canonical_url: &str,
    mode: ExtractionMode,
    model: &str,
    prompt_version: u32,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source_kind.as_bytes());
    hasher.update(b"\x1f");
    hasher.update(canonical_url.as_bytes());
    hasher.update(b"\x1f");
    hasher.update(mode.as_wire().as_bytes());
    hasher.update(b"\x1f");
    hasher.update(model.as_bytes());
    hasher.update(b"\x1f");
    hasher.update(prompt_version.to_string().as_bytes());
    hex(hasher.finalize())
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

// ─── Fetched source ──────────────────────────────────────────────────────────

pub async fn get_fetched_source(
    pool: &SqlitePool,
    cache_key: &str,
) -> Result<Option<FetchedSourceContent>> {
    let now = Utc::now();
    let row: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT fetched_content_json, expires_at
        FROM extraction_cache
        WHERE cache_key = ?1 AND expires_at > ?2
        LIMIT 1
        "#,
    )
    .bind(cache_key)
    .bind(now.to_rfc3339())
    .fetch_optional(pool)
    .await?;

    let Some((json, _expires)) = row else {
        return Ok(None);
    };
    let mut content: FetchedSourceContent = serde_json::from_str(&json)?;
    content.cached = true;
    Ok(Some(content))
}

pub async fn put_fetched_source(
    pool: &SqlitePool,
    cache_key: &str,
    source_kind: &str,
    origin_url: &str,
    content: &FetchedSourceContent,
) -> Result<()> {
    let json = serde_json::to_string(content)?;
    let now = Utc::now();
    let expires = now + Duration::days(FETCHED_TTL_DAYS);
    sqlx::query(
        r#"
        INSERT INTO extraction_cache
            (cache_key, source_kind, origin_url, fetched_content_json, fetched_at, expires_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(cache_key) DO UPDATE SET
            source_kind = excluded.source_kind,
            origin_url = excluded.origin_url,
            fetched_content_json = excluded.fetched_content_json,
            fetched_at = excluded.fetched_at,
            expires_at = excluded.expires_at
        "#,
    )
    .bind(cache_key)
    .bind(source_kind)
    .bind(origin_url)
    .bind(&json)
    .bind(now.to_rfc3339())
    .bind(expires.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

// ─── Candidates ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CachedCandidates {
    pub response: ExtractionResponse,
    pub created_at: DateTime<Utc>,
}

pub async fn get_candidates(
    pool: &SqlitePool,
    cache_key: &str,
) -> Result<Option<CachedCandidates>> {
    let now = Utc::now();
    let row: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT response_json, created_at
        FROM extraction_candidates_cache
        WHERE cache_key = ?1 AND expires_at > ?2
        LIMIT 1
        "#,
    )
    .bind(cache_key)
    .bind(now.to_rfc3339())
    .fetch_optional(pool)
    .await?;

    let Some((json, created_at)) = row else {
        return Ok(None);
    };
    let response: ExtractionResponse = serde_json::from_str(&json)?;
    let created_at = DateTime::parse_from_rfc3339(&created_at)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    Ok(Some(CachedCandidates { response, created_at }))
}

pub async fn put_candidates(
    pool: &SqlitePool,
    cache_key: &str,
    source_kind: &str,
    canonical_url: &str,
    mode: ExtractionMode,
    model: &str,
    prompt_version: u32,
    response: &ExtractionResponse,
) -> Result<()> {
    let json = serde_json::to_string(response)?;
    let now = Utc::now();
    let expires = now + Duration::days(CANDIDATES_TTL_DAYS);
    sqlx::query(
        r#"
        INSERT INTO extraction_candidates_cache
            (cache_key, source_kind, canonical_url, extraction_mode, model, prompt_version,
             response_json, created_at, expires_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(cache_key) DO UPDATE SET
            response_json = excluded.response_json,
            created_at = excluded.created_at,
            expires_at = excluded.expires_at
        "#,
    )
    .bind(cache_key)
    .bind(source_kind)
    .bind(canonical_url)
    .bind(mode.as_wire())
    .bind(model)
    .bind(prompt_version as i64)
    .bind(&json)
    .bind(now.to_rfc3339())
    .bind(expires.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

/// Best-effort purge of all expired rows in both cache tables. Run from a
/// background task or at startup.
pub async fn purge_expired(pool: &SqlitePool) -> Result<u64> {
    let now = Utc::now().to_rfc3339();
    let r1 = sqlx::query("DELETE FROM extraction_cache WHERE expires_at <= ?1")
        .bind(&now)
        .execute(pool)
        .await?;
    let r2 = sqlx::query("DELETE FROM extraction_candidates_cache WHERE expires_at <= ?1")
        .bind(&now)
        .execute(pool)
        .await?;
    Ok(r1.rows_affected() + r2.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::source::{ArticleSource, Source};
    use crate::extraction::types::{
        CandidateConfidence, CandidatePrompt, ExtractionMode, ExtractionResponse,
        FetchedSourceContent, LaunchDefaultsPatch,
    };
    use crate::index::db::in_memory_connect_options;
    use crate::index::migrations::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(in_memory_connect_options())
            .await
            .unwrap();
        run_migrations(&pool).await.unwrap();
        pool
    }

    fn sample_content() -> FetchedSourceContent {
        FetchedSourceContent {
            source: Source::Article(ArticleSource {
                origin_url: Some("https://example.com/x".into()),
                title: Some("Title".into()),
                author: None,
                fetched_at: Some(Utc::now()),
                content_hash: None,
                site_name: None,
                byline: None,
                published_at: None,
            }),
            canonical_url: "https://example.com/x".into(),
            fetched_at: Utc::now(),
            title: Some("Title".into()),
            author: None,
            text: "Hello body".into(),
            chunks: vec![],
            raw_metadata: Default::default(),
            content_hash: "abc".into(),
            cached: false,
        }
    }

    fn sample_response() -> ExtractionResponse {
        ExtractionResponse {
            schema_version: 1,
            candidates: vec![CandidatePrompt {
                title: "Refactor X for performance".into(),
                summary: "Sum".into(),
                body: "x".repeat(300),
                tags: vec!["one".into()],
                variables: vec![],
                launch_defaults_patch: LaunchDefaultsPatch::default(),
                confidence: CandidateConfidence::Medium,
                rationale: "because".into(),
                source_anchors: vec![],
            }],
        }
    }

    #[test]
    fn source_key_is_stable_across_calls() {
        let a = compute_source_cache_key("youtube", "https://www.youtube.com/watch?v=abc");
        let b = compute_source_cache_key("youtube", "https://www.youtube.com/watch?v=abc");
        assert_eq!(a, b);
        let c = compute_source_cache_key("article", "https://www.youtube.com/watch?v=abc");
        assert_ne!(a, c);
    }

    #[test]
    fn candidates_key_depends_on_mode_and_version() {
        let a = compute_candidates_cache_key(
            "article",
            "https://example.com/x",
            ExtractionMode::Standard,
            "claude-sonnet-4-6",
            1,
        );
        let b = compute_candidates_cache_key(
            "article",
            "https://example.com/x",
            ExtractionMode::Deep,
            "claude-sonnet-4-6",
            1,
        );
        let c = compute_candidates_cache_key(
            "article",
            "https://example.com/x",
            ExtractionMode::Standard,
            "claude-sonnet-4-6",
            2,
        );
        assert_ne!(a, b);
        assert_ne!(a, c);
    }

    #[tokio::test]
    async fn fetched_source_round_trip_marks_cached() {
        let pool = pool().await;
        let key = compute_source_cache_key("article", "https://example.com/x");
        assert!(get_fetched_source(&pool, &key).await.unwrap().is_none());

        put_fetched_source(&pool, &key, "article", "https://example.com/x", &sample_content())
            .await
            .unwrap();

        let got = get_fetched_source(&pool, &key).await.unwrap().unwrap();
        assert!(got.cached, "served-from-cache flag must flip true");
        assert_eq!(got.text, "Hello body");
    }

    #[tokio::test]
    async fn candidates_round_trip() {
        let pool = pool().await;
        let key = compute_candidates_cache_key(
            "article",
            "https://example.com/x",
            ExtractionMode::Standard,
            "claude-sonnet-4-6",
            1,
        );
        assert!(get_candidates(&pool, &key).await.unwrap().is_none());

        put_candidates(
            &pool,
            &key,
            "article",
            "https://example.com/x",
            ExtractionMode::Standard,
            "claude-sonnet-4-6",
            1,
            &sample_response(),
        )
        .await
        .unwrap();

        let cached = get_candidates(&pool, &key).await.unwrap().unwrap();
        assert_eq!(cached.response.candidates.len(), 1);
    }

    #[tokio::test]
    async fn put_is_idempotent_overwrite() {
        let pool = pool().await;
        let key = compute_source_cache_key("article", "https://example.com/x");
        for body in &["one", "two", "three"] {
            let mut content = sample_content();
            content.text = (*body).into();
            put_fetched_source(&pool, &key, "article", "https://example.com/x", &content)
                .await
                .unwrap();
        }
        let got = get_fetched_source(&pool, &key).await.unwrap().unwrap();
        assert_eq!(got.text, "three");
    }

    #[tokio::test]
    async fn expired_row_returns_none() {
        let pool = pool().await;
        let key = compute_source_cache_key("article", "https://example.com/x");
        let json = serde_json::to_string(&sample_content()).unwrap();
        let past = (Utc::now() - Duration::days(1)).to_rfc3339();
        let fetched = (Utc::now() - Duration::days(10)).to_rfc3339();
        sqlx::query(
            r#"INSERT INTO extraction_cache
               (cache_key, source_kind, origin_url, fetched_content_json, fetched_at, expires_at)
               VALUES (?1,?2,?3,?4,?5,?6)"#,
        )
        .bind(&key)
        .bind("article")
        .bind("https://example.com/x")
        .bind(&json)
        .bind(&fetched)
        .bind(&past)
        .execute(&pool)
        .await
        .unwrap();

        assert!(get_fetched_source(&pool, &key).await.unwrap().is_none());

        let purged = purge_expired(&pool).await.unwrap();
        assert_eq!(purged, 1);
    }
}
