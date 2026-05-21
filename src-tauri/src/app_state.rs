//! Shared Tauri state container.
//!
//! L1 holds the vault + SQLite pool. L4 adds the extraction services
//! (secret store, HTTP client, rate limiter, yt-dlp runner, Anthropic
//! transport). L5 will layer in the watcher handle, settings store, PTY
//! pool, embedding service.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::{Mutex, RwLock};

use crate::extraction::anthropic::{AnthropicTransport, HttpAnthropicTransport};
use crate::extraction::fetchers::youtube::{RealYtDlpRunner, YtDlpRunner};
use crate::extraction::rate_limit::RateLimiter;
use crate::assistant::transport::{
    HttpStreamingAnthropicTransport, StreamingAnthropicTransport,
};
use crate::index::embeddings::{EmbeddingService, MockEmbeddingService};
use crate::launch::pty_pool::PtyPool;
use crate::settings::secret_store::SecretStore;
#[cfg(any(
    test,
    not(any(target_os = "macos", target_os = "linux", target_os = "windows"))
))]
use crate::settings::secret_store::InMemorySecretStore;
#[cfg(all(
    not(test),
    any(target_os = "macos", target_os = "linux", target_os = "windows")
))]
use crate::settings::secret_store::KeychainStore;
use crate::vault::paths::VaultPaths;

pub struct AppServices {
    pub state: RwLock<MutableState>,
    /// Global serialization gate for create_prompt. Without this, two
    /// concurrent IPC calls can both observe the same slug as free,
    /// derive the same vault_path, and race the atomic_write rename →
    /// duplicate-vault_path rows + content corruption. SCA-589.
    pub create_prompt_lock: Mutex<()>,
    /// Backend-agnostic secret store. Prod uses `KeychainStore`; tests use
    /// `InMemorySecretStore` so they never touch the real keychain.
    pub secrets: Arc<dyn SecretStore>,
    /// Shared HTTP client for the extraction pipeline. 60s timeout, gzip
    /// + brotli enabled (configured at the dep level).
    pub http: reqwest::Client,
    /// Per-provider extraction rate limiter (spec §6).
    pub rate_limiter: Arc<RateLimiter>,
    /// yt-dlp subprocess runner. Tests can swap in a mock.
    pub yt_dlp: Arc<dyn YtDlpRunner>,
    /// Anthropic Messages API transport. Tests can swap in a mock that
    /// replays canned JSON responses without hitting the real API.
    pub anthropic_transport: Arc<dyn AnthropicTransport>,
    /// Streaming Anthropic transport for the in-app assistant. Independent
    /// of `anthropic_transport` because the streaming SSE shape and
    /// tool-use request schema diverge from the one-shot extraction path.
    pub streaming_anthropic_transport: Arc<dyn StreamingAnthropicTransport>,
    /// Directory where yt-dlp drops transcript files. Created on demand.
    pub extraction_temp_dir: PathBuf,
    /// SCA-782: AppData root for the local settings JSON, the embedding
    /// model cache (V2), and any other host-machine-scoped state.
    /// macOS: `~/Library/Application Support/com.promptibrary.app`;
    /// Linux: `$XDG_CONFIG_HOME/promptibrary` or `~/.config/promptibrary`;
    /// Windows: `%APPDATA%\promptibrary`. Tests use a tempdir override.
    pub app_data_dir: PathBuf,
    /// SCA-784: backend-agnostic embedding service. Defaults to
    /// `MockEmbeddingService` until `fastembed = "5.13"` is enabled in
    /// Cargo.toml — see `src/index/embeddings.rs` module docstring for
    /// the enable procedure.
    pub embedding_service: Arc<dyn EmbeddingService>,
    /// SCA-809: live PTY sessions keyed by RunId. commands::launches::
    /// start_launch inserts; stop_run / natural exit removes;
    /// send_terminal_input + resize_terminal look up by id.
    pub pty_pool: Arc<PtyPool>,
}

#[derive(Default)]
pub struct MutableState {
    pub vault: Option<VaultPaths>,
    pub db: Option<SqlitePool>,
}

