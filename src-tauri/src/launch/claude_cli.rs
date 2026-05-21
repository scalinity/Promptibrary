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
//! ## SCA-913 — $SHELL allow-list (CWE-78 / CWE-426)
//!
//! `$SHELL` is environment-controllable; a malicious launcher
//! (or a user-level `launchctl setenv` on macOS) could point us at
//! an attacker-supplied binary. We allow-list `$SHELL` against the
//! standard POSIX shell set and fall back to `/bin/sh` on mismatch.
//! After lookup, the resolved claude path is canonicalized so the
//! eventual PTY spawn uses an absolute, symlink-resolved location.
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

/// Shells we'll trust to run our lookup `-lc 'command -v claude'`.
/// Anything outside this list falls back to `/bin/sh`. Order matters
/// only for readability; the membership check is set-based.
const ALLOWED_SHELLS: &[&str] = &[
    "/bin/sh",
    "/bin/bash",
    "/bin/zsh",
    "/bin/dash",
    "/bin/ksh",
    "/usr/bin/bash",
    "/usr/bin/zsh",
    "/usr/local/bin/bash",
    "/usr/local/bin/zsh",
    "/usr/local/bin/fish",
    "/opt/homebrew/bin/bash",
    "/opt/homebrew/bin/zsh",
    "/opt/homebrew/bin/fish",
];

/// Pure allow-list check. Extracted from `safe_shell` so tests can
/// exercise the policy without touching the process-wide `$SHELL` env
/// (which races across parallel `cargo test` workers).
fn select_safe_shell(candidate: &str) -> &'static str {
    if let Some(matched) = ALLOWED_SHELLS.iter().find(|s| **s == candidate) {
        matched
    } else {
        if !candidate.is_empty() {
            tracing::warn!(
                shell = %candidate,
                "SCA-913: $SHELL not in allow-list, falling back to /bin/sh for claude lookup"
            );
        }
        "/bin/sh"
    }
}

/// Return the shell to use for lookup. If `$SHELL` matches the
/// allow-list, return it; otherwise log + fall back to `/bin/sh`.
fn safe_shell() -> String {
    let user_shell = std::env::var("SHELL").unwrap_or_default();
    select_safe_shell(&user_shell).to_string()
}

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
    let shell = safe_shell();
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
    // SCA-913: canonicalize so the cached path is symlink-resolved.
    // If canonicalization fails (e.g. the binary was removed between
    // the lookup and now) we still surface the literal path — the
    // PTY spawn will then produce its own missing-file error.
    let raw = PathBuf::from(path);
    let canonical = std::fs::canonicalize(&raw).unwrap_or(raw);
    tracing::info!(claude_path = %canonical.display(), "claude resolved");
    Ok(canonical)
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
        let shell = safe_shell();
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
        let shell = safe_shell();
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

    /// SCA-913: a $SHELL outside the allow-list must NOT be used.
    /// The pure helper avoids touching the process-wide env var so
    /// parallel `cargo test` workers don't race each other.
    #[test]
    fn hostile_shell_falls_back_to_bin_sh() {
        assert_eq!(select_safe_shell("/tmp/evil"), "/bin/sh");
        assert_eq!(
            select_safe_shell("/Users/attacker/.local/bin/zsh"),
            "/bin/sh"
        );
        assert_eq!(select_safe_shell(""), "/bin/sh");
    }

    #[test]
    fn allow_listed_shells_are_honored() {
        for shell in [
            "/bin/zsh",
            "/bin/bash",
            "/bin/sh",
            "/opt/homebrew/bin/fish",
        ] {
            assert_eq!(select_safe_shell(shell), shell);
        }
    }
}
