//! Stop escalation per spec §7 *Stop action*.
//!
//! Two modes:
//!
//! * **Graceful** — SIGINT → 5s wait → SIGINT → 5s wait → SIGTERM →
//!   3s wait → SIGKILL.  Gives claude time to flush its session
//!   state and write a clean exit message.
//! * **Force** — SIGTERM → 1s wait → SIGKILL.  Used when the user
//!   explicitly chooses "force stop" or when graceful escalation
//!   exhausted its budget.
//!
//! Signals are sent DIRECTLY to the `claude` child PID — no shell
//! intermediary. portable-pty's `Child::kill()` does SIGKILL on Unix;
//! we use `nix::sys::signal::kill` for SIGINT / SIGTERM.

use std::time::Duration;

use crate::error::{AppError, AppErrorKind, Result};

#[cfg(unix)]
use nix::sys::signal::{self, Signal};
#[cfg(unix)]
use nix::unistd::Pid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopMode {
    Graceful,
    Force,
}

/// Send `signal` to `pid`. Returns Err on EPERM / ESRCH (no such
/// process). On non-Unix platforms returns Err because portable-pty's
/// signal model only ships SIGKILL via Child::kill there.
#[cfg(unix)]
pub fn send_signal(pid: i32, signal: Signal) -> Result<()> {
    signal::kill(Pid::from_raw(pid), signal).map_err(|e| {
        AppError::new(
            AppErrorKind::Internal,
            format!("kill({pid}, {signal:?}) failed: {e}"),
        )
    })
}

#[cfg(not(unix))]
#[allow(dead_code)]
pub fn send_signal(_pid: i32, _signal: u32) -> Result<()> {
    Err(AppError::new(
        AppErrorKind::Internal,
        "signal escalation is Unix-only; non-Unix builds use SIGKILL via Child::kill",
    ))
}

/// Graceful stop escalation timings per spec §7.
pub const GRACEFUL_SIGINT_INTERVAL: Duration = Duration::from_secs(5);
pub const GRACEFUL_SIGTERM_WAIT: Duration = Duration::from_secs(3);
pub const FORCE_SIGTERM_WAIT: Duration = Duration::from_secs(1);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graceful_timings_match_spec() {
        assert_eq!(GRACEFUL_SIGINT_INTERVAL.as_secs(), 5);
        assert_eq!(GRACEFUL_SIGTERM_WAIT.as_secs(), 3);
    }

    #[test]
    fn force_timings_match_spec() {
        assert_eq!(FORCE_SIGTERM_WAIT.as_secs(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn send_signal_to_invalid_pid_errors() {
        // PID 1 is init — kill -0 (signal 0) tests if a process
        // exists; sending an actual signal would be dangerous. Use a
        // PID that definitely doesn't exist on any sane system.
        let result = send_signal(0x7fff_ffff, Signal::SIGTERM);
        assert!(result.is_err());
    }
}
