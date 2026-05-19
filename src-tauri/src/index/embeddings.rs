//! Local embedding generation and vector search per spec §9.
//!
//! ## Architecture
//!
//! - `EmbeddingService` trait: testable surface. `embed(text)` returns
//!   a `Vec<f32>` of dimension `EMBEDDING_DIM` (384 for bge-small).
//! - `MockEmbeddingService` (in-tree, always available): produces
//!   deterministic vectors via a SHA-256 hash of the input text,
//!   expanded into 384 f32 lanes. Useful for testing the cosine
//!   storage layer without an ONNX runtime.
//! - `FastembedService` (deferred, see *Enabling fastembed* below):
//!   the real bge-small-en-v1.5 implementation. Off by default —
//!   enabling the `fastembed` dep is a substantial compile-time cost
//!   (ORT native runtime) plus a ~130 MB model download.
//!
//! Storage: per-prompt embeddings live in the `prompt_embeddings`
//! table (migration 0003). Each row carries the prompt id, the
//! producing model identifier, the embedding dimension, the raw
//! little-endian `f32` bytes, the indexed content checksum, and the
//! write timestamp. The `sqlite-vec` `vec0` virtual table that
//! provides nearest-neighbour search is created at runtime (the
//! migration intentionally leaves it out because `sqlx::migrate!`
//! doesn't load extensions).
//!
//! ## Enabling fastembed (V2 follow-up — SCA-784)
//!
//! 1. Uncomment `fastembed = "5.13"` in `src-tauri/Cargo.toml`. (Adds
//!    `ort` ONNX runtime as a transitive dep; first compile is ~3 min.)
//! 2. Implement `FastembedService::new(app_data_dir)` to lazy-load the
//!    model from `<app_data_dir>/embeddings/bge-small-en-v1.5/`.
//! 3. First-run model download with `index://progress` events,
//!    SHA-256 manifest baked into the binary at build time, fail-
//!    closed on hash mismatch.
//! 4. Set `AppServices::embedding_service` to the real impl rather
//!    than the mock in production builds.
//! 5. Flip the hybrid path's semantic component from constant 0 to
//!    the real cosine value (`commands::search::compose_scored_result`).
//!
//! Until step 1 lands, every code path that requests an embedding
//! either uses the mock (in tests) or silently degrades to text-only
//! hybrid ranking (`SearchMode::Hybrid` with semantic weight 0).

use std::sync::Arc;

use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::error::{AppError, Result};

/// Dimension of the bge-small-en-v1.5 model. Other models would need
/// a different value here AND a schema migration (the
/// `prompt_embeddings.dim` column gates which rows are usable).
pub const EMBEDDING_DIM: usize = 384;

/// Model identifier stored in `prompt_embeddings.model`. Used to
/// invalidate embeddings when the model changes — V2 will compare
/// `prompt_embeddings.model` against the current `EmbeddingService::id()`
/// and trigger a reindex on mismatch.
pub const MOCK_MODEL_ID: &str = "mock-sha256-v1";

/// Backend-agnostic embedding service. Tests use `MockEmbeddingService`;
/// production (once fastembed is enabled per the module docstring) uses
/// `FastembedService`.
pub trait EmbeddingService: Send + Sync {
    /// Unique identifier for this model. Stored in
    /// `prompt_embeddings.model` and compared against on cold-start
    /// reindex.
    fn id(&self) -> &'static str;

