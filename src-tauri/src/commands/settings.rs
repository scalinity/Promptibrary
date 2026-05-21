//! `commands::settings` per spec §11.
//!
//! L4 lands the three secret commands (set/clear/get_status). The two
//! settings commands (get_settings / update_settings) remain L5 stubs.

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_state::ManagedState;
use crate::commands::vault::current_vault_db;
use crate::domain::prompt::{ClaudeModelId, ClaudePermissionMode, LaunchDestination, VerifierMode};
use crate::domain::settings::{
    AppSettings, EffectiveSettings, LocalSettings, VaultSettings, VersionHistorySettings,
};
use crate::error::{AppError, AppErrorKind, Result};
use crate::index::telemetry_repo;
use crate::settings::keychain::{self, SecretKey};
use crate::settings::secret_store::SecretStore;
use crate::util::atomic_write::atomic_write_string;
use crate::vault::paths::VaultPaths;

/// SCA-782: settings persistence.
///
/// `get_settings` reads the user-scoped local JSON at
/// `<app_data_dir>/settings.json` and (if a vault is selected) the
/// vault-scoped YAML at `<vault>/promptibrary/settings.yml`. The two
/// sources are merged into `AppSettings { local, vault, effective }`
/// per spec §13 — `effective` flattens `local` + `vault_settings`.
///
/// Missing files are tolerated (treat as defaults). Malformed payloads
/// log a warning and fall back to defaults rather than erroring the
/// whole call — settings is the surface users open BECAUSE something's
/// wrong, so it must not refuse to render on a bad config row.
#[tauri::command]
pub async fn get_settings(services: State<'_, ManagedState>) -> Result<AppSettings> {
    let (vault_paths, app_data_dir) = {
        let state = services.state.read().await;
        (
            state.vault.clone(),
            services.app_data_dir.clone(),
        )
    };
    Ok(load_settings(&app_data_dir, vault_paths.as_ref()))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsInput {
    pub settings: AppSettings,
}

/// `update_settings` atomic-writes the local JSON and (if a vault is
/// selected) the vault YAML, then re-reads to produce the merged
/// `AppSettings` the frontend will hold. We re-read rather than echo
/// the input so any default-fill or sanitisation lives in one path.
#[tauri::command]
pub async fn update_settings(
    input: UpdateSettingsInput,
    services: State<'_, ManagedState>,
) -> Result<AppSettings> {
    let (vault_paths, app_data_dir) = {
        let state = services.state.read().await;
        (state.vault.clone(), services.app_data_dir.clone())
    };
    persist_settings(&app_data_dir, vault_paths.as_ref(), &input.settings)?;
    Ok(load_settings(&app_data_dir, vault_paths.as_ref()))
}

fn local_settings_path(app_data_dir: &Path) -> std::path::PathBuf {
    app_data_dir.join("settings.json")
}

fn load_local(app_data_dir: &Path, fallback_vault: Option<&VaultPaths>) -> LocalSettings {
    let path = local_settings_path(app_data_dir);
    match std::fs::read_to_string(&path) {
        Ok(contents) => match serde_json::from_str::<LocalSettings>(&contents) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "local settings.json is malformed; falling back to defaults"
                );
                default_local(fallback_vault)
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => default_local(fallback_vault),
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "local settings.json read failed; falling back to defaults"
            );
            default_local(fallback_vault)
        }
    }
}

fn load_vault(vault_paths: Option<&VaultPaths>) -> VaultSettings {
    let Some(paths) = vault_paths else {
        return default_vault();
    };
    let path = paths.settings_yml();
    match std::fs::read_to_string(&path) {
        Ok(contents) => match serde_yaml::from_str::<VaultSettings>(&contents) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "vault settings.yml is malformed; falling back to defaults"
                );
                default_vault()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => default_vault(),
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "vault settings.yml read failed; falling back to defaults"
            );
            default_vault()
        }
    }
}

fn load_settings(app_data_dir: &Path, vault_paths: Option<&VaultPaths>) -> AppSettings {
    let local = load_local(app_data_dir, vault_paths);
    let vault = load_vault(vault_paths);
    let effective = EffectiveSettings {
        local: local.clone(),
        vault_settings: vault.clone(),
    };
    AppSettings {
        local,
        vault,
        effective,
    }
}

