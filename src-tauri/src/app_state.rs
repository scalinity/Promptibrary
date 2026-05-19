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
    /// Directory where yt-dlp drops transcript files. Created on demand.
    pub extraction_temp_dir: PathBuf,
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
        let yt_dlp: Arc<dyn YtDlpRunner> = Arc::new(RealYtDlpRunner);

        let extraction_temp_dir = std::env::temp_dir().join("promptibrary").join("extraction");

        Self {
            state: RwLock::default(),
            create_prompt_lock: Mutex::new(()),
            secrets,
            http,
            rate_limiter: Arc::new(RateLimiter::new()),
            yt_dlp,
            anthropic_transport,
            extraction_temp_dir,
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
        Self {
            state: RwLock::default(),
            create_prompt_lock: Mutex::new(()),
            secrets,
            http,
            rate_limiter: Arc::new(RateLimiter::new()),
            yt_dlp: Arc::new(RealYtDlpRunner),
            anthropic_transport,
            extraction_temp_dir: std::env::temp_dir().join("promptibrary").join("extraction"),
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
}

impl Default for AppServices {
    fn default() -> Self {
        Self::new()
    }
}

pub type ManagedState = Arc<AppServices>;
