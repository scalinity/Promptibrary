//! Secret storage abstraction.
//!
//! Per spec §13 secrets live in the OS keychain (service
//! `com.promptibrary.secrets`) and never appear on disk. The `SecretStore`
//! trait lets tests swap in an in-memory backend so they don't need a real
//! keychain (D-Bus, login keychain, etc.). Production wires `KeychainStore`,
//! which routes to `apple-native-keyring-store` on macOS,
//! `dbus-secret-service-keyring-store` on Linux, and
//! `windows-native-keyring-store` on Windows via `keyring-core`'s trait.
//!
//! Naming: the public verbs are `get` / `set` / `delete` to match the keyring
//! crate's terminology; the IPC commands speak `set_secret`/`clear_secret`/
//! `get_secret_status` (spec §11).

use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{AppError, AppErrorKind, Result};

/// V1 secret service identifier. Per spec §13 *Secrets*.
pub const KEYCHAIN_SERVICE: &str = "com.promptibrary.secrets";

/// Backend-agnostic secret store. All methods are infallible on the "missing"
/// path — `get` returns `Ok(None)`, `delete` is idempotent. Backend faults
/// (D-Bus down, locked keychain, etc.) surface as `KeychainError`.
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

fn keychain_err(e: impl std::fmt::Display) -> AppError {
    AppError::new(AppErrorKind::KeychainError, format!("keychain: {e}"))
}

/// Production backend — the OS keychain via `keyring-core`. Only built for
/// supported targets; non-test code paths must check `keychain_available()`
/// first.
#[cfg(all(not(test), target_os = "macos"))]
pub struct KeychainStore;

#[cfg(all(not(test), target_os = "macos"))]
impl KeychainStore {
    pub fn new() -> Self {
        Self
    }

    fn entry(&self, key: &str) -> Result<keyring_core::Entry> {
        use apple_native_keyring_store::keychain::{Cred, MacKeychainDomain};
        Cred::build(MacKeychainDomain::User, KEYCHAIN_SERVICE, key).map_err(keychain_err)
    }
}

#[cfg(all(not(test), target_os = "linux"))]
pub struct KeychainStore;

#[cfg(all(not(test), target_os = "linux"))]
impl KeychainStore {
    pub fn new() -> Self {
        Self
    }

    fn entry(&self, key: &str) -> Result<keyring_core::Entry> {
        use dbus_secret_service_keyring_store::Cred;
        Cred::build(KEYCHAIN_SERVICE, key).map_err(keychain_err)
    }
}

#[cfg(all(not(test), target_os = "windows"))]
pub struct KeychainStore;

#[cfg(all(not(test), target_os = "windows"))]
impl KeychainStore {
    pub fn new() -> Self {
        Self
    }

    fn entry(&self, key: &str) -> Result<keyring_core::Entry> {
        use windows_native_keyring_store::Cred;
        Cred::build(KEYCHAIN_SERVICE, key).map_err(keychain_err)
    }
}

// One impl-of-trait that all platform builds share. Each platform's
// `KeychainStore::entry(...)` returns the platform-specific Entry; the
// trait surface is identical.
#[cfg(all(
    not(test),
    any(target_os = "macos", target_os = "linux", target_os = "windows")
))]
impl SecretStore for KeychainStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        use keyring_core::error::Error::NoEntry;
        let entry = self.entry(key)?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(NoEntry) => Ok(None),
            Err(e) => Err(keychain_err(e)),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        let entry = self.entry(key)?;
        entry.set_password(value).map_err(keychain_err)
    }

    fn delete(&self, key: &str) -> Result<()> {
        use keyring_core::error::Error::NoEntry;
        let entry = self.entry(key)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(NoEntry) => Ok(()),
            Err(e) => Err(keychain_err(e)),
        }
    }
}

/// In-memory secret store. Used in tests (compiled when `cfg(test)`) and in
/// any build that hits an unsupported target — keeps the code path safe.
#[derive(Default)]
pub struct InMemorySecretStore {
    map: Mutex<HashMap<String, String>>,
}

impl InMemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for InMemorySecretStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.map.lock().unwrap().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        self.map
            .lock()
            .unwrap()
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<()> {
        self.map.lock().unwrap().remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_round_trip() {
        let store = InMemorySecretStore::new();
        assert_eq!(store.get("k").unwrap(), None);
        store.set("k", "v1").unwrap();
        assert_eq!(store.get("k").unwrap(), Some("v1".into()));
        store.set("k", "v2").unwrap();
        assert_eq!(store.get("k").unwrap(), Some("v2".into()));
    }

    #[test]
    fn in_memory_delete_is_idempotent() {
        let store = InMemorySecretStore::new();
        store.delete("missing").unwrap();
        store.set("k", "v").unwrap();
        store.delete("k").unwrap();
        store.delete("k").unwrap();
        assert_eq!(store.get("k").unwrap(), None);
    }
}
