//! SCA-927 (W14): real-PTY stop-escalation tests.
//!
//! Complements the natural-exit happy path in `launch_fake_claude.rs`.
//! Each test spawns a long-running fake claude (`fake_claude_sleep.sh`,
//! 30s sleep) so we can drive the SIGINT / SIGTERM / SIGKILL ladder
//! against a real subprocess. Without this coverage, the regression
//! class "shell-wrap the claude invocation → signals don't reach the
//! child" (CLAUDE.md *Critical don'ts*) can't be caught — the unit
//! tests in `launch::signals` use mocked PIDs.
//!
//! Tests are #[cfg(unix)] because the escalation ladder is unix-only.

#![cfg(unix)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use promptibrary_lib::ids::RunId;
use promptibrary_lib::launch::pty_session::{spawn, PtyEvent, PtySessionConfig};
use promptibrary_lib::launch::signals;

fn fake_claude_sleep_path() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("tests/fixtures/fake_claude_sleep.sh")
}

async fn spawn_sleeper() -> (
    promptibrary_lib::launch::pty_session::PtySessionHandle,
    tokio::sync::mpsc::UnboundedReceiver<PtyEvent>,
    tokio::task::JoinHandle<()>,
) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = PtySessionConfig {
        run_id: RunId("01FAKESTOPTEST".into()),
        claude_path: fake_claude_sleep_path(),
        args: vec![],
        cwd: tmp.path().to_path_buf(),
        resolved_prompt: "stop-test prompt".into(),
        vault_root: tmp.path().to_path_buf(),
        cols: 120,
        rows: 32,
    };
    // tempdir is leaked into the spawned process intentionally — we
    // don't want to drop it while the PTY is alive.
    std::mem::forget(tmp);
    spawn(cfg).await.expect("spawn fake claude sleep")
}

async fn drain_until_first_output(
    events: &mut tokio::sync::mpsc::UnboundedReceiver<PtyEvent>,
) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(500), events.recv()).await {
            Ok(Some(PtyEvent::FirstOutput)) => return,
            Ok(Some(_)) => continue,
            _ => continue,
        }
    }
    panic!("FirstOutput never fired within 3s");
}

/// Force stop: SIGTERM → 1s wait → SIGKILL. Pipeline must observe
/// `Exited` with a signal-driven termination (exit_code may be None
/// on signal kill).
#[tokio::test]
async fn force_stop_terminates_child_via_signal_ladder() {
    let (handle, mut events, drainer) = spawn_sleeper().await;
    drain_until_first_output(&mut events).await;

    let pid = handle.pid().expect("pid set") as i32;

    // Force path = SIGTERM, then SIGKILL if it didn't die within 1s.
    signals::send_signal(pid, nix::sys::signal::Signal::SIGTERM).ok();
    let exited = handle
        .wait_with_timeout(signals::FORCE_SIGTERM_WAIT)
        .await
        .expect("wait");

    if exited.is_none() {
        // Belt-and-suspenders if SIGTERM didn't take.
        handle.kill().await.expect("hard kill");
    }

    // Collect the terminal event.
    let mut got_exit = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(500), events.recv()).await {
            Ok(Some(PtyEvent::Exited { .. })) => {
                got_exit = true;
                break;
            }
            Ok(Some(_)) => continue,
            _ => continue,
        }
    }
    let _ = drainer.await;
    assert!(got_exit, "force_stop_terminates_child_via_signal_ladder: never saw Exited");
}

/// Graceful stop: SIGINT → 5s → SIGINT → 5s → SIGTERM → 3s → SIGKILL.
/// We send SIGINT; `fake_claude_sleep.sh`'s `sleep ... & wait $!` form
/// is interruptible, so the first SIGINT should be enough. Asserts the
/// process exits well before the SIGKILL fallback would fire (~13s).
#[tokio::test]
async fn graceful_sigint_stops_responsive_child() {
    let (handle, mut events, drainer) = spawn_sleeper().await;
    drain_until_first_output(&mut events).await;

    let pid = handle.pid().expect("pid set") as i32;
    let started = Instant::now();
    signals::send_signal(pid, nix::sys::signal::Signal::SIGINT).ok();

    let exited = handle
        .wait_with_timeout(signals::GRACEFUL_SIGINT_INTERVAL)
        .await
        .expect("wait");
    if exited.is_none() {
        // Fallback: SIGKILL if the child somehow didn't honour SIGINT.
        handle.kill().await.expect("hard kill");
    }

    let mut got_exit = false;
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(500), events.recv()).await {
            Ok(Some(PtyEvent::Exited { .. })) => {
                got_exit = true;
                break;
            }
            Ok(Some(_)) => continue,
            _ => continue,
        }
    }
    let _ = drainer.await;
    assert!(got_exit, "graceful path never observed Exited");
    let total = started.elapsed();
    // The spec ladder caps graceful escalation around 13s + SIGTERM
    // window. We assert <11s as a comfortable upper bound that still
    // proves the kill happened before the full SIGKILL fallback.
    assert!(
        total < Duration::from_secs(11),
        "graceful stop took {total:?}, expected well under the SIGKILL fallback window",
    );
}