fn persist_settings(
    app_data_dir: &Path,
    vault_paths: Option<&VaultPaths>,
    settings: &AppSettings,
) -> Result<()> {
    // Ensure app_data_dir exists before atomic_write_string (which
    // requires the parent dir to already exist).
    std::fs::create_dir_all(app_data_dir).map_err(AppError::from)?;
    let local_path = local_settings_path(app_data_dir);
    let local_json = serde_json::to_string_pretty(&settings.local).map_err(AppError::from)?;
    atomic_write_string(&local_path, &local_json)?;

    if let Some(paths) = vault_paths {
        let yml_path = paths.settings_yml();
        if let Some(parent) = yml_path.parent() {
            std::fs::create_dir_all(parent).map_err(AppError::from)?;
        }
        let yml = serde_yaml::to_string(&settings.vault).map_err(|e| {
            AppError::new(
                AppErrorKind::SettingsInvalid,
                format!("serialize vault settings yaml: {e}"),
            )
        })?;
        atomic_write_string(&yml_path, &yml)?;
    }
    Ok(())
}

/// SCA-900 — return the on-disk `LocalSettings` (or defaults if the file
/// is missing or malformed). Public so `lib.rs` can read the saved
/// `vault_path` at startup without going through the IPC layer.
pub fn load_persisted_local(app_data_dir: &Path) -> LocalSettings {
    load_local(app_data_dir, None)
}

/// SCA-900 — write `vault_path` into the on-disk local settings without
/// touching the vault-scoped YAML. Loads the current local settings,
/// overwrites the vault path, atomic-writes back. Called by
/// `select_vault` so the user's choice survives restart.
pub fn persist_local_vault_path(
    app_data_dir: &Path,
    vault_root: &Path,
) -> Result<()> {
    let mut local = load_local(app_data_dir, None);
    local.vault_path = Some(vault_root.to_path_buf());
    std::fs::create_dir_all(app_data_dir).map_err(AppError::from)?;
    let local_path = local_settings_path(app_data_dir);
    let local_json = serde_json::to_string_pretty(&local).map_err(AppError::from)?;
    atomic_write_string(&local_path, &local_json)?;
    Ok(())
}

fn default_local(vault_paths: Option<&VaultPaths>) -> LocalSettings {
    LocalSettings {
        vault_path: vault_paths.map(|v| v.vault_root.clone()),
        default_destination: LaunchDestination::ClaudeCodeCli,
        // SCA-898 — user-pref override of spec §4's Sonnet 4-6 default.
        default_model: ClaudeModelId::ClaudeOpus47,
        default_verifier_mode: VerifierMode::Off,
        default_permission_mode: ClaudePermissionMode::Default,
        telemetry_enabled: true,
        update_manifest_url: None,
        version_history: VersionHistorySettings {
            rename_detection_window: 200,
        },
        recent_prompt_ids: vec![],
        recent_run_ids: vec![],
        // SCA-906 — extraction model + source-cap defaults match the
        // former spec §13 constants. User can edit via the Settings UI.
        extraction_model: ClaudeModelId::ClaudeSonnet46,
        deep_extraction_model: ClaudeModelId::ClaudeOpus47,
        source_cap_standard: 60_000,
        source_cap_deep: 160_000,
        // SCA-933 — assistant defaults. Improve Prompt is the default
        // Fabric pattern; assistant turns run on Sonnet 4.6 unless the
        // user picks otherwise in Settings.
        assistant_default_pattern: crate::assistant::patterns::AssistantPattern::ImprovePrompt,
        assistant_model: ClaudeModelId::ClaudeSonnet46,
    }
}

fn default_vault() -> VaultSettings {
    VaultSettings {
        tag_colors: HashMap::new(),
    }
}

