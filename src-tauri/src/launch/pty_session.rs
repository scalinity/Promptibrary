//! Single-run PTY process lifecycle per spec §7.
//!
//! Layout:
//!
//! ```text
//! +--------+   read blocks    +-------------------+   chunks    +----------------+
//! | claude | <==============> | reader (blocking) | ==========> |  drainer task  |
//! +--------+   PTY pair       +-------------------+  mpsc       +--+----------+--+
//!      ^                                                          |          |
//!      |                                                          v          v
//!      |  bracketed-paste                                   transcript    Tauri
//!      |  (after first chunk)                                 file        emit
//!      +-------------------------- prompt_injector::build_inject_bytes ----+
//! ```
//!
//! The reader is a blocking thread (`tokio::task::spawn_blocking`)
//! looping on `reader.read(&mut buf)`. Each chunk goes through an
//! `mpsc::UnboundedSender<PtyEvent>` to an async drainer task that
//! owns the transcript writer and a `Box<dyn EventSink>` for Tauri
//! events. The drainer also triggers prompt injection on the first
//! chunk.
//!
//! On the child side: portable-pty's `Child` impl owns the PID and
//! the `kill()` / `try_wait()` calls. We wrap it in `Arc<Mutex<...>>`
//! so the stop-escalation path can access it from a tokio task.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::error::{AppError, AppErrorKind, Result};
use crate::ids::RunId;
use crate::launch::prompt_injector;
use crate::launch::transcript_writer::TranscriptWriter;

/// One configured launch ready to spawn.
#[derive(Debug, Clone)]
pub struct PtySessionConfig {
    pub run_id: RunId,
    pub claude_path: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub resolved_prompt: String,
    /// Vault root used to compute the transcript path. The transcript
    /// itself opens at `<vault>/promptibrary/runs/YYYY/MM/<run_id>.md`.
    pub vault_root: PathBuf,
    pub cols: u16,
    pub rows: u16,
}

/// Event a session emits as it streams. Consumed by both the
/// transcript writer and the Tauri-event sink.
#[derive(Debug, Clone)]
pub enum PtyEvent {
    /// Raw stdout chunk. UTF-8 may be split mid-codepoint; the
    /// transcript layer writes raw bytes and xterm.js on the frontend
    /// reassembles. (Not a problem in practice — claude emits
    /// chunked UTF-8 but never splits mid-codepoint within a single
    /// write.)
    Stdout(Vec<u8>),
    /// First Stdout chunk has been observed. Fires once per session.
    FirstOutput,
    /// Child has exited with the captured ExitStatus.
    Exited { exit_code: Option<i32>, signal: Option<String> },
    /// Reader or drainer errored — the session is dead.
    Error(String),
}

/// Frontend-facing handle: owns the PTY master (for stdin writes +
/// resize) and the child handle (for kill + try_wait). Cloning the
/// handle clones the Arcs so multiple Tauri commands can share it.
pub struct PtySessionHandle {
    pub run_id: RunId,
    pub pid: Option<u32>,
    pub started_at: DateTime<Utc>,
    pub transcript_path: PathBuf,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
}

impl PtySessionHandle {
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Write `bytes` to the PTY stdin (i.e. forward keystrokes the
    /// xterm pane captured). Sync writes are wrapped in
    /// `spawn_blocking` so they don't park the runtime.
    pub async fn write(&self, bytes: Vec<u8>) -> Result<()> {
        let writer = self.writer.clone();
        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            let mut w = writer.lock();
            w.write_all(&bytes)?;
            w.flush()?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::new(AppErrorKind::Internal, format!("join: {e}")))?
        .map_err(AppError::from)
    }