    /// Produce a fixed-dimension embedding for the given text.
    fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

/// Deterministic, hash-based embedding service. Always available;
/// useful for testing storage + cosine search without ORT. Two texts
/// with the same SHA-256 hash produce identical vectors; otherwise
/// the lanes vary smoothly.
pub struct MockEmbeddingService;

impl MockEmbeddingService {
    pub fn new() -> Arc<dyn EmbeddingService> {
        Arc::new(Self) as Arc<dyn EmbeddingService>
    }
}

impl Default for MockEmbeddingService {
    fn default() -> Self {
        Self
    }
}

impl EmbeddingService for MockEmbeddingService {
    fn id(&self) -> &'static str {
        MOCK_MODEL_ID
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Hash the text into 32 bytes, then expand to EMBEDDING_DIM
        // lanes by tiling. Convert each lane to f32 in [-1, 1].
        let digest = Sha256::digest(text.as_bytes());
        let mut out = Vec::with_capacity(EMBEDDING_DIM);
        for i in 0..EMBEDDING_DIM {
            let byte = digest[i % digest.len()];
            // Map [0..256) → [-1.0..1.0) linearly. Add a small
            // i-dependent perturbation so the vector isn't just a
            // tiling of 32 distinct values.
            let lane = (byte as f32 / 128.0) - 1.0;
            let perturb = ((i as f32) * 0.001).sin() * 0.05;
            out.push(lane + perturb);
        }
        Ok(normalize(&out))
    }
}

/// L2-normalize a vector in place-but-returned. Required for cosine
/// similarity to reduce to a dot product. Returns a zero vector if
/// the input is all zeros (degenerate but legal — handled by
/// downstream cosine).
pub fn normalize(v: &[f32]) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        return v.to_vec();
    }
    v.iter().map(|x| x / norm).collect()
}

/// Cosine similarity for two equal-length L2-normalized vectors.
/// (Reduces to dot product for L2-normalized inputs.)
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "vector dimension mismatch");
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

// ─── Storage layer (sqlx) ─────────────────────────────────────────────

/// Serialize a vector to the little-endian f32 byte layout we store
/// in `prompt_embeddings.embedding`.
pub fn vector_to_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

/// Inverse of `vector_to_bytes`. Returns Err if the byte length is
/// not a multiple of 4 (corrupt row).
pub fn bytes_to_vector(bytes: &[u8]) -> Result<Vec<f32>> {
    if bytes.len() % 4 != 0 {
        return Err(AppError::new(
            crate::error::AppErrorKind::Internal,
            format!(
                "prompt_embeddings.embedding has {} bytes, not a multiple of 4",
                bytes.len()
            ),
        ));
    }
    let mut out = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let arr: [u8; 4] = chunk.try_into().expect("chunks_exact(4) ⇒ len 4");
        out.push(f32::from_le_bytes(arr));
    }
    Ok(out)
}

/// Upsert an embedding for a prompt. `model` is the producing
/// service's id (so a future migration can invalidate stale rows on
/// model change). `indexed_checksum_sha256` ties the embedding to a
/// specific snapshot of the prompt body — when the body changes the
/// checksum changes and the reindex queue picks the row up.
pub async fn upsert_embedding(
    db: &SqlitePool,
    prompt_id: &str,
    model: &str,
    vector: &[f32],
    indexed_checksum_sha256: &str,
) -> Result<()> {
    let bytes = vector_to_bytes(vector);
    let dim = vector.len() as i64;
    let now = crate::time::now_iso8601();
    sqlx::query(
        "INSERT INTO prompt_embeddings (prompt_id, model, dim, embedding,
            indexed_checksum_sha256, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(prompt_id) DO UPDATE SET
            model = excluded.model,
            dim = excluded.dim,
            embedding = excluded.embedding,
            indexed_checksum_sha256 = excluded.indexed_checksum_sha256,
            updated_at = excluded.updated_at",
    )
    .bind(prompt_id)
    .bind(model)
    .bind(dim)
    .bind(&bytes)
    .bind(indexed_checksum_sha256)
    .bind(now)
    .execute(db)
    .await
    .map_err(AppError::from)?;
    Ok(())
}

/// Read all embeddings for a model. Returns `(prompt_id, vector)` pairs.
/// For V1 scale (~hundreds of prompts) loading the full set into
/// memory and ranking by cosine is fast enough — the `sqlite-vec` vec0
/// table is a future optimization documented in V2-CANDIDATES.
pub async fn load_all_embeddings(
    db: &SqlitePool,
    model: &str,
) -> Result<Vec<(String, Vec<f32>)>> {
    let rows: Vec<(String, Vec<u8>)> = sqlx::query_as(
        "SELECT prompt_id, embedding FROM prompt_embeddings WHERE model = ?",
    )
    .bind(model)
    .fetch_all(db)
    .await
    .map_err(AppError::from)?;
    let mut out = Vec::with_capacity(rows.len());
    for (pid, bytes) in rows {
        out.push((pid, bytes_to_vector(&bytes)?));
    }
    Ok(out)
}

