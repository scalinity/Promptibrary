//! Secret keys and convenience accessors over `SecretStore`.
//!
//! V1 secret keys (mirrors `src/shared/types/settings.ts::SecretKey`):
//! - `anthropic_api_key` — Anthropic Messages API key (extraction pipeline).
//! - `x_bearer_token`    — X API v2 bearer token (thread reconstruction).
//!
//! All extraction code that needs a secret goes through `get_secret(store, key)`.
//! Frontend never sees the value — only `SecretStatus { exists, ... }`.

use serde::{Deserialize, Serialize};

use crate::error::Result;

use super::secret_store::SecretStore;

/// Account name used by `commands::system::probe_keychain` for its
/// write-read-delete round-trip on the keychain (SCA-731). This MUST
/// be disjoint from every `SecretKey::as_keychain_account()` return
/// value so a probe failure or interrupted process never overwrites
/// the user's real secrets.
///
/// We deliberately keep this OUT of the `SecretKey` enum: probe slots
/// shouldn't surface in `get_secret_status()` or in the spec §13
/// secrets list. A constant string under the same `KEYCHAIN_SERVICE`
/// is the simplest correct shape.
pub const KEYCHAIN_PROBE_ACCOUNT: &str = "__diagnostics_probe__";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretKey {
    AnthropicApiKey,
    XBearerToken,
}

impl SecretKey {
    pub fn as_keychain_account(self) -> &'static str {
        match self {
            SecretKey::AnthropicApiKey => "anthropic_api_key",
            SecretKey::XBearerToken => "x_bearer_token",
        }
    }

    pub fn all() -> &'static [SecretKey] {
        &[SecretKey::AnthropicApiKey, SecretKey::XBearerToken]
    }
}

pub fn get_secret(store: &dyn SecretStore, key: SecretKey) -> Result<Option<String>> {
    store.get(key.as_keychain_account())
}

pub fn set_secret(store: &dyn SecretStore, key: SecretKey, value: &str) -> Result<()> {
    store.set(key.as_keychain_account(), value)
}

pub fn clear_secret(store: &dyn SecretStore, key: SecretKey) -> Result<()> {
    store.delete(key.as_keychain_account())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::secret_store::InMemorySecretStore;

    #[test]
    fn round_trip_per_key() {
        let store = InMemorySecretStore::new();
        for k in SecretKey::all() {
            assert_eq!(get_secret(&store, *k).unwrap(), None);
            set_secret(&store, *k, "value").unwrap();
            assert_eq!(get_secret(&store, *k).unwrap(), Some("value".into()));
            clear_secret(&store, *k).unwrap();
            assert_eq!(get_secret(&store, *k).unwrap(), None);
        }
    }

    #[test]
    fn keys_serialize_snake_case() {
        let json = serde_json::to_string(&SecretKey::AnthropicApiKey).unwrap();
        assert_eq!(json, "\"anthropic_api_key\"");
        let json = serde_json::to_string(&SecretKey::XBearerToken).unwrap();
        assert_eq!(json, "\"x_bearer_token\"");
    }
}