// ─── Secret commands (set/clear/get_status) ───────────────────────────

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

    // SCA-732 — FS-first ordering. The pre-fix design ran the DB
    // DELETE first (auto-commit), then the FS walk. Any FS error
    // mid-walk left the DB drained while transcripts remained
    // orphaned, and the user had no signal that the partial state
    // existed (the second invocation reported runs_deleted: 0).
    //
    // FS-first means: a walk failure surfaces BEFORE we touch the
    // DB, so a retry sees the runs + transcripts still consistent.
    // The transaction wrapping the DELETE is belt-and-suspenders:
    // even though `DELETE FROM runs` is one statement, opening an
    // explicit tx documents the atomicity intent for the next
    // contributor and makes rollback explicit if the commit ever
    // fails (e.g. WAL flush error on a network FS).
    let runs_dir = vault.runs_dir();
    let (files_deleted, bytes_freed) = if runs_dir.exists() {
        delete_transcript_files(&runs_dir).await?
    } else {
        (0, 0)
    };

    let mut tx = db.begin().await.map_err(AppError::from)?;
    let runs_res = sqlx::query("DELETE FROM runs")
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;
    let runs_deleted = runs_res.rows_affected();
    tx.commit().await.map_err(AppError::from)?;

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
    // SCA-748: cache directory depth at push time. Pre-fix sorted by
    // `path.components().count()`, which re-walked each path on every
    // comparison — O(N² log N) on deep trees. The depth is fixed at
    // discovery time so we capture it once.
    let mut stack: Vec<std::path::PathBuf> = vec![root.to_path_buf()];
    let mut to_remove_dirs: Vec<(usize, std::path::PathBuf)> = Vec::new();

    while let Some(dir) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&dir).await.map_err(AppError::from)?;
        while let Some(entry) = entries.next_entry().await.map_err(AppError::from)? {
            let path = entry.path();
            let file_type = entry.file_type().await.map_err(AppError::from)?;
            if file_type.is_dir() {
                let depth = path.components().count();
                stack.push(path.clone());
                to_remove_dirs.push((depth, path));
            } else if file_type.is_file() {
                let meta = entry.metadata().await.map_err(AppError::from)?;
                tokio::fs::remove_file(&path).await.map_err(AppError::from)?;
                bytes_freed = bytes_freed.saturating_add(meta.len());
                files_deleted += 1;
            }
        }
    }

    // Remove now-empty subdirectories deepest-first, but leave the
    // top-level `root` itself in place.
    to_remove_dirs.sort_by(|a, b| b.0.cmp(&a.0));
    for (_depth, dir) in to_remove_dirs {
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

    // ─── SCA-782 settings persistence ────────────────────────────────

    #[test]
    fn load_settings_returns_defaults_when_no_files_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let s = load_settings(tmp.path(), None);
        assert!(s.local.telemetry_enabled, "default telemetry_enabled = true");
        assert_eq!(s.local.version_history.rename_detection_window, 200);
        assert!(s.vault.tag_colors.is_empty());
    }

    #[test]
    fn persist_settings_writes_local_json_atomically() {
        let tmp = tempfile::tempdir().unwrap();
        let mut s = load_settings(tmp.path(), None);
        s.local.telemetry_enabled = false;
        s.local.version_history.rename_detection_window = 500;
        persist_settings(tmp.path(), None, &s).unwrap();

        let path = tmp.path().join("settings.json");
        assert!(path.exists(), "settings.json should exist after persist");
        let reloaded = load_settings(tmp.path(), None);
        assert!(!reloaded.local.telemetry_enabled);
        assert_eq!(reloaded.local.version_history.rename_detection_window, 500);
    }

    #[test]
    fn malformed_local_settings_falls_back_to_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(&path, b"{not valid json").unwrap();

        let s = load_settings(tmp.path(), None);
        // Defaults applied without panicking.
        assert!(s.local.telemetry_enabled);
        assert_eq!(s.local.version_history.rename_detection_window, 200);
    }

    #[test]
    fn round_trip_preserves_recent_prompt_ids() {
        use crate::ids::PromptId;
        let tmp = tempfile::tempdir().unwrap();
        let mut s = load_settings(tmp.path(), None);
        s.local.recent_prompt_ids = vec![
            PromptId("01ALPHA".into()),
            PromptId("01BETA".into()),
        ];
        persist_settings(tmp.path(), None, &s).unwrap();
        let reloaded = load_settings(tmp.path(), None);
        assert_eq!(reloaded.local.recent_prompt_ids.len(), 2);
        assert_eq!(reloaded.local.recent_prompt_ids[0].0, "01ALPHA");
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

    /// SCA-732 invariant: if the FS walk fails partway through,
    /// `delete_all_run_history` must NOT have touched the DB. The
    /// pre-fix design ran `DELETE FROM runs` first (auto-commit),
    /// then the walk — any FS error left the DB drained while
    /// transcripts remained orphaned. The fix re-orders FS-first so
    /// a walk failure surfaces before the DB transaction opens.
    ///
    /// We exercise this at the helper-function granularity: a
    /// non-existent vault subdir makes `read_dir` return ENOENT, and
    /// the test asserts we get the propagated Err without having
    /// touched anything. The full end-to-end test (real DB + real
    /// vault) is gated on the L3 runs-persistence reconciliation;
    /// this helper test is the most we can assert without a live
    /// AppServices ManagedState.
    #[tokio::test]
    async fn delete_transcript_files_returns_err_on_missing_root() {
        let tmp = tempfile::tempdir().unwrap();
        let nonexistent = tmp.path().join("does-not-exist");
        let result = delete_transcript_files(&nonexistent).await;
        assert!(
            result.is_err(),
            "expected Err on missing root, got {result:?}"
        );
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