/// Cosine-rank all stored embeddings for `model` against `query_vec`.
/// Returns prompt ids in descending similarity order, up to `limit`.
/// Archived prompts are excluded via a JOIN on `prompts.archived_at`.
pub async fn search_semantic(
    db: &SqlitePool,
    model: &str,
    query_vec: &[f32],
    limit: usize,
) -> Result<Vec<(String, f32)>> {
    let rows: Vec<(String, Vec<u8>)> = sqlx::query_as(
        "SELECT pe.prompt_id, pe.embedding
           FROM prompt_embeddings pe
           JOIN prompts p ON p.id = pe.prompt_id
          WHERE pe.model = ?
            AND p.archived_at IS NULL",
    )
    .bind(model)
    .fetch_all(db)
    .await
    .map_err(AppError::from)?;

    let mut scored: Vec<(String, f32)> = Vec::with_capacity(rows.len());
    for (pid, bytes) in rows {
        let vec = bytes_to_vector(&bytes)?;
        if vec.len() != query_vec.len() {
            // Wrong-dim row (model swap in flight, or migration drift).
            // Skip rather than fail the whole search.
            continue;
        }
        scored.push((pid, cosine(&vec, query_vec)));
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);
    Ok(scored)
}

/// Delete a single prompt's embedding. Called when the prompt is
/// deleted from the index.
pub async fn delete_embedding(db: &SqlitePool, prompt_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM prompt_embeddings WHERE prompt_id = ?")
        .bind(prompt_id)
        .execute(db)
        .await
        .map_err(AppError::from)?;
    Ok(())
}

