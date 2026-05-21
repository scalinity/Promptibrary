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
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;
use tokio::time::timeout;

use crate::app_state::ManagedState;
use crate::error::{AppError, AppErrorKind, Result};
use crate::settings::keychain::KEYCHAIN_PROBE_ACCOUNT;
use crate::settings::secret_store::SecretStore;

/// Per-binary probe wall-clock budget. A hung `claude --version` (or
/// any other diagnostics-probed CLI) must NOT block the IPC future
/// indefinitely; on timeout we kill the child and return
/// `ProbeStatus::Error` with a hint that flags the timeout.
const PROBE_BINARY_TIMEOUT: Duration = Duration::from_secs(5);

/// Wall-clock budget for the `open(1)` / `xdg-open` / Terminal.app
/// launch step. Real-world `open` returns nearly instantly (Launch
/// Services dispatch); 10 s is the slow-Mac upper bound that still
/// keeps the user from staring at a hung dialog.
const SPAWN_LAUNCH_TIMEOUT: Duration = Duration::from_secs(10);

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
    // SCA-758: the three binary probes are independent and each has
    // its own 5 s timeout (SCA-733). Run them concurrently so the
    // wall-clock cost is ~max instead of ~sum (15 s worst case
    // serial → 5 s worst case parallel).
    let (claude, ytdlp, git) = tokio::join!(
        probe_binary(
            "claude",
            &["--version"],
            Some("Install Claude Code CLI: https://docs.anthropic.com/en/docs/claude-code"),
        ),
        probe_binary(
            "yt-dlp",
            &["--version"],
            Some("Install yt-dlp: brew install yt-dlp / pipx install yt-dlp"),
        ),
        probe_binary(
            "git",
            &["--version"],
            Some("Install Git: brew install git"),
        ),
    );

    let mut probes = vec![claude, ytdlp, git];
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
    probe_binary_with_timeout(name, args, install_hint, PROBE_BINARY_TIMEOUT).await
}

/// Inner helper for `probe_binary` parameterized on the wall-clock
/// budget so tests can exercise the timeout path without waiting
/// for the production 5 s.
async fn probe_binary_with_timeout(
    name: &'static str,
    args: &[&str],
    install_hint: Option<&'static str>,
    budget: Duration,
) -> DependencyProbe {
    let spawn = tokio::process::Command::new(name)
        .args(args)
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    let child = match spawn {
        Ok(c) => c,
        Err(_) => {
            return DependencyProbe {
                name,
                status: ProbeStatus::Missing,
                version: None,
                install_hint,
            }
        }
    };

    let wait = child.wait_with_output();
    match timeout(budget, wait).await {
        Ok(Ok(out)) if out.status.success() => {
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
        Ok(Ok(_)) => DependencyProbe {
            name,
            status: ProbeStatus::Error,
            version: None,
            install_hint,
        },
        Ok(Err(_)) => DependencyProbe {
            name,
            status: ProbeStatus::Missing,
            version: None,
            install_hint,
        },
        Err(_elapsed) => DependencyProbe {
            name,
            status: ProbeStatus::Error,
            version: None,
            install_hint: Some(
                "binary did not respond within 5 s — check that it isn't blocked on stdin or a network call",
            ),
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
            AppErrorKind::PathNotFound,
            "path does not exist",
        ));
    }
    spawn_terminal(&input.path).await
}

#[tauri::command]
pub async fn open_path(input: PathInput) -> Result<()> {
    if !input.path.exists() {
        return Err(AppError::new(
            AppErrorKind::PathNotFound,
            "path does not exist",
        ));
    }
    spawn_opener(&input.path).await
}

/// SCA-743 trust posture: the V1 threat model is "user owns the
/// frontend"; the IPC accepts any path the process has read access to.
/// Vault-ancestry restriction is a V2 candidate. We do however add
/// the `--` argument terminator below so a path starting with `-`
/// (e.g. `-h`) is not parsed as a flag by `open` / `xdg-open` —
/// without it, `open -- "-h"` would print help instead of opening a
/// file literally named `-h`. Same precaution for `spawn_terminal`.
#[cfg(target_os = "macos")]
async fn spawn_terminal(path: &std::path::Path) -> Result<()> {
    let fut = tokio::process::Command::new("open")
        .args(["-a", "Terminal", "--"])
        .arg(path)
        .status();
    match timeout(SPAWN_LAUNCH_TIMEOUT, fut).await {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(_)) => Err(AppError::new(
            AppErrorKind::Internal,
            "Terminal.app failed to launch",
        )),
        Ok(Err(e)) => Err(AppError::from(e)),
        Err(_) => Err(AppError::new(
            AppErrorKind::Internal,
            "Terminal.app launch timed out after 10 s",
        )),
    }
}

#[cfg(target_os = "linux")]
async fn spawn_terminal(path: &std::path::Path) -> Result<()> {
    for bin in ["x-terminal-emulator", "gnome-terminal", "konsole"] {
        let fut = tokio::process::Command::new(bin)
            .args(["--working-directory", "--"])
            .arg(path)
            .status();
        if let Ok(Ok(s)) = timeout(SPAWN_LAUNCH_TIMEOUT, fut).await {
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
    let fut = tokio::process::Command::new("open")
        .arg("--")
        .arg(path)
        .status();
    match timeout(SPAWN_LAUNCH_TIMEOUT, fut).await {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(_)) => Err(AppError::new(AppErrorKind::Internal, "open(1) failed")),
        Ok(Err(e)) => Err(AppError::from(e)),
        Err(_) => Err(AppError::new(
            AppErrorKind::Internal,
            "open(1) timed out after 10 s",
        )),
    }
}

#[cfg(target_os = "linux")]
async fn spawn_opener(path: &std::path::Path) -> Result<()> {
    let fut = tokio::process::Command::new("xdg-open")
        .arg("--")
        .arg(path)
        .status();
    match timeout(SPAWN_LAUNCH_TIMEOUT, fut).await {
        Ok(Ok(status)) if status.success() => Ok(()),
        Ok(Ok(_)) => Err(AppError::new(AppErrorKind::Internal, "xdg-open failed")),
        Ok(Err(e)) => Err(AppError::from(e)),
        Err(_) => Err(AppError::new(
            AppErrorKind::Internal,
            "xdg-open timed out after 10 s",
        )),
    }
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

    /// SCA-733 invariant: a hung binary must not block diagnostics
    /// indefinitely. We probe `sleep 10` with a 200 ms budget; the
    /// timeout fires, the child is killed via `kill_on_drop`, and
    /// the probe returns `Error` with a timeout-flavored install
    /// hint.
    #[tokio::test]
    async fn probe_binary_with_timeout_returns_error_on_hung_binary() {
        let p = probe_binary_with_timeout(
            "sleep",
            &["10"],
            None,
            Duration::from_millis(200),
        )
        .await;
        assert!(
            matches!(p.status, ProbeStatus::Error),
            "expected Error status on timeout, got {p:?}"
        );
        assert!(
            p.install_hint
                .map(|h| h.contains("did not respond"))
                .unwrap_or(false),
            "expected timeout install_hint, got {:?}",
            p.install_hint
        );
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
