//! Shared Tauri state container.
//!
//! L1 holds:
//!   - `vault`: the currently-selected `VaultPaths`, populated by
//!     `commands::vault::select_vault`. `None` until a vault is selected.
//!   - `db`: the SQLite pool tied to the currently-selected vault. The
//!     pool itself lives outside the vault (app-support dir per spec §4)
//!     but is bound to the vault's lifecycle.
//!
//! Subsequent layers add: watcher handle (L1.12 / L2), settings store
//! (L5), keychain (L5), PTY pool (L3), embedding service (L5).

use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{Mutex, RwLock};

use crate::vault::paths::VaultPaths;

#[derive(Default)]
pub struct AppServices {
    pub state: RwLock<MutableState>,
    /// Global serialization gate for create_prompt. Without this, two
    /// concurrent IPC calls can both observe the same slug as free,
    /// derive the same vault_path, and race the atomic_write rename →
    /// duplicate-vault_path rows + content corruption. SCA-589.
    pub create_prompt_lock: Mutex<()>,
}

#[derive(Default)]
pub struct MutableState {
    pub vault: Option<VaultPaths>,
    pub db: Option<SqlitePool>,
}

impl AppServices {
    pub fn new() -> Self {
        Self::default()
    }
}

pub type ManagedState = Arc<AppServices>;
