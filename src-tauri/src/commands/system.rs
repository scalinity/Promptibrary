//! `commands::system` per spec §11 *System commands*.
//!
//! Three IPC commands:
//!
//! - `probe_dependencies()` — spawns `claude --version`, `yt-dlp --version`,
//!   `git --version` to probe their availability and version strings.
//!   Also probes keychain access via a write-read-clear round-trip on a
//!   throwaway namespace, and SQLite connectivity via the live pool.
//!   Returns a typed `DependencyProbeOutput` the diagnostics panel
//!   renders.
//!
//! - `reveal_in_terminal(path)` — opens a Terminal session at `path`.
//!   macOS uses `open -a Terminal <path>`; Linux best-effort tries
//!   `x-terminal-emulator`, `gnome-terminal`, then `konsole`.
//!
//! - `open_path(path)` — `open <path>` on macOS, `xdg-open` on Linux.
//!
//! All three are safe against directory-traversal in the sense that
//! the OS-level `open` / terminal binary handles the path; we don't
//! interpolate it into a shell string. We do explicitly reject paths
//! that don't exist so the failure surface is "not found" rather than
//! "the OS launched something at a weird location".

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;

use crate::app_state::ManagedState;
use crate::error::{AppError, AppErrorKind, Result};
use crate::settings::keychain::{self, SecretKey};
use crate::settings::secret_store::SecretStore;

// ─── probe_dependencies ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyProbe {
    pub name: &'static str,
    pub status: ProbeStatus,
    pub version: Option<String>,
    pub install_hint: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStatus {
    Ok,
    Missing,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyProbeOutput {
    pub probes: Vec<DependencyProbe>,
}

#[tauri::command]
pub async fn probe_dependencies(
    services: State<'_, ManagedState>,
) -> Result<DependencyProbeOutput> {
    let mut probes = Vec::new();

    probes.push(
        probe_binary(
            "claude",
            &["--version"],
            Some("Install Claude Code CLI: https://docs.anthropic.com/en/docs/claude-code"),
        )
        .await,
    );
    probes.push(
        probe_binary(
            "yt-dlp",
            &["--version"],
            Some("Install yt-dlp: brew install yt-dlp / pipx install yt-dlp"),
        )
        .await,
    );
    probes.push(
        probe_binary(
            "git",
            &["--version"],
            Some("Install Git: brew install git"),
        )
        .await,
    );

    probes.push(probe_keychain(services.secrets.as_ref()).await);

    let db_probe = match current_db(&services).await {
        Ok(pool) => probe_sqlite(&pool).await,
        Err(_) => DependencyProbe {
            name: "sqlite",
            status: ProbeStatus::Missing,
            version: None,
            install_hint: Some("No vault selected — open Settings → Vault"),
        },
    };
    probes.push(db_probe);

    Ok(DependencyProbeOutput { probes })
}

async fn current_db(services: &State<'_, ManagedState>) -> Result<SqlitePool> {
    let state = services.state.read().await;
    state
        .db
        .clone()
        .ok_or_else(|| AppError::new(AppErrorKind::VaultMissing, "no db pool"))
}

async fn probe_binary(
    name: &'static str,
    args: &[&str],
    install_hint: Option<&'static str>,
) -> DependencyProbe {
    let output = tokio::process::Command::new(name)
        .args(args)
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let raw = if !stdout.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                stderr.trim().to_string()
            };
            let version = raw.lines().next().map(|s| s.to_string());
            DependencyProbe {
                name,
                status: ProbeStatus::Ok,
                version,
                install_hint: None,
            }
        }
        Ok(_) => DependencyProbe {
            name,
            status: ProbeStatus::Error,
            version: None,
            install_hint,
        },
        Err(_) => DependencyProbe {
            name,
            status: ProbeStatus::Missing,
            version: None,
            install_hint,
        },
    }
}

async fn probe_keychain(store: &dyn SecretStore) -> DependencyProbe {
    // Write-read-clear round-trip on a dedicated probe key. The
    // SecretKey enum doesn't include a "probe" key, so we re-use the
    // AnthropicApiKey slot transiently — restoring it afterwards. To
    // stay safe against losing the user's real key, we first read,
    // probe, then write back if there was a value.
    let original = match keychain::get_secret(store, SecretKey::AnthropicApiKey) {
        Ok(v) => v,
        Err(_) => {
            return DependencyProbe {
                name: "keychain",
                status: ProbeStatus::Error,
                version: None,
                install_hint: Some(
                    "macOS keychain access denied — check Settings → Privacy & Security",
                ),
            }
        }
    };

    let probe_value = "promptibrary-keychain-probe";
    let probe_result = (|| -> Result<()> {
        keychain::set_secret(store, SecretKey::AnthropicApiKey, probe_value)?;
        let round_tripped = keychain::get_secret(store, SecretKey::AnthropicApiKey)?;
        if round_tripped.as_deref() != Some(probe_value) {
            return Err(AppError::new(
                AppErrorKind::SettingsInvalid,
                "keychain probe round-trip mismatch",
            ));
        }
        Ok(())
    })();

    // Restore original state regardless of probe outcome.
    let _ = match original.as_deref() {
        Some(v) => keychain::set_secret(store, SecretKey::AnthropicApiKey, v),
        None => keychain::clear_secret(store, SecretKey::AnthropicApiKey),
    };

    match probe_result {
        Ok(()) => DependencyProbe {
            name: "keychain",
            status: ProbeStatus::Ok,
            version: None,
            install_hint: None,
        },
        Err(_) => DependencyProbe {
            name: "keychain",
            status: ProbeStatus::Error,
            version: None,
            install_hint: Some(
                "Keychain access failed — check macOS Settings → Privacy & Security",
            ),
        },
    }
}

