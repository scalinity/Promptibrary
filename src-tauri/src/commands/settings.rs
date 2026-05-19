//! `commands::settings` per spec §11.
//!
//! L4 lands the three secret commands (set/clear/get_status). The two
//! settings commands (get_settings / update_settings) remain L5 stubs.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use crate::app_state::ManagedState;
use crate::commands::not_yet_implemented_stub;
use crate::error::{AppError, AppErrorKind, Result};
use crate::settings::keychain::{self, SecretKey};
use crate::settings::secret_store::SecretStore;

#[tauri::command]
pub async fn get_settings() -> Result<Value> {
    not_yet_implemented_stub("commands::settings::get_settings")
}

#[tauri::command]
pub async fn update_settings(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::settings::update_settings")
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSecretInput {
    pub key: SecretKey,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearSecretInput {
    pub key: SecretKey,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SecretValidationStatus {
    Unknown,
    Valid,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretStatusDto {
    pub key: SecretKey,
    pub exists: bool,
    pub last_validated_at: Option<DateTime<Utc>>,
    pub validation_status: SecretValidationStatus,
}

impl SecretStatusDto {
    pub fn from_exists(key: SecretKey, exists: bool) -> Self {
        Self {
            key,
            exists,
            last_validated_at: None,
            validation_status: SecretValidationStatus::Unknown,
        }
    }
}

// --- Inner helpers — used by both #[tauri::command] wrappers and unit tests. ---

fn set_secret_inner(input: SetSecretInput, store: &dyn SecretStore) -> Result<SecretStatusDto> {
    let trimmed = input.value.trim();
    if trimmed.is_empty() {
        return Err(AppError::new(
            AppErrorKind::SettingsInvalid,
            "secret value cannot be empty",
        ));
    }
    keychain::set_secret(store, input.key, trimmed)?;
    Ok(SecretStatusDto::from_exists(input.key, true))
}

fn clear_secret_inner(input: ClearSecretInput, store: &dyn SecretStore) -> Result<SecretStatusDto> {
    keychain::clear_secret(store, input.key)?;
    Ok(SecretStatusDto::from_exists(input.key, false))
}

fn get_secret_status_inner(
    store: &dyn SecretStore,
) -> Result<HashMap<SecretKey, SecretStatusDto>> {
    SecretKey::all()
        .iter()
        .map(|key| {
            let exists = keychain::get_secret(store, *key)?.is_some();
            Ok((*key, SecretStatusDto::from_exists(*key, exists)))
        })
        .collect()
}

// --- Tauri commands ---

#[tauri::command]
pub async fn set_secret(
    input: SetSecretInput,
    services: State<'_, ManagedState>,
) -> Result<SecretStatusDto> {
    set_secret_inner(input, services.secrets.as_ref())
}

#[tauri::command]
pub async fn clear_secret(
    input: ClearSecretInput,
    services: State<'_, ManagedState>,
) -> Result<SecretStatusDto> {
    clear_secret_inner(input, services.secrets.as_ref())
}

#[tauri::command]
pub async fn get_secret_status(
    services: State<'_, ManagedState>,
) -> Result<HashMap<SecretKey, SecretStatusDto>> {
    get_secret_status_inner(services.secrets.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::secret_store::InMemorySecretStore;

    #[test]
    fn empty_value_rejected() {
        let store = InMemorySecretStore::new();
        let err = set_secret_inner(
            SetSecretInput {
                key: SecretKey::AnthropicApiKey,
                value: "   ".into(),
            },
            &store,
        )
        .unwrap_err();
        assert_eq!(err.kind, AppErrorKind::SettingsInvalid);
    }

    #[test]
    fn round_trip_set_status_clear() {
        let store = InMemorySecretStore::new();

        // Initially missing.
        let status = get_secret_status_inner(&store).unwrap();
        assert!(status.iter().all(|(_, s)| !s.exists));

        // Set anthropic key.
        let s = set_secret_inner(
            SetSecretInput {
                key: SecretKey::AnthropicApiKey,
                value: "sk-test".into(),
            },
            &store,
        )
        .unwrap();
        assert!(s.exists);

        // Status reflects it; X bearer still missing.
        let status = get_secret_status_inner(&store).unwrap();
        assert!(status.get(&SecretKey::AnthropicApiKey).unwrap().exists);
        assert!(!status.get(&SecretKey::XBearerToken).unwrap().exists);

        // Clear is idempotent.
        clear_secret_inner(
            ClearSecretInput {
                key: SecretKey::AnthropicApiKey,
            },
            &store,
        )
        .unwrap();
        clear_secret_inner(
            ClearSecretInput {
                key: SecretKey::AnthropicApiKey,
            },
            &store,
        )
        .unwrap();
        let status = get_secret_status_inner(&store).unwrap();
        assert!(status.iter().all(|(_, s)| !s.exists));
    }
}
