//! Debouncing helper used by the watcher.
//!
//! Spec §3 contract for `util::debounce`: coalesce events keyed by `K` within
//! a configured window. The watcher uses this to collapse rapid bursts (macOS
//! emits multiple events for an atomic rename; Obsidian/iCloud can re-emit
//! the same path multiple times within tens of milliseconds).
//!
//! Each `push(k)` resets the timer for that key. When `window` elapses with
//! no further `push(k)`, the receiver gets `k`.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct Debouncer<K>
where
    K: Eq + Hash + Clone + Send + 'static,
{
    window: Duration,
    inner: Arc<Mutex<HashMap<K, JoinHandle<()>>>>,
    tx: mpsc::Sender<K>,
}

impl<K> Debouncer<K>
where
    K: Eq + Hash + Clone + Send + 'static,
{
    pub fn new(window: Duration, tx: mpsc::Sender<K>) -> Self {
        Self {
            window,
            inner: Arc::new(Mutex::new(HashMap::new())),
            tx,
        }
    }

    pub async fn push(&self, key: K) {
        let mut map = self.inner.lock().await;
        if let Some(handle) = map.remove(&key) {
            handle.abort();
        }
        let window = self.window;
        let tx = self.tx.clone();
        let inner = self.inner.clone();
        let emit_key = key.clone();
        let payload = key.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(window).await;
            {
                let mut guard = inner.lock().await;
                guard.remove(&emit_key);
            }
            let _ = tx.send(payload).await;
        });
        map.insert(key, handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn single_push_fires_after_window() {
        let (tx, mut rx) = mpsc::channel::<u32>(16);
        let d = Debouncer::new(Duration::from_millis(100), tx);

        d.push(7).await;

        tokio::time::advance(Duration::from_millis(50)).await;
        assert!(rx.try_recv().is_err());

        tokio::time::advance(Duration::from_millis(60)).await;
        // Yield to let the spawned task wake.
        tokio::task::yield_now().await;
        let got = rx.recv().await.unwrap();
        assert_eq!(got, 7);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn rapid_pushes_coalesce_to_one_emit() {
        let (tx, mut rx) = mpsc::channel::<&'static str>(16);
        let d = Debouncer::new(Duration::from_millis(100), tx);

        d.push("a").await;
        tokio::time::advance(Duration::from_millis(50)).await;
        d.push("a").await;
        tokio::time::advance(Duration::from_millis(50)).await;
        d.push("a").await;
        tokio::time::advance(Duration::from_millis(110)).await;
        tokio::task::yield_now().await;

        let first = rx.recv().await.unwrap();
        assert_eq!(first, "a");
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn distinct_keys_emit_independently() {
        let (tx, mut rx) = mpsc::channel::<&'static str>(16);
        let d = Debouncer::new(Duration::from_millis(100), tx);

        d.push("a").await;
        d.push("b").await;
        tokio::time::advance(Duration::from_millis(150)).await;

        let mut got: Vec<&'static str> = vec![];
        got.push(rx.recv().await.unwrap());
        got.push(rx.recv().await.unwrap());
        got.sort();
        assert_eq!(got, vec!["a", "b"]);
    }
}
