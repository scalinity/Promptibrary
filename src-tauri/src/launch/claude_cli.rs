//! Resolve the user's `claude` executable per spec §7.
//!
//! Strategy: run the user's login shell and ask it where `claude` is.
//! This mirrors what an interactive terminal session would see and
//! correctly handles npm-global / pnpm-global / homebrew install
//! locations that aren't on the GUI app's PATH.
//!
//! ```bash
//! "$SHELL" -lc 'command -v claude'
//! ```
//!
//! The result is cached on first success in an `OnceCell`. Settings →
//! Run diagnostics re-runs this lookup explicitly to pick up
//! post-install changes (see `commands::system::probe_dependencies`).

use std::path::PathBuf;
use std::time::Duration;

use once_cell::sync::OnceCell;
use tokio::process::Command;
use tokio::time::timeout;

use crate::error::{AppError, AppErrorKind, Result};

static CACHE: OnceCell<PathBuf> = OnceCell::new();

const RESOLVE_TIMEOUT: Duration = Duration::from_secs(5);

/// Resolve the absolute path to the `claude` binary. Cached for the
/// process lifetime after the first success.
///
/// Returns `AppErrorKind::ClaudeCliMissing` when the lookup fails or
/// times out — the diagnostics card surfaces this as an install hint.
pub async fn resolve_claude_path() -> Result<PathBuf> {
    if let Some(path) = CACHE.get() {
        return Ok(path.clone());
    }
    let resolved = resolve_claude_path_uncached().await?;
    let _ = CACHE.set(resolved.clone());
    Ok(resolved)
}

/// Re-run the shell lookup, bypassing the cache. Used by the
/// diagnostics card when the user clicks *Run diagnostics again*
/// after installing claude post-launch.
pub async fn resolve_claude_path_uncached() -> Result<PathBuf> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let fut = Command::new(&shell)
        .args(["-lc", "command -v claude"])
        .output();
    let out = match timeout(RESOLVE_TIMEOUT, fut).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            return Err(AppError::new(
                AppErrorKind::ClaudeCliMissing,
                format!("spawn {shell} -lc failed: {e}"),
            ))
        }
        Err(_) => {
            return Err(AppError::new(
                AppErrorKind::ClaudeCliMissing,
                "login-shell `command -v claude` timed out after 5 s",
            ))
        }
    };
    if !out.status.success() {
        return Err(AppError::new(
            AppErrorKind::ClaudeCliMissing,
            "claude not found on the user's login-shell PATH (run `command -v claude` in a terminal to debug)",
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let path = stdout.trim();
    if path.is_empty() {
        return Err(AppError::new(
            AppErrorKind::ClaudeCliMissing,
            "login-shell returned an empty path for claude",
        ));
    }
    Ok(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: the login-shell-based lookup mechanism works when
    /// the target binary exists. We use `echo` (guaranteed on PATH)
    /// in place of claude so the test doesn't depend on claude being
    /// installed in the test environment.
    #[tokio::test]
    async fn login_shell_lookup_finds_echo() {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let out = Command::new(&shell)
            .args(["-lc", "command -v echo"])
            .output()
            .await
            .unwrap();
        assert!(out.status.success(), "command -v echo should succeed");
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        assert!(!path.is_empty(), "echo should resolve to a path");
    }

    #[tokio::test]
    async fn login_shell_lookup_misses_bogus_binary() {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let out = Command::new(&shell)
            .args([
                "-lc",
                "command -v definitely-not-a-real-binary-xyz-promptibrary",
            ])
            .output()
            .await
            .unwrap();
        assert!(
            !out.status.success(),
            "command -v on a fake binary must fail"
        );
    }
}
