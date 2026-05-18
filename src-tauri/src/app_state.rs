//! Shared Tauri state container.
//!
//! L0 scaffold — `AppServices` is the holder for indexes, settings, PTY pool,
//! etc. that subsequent layers populate. Today it holds nothing.

#[derive(Default)]
pub struct AppServices {
    // TODO(L1+): SQLite pool, vault watcher, settings store, secrets keychain
    // handle, PTY pool, embedding service, etc. all land here.
}

impl AppServices {
    pub fn new() -> Self {
        Self::default()
    }
}

pub type ManagedState = std::sync::Arc<AppServices>;