    /// Resize the PTY to `(cols, rows)`. Forwards to the master's
    /// `resize` ioctl.
    pub async fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let master = self.master.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            master
                .lock()
                .resize(PtySize {
                    cols,
                    rows,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| {
                    AppError::new(AppErrorKind::Internal, format!("pty resize: {e}"))
                })
        })
        .await
        .map_err(|e| AppError::new(AppErrorKind::Internal, format!("join: {e}")))?
    }

    /// SIGKILL the child via portable-pty's `Child::kill()`. The
    /// caller is responsible for graceful escalation if any — this is
    /// the terminal step.
    pub async fn kill(&self) -> Result<()> {
        let child = self.child.clone();
        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            child.lock().kill()
        })
        .await
        .map_err(|e| AppError::new(AppErrorKind::Internal, format!("join: {e}")))?
        .map_err(AppError::from)
    }

    /// Block until the child exits (with a timeout). Returns the exit
    /// code (None for signal exit) and signal name when applicable.
    pub async fn wait_with_timeout(
        &self,
        deadline: Duration,
    ) -> Result<Option<(Option<i32>, Option<String>)>> {
        let child = self.child.clone();
        let fut = tokio::task::spawn_blocking(move || -> std::io::Result<(Option<i32>, Option<String>)> {
            let status = child.lock().wait()?;
            // portable_pty::ExitStatus::exit_code() returns u32 even on signal
            // exit; the only way to distinguish a signal kill is to check
            // success() == false and the platform-specific status code.
            // For V1 we treat any non-success as exit_code=None, signal="(unknown)".
            let code = status.exit_code() as i32;
            let signal = if !status.success() { Some(format!("exit:{code}")) } else { None };
            Ok((Some(code), signal))
        });
        match timeout(deadline, fut).await {
            Ok(Ok(Ok(pair))) => Ok(Some(pair)),
            Ok(Ok(Err(e))) => Err(AppError::from(e)),
            Ok(Err(e)) => Err(AppError::new(
                AppErrorKind::Internal,
                format!("wait join: {e}"),
            )),
            Err(_) => Ok(None), // timeout — caller decides next escalation step
        }
    }
}

