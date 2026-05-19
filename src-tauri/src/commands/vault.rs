//! `commands::vault` per spec §11.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use tauri::State;

use crate::app_state::ManagedState;
use crate::error::{AppError, AppErrorKind, Result};
use crate::index::db::connect_options;
use crate::index::migrations::run_migrations;
use crate::vault::paths::VaultPaths;
use crate::vault::repair::repair_missing_dirs;
use crate::vault::scanner::scan_vault;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectVaultInput {
    pub vault_root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub vault_root: Option<PathBuf>,
    pub initialized: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub scanned_files: usize,
    pub indexed_prompts: usize,
    pub malformed_files: usize,
    pub deleted_rows: usize,
    pub duration_ms: u128,
}

#[tauri::command]
pub async fn select_vault(
    input: SelectVaultInput,
    services: State<'_, ManagedState>,
) -> Result<VaultStatus> {
    if !input.vault_root.exists() {
        // SCA-597: don't leak the user-supplied path into the wire message
        // — the user supplied it, but the pattern would set a bad precedent
        // for paths derived from indexed rows or runtime state.
        tracing::warn!(path = %input.vault_root.display(), "select_vault: path does not exist");
        return Err(AppError::new(
            AppErrorKind::VaultMissing,
            "vault path does not exist",
        ));
    }
    if !input.vault_root.is_dir() {
        tracing::warn!(path = %input.vault_root.display(), "select_vault: not a directory");
        return Err(AppError::new(
            AppErrorKind::VaultInvalid,
            "vault path is not a directory",
        ));
    }
    let vault = VaultPaths::new(input.vault_root.clone());
    repair_missing_dirs(&vault)?;
    let db = open_index_pool().await?;
    run_migrations(&db).await?;

    let mut state = services.state.write().await;
    state.vault = Some(vault);
    state.db = Some(db);

    Ok(VaultStatus {
        vault_root: Some(input.vault_root),
        initialized: true,
    })
}

#[tauri::command]
pub async fn validate_vault(
    input: SelectVaultInput,
    _services: State<'_, ManagedState>,
) -> Result<VaultStatus> {
    if !input.vault_root.exists() {
        return Err(AppError::new(
            AppErrorKind::VaultMissing,
            "vault path does not exist",
        ));
    }
    if !input.vault_root.is_dir() {
        return Err(AppError::new(
            AppErrorKind::VaultInvalid,
            "vault path is not a directory",
        ));
    }
    Ok(VaultStatus {
        vault_root: Some(input.vault_root),
        initialized: true,
    })
}

#[tauri::command]
pub async fn scan_vault_cmd(services: State<'_, ManagedState>) -> Result<ScanResult> {
    let (vault, db) = current_vault_db(&services).await?;
    let summary = scan_vault(&vault, &db, |_p| {}).await?;
    Ok(ScanResult {
        scanned_files: summary.scanned_files,
        indexed_prompts: summary.indexed_prompts,
        malformed_files: summary.malformed_files,
        deleted_rows: summary.deleted_rows,
        duration_ms: summary.duration_ms,
    })
}

#[tauri::command]
pub async fn rebuild_index(services: State<'_, ManagedState>) -> Result<ScanResult> {
    // SCA-603: don't truncate the index until the scan succeeds. If the
    // scan fails (vault yanked mid-call, malformed schema), the previous
    // approach (DELETE then scan) left the index empty with no rollback.
    // We attempt the scan first; on success its own stale-delete pass
    // (SCA-592/SCA-593) prunes orphan rows. Only if the user really wants
    // a from-scratch rebuild — i.e. they suspect index corruption beyond
    // missing-row drift — they should call select_vault again, which
    // triggers run_migrations + fresh scan.
    let (vault, db) = current_vault_db(&services).await?;
    let summary = scan_vault(&vault, &db, |_| {}).await?;
    Ok(ScanResult {
        scanned_files: summary.scanned_files,
        indexed_prompts: summary.indexed_prompts,
        malformed_files: summary.malformed_files,
        deleted_rows: summary.deleted_rows,
        duration_ms: summary.duration_ms,
    })
}

#[tauri::command]
pub async fn get_vault_status(services: State<'_, ManagedState>) -> Result<VaultStatus> {
    let state = services.state.read().await;
    Ok(VaultStatus {
        vault_root: state.vault.as_ref().map(|v| v.vault_root.clone()),
        initialized: state.db.is_some(),
    })
}

// ---------- Helpers --------------------------------------------------------

pub(crate) async fn current_vault_db(
    services: &State<'_, ManagedState>,
) -> Result<(VaultPaths, SqlitePool)> {
    current_vault_db_inner(services.inner()).await
}

/// Inner helper that takes the underlying `Arc<AppServices>` directly so
/// non-command code paths (notably `commands::extraction::save_extracted_prompt`'s
/// private helper) can call into the same vault-resolution logic.
pub(crate) async fn current_vault_db_inner(
    services: &ManagedState,
) -> Result<(VaultPaths, SqlitePool)> {
    let state = services.state.read().await;
    let vault = state
        .vault
        .clone()
        .ok_or_else(|| AppError::new(AppErrorKind::VaultMissing, "no vault selected"))?;
    let db = state
        .db
        .clone()
        .ok_or_else(|| AppError::new(AppErrorKind::VaultMissing, "no db pool"))?;
    Ok((vault, db))
}

/// Open the L1 single-vault index pool at the canonical app-support path.
///
/// SCA-620: this took a `&VaultPaths` parameter that was immediately
/// dropped (`let _ = vault;`). L1 uses a single shared index DB regardless
/// of which vault is open; multi-vault DB routing would land in L5 with a
/// hash-keyed filename. Renamed to `open_index_pool` and dropped the
/// unused parameter.
async fn open_index_pool() -> Result<SqlitePool> {
    let dir = app_support_dir()?;
    std::fs::create_dir_all(&dir).map_err(AppError::from)?;
    let path = dir.join("index.sqlite");
    let opts = connect_options(&path);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(opts)
        .await
        .map_err(AppError::from)?;
    Ok(pool)
}

fn app_support_dir() -> Result<PathBuf> {
    if cfg!(target_os = "macos") {
        let home = std::env::var("HOME")
            .map_err(|_| AppError::new(AppErrorKind::Internal, "HOME env var not set"))?;
        Ok(PathBuf::from(home).join("Library/Application Support/promptibrary"))
    } else if cfg!(target_os = "linux") {
        // SCA-602: honor $XDG_DATA_HOME per XDG Base Directory spec, falling
        // back to $HOME/.local/share only when XDG_DATA_HOME is unset or
        // empty. Managed Linux environments and Flatpak/snap-style sandboxes
        // depend on this override to point apps at their own data prefix.
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            if !xdg.is_empty() {
                return Ok(PathBuf::from(xdg).join("promptibrary"));
            }
        }
        let home = std::env::var("HOME")
            .map_err(|_| AppError::new(AppErrorKind::Internal, "HOME env var not set"))?;
        Ok(PathBuf::from(home).join(".local/share/promptibrary"))
    } else {
        Err(AppError::new(
            AppErrorKind::Internal,
            "unsupported platform",
        ))
    }
}