impl AppServices {
    pub fn new() -> Self {
        #[cfg(all(
            not(test),
            any(target_os = "macos", target_os = "linux", target_os = "windows")
        ))]
        let secrets: Arc<dyn SecretStore> = Arc::new(KeychainStore::new());
        #[cfg(any(
            test,
            not(any(target_os = "macos", target_os = "linux", target_os = "windows"))
        ))]
        let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::new());

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("Promptibrary/1.0 (+local desktop importer)")
            .build()
            .expect("reqwest client builds");

        let anthropic_transport: Arc<dyn AnthropicTransport> = Arc::new(
            HttpAnthropicTransport::new(http.clone(), secrets.clone()),
        );
        let streaming_anthropic_transport: Arc<dyn StreamingAnthropicTransport> = Arc::new(
            HttpStreamingAnthropicTransport::new(http.clone(), secrets.clone()),
        );
        let yt_dlp: Arc<dyn YtDlpRunner> = Arc::new(RealYtDlpRunner);

        let extraction_temp_dir = std::env::temp_dir().join("promptibrary").join("extraction");
        let app_data_dir = default_app_data_dir();

        // SCA-808: real semantic search when `fastembed` cargo feature
        // is enabled. The model download is lazy and ~130 MB on first
        // run; if it fails we fall back to MockEmbeddingService so the
        // app still starts (the hybrid score's semantic component
        // degrades gracefully to 0 rather than crashing the index).
        let embedding_service: Arc<dyn EmbeddingService> = {
            #[cfg(feature = "fastembed")]
            {
                match crate::index::fastembed_service::FastembedService::try_new(&app_data_dir) {
                    Ok(svc) => Arc::new(svc) as Arc<dyn EmbeddingService>,
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "fastembed initialization failed; falling back to MockEmbeddingService"
                        );
                        MockEmbeddingService::new()
                    }
                }
            }
            #[cfg(not(feature = "fastembed"))]
            {
                MockEmbeddingService::new()
            }
        };

        Self {
            state: RwLock::default(),
            create_prompt_lock: Mutex::new(()),
            secrets,
            http,
            rate_limiter: Arc::new(RateLimiter::new()),
            yt_dlp,
            anthropic_transport,
            streaming_anthropic_transport,
            extraction_temp_dir,
            app_data_dir,
            embedding_service,
            pty_pool: Arc::new(PtyPool::new()),
        }
    }

    /// Constructor injecting a specific secret backend. Test-time helper to
    /// swap in `InMemorySecretStore`.
    pub fn with_secret_store(secrets: Arc<dyn SecretStore>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("reqwest client builds");
        let anthropic_transport: Arc<dyn AnthropicTransport> = Arc::new(
            HttpAnthropicTransport::new(http.clone(), secrets.clone()),
        );
        let streaming_anthropic_transport: Arc<dyn StreamingAnthropicTransport> = Arc::new(
            HttpStreamingAnthropicTransport::new(http.clone(), secrets.clone()),
        );
        Self {
            state: RwLock::default(),
            create_prompt_lock: Mutex::new(()),
            secrets,
            http,
            rate_limiter: Arc::new(RateLimiter::new()),
            yt_dlp: Arc::new(RealYtDlpRunner),
            anthropic_transport,
            streaming_anthropic_transport,
            extraction_temp_dir: std::env::temp_dir().join("promptibrary").join("extraction"),
            app_data_dir: default_app_data_dir(),
            embedding_service: MockEmbeddingService::new(),
            pty_pool: Arc::new(PtyPool::new()),
        }
    }

    /// Test helper that overrides the AppData directory so disk-write
    /// surfaces (settings persistence, embedding model cache) target
    /// an isolated tempdir.
    pub fn with_app_data_dir(self, app_data_dir: PathBuf) -> Self {
        Self {
            app_data_dir,
            ..self
        }
    }

    /// Test helper that overrides the embedding service so a test can
    /// assert against deterministic vectors or skip the embedding path
    /// entirely.
    pub fn with_embedding_service(self, embedding_service: Arc<dyn EmbeddingService>) -> Self {
        Self {
            embedding_service,
            ..self
        }
    }

    /// Test helper that replaces the Anthropic transport with a mock so a
    /// command-level test can assert end-to-end behavior without hitting
    /// the real API.
    pub fn with_extraction_overrides(
        self,
        anthropic_transport: Arc<dyn AnthropicTransport>,
        yt_dlp: Arc<dyn YtDlpRunner>,
    ) -> Self {
        Self {
            anthropic_transport,
            yt_dlp,
            ..self
        }
    }

    /// Test helper that replaces the streaming Anthropic transport with a
    /// mock so assistant-command tests can drive canned event sequences.
    pub fn with_streaming_anthropic_transport(
        self,
        streaming_anthropic_transport: Arc<dyn StreamingAnthropicTransport>,
    ) -> Self {
        Self {
            streaming_anthropic_transport,
            ..self
        }
    }
}

/// Resolve the host-machine-scoped AppData directory for Promptibrary.
///
/// Per spec §16 *Build, package, distribution* the canonical paths are:
/// - macOS: `~/Library/Application Support/com.promptibrary.app`
/// - Linux: `$XDG_CONFIG_HOME/promptibrary` (else `~/.config/promptibrary`)
/// - Windows: `%APPDATA%\promptibrary`
///
/// We use stdlib env probing rather than pulling in the `directories`
/// crate — every platform has one or two well-known env vars and a
/// `$HOME` fallback. If every probe fails we fall back to
/// `std::env::temp_dir().join("promptibrary")` so the app still starts
/// and the user can recover by setting `$HOME`.
fn default_app_data_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("com.promptibrary.app");
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(dir).join("promptibrary");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".config").join("promptibrary");
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(dir) = std::env::var("APPDATA") {
            return PathBuf::from(dir).join("promptibrary");
        }
    }
    std::env::temp_dir().join("promptibrary")
}

impl Default for AppServices {
    fn default() -> Self {
        Self::new()
    }
}

pub type ManagedState = Arc<AppServices>;