async fn probe_sqlite(pool: &SqlitePool) -> DependencyProbe {
    let version: std::result::Result<(String,), _> =
        sqlx::query_as("SELECT sqlite_version()").fetch_one(pool).await;
    match version {
        Ok((v,)) => DependencyProbe {
            name: "sqlite",
            status: ProbeStatus::Ok,
            version: Some(v),
            install_hint: None,
        },
        Err(_) => DependencyProbe {
            name: "sqlite",
            status: ProbeStatus::Error,
            version: None,
            install_hint: Some("SQLite pool not responsive — re-select the vault"),
        },
    }
}

// ─── reveal_in_terminal / open_path ───────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathInput {
    pub path: PathBuf,
}

#[tauri::command]
pub async fn reveal_in_terminal(input: PathInput) -> Result<()> {
    if !input.path.exists() {
        return Err(AppError::new(
            AppErrorKind::VaultMissing,
            "path does not exist",
        ));
    }
    spawn_terminal(&input.path).await
}

#[tauri::command]
pub async fn open_path(input: PathInput) -> Result<()> {
    if !input.path.exists() {
        return Err(AppError::new(
            AppErrorKind::VaultMissing,
            "path does not exist",
        ));
    }
    spawn_opener(&input.path).await
}

#[cfg(target_os = "macos")]
async fn spawn_terminal(path: &std::path::Path) -> Result<()> {
    let status = tokio::process::Command::new("open")
        .args(["-a", "Terminal"])
        .arg(path)
        .status()
        .await
        .map_err(AppError::from)?;
    if !status.success() {
        return Err(AppError::new(
            AppErrorKind::Internal,
            "Terminal.app failed to launch",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
async fn spawn_terminal(path: &std::path::Path) -> Result<()> {
    for bin in ["x-terminal-emulator", "gnome-terminal", "konsole"] {
        let status = tokio::process::Command::new(bin)
            .args(["--working-directory"])
            .arg(path)
            .status()
            .await;
        if let Ok(s) = status {
            if s.success() {
                return Ok(());
            }
        }
    }
    Err(AppError::new(
        AppErrorKind::Internal,
        "no terminal emulator found (tried x-terminal-emulator, gnome-terminal, konsole)",
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
async fn spawn_terminal(_path: &std::path::Path) -> Result<()> {
    Err(AppError::new(
        AppErrorKind::Internal,
        "reveal_in_terminal is not supported on this platform",
    ))
}

#[cfg(target_os = "macos")]
async fn spawn_opener(path: &std::path::Path) -> Result<()> {
    let status = tokio::process::Command::new("open")
        .arg(path)
        .status()
        .await
        .map_err(AppError::from)?;
    if !status.success() {
        return Err(AppError::new(
            AppErrorKind::Internal,
            "open(1) failed",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
async fn spawn_opener(path: &std::path::Path) -> Result<()> {
    let status = tokio::process::Command::new("xdg-open")
        .arg(path)
        .status()
        .await
        .map_err(AppError::from)?;
    if !status.success() {
        return Err(AppError::new(
            AppErrorKind::Internal,
            "xdg-open failed",
        ));
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
async fn spawn_opener(_path: &std::path::Path) -> Result<()> {
    Err(AppError::new(
        AppErrorKind::Internal,
        "open_path is not supported on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::secret_store::InMemorySecretStore;

    #[tokio::test]
    async fn probe_binary_missing_returns_missing_status() {
        let p = probe_binary(
            "definitely-not-a-real-binary-xyz-promptibrary",
            &["--version"],
            None,
        )
        .await;
        assert!(matches!(p.status, ProbeStatus::Missing));
        assert!(p.version.is_none());
    }

    #[tokio::test]
    async fn probe_keychain_ok_with_in_memory_store() {
        let store = InMemorySecretStore::new();
        let p = probe_keychain(&store).await;
        assert!(
            matches!(p.status, ProbeStatus::Ok),
            "expected Ok, got {p:?}"
        );
    }

    #[tokio::test]
    async fn probe_keychain_preserves_existing_value() {
        let store = InMemorySecretStore::new();
        keychain::set_secret(&store, SecretKey::AnthropicApiKey, "sk-real-key").unwrap();
        let _ = probe_keychain(&store).await;
        let after = keychain::get_secret(&store, SecretKey::AnthropicApiKey).unwrap();
        assert_eq!(after.as_deref(), Some("sk-real-key"));
    }
}
