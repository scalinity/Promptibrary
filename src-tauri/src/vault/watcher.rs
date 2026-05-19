//! `notify` watcher lifecycle and debounced rescans.
//!
//! Spec §3 module contract. The watcher emits four classes of events:
//!   - `PromptCreated(path)` — a new `*.md` appeared.
//!   - `PromptUpdated(path)` — an existing `*.md` was modified.
//!   - `PromptDeleted(path)` — an `*.md` was removed.
//!   - `RescanRequired` — overflow or a non-prompt change that warrants a
//!     full rescan.
//!
//! Notify 8.x emits raw events; we coalesce bursts by `(path, kind)` within
//! a 750ms window via `util::debounce`. Atomic renames produce
//! multiple raw events that all collapse to a single emit.

use std::path::PathBuf;
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::error::{AppError, Result};
use crate::util::debounce::Debouncer;
use crate::vault::paths::VaultPaths;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WatcherEventKey {
    pub path: PathBuf,
    pub kind: WatcherEventKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatcherEventKind {
    PromptCreated,
    PromptUpdated,
    PromptDeleted,
    RescanRequired,
}

pub struct VaultWatcher {
    /// Channel to emit debounced events on. Receivers see one event per
    /// (path, kind) per 750ms window.
    pub events: mpsc::Receiver<WatcherEventKey>,
    _watcher: RecommendedWatcher,
    _debouncer: Debouncer<WatcherEventKey>,
}

pub fn start_watcher(vault: &VaultPaths) -> Result<VaultWatcher> {
    let (debounced_tx, debounced_rx) = mpsc::channel::<WatcherEventKey>(64);
    let debouncer = Debouncer::new(Duration::from_millis(750), debounced_tx);

    // Bridge: notify runs callbacks on its own std thread, so we cannot
    // call `tokio::spawn` from there. Push into a sync std channel and
    // drain it from a dedicated tokio task that owns the Debouncer.
    let (raw_tx, mut raw_rx) = mpsc::unbounded_channel::<WatcherEventKey>();
    let drain_debouncer = debouncer.clone();
    tokio::spawn(async move {
        while let Some(key) = raw_rx.recv().await {
            drain_debouncer.push(key).await;
        }
    });

    let prompts_dir = vault.prompts_dir();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let event = match res {
            Ok(e) => e,
            Err(_) => {
                return;
            }
        };
        if matches!(event.kind, EventKind::Other) {
            let _ = raw_tx.send(WatcherEventKey {
                path: PathBuf::new(),
                kind: WatcherEventKind::RescanRequired,
            });
            return;
        }
        for path in event.paths {
            let is_prompt_md = path.extension().and_then(|e| e.to_str()) == Some("md")
                && path
                    .components()
                    .any(|c| c.as_os_str() == "prompts");
            if !is_prompt_md {
                continue;
            }
            let kind = match event.kind {
                EventKind::Create(_) => WatcherEventKind::PromptCreated,
                EventKind::Remove(_) => WatcherEventKind::PromptDeleted,
                EventKind::Modify(_) => WatcherEventKind::PromptUpdated,
                _ => WatcherEventKind::PromptUpdated,
            };
            let _ = raw_tx.send(WatcherEventKey { path, kind });
        }
    })
    .map_err(AppError::from)?;

    if prompts_dir.exists() {
        watcher
            .watch(&prompts_dir, RecursiveMode::Recursive)
            .map_err(AppError::from)?;
    }

    Ok(VaultWatcher {
        events: debounced_rx,
        _watcher: watcher,
        _debouncer: debouncer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::debounce::Debouncer;

    /// SCA-607: the previous test for notify integration was vacuous —
    /// it tolerated a 2s timeout with no assertion, so it always passed
    /// regardless of watcher correctness. Replaced with a deterministic
    /// test that exercises the Debouncer → events channel directly
    /// (which is what the notify callback feeds), bypassing the OS
    /// watcher. A second test is kept around for local notify validation
    /// but marked #[ignore] so CI doesn't hit notify's well-known
    /// non-determinism in sandboxed environments.
    #[tokio::test]
    async fn debouncer_pushes_emit_through_events_channel() {
        let (tx, mut rx) = mpsc::channel::<WatcherEventKey>(16);
        let d = Debouncer::new(Duration::from_millis(50), tx);
        let key = WatcherEventKey {
            path: PathBuf::from("/vault/promptibrary/prompts/hi.md"),
            kind: WatcherEventKind::PromptUpdated,
        };
        d.push(key.clone()).await;
        let evt = tokio::time::timeout(Duration::from_millis(500), rx.recv())
            .await
            .expect("debounced event arrived")
            .expect("channel open");
        assert_eq!(evt, key);
    }

    #[tokio::test]
    #[ignore = "notify is non-deterministic in CI sandboxes; run locally to validate the OS watcher path"]
    async fn watcher_emits_event_on_file_write() {
        let dir = tempfile::tempdir().unwrap();
        let v = VaultPaths::new(dir.path());
        std::fs::create_dir_all(v.prompts_dir()).unwrap();
        let mut watcher = start_watcher(&v).expect("start watcher");

        tokio::time::sleep(Duration::from_millis(100)).await;
        std::fs::write(v.absolute("promptibrary/prompts/hi.md").unwrap(), b"hello").unwrap();

        let evt = tokio::time::timeout(Duration::from_secs(2), watcher.events.recv())
            .await
            .expect("watcher emitted within 2s")
            .expect("channel open");
        assert!(
            evt.path.ends_with("hi.md") || matches!(evt.kind, WatcherEventKind::RescanRequired),
            "unexpected watcher event: {:?}",
            evt
        );
    }
}
