//! Real `EmbeddingService` impl backed by `fastembed` + bge-small-en-v1.5.
//!
//! Compiled only when the `fastembed` cargo feature is enabled
//! (`cargo build --features fastembed`). Default builds use
//! `MockEmbeddingService` from `index::embeddings`.
//!
//! ## Behavior
//!
//! - `FastembedService::try_new(app_data_dir)` lazy-resolves the
//!   bge-small-en-v1.5 model. fastembed downloads ONNX weights to
//!   `<app_data_dir>/embeddings/bge-small-en-v1.5/` on first use; the
//!   download is ~130 MB and requires network. Re-runs read from the
//!   cache.
//! - `embed(text)` produces a single `Vec<f32>` of dim 384. The model
//!   already L2-normalizes its outputs but we re-normalize defensively
//!   so cosine math reduces to a dot product everywhere.
//! - `id()` returns `"bge-small-en-v1.5"` — this is what gets stored in
//!   `prompt_embeddings.model` so a future model swap can trigger a
//!   reindex on `EmbeddingService::id()` mismatch.
//!
//! ## SHA-256 manifest (V2)
//!
//! The fastembed crate validates its own downloads via Hugging Face
//! repo SHAs. The spec §9 *fail-closed on hash mismatch* requirement
//! talks about Promptibrary baking the expected SHAs into the binary
//! at build time. That's a follow-up — for now we trust the upstream
//! validation. Filed under V2-CANDIDATES as the *SHA manifest* line
//! item.

use std::path::PathBuf;
use std::sync::Mutex;

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use crate::error::{AppError, AppErrorKind, Result};
use crate::index::embeddings::{normalize, EmbeddingService, EMBEDDING_DIM};

/// Model identifier stored in `prompt_embeddings.model`.
pub const FASTEMBED_MODEL_ID: &str = "bge-small-en-v1.5";

pub struct FastembedService {
    inner: Mutex<TextEmbedding>,
}

impl FastembedService {
    /// Build a new service rooted at `app_data_dir / "embeddings"`. The
    /// fastembed cache path is set via `InitOptions::cache_dir` so the
    /// downloaded ONNX files live with the rest of Promptibrary's
    /// AppData (per spec §16).
    pub fn try_new(app_data_dir: &std::path::Path) -> Result<Self> {
        let cache_dir: PathBuf = app_data_dir.join("embeddings");
        std::fs::create_dir_all(&cache_dir).map_err(AppError::from)?;
        let opts = InitOptions::new(EmbeddingModel::BGESmallENV15)
            .with_cache_dir(cache_dir)
            .with_show_download_progress(false);
        let model = TextEmbedding::try_new(opts).map_err(|e| {
            AppError::new(
                AppErrorKind::Internal,
                format!("fastembed init failed: {e}"),
            )
        })?;
        Ok(Self {
            inner: Mutex::new(model),
        })
    }
}

impl EmbeddingService for FastembedService {
    fn id(&self) -> &'static str {
        FASTEMBED_MODEL_ID
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut model = self.inner.lock().map_err(|_| {
            AppError::new(
                AppErrorKind::Internal,
                "fastembed model mutex poisoned",
            )
        })?;
        // `TextEmbedding::embed` takes a Vec of documents and returns
        // one Vec<f32> per document. Single-document call here; batch
        // size None lets fastembed pick its default (~256).
        let batch: Vec<String> = vec![text.to_string()];
        let vectors = model.embed(batch, None).map_err(|e| {
            AppError::new(
                AppErrorKind::Internal,
                format!("fastembed embed failed: {e}"),
            )
        })?;
        let mut v = vectors.into_iter().next().ok_or_else(|| {
            AppError::new(
                AppErrorKind::Internal,
                "fastembed returned an empty embedding vector",
            )
        })?;
        // Defensive: assert the dim matches the rest of the index. A
        // model swap would otherwise produce wrong-dim rows that fail
        // cosine silently.
        if v.len() != EMBEDDING_DIM {
            return Err(AppError::new(
                AppErrorKind::Internal,
                format!(
                    "fastembed produced dim {}, expected {}",
                    v.len(),
                    EMBEDDING_DIM
                ),
            ));
        }
        v = normalize(&v);
        Ok(v)
    }
}
