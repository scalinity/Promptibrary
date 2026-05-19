//! Shared Tauri state container.
//!
//! L1 holds the vault + SQLite pool. L4 adds the secret store (used by the
//! extraction pipeline and X/Twitter fetcher). L5 will layer in the watcher
//! handle, settings store, PTY pool, embedding service.

use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{Mutex, RwLock};

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
    /// Backend-agnostic secret store. Prod uses `KeyringStore`; tests use
    /// `InMemorySecretStore` so they never touch the real keychain.
    pub secrets: Arc<dyn SecretStore>,
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
        Self {
            state: RwLock::default(),
            create_prompt_lock: Mutex::new(()),
            secrets,
        }
    }

    /// Constructor injecting a specific secret backend. Test-time helper to
    /// swap in `InMemorySecretStore`.
    pub fn with_secret_store(secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            state: RwLock::default(),
            create_prompt_lock: Mutex::new(()),
            secrets,
        }
    }
}

impl Default for AppServices {
    fn default() -> Self {
        Self::new()
    }
}

pub type ManagedState = Arc<AppServices>;
