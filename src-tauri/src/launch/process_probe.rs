//! `claude --version` probe used by `commands::system::probe_dependencies`.
//!
//! The dependency-probe surface in `commands::system::probe_binary`
//! already handles the generic case; this module is a typed wrapper
//! that prefers the resolved `claude` path from `launch::claude_cli`
//! over a bare PATH lookup. That distinction matters when the user
//! has claude installed somewhere only their login shell knows about
//! (npm-global, pnpm-global, brew-prefix that isn't on the GUI app's
//! PATH).

use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;

use crate::error::{AppError, AppErrorKind, Result};

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct ClaudeVersion {
    pub path: std::path::PathBuf,
    pub version: String,
}

/// Resolve claude via login shell and run `<claude> --version`.
/// Returns the absolute path it ran AND the trimmed first line of
/// stdout (claude prints its version on the first line).
pub async fn probe_claude_version() -> Result<ClaudeVersion> {
    let path = super::claude_cli::resolve_claude_path().await?;
    let fut = Command::new(&path).arg("--version").output();
    let out = match timeout(PROBE_TIMEOUT, fut).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            return Err(AppError::new(
                AppErrorKind::ClaudeCliFailed,
                format!("spawn {} --version: {e}", path.display()),
            ))
        }
        Err(_) => {
            return Err(AppError::new(
                AppErrorKind::ClaudeCliFailed,
                "claude --version timed out after 5 s",
            ))
        }
    };
    if !out.status.success() {
        return Err(AppError::new(
            AppErrorKind::ClaudeCliFailed,
            format!(
                "claude --version returned non-zero (stderr: {})",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let version = stdout.lines().next().unwrap_or("").trim().to_string();
    Ok(ClaudeVersion { path, version })
}
