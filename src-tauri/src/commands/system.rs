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
use crate::settings::keychain::KEYCHAIN_PROBE_ACCOUNT;
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
    // Probe a disjoint account name so we never touch the live
    // AnthropicApiKey slot (SCA-731). The pre-fix design read-wrote-
    // restored the real slot, which lost the user's key on any failure
    // between probe-write and restore.
    const PROBE_VALUE: &str = "promptibrary-keychain-probe";

    let probe_result = (|| -> Result<()> {
        store.set(KEYCHAIN_PROBE_ACCOUNT, PROBE_VALUE)?;
        let round_tripped = store.get(KEYCHAIN_PROBE_ACCOUNT)?;
        if round_tripped.as_deref() != Some(PROBE_VALUE) {
            return Err(AppError::new(
                AppErrorKind::SettingsInvalid,
                "keychain probe round-trip mismatch",
            ));
        }
        Ok(())
    })();

    // Best-effort cleanup. If delete fails, the stale probe value is
    // confined to the diagnostics-probe slot — not to any real secret
    // — and the next probe will overwrite it. We log but do not fail.
    if let Err(e) = store.delete(KEYCHAIN_PROBE_ACCOUNT) {
        tracing::warn!(error = %e, "keychain probe cleanup failed");
    }

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

    /// SCA-731 invariant: the probe MUST NOT touch the user's real
    /// AnthropicApiKey slot. Pre-fix design read-wrote-restored the
    /// real slot; any failure between probe-write and restore lost
    /// the user's key. The disjoint-account fix sidesteps that entire
    /// failure mode.
    #[tokio::test]
    async fn probe_keychain_never_touches_real_secret_slot() {
        use crate::settings::keychain::{self, SecretKey, KEYCHAIN_PROBE_ACCOUNT};

        let store = InMemorySecretStore::new();
        keychain::set_secret(&store, SecretKey::AnthropicApiKey, "sk-real-key").unwrap();

        let p = probe_keychain(&store).await;
        assert!(matches!(p.status, ProbeStatus::Ok));

        // Real secret preserved.
        let after = keychain::get_secret(&store, SecretKey::AnthropicApiKey).unwrap();
        assert_eq!(after.as_deref(), Some("sk-real-key"));

        // Probe slot cleaned up successfully on the happy path.
        assert_eq!(store.get(KEYCHAIN_PROBE_ACCOUNT).unwrap(), None);
    }

    /// SCA-731 fault-injection: if the cleanup `delete` fails, the
    /// probe still succeeds because no real secret was ever at risk.
    /// Confirms the failure mode is isolated to the disjoint probe
    /// slot.
    #[tokio::test]
    async fn probe_keychain_succeeds_even_when_cleanup_delete_fails() {
        use crate::settings::keychain::{self, SecretKey};

        struct CleanupFailingStore {
            inner: InMemorySecretStore,
        }

        impl SecretStore for CleanupFailingStore {
            fn get(&self, key: &str) -> Result<Option<String>> {
                self.inner.get(key)
            }
            fn set(&self, key: &str, value: &str) -> Result<()> {
                self.inner.set(key, value)
            }
            fn delete(&self, _key: &str) -> Result<()> {
                Err(AppError::new(
                    AppErrorKind::KeychainError,
                    "simulated cleanup failure",
                ))
            }
        }

        let store = CleanupFailingStore {
            inner: InMemorySecretStore::new(),
        };
        keychain::set_secret(&store, SecretKey::AnthropicApiKey, "sk-real-key").unwrap();

        let p = probe_keychain(&store).await;
        assert!(
            matches!(p.status, ProbeStatus::Ok),
            "probe must still report Ok when only cleanup fails; got {p:?}"
        );

        // The user's real secret survives the probe regardless of
        // cleanup failure — this is the whole point of CRIT-1's fix.
        let after = keychain::get_secret(&store, SecretKey::AnthropicApiKey).unwrap();
        assert_eq!(after.as_deref(), Some("sk-real-key"));
    }
}