/// Drop all embeddings — invoked when the embedding model changes
/// (e.g. operator swaps bge-small for a different model). The reindex
/// queue rebuilds from scratch.
pub async fn clear_all_embeddings(db: &SqlitePool) -> Result<u64> {
    let res = sqlx::query("DELETE FROM prompt_embeddings")
        .execute(db)
        .await
        .map_err(AppError::from)?;
    Ok(res.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::db::in_memory_connect_options;
    use crate::index::migrations::run_migrations;
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

    async fn seed_prompt(db: &SqlitePool, id: &str, archived: bool) {
        let archived_at = if archived {
            Some("2026-05-19T00:00:00Z")
        } else {
            None
        };
        sqlx::query(
            "INSERT INTO prompts (id, title, slug, summary, body, vault_path,
                created_at, updated_at, archived_at, source_kind, source_json,
                variables_json, launch_defaults_json, checksum_sha256)
             VALUES (?, 'T', ?, '', '', ?, ?, ?, ?, 'manual', '{}', '[]', '{}', 'sha256:0')",
        )
        .bind(id)
        .bind(id)
        .bind(format!("promptibrary/prompts/{id}.md"))
        .bind("2026-05-19T00:00:00Z")
        .bind("2026-05-19T00:00:00Z")
        .bind(archived_at)
        .execute(db)
        .await
        .unwrap();
    }

    #[test]
    fn mock_service_produces_normalized_vector_of_correct_dim() {
        let svc = MockEmbeddingService;
        let v = svc.embed("hello world").unwrap();
        assert_eq!(v.len(), EMBEDDING_DIM);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "norm = {norm}");
    }

    #[test]
    fn mock_service_is_deterministic() {
        let svc = MockEmbeddingService;
        let a = svc.embed("kubernetes deployment guide").unwrap();
        let b = svc.embed("kubernetes deployment guide").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn different_inputs_yield_different_vectors() {
        let svc = MockEmbeddingService;
        let a = svc.embed("kubernetes").unwrap();
        let b = svc.embed("anthropic").unwrap();
        assert_ne!(a, b);
        // Cosine should be substantially less than 1 for different inputs.
        assert!(cosine(&a, &b) < 0.999);
    }

    #[test]
    fn vector_byte_round_trip() {
        let v = vec![0.1f32, -0.5, 1.0e-3, 42.5];
        let bytes = vector_to_bytes(&v);
        let back = bytes_to_vector(&bytes).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn bytes_to_vector_rejects_wrong_length() {
        let bytes = vec![0u8; 5];
        let err = bytes_to_vector(&bytes).unwrap_err();
        assert!(err.message.contains("not a multiple of 4"));
    }

    #[tokio::test]
    async fn upsert_and_search_finds_closest_match() {
        let db = temp_pool().await;
        seed_prompt(&db, "p1", false).await;
        seed_prompt(&db, "p2", false).await;
        seed_prompt(&db, "p3", false).await;

        let svc = MockEmbeddingService;
        let v1 = svc.embed("kubernetes deployment").unwrap();
        let v2 = svc.embed("python web framework").unwrap();
        let v3 = svc.embed("k8s cluster setup").unwrap();
        upsert_embedding(&db, "p1", svc.id(), &v1, "sha256:1").await.unwrap();
        upsert_embedding(&db, "p2", svc.id(), &v2, "sha256:2").await.unwrap();
        upsert_embedding(&db, "p3", svc.id(), &v3, "sha256:3").await.unwrap();

        // Query close to v1 (same text) — p1 should rank first.
        let query = svc.embed("kubernetes deployment").unwrap();
        let results = search_semantic(&db, svc.id(), &query, 3).await.unwrap();
        assert_eq!(results[0].0, "p1");
        assert!(
            results[0].1 > 0.99,
            "expected near-1 cosine for identical text, got {}",
            results[0].1
        );
    }

    #[tokio::test]
    async fn search_semantic_excludes_archived_prompts() {
        let db = temp_pool().await;
        seed_prompt(&db, "p1", true).await; // archived
        seed_prompt(&db, "p2", false).await;
        let svc = MockEmbeddingService;
        let v1 = svc.embed("kubernetes").unwrap();
        let v2 = svc.embed("kubernetes").unwrap();
        upsert_embedding(&db, "p1", svc.id(), &v1, "sha256:1").await.unwrap();
        upsert_embedding(&db, "p2", svc.id(), &v2, "sha256:2").await.unwrap();

        let query = svc.embed("kubernetes").unwrap();
        let results = search_semantic(&db, svc.id(), &query, 5).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "p2");
    }

    #[tokio::test]
    async fn upsert_replaces_existing_row() {
        let db = temp_pool().await;
        seed_prompt(&db, "p1", false).await;
        let svc = MockEmbeddingService;
        let v1 = svc.embed("first version").unwrap();
        let v2 = svc.embed("second version").unwrap();
        upsert_embedding(&db, "p1", svc.id(), &v1, "sha256:1").await.unwrap();
        upsert_embedding(&db, "p1", svc.id(), &v2, "sha256:2").await.unwrap();

        let all = load_all_embeddings(&db, svc.id()).await.unwrap();
        assert_eq!(all.len(), 1, "ON CONFLICT replaces rather than dupes");
        // Stored vector matches v2 (latest).
        assert_eq!(all[0].1, v2);
    }

    #[tokio::test]
    async fn delete_embedding_removes_row() {
        let db = temp_pool().await;
        seed_prompt(&db, "p1", false).await;
        let svc = MockEmbeddingService;
        let v = svc.embed("x").unwrap();
        upsert_embedding(&db, "p1", svc.id(), &v, "sha256:1").await.unwrap();
        delete_embedding(&db, "p1").await.unwrap();
        let all = load_all_embeddings(&db, svc.id()).await.unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn clear_all_embeddings_reports_count() {
        let db = temp_pool().await;
        seed_prompt(&db, "p1", false).await;
        seed_prompt(&db, "p2", false).await;
        let svc = MockEmbeddingService;
        upsert_embedding(&db, "p1", svc.id(), &svc.embed("a").unwrap(), "sha256:a")
            .await
            .unwrap();
        upsert_embedding(&db, "p2", svc.id(), &svc.embed("b").unwrap(), "sha256:b")
            .await
            .unwrap();
        let n = clear_all_embeddings(&db).await.unwrap();
        assert_eq!(n, 2);
    }
}
