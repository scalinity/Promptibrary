//! Atomic write helper: tmp.<pid> → fsync → rename.
//!
//! Spec §3 contract for `util::atomic_write`. The rename is atomic on POSIX
//! filesystems; the explicit fsync before rename guarantees the data hits the
//! disk before another process can see the final path. We intentionally do
//! **not** fsync the directory — that path is not portable to Linux without
//! special handling, and the spec calls it out as unnecessary.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::Result;

/// SCA-916 (W4): monotonic process-local counter. Replaces the
/// previous `subsec_nanos()` nonce which was birthday-bound at
/// ~31_623 concurrent calls (and worse on coarse-clock platforms).
/// `fetch_add(Relaxed)` is a single atomic op — no contention.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn tmp_path_for(path: &Path) -> PathBuf {
    // SCA-614 + SCA-916: pid alone isn't enough — two threads writing
    // to the same path concurrently would produce identical tmp names
    // and race File::create / write_all / rename. The counter is
    // strictly monotonic so intra-process collisions are impossible.
    let pid = process::id();
    let nonce = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".tmp.{pid}.{nonce}"));
    PathBuf::from(tmp)
}

pub fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = tmp_path_for(path);

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(crate::error::AppError::new(
                crate::error::AppErrorKind::Internal,
                format!("parent directory does not exist: {}", parent.display()),
            ));
        }
    }

    let write_result = (|| -> std::io::Result<()> {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(crate::error::AppError::from(e));
    }

    if let Err(e) = fs::rename(&tmp, path) {
        return Err(crate::error::AppError::from(e));
    }

    Ok(())
}

pub fn atomic_write_string(path: &Path, contents: &str) -> Result<()> {
    atomic_write_bytes(path, contents.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::fs;

    #[test]
    fn round_trip_writes_full_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hello.txt");
        atomic_write_string(&path, "hello, world\n").unwrap();
        let read = fs::read_to_string(&path).unwrap();
        assert_eq!(read, "hello, world\n");
        assert!(!tmp_path_for(&path).exists());
    }

    #[test]
    fn replaces_existing_file_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hello.txt");
        atomic_write_string(&path, "first").unwrap();
        atomic_write_string(&path, "second longer payload").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second longer payload");
    }

    #[test]
    fn errors_when_parent_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope/hello.txt");
        let result = atomic_write_string(&path, "x");
        assert!(result.is_err(), "expected missing-parent error");
    }

    #[test]
    fn tmp_path_includes_pid_and_nonce() {
        let p = Path::new("/tmp/example.md");
        let tmp = tmp_path_for(p);
        let s = tmp.to_string_lossy().to_string();
        assert!(s.starts_with("/tmp/example.md.tmp."), "missing prefix: {s}");
        let suffix = s.trim_start_matches("/tmp/example.md.tmp.");
        let parts: Vec<&str> = suffix.split('.').collect();
        assert_eq!(parts.len(), 2, "expected pid.nonce: {s}");
        assert!(parts[0].parse::<u32>().is_ok(), "pid: {s}");
        assert!(parts[1].parse::<u64>().is_ok(), "nonce: {s}");
    }

    #[test]
    fn tmp_path_for_distinct_across_calls() {
        // SCA-614: two consecutive calls must yield distinct tmp paths.
        let p = Path::new("/tmp/example.md");
        let a = tmp_path_for(p);
        let b = tmp_path_for(p);
        assert_ne!(a, b);
    }

    /// SCA-916 (W4): under heavy concurrent pressure every tmp path
    /// must still be unique. With the previous subsec_nanos nonce this
    /// test would fail intermittently on macOS where clock resolution
    /// is coarser than the spawn rate.
    #[test]
    fn tmp_path_for_uniqueness_under_concurrency() {
        use std::sync::Arc;
        use std::sync::Mutex;
        use std::thread;

        let p = Arc::new(PathBuf::from("/tmp/example.md"));
        let collected: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
        let mut handles = vec![];
        for _ in 0..16 {
            let p = p.clone();
            let collected = collected.clone();
            handles.push(thread::spawn(move || {
                let mut local: Vec<PathBuf> = Vec::with_capacity(1024);
                for _ in 0..1024 {
                    local.push(tmp_path_for(&p));
                }
                let mut g = collected.lock().unwrap();
                for t in local {
                    g.insert(t);
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let g = collected.lock().unwrap();
        assert_eq!(g.len(), 16 * 1024, "every concurrent tmp path must be unique");
    }
}
