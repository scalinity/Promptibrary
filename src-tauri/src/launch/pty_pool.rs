//! Active PTY registry keyed by `RunId`.
//!
//! `PtyPool` is mounted on `AppServices` and holds every live
//! `PtySessionHandle`. `commands::launches::start_launch` inserts a
//! session at spawn time; `stop_run` / natural exit removes it.
//! `send_terminal_input` and `resize_terminal` look up the session by
//! `RunId` and forward to its handle.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::error::{AppError, AppErrorKind, Result};
use crate::ids::RunId;
use crate::launch::pty_session::PtySessionHandle;

#[derive(Default)]
pub struct PtyPool {
    inner: RwLock<HashMap<String, Arc<PtySessionHandle>>>,
}

impl PtyPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert(&self, session: Arc<PtySessionHandle>) {
        let key = session.run_id.0.clone();
        self.inner.write().await.insert(key, session);
    }

    pub async fn remove(&self, run_id: &RunId) -> Option<Arc<PtySessionHandle>> {
        self.inner.write().await.remove(&run_id.0)
    }

    pub async fn get(&self, run_id: &RunId) -> Result<Arc<PtySessionHandle>> {
        self.inner
            .read()
            .await
            .get(&run_id.0)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::RunNotActive,
                    format!("no active PTY session for run {}", run_id.0),
                )
            })
    }

    pub async fn active_count(&self) -> usize {
        self.inner.read().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::pty_session::PtySessionConfig;
    use std::path::PathBuf;

    #[tokio::test]
    async fn insert_get_remove_round_trip() {
        let pool = PtyPool::new();
        let tmp = tempfile::tempdir().unwrap();
        let cfg = PtySessionConfig {
            run_id: RunId("01POOLTEST".into()),
            claude_path: PathBuf::from("/usr/bin/printf"),
            args: vec!["pool-test\n".into()],
            cwd: tmp.path().to_path_buf(),
            resolved_prompt: String::new(),
            vault_root: tmp.path().to_path_buf(),
            cols: 80,
            rows: 24,
        };
        let (handle, _events, drainer) =
            crate::launch::pty_session::spawn(cfg).await.unwrap();
        let arc_handle = Arc::new(handle);
        pool.insert(arc_handle.clone()).await;
        assert_eq!(pool.active_count().await, 1);

        let got = pool.get(&RunId("01POOLTEST".into())).await.unwrap();
        assert_eq!(got.run_id.0, "01POOLTEST");

        let removed = pool.remove(&RunId("01POOLTEST".into())).await;
        let was_some = removed.is_some();
        assert!(was_some, "expected handle present on remove");
        assert_eq!(pool.active_count().await, 0);

        // Drain the drainer so the test doesn't leak.
        let _ = drainer.await;
    }

    #[tokio::test]
    async fn get_unknown_run_errors_with_not_active() {
        let pool = PtyPool::new();
        match pool.get(&RunId("01MISSING".into())).await {
            Ok(_) => panic!("expected RunNotActive error"),
            Err(e) => assert_eq!(e.kind, AppErrorKind::RunNotActive),
        }
    }
}
