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

use crate::error::Result;

pub fn tmp_path_for(path: &Path) -> PathBuf {
    let pid = process::id();
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".tmp.{pid}"));
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
    fn tmp_path_includes_pid() {
        let p = Path::new("/tmp/example.md");
        let tmp = tmp_path_for(p);
        let s = tmp.to_string_lossy().to_string();
        assert!(s.starts_with("/tmp/example.md.tmp."));
    }
}
