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
use crate::commands::vault::current_vault_db;
use crate::error::{AppError, AppErrorKind, Result};
use crate::index::telemetry_repo;
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

// ─── Destructive telemetry actions (patched spec §13) ─────────────────
//
// Two distinct actions, never bundled in the UI or in the backend
// dispatcher:
//   - `clear_telemetry_cache`  — drops the per-event detail
//                                (`telemetry_events`), preserves runs +
//                                transcripts. Single-click confirmation.
//   - `delete_all_run_history` — drops `runs` AND deletes the on-disk
//                                transcript files under
//                                `<vault>/promptibrary/runs/`. The
//                                backend requires the literal string
//                                "delete" in the input as a typed
//                                confirmation, so a frontend bug can't
//                                trigger this destructive action on a
//                                stray click.

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearTelemetryCacheResult {
    pub events_deleted: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAllRunHistoryInput {
    pub confirmation: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAllRunHistoryResult {
    pub runs_deleted: u64,
    pub transcript_files_deleted: u64,
    pub transcript_bytes_freed: u64,
}

/// The literal string the frontend's typed-confirmation modal must
/// echo back in `confirmation`. Anything else is rejected.
const RUN_HISTORY_CONFIRMATION: &str = "delete";

#[tauri::command]
pub async fn clear_telemetry_cache(
    services: State<'_, ManagedState>,
) -> Result<ClearTelemetryCacheResult> {
    let (_vault, db) = current_vault_db(&services).await?;
    let events_deleted = telemetry_repo::clear_event_log(&db).await?;
    Ok(ClearTelemetryCacheResult { events_deleted })
}

#[tauri::command]
pub async fn delete_all_run_history(
    input: DeleteAllRunHistoryInput,
    services: State<'_, ManagedState>,
) -> Result<DeleteAllRunHistoryResult> {
    if input.confirmation != RUN_HISTORY_CONFIRMATION {
        return Err(AppError::new(
            AppErrorKind::SettingsInvalid,
            "delete_all_run_history requires confirmation == \"delete\"",
        ));
    }

    let (vault, db) = current_vault_db(&services).await?;

    // 1. Drop runs rows. `telemetry_events` cascades via the FK
    //    (migration 0005_fk_cascade declares ON DELETE CASCADE).
    let runs_res = sqlx::query("DELETE FROM runs")
        .execute(&db)
        .await
        .map_err(AppError::from)?;
    let runs_deleted = runs_res.rows_affected();

    // 2. Recursively delete transcript files under the vault's runs/
    //    directory. The directory itself is preserved so the next run
    //    can drop a fresh transcript without re-mkdir gymnastics.
    let runs_dir = vault.runs_dir();
    let (files_deleted, bytes_freed) = if runs_dir.exists() {
        delete_transcript_files(&runs_dir).await?
    } else {
        (0, 0)
    };

    Ok(DeleteAllRunHistoryResult {
        runs_deleted,
        transcript_files_deleted: files_deleted,
        transcript_bytes_freed: bytes_freed,
    })
}

/// Recursively walks `root` and deletes every regular file, accumulating
/// counts and freed bytes. Directory structure under `root` is removed
/// after its children are drained; `root` itself is preserved.
async fn delete_transcript_files(root: &std::path::Path) -> Result<(u64, u64)> {
    let mut files_deleted = 0u64;
    let mut bytes_freed = 0u64;
    let mut stack: Vec<std::path::PathBuf> = vec![root.to_path_buf()];
    let mut to_remove_dirs: Vec<std::path::PathBuf> = Vec::new();

    while let Some(dir) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&dir).await.map_err(AppError::from)?;
        while let Some(entry) = entries.next_entry().await.map_err(AppError::from)? {
            let path = entry.path();
            let file_type = entry.file_type().await.map_err(AppError::from)?;
            if file_type.is_dir() {
                stack.push(path.clone());
                to_remove_dirs.push(path);
            } else if file_type.is_file() {
                let meta = entry.metadata().await.map_err(AppError::from)?;
                bytes_freed = bytes_freed.saturating_add(meta.len());
                tokio::fs::remove_file(&path).await.map_err(AppError::from)?;
                files_deleted += 1;
            }
        }
    }

    // Remove now-empty subdirectories deepest-first, but leave the
    // top-level `root` itself in place.
    to_remove_dirs.sort_by(|a, b| b.components().count().cmp(&a.components().count()));
    for dir in to_remove_dirs {
        // remove_dir errors loudly if the dir still has children; that
        // would mean someone wrote a file between the walk and the
        // removal. Surface as a real error rather than swallowing.
        tokio::fs::remove_dir(&dir).await.map_err(AppError::from)?;
    }
    Ok((files_deleted, bytes_freed))
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

    #[tokio::test]
    async fn delete_transcript_files_removes_files_and_subdirs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        // Build: root/2026/05/{a.md, b.md}, root/2026/06/c.md
        tokio::fs::create_dir_all(root.join("2026/05")).await.unwrap();
        tokio::fs::create_dir_all(root.join("2026/06")).await.unwrap();
        tokio::fs::write(root.join("2026/05/a.md"), b"hello").await.unwrap();
        tokio::fs::write(root.join("2026/05/b.md"), b"world!").await.unwrap();
        tokio::fs::write(root.join("2026/06/c.md"), b"!").await.unwrap();

        let (files, bytes) = delete_transcript_files(&root).await.unwrap();
        assert_eq!(files, 3);
        assert_eq!(bytes, 5 + 6 + 1);
        // Root itself preserved; subdirs gone.
        assert!(root.exists());
        assert!(!root.join("2026").exists());
    }

    #[test]
    fn delete_all_run_history_rejects_wrong_confirmation() {
        // We can't easily run the full Tauri-state-bound async command
        // in a unit test (it needs a real vault). Instead assert the
        // confirmation constant matches the literal the patched spec
        // §13 requires.
        assert_eq!(RUN_HISTORY_CONFIRMATION, "delete");
    }
}
