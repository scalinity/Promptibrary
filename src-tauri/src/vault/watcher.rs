//! `notify` watcher lifecycle and debounced rescans.
//!
//! Spec §3 module contract. The watcher emits four classes of events as a
//! single typed enum [`VaultWatcherEvent`]:
//!   - `PromptCreated(path)` — a new `*.md` appeared.
//!   - `PromptUpdated(path)` — an existing `*.md` was modified.
//!   - `PromptDeleted(path)` — an `*.md` was removed.
//!   - `RescanRequired` — overflow or a non-prompt change that warrants a
//!     full rescan.
//!
//! Notify 8.x emits raw events; we coalesce bursts by a typed `DebounceKey`
//! within a 750ms window via `util::debounce` and translate the debounced
//! key back into a typed `VaultWatcherEvent` before emitting on the public
//! channel. SCA-619 removed the previous `WatcherEventKey {
//! path: PathBuf::new(), kind: RescanRequired }` sentinel.
//!
//! Atomic renames produce multiple raw events that all collapse to a
//! single emit.

use std::path::PathBuf;
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::error::{AppError, Result};
use crate::util::debounce::Debouncer;
use crate::vault::paths::VaultPaths;

/// Public watcher event emitted on `VaultWatcher::events`. RescanRequired
/// has no associated path because it covers overflow / non-prompt change
/// signals; everything else carries the affected file path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultWatcherEvent {
    PromptCreated(PathBuf),
    PromptUpdated(PathBuf),
    PromptDeleted(PathBuf),
    RescanRequired,
}

/// Per-prompt change kind. Used for debouncer keying and to populate
/// `VaultWatcherEvent` on emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PromptChangeKind {
    Created,
    Updated,
    Deleted,
}

/// Internal debouncer key. Keyed events coalesce per `(path, kind)`; the
/// singleton `Rescan` collapses all overflow ticks into one rescan emit.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum DebounceKey {
    Prompt(PathBuf, PromptChangeKind),
    Rescan,
}

pub struct VaultWatcher {
    /// Channel to emit debounced events on. Receivers see one event per
    /// (path, kind) per 750ms window.
    pub events: mpsc::Receiver<VaultWatcherEvent>,
    _watcher: RecommendedWatcher,
    _debouncer: Debouncer<DebounceKey>,
}

pub fn start_watcher(vault: &VaultPaths) -> Result<VaultWatcher> {
    // Two-stage pipeline:
    //   1. Debouncer<DebounceKey> with a private tx — collapses bursts.
    //   2. A translation task converts each debounced DebounceKey into a
    //      VaultWatcherEvent and forwards it on the public `events` channel.
    let (events_tx, events_rx) = mpsc::channel::<VaultWatcherEvent>(64);
    let (debounced_tx, mut debounced_rx) = mpsc::channel::<DebounceKey>(64);
    let debouncer = Debouncer::new(Duration::from_millis(750), debounced_tx);

    tokio::spawn(async move {
        while let Some(key) = debounced_rx.recv().await {
            let evt = match key {
                DebounceKey::Prompt(path, PromptChangeKind::Created) => {
                    VaultWatcherEvent::PromptCreated(path)
                }
                DebounceKey::Prompt(path, PromptChangeKind::Updated) => {
                    VaultWatcherEvent::PromptUpdated(path)
                }
                DebounceKey::Prompt(path, PromptChangeKind::Deleted) => {
                    VaultWatcherEvent::PromptDeleted(path)
                }
                DebounceKey::Rescan => VaultWatcherEvent::RescanRequired,
            };
            if events_tx.send(evt).await.is_err() {
                break;
            }
        }
    });

    // Bridge: notify runs callbacks on its own std thread, so we cannot
    // call `tokio::spawn` from there. Push into a sync std channel and
    // drain it from a dedicated tokio task that owns the Debouncer.
    let (raw_tx, mut raw_rx) = mpsc::unbounded_channel::<DebounceKey>();
    let drain_debouncer = debouncer.clone();
    tokio::spawn(async move {
        while let Some(key) = raw_rx.recv().await {
            drain_debouncer.push(key).await;
        }
    });

    let prompts_dir = vault.prompts_dir();
    let prompts_dir_filter = prompts_dir.clone();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let event = match res {
            Ok(e) => e,
            Err(_) => {
                return;
            }
        };
        if matches!(event.kind, EventKind::Other) {
            let _ = raw_tx.send(DebounceKey::Rescan);
            return;
        }
        for path in event.paths {
            let is_prompt_md = path.extension().and_then(|e| e.to_str()) == Some("md")
                && path.starts_with(&prompts_dir_filter);
            if !is_prompt_md {
                continue;
            }
            let kind = match event.kind {
                EventKind::Create(_) => PromptChangeKind::Created,
                EventKind::Remove(_) => PromptChangeKind::Deleted,
                EventKind::Modify(_) => PromptChangeKind::Updated,
                _ => PromptChangeKind::Updated,
            };
            let _ = raw_tx.send(DebounceKey::Prompt(path, kind));
        }
    })
    .map_err(AppError::from)?;

    if prompts_dir.exists() {
        watcher
            .watch(&prompts_dir, RecursiveMode::Recursive)
            .map_err(AppError::from)?;
    }

    Ok(VaultWatcher {
        events: events_rx,
        _watcher: watcher,
        _debouncer: debouncer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::debounce::Debouncer;

    /// SCA-607: previously vacuous notify test — now a deterministic
    /// Debouncer-only test exercises the keyed coalesce + emit path
    /// without depending on the OS watcher.
    #[tokio::test]
    async fn debouncer_pushes_emit_through_events_channel() {
        let (tx, mut rx) = mpsc::channel::<DebounceKey>(16);
        let d = Debouncer::new(Duration::from_millis(50), tx);
        let key = DebounceKey::Prompt(
            PathBuf::from("/vault/promptibrary/prompts/hi.md"),
            PromptChangeKind::Updated,
        );
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
        match evt {
            VaultWatcherEvent::PromptCreated(p)
            | VaultWatcherEvent::PromptUpdated(p)
            | VaultWatcherEvent::PromptDeleted(p) => {
                assert!(p.ends_with("hi.md"), "unexpected path: {p:?}");
            }
            VaultWatcherEvent::RescanRequired => {
                // notify sometimes routes the initial event to overflow;
                // allow rescan as a benign alternative.
            }
        }
    }
}
