//! §15 *Fake Claude launch* integration test per spec.
//!
//! Spec §15 requires the PTY launch pipeline to be exercised against a
//! real subprocess in CI — not simulated against a mocked `MasterPty`.
//! Without this, signal escalation, transcript writing, and bracketed-
//! paste injection regressions silently sneak through unit tests that
//! only run on shaped/in-memory data.
//!
//! What this test covers:
//!   1. Direct PTY spawn — `CommandBuilder::new(<binary>)` with no
//!      `/bin/sh -c` wrap (spec §7 *Stop action* depends on this).
//!   2. Event-driven prompt injection — bracketed paste fires *after*
//!      the first PTY output chunk arrives, not after a fixed sleep.
//!   3. Transcript writing to
//!      `<vault>/promptibrary/runs/YYYY/MM/<run_id>.md`.
//!   4. Natural exit propagation — the drainer task observes EOF, waits
//!      the child, and emits `PtyEvent::Exited` with code 0.
//!
//! The "fake claude" is a POSIX shell script at
//! `tests/fixtures/fake_claude.sh`. It prints a banner (so the drainer
//! sees a first chunk and runs the injection), reads one line of stdin
//! (the injected prompt), and echoes it back.

use std::path::PathBuf;

use promptibrary_lib::ids::RunId;
use promptibrary_lib::launch::pty_session::{spawn, PtyEvent, PtySessionConfig};

fn fake_claude_path() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("tests/fixtures/fake_claude.sh")
}

#[tokio::test]
async fn fake_claude_launch_exercises_pty_pipeline_end_to_end() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = PtySessionConfig {
        run_id: RunId("01FAKECLAUDELAUNCH".into()),
        claude_path: fake_claude_path(),
        // Real-claude-style args so the arg-builder code path is
        // exercised even though the fake binary ignores them.
        args: vec![
            "--model".into(),
            "claude-sonnet-4-6".into(),
            "--permission-mode".into(),
            "default".into(),
        ],
        cwd: tmp.path().to_path_buf(),
        resolved_prompt: "do the thing per spec §15".into(),
        vault_root: tmp.path().to_path_buf(),
        cols: 120,
        rows: 32,
    };

    let (handle, mut events, drainer) = spawn(cfg).await.expect("spawn fake claude");
    let transcript_path = handle.transcript_path.clone();

    // Drain events until natural exit.
    let mut got_first = false;
    let mut got_exited_zero = false;
    let mut stdout_bytes: Vec<u8> = Vec::new();
    while let Some(ev) = events.recv().await {
        match ev {
            PtyEvent::FirstOutput => {
                got_first = true;
            }
            PtyEvent::Stdout(chunk) => {
                stdout_bytes.extend_from_slice(&chunk);
            }
            PtyEvent::Exited { exit_code, .. } => {
                got_exited_zero = exit_code == Some(0);
                break;
            }
            PtyEvent::Error(e) => panic!("unexpected PTY error: {e}"),
        }
    }
    // Reap the drainer so we don't leak the tokio task.
    let _ = drainer.await;

    assert!(got_first, "FirstOutput event was never emitted");
    assert!(got_exited_zero, "fake claude did not exit 0");

    let stdout_str = String::from_utf8_lossy(&stdout_bytes);
    assert!(
        stdout_str.contains("fake-claude ready"),
        "missing banner in stdout: {stdout_str:?}",
    );
    // The bracketed-paste injection should have delivered our prompt to
    // the fake binary, which echoes it back via `received-prompt: ...`.
    assert!(
        stdout_str.contains("received-prompt:"),
        "fake claude never observed an injected prompt — bracketed-paste injection regressed. stdout: {stdout_str:?}",
    );
    assert!(
        stdout_str.contains("do the thing per spec"),
        "injected prompt body never reached fake claude; stdout: {stdout_str:?}",
    );

    // Transcript file landed where TranscriptWriter::open promised.
    let transcript = tokio::fs::read_to_string(&transcript_path)
        .await
        .expect("transcript file exists");
    assert!(
        transcript.contains("fake-claude ready"),
        "transcript missing banner: {transcript:?}",
    );
    assert!(
        transcript_path
            .components()
            .any(|c| c.as_os_str() == "promptibrary"),
        "transcript path not under <vault>/promptibrary/runs/: {transcript_path:?}",
    );
}