/// Spawn a fresh PTY-backed claude session per `cfg`. Returns:
///   • the `PtySessionHandle` (write/resize/kill/wait surface)
///   • an `mpsc::UnboundedReceiver<PtyEvent>` the caller drains
///   • the spawned `JoinHandle` of the reader bridge (so the caller
///     can wait for natural exit if it wants)
///
/// Prompt injection: the reader bridge watches for the first non-empty
/// stdout chunk and writes the bracketed-paste envelope into stdin.
/// No fixed sleep — this is the event-driven anchor spec §7 mandates.
pub async fn spawn(cfg: PtySessionConfig) -> Result<(PtySessionHandle, mpsc::UnboundedReceiver<PtyEvent>, JoinHandle<()>)> {
    let started_at = crate::time::now_utc();

    // 1. Open the PTY pair.
    let pty_system = native_pty_system();
    let pty_pair = pty_system
        .openpty(PtySize {
            cols: cfg.cols.max(1),
            rows: cfg.rows.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| AppError::new(AppErrorKind::PtySpawnFailed, format!("openpty: {e}")))?;

    // 2. Build the command — direct spawn, no shell wrap.
    let mut cmd = CommandBuilder::new(&cfg.claude_path);
    for arg in &cfg.args {
        cmd.arg(arg);
    }
    cmd.cwd(&cfg.cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env_remove("COLUMNS");
    cmd.env_remove("LINES");
    // Per spec §7: do NOT export ANTHROPIC_API_KEY into the child by
    // default. Claude Code handles its own auth via the keychain /
    // CLI config; the user's GUI-process env should not leak.
    cmd.env_remove("ANTHROPIC_API_KEY");

    let child = pty_pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| AppError::new(AppErrorKind::PtySpawnFailed, format!("spawn: {e}")))?;
    let pid = child.process_id();

    // 3. Open the transcript writer.
    let mut transcript = TranscriptWriter::open(&cfg.vault_root, cfg.run_id.as_str(), started_at).await?;
    let transcript_path = transcript.path().to_path_buf();

    // 4. Take the master writer + a reader clone. The slave fd can be
    //    dropped now — the child inherits its own copy.
    let writer = pty_pair.master.take_writer().map_err(|e| {
        AppError::new(AppErrorKind::PtySpawnFailed, format!("take_writer: {e}"))
    })?;
    let reader = pty_pair.master.try_clone_reader().map_err(|e| {
        AppError::new(AppErrorKind::PtySpawnFailed, format!("clone_reader: {e}"))
    })?;
    let master = Arc::new(Mutex::new(pty_pair.master));
    let writer = Arc::new(Mutex::new(writer));
    let child = Arc::new(Mutex::new(child));

    // 5. Wire the byte pipeline. Reader thread → mpsc → drainer task.
    let (chunk_tx, mut chunk_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let reader_handle = std::thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF — child closed the TTY
                Ok(n) => {
                    if chunk_tx.send(buf[..n].to_vec()).is_err() {
                        break; // drainer hung up
                    }
                }
                Err(_) => break,
            }
        }
    });
    // The reader_handle is owned by the drainer task; we join it on
    // drop so the thread doesn't leak.

    let (event_tx, event_rx) = mpsc::unbounded_channel::<PtyEvent>();

    // 6. Drainer task: byte stream → transcript + Tauri events. Also
    //    triggers prompt injection on first chunk.
    let writer_for_inject = writer.clone();
    let resolved_prompt = cfg.resolved_prompt.clone();
    let child_for_wait = child.clone();
    let drainer = tokio::spawn(async move {
        let mut seen_first = false;
        while let Some(chunk) = chunk_rx.recv().await {
            if !seen_first && !chunk.is_empty() {
                seen_first = true;
                if event_tx.send(PtyEvent::FirstOutput).is_err() {
                    break;
                }
                // Inject the bracketed-paste prompt immediately
                // after the first PTY output chunk fires.
                let inject_bytes = prompt_injector::build_inject_bytes(&resolved_prompt);
                let w = writer_for_inject.clone();
                let _ = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
                    let mut handle = w.lock();
                    handle.write_all(&inject_bytes)?;
                    handle.flush()?;
                    Ok(())
                })
                .await;
                // Errors on injection are non-fatal: the user can
                // still type the prompt manually. We swallow rather
                // than crash the drainer.
            }
            if let Err(e) = transcript.write(&chunk).await {
                let _ = event_tx.send(PtyEvent::Error(format!("transcript write: {e}")));
                break;
            }
            if event_tx.send(PtyEvent::Stdout(chunk)).is_err() {
                break;
            }
        }
        // Reader hit EOF or errored. Flush transcript, wait for child,
        // emit the terminal event.
        let _ = transcript.flush().await;
        let _ = reader_handle.join(); // reap the reader thread
        let (code, signal) =
            match tokio::task::spawn_blocking(move || child_for_wait.lock().wait()).await {
                Ok(Ok(status)) => {
                    let code = status.exit_code() as i32;
                    let signal = if !status.success() {
                        Some(format!("exit:{code}"))
                    } else {
                        None
                    };
                    (Some(code), signal)
                }
                _ => (None, None),
            };
        let _ = event_tx.send(PtyEvent::Exited {
            exit_code: code,
            signal,
        });
    });

    let handle = PtySessionHandle {
        run_id: cfg.run_id,
        pid,
        started_at,
        transcript_path,
        master,
        writer,
        child,
    };
    Ok((handle, event_rx, drainer))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end smoke: spawn `printf 'hello\n'` and verify we see
    /// the FirstOutput + Stdout(hello) + Exited events, and that the
    /// transcript file contains the bytes.
    #[tokio::test]
    async fn spawn_simple_command_streams_and_transcripts() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = PtySessionConfig {
            run_id: RunId("01TESTSPAWN".into()),
            claude_path: PathBuf::from("/usr/bin/printf"),
            args: vec!["hello\n".to_string()],
            cwd: tmp.path().to_path_buf(),
            resolved_prompt: String::new(),
            vault_root: tmp.path().to_path_buf(),
            cols: 80,
            rows: 24,
        };
        let (handle, mut events, drainer) = spawn(cfg).await.unwrap();

        let mut got_first = false;
        let mut got_stdout = false;
        let mut got_exited = false;
        let mut bytes = Vec::new();
        while let Some(ev) = events.recv().await {
            match ev {
                PtyEvent::FirstOutput => got_first = true,
                PtyEvent::Stdout(b) => {
                    got_stdout = true;
                    bytes.extend_from_slice(&b);
                }
                PtyEvent::Exited { .. } => {
                    got_exited = true;
                    break;
                }
                PtyEvent::Error(e) => panic!("unexpected error event: {e}"),
            }
        }
        let _ = drainer.await;
        assert!(got_first);
        assert!(got_stdout);
        assert!(got_exited);
        // PTY may add CR before LF, so check for "hello" substring rather
        // than exact match.
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("hello"), "expected 'hello' in {s:?}");

        // Transcript file exists and contains the bytes.
        let read = tokio::fs::read_to_string(&handle.transcript_path)
            .await
            .unwrap();
        assert!(read.contains("hello"), "transcript: {read:?}");
    }
}
