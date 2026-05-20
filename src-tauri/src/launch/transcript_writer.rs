//! Append-only transcript writer per spec §10 *Run record lifecycle*.
//!
//! The writer creates `<vault>/promptibrary/runs/YYYY/MM/<run_id>.md`
//! (parent dirs auto-created) and copies stdout bytes from the PTY to
//! the file in order. ANSI normalization for V1 is minimal — we write
//! the raw byte stream so xterm.js on the frontend gets the same
//! sequence the user would see in a real terminal. V2 follow-ups
//! covered in `docs/V2-CANDIDATES.md`:
//!   • strip OSC 52 (clipboard) + OSC 0/1/2 (title) sequences
//!   • preserve 24-bit color SGR + OSC 8 hyperlinks (already preserved
//!     by virtue of being raw passthrough, but a normalization pass
//!     would canonicalize the SGR forms)
//!
//! Files are opened with `OpenOptions::create_new` so we never
//! overwrite an existing transcript by accident (would indicate two
//! launches collided on the same run_id, which should be impossible
//! given ULID generation but we fail closed rather than silently
//! merge).

use std::path::{Path, PathBuf};

use chrono::{DateTime, Datelike, Utc};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;

use crate::error::{AppError, Result};

pub struct TranscriptWriter {
    path: PathBuf,
    file: tokio::fs::File,
    bytes_written: u64,
}

impl TranscriptWriter {
    /// Compute the canonical transcript path for a run given the
    /// vault root + run id + started_at timestamp.
    pub fn transcript_path(vault_root: &Path, run_id: &str, started_at: DateTime<Utc>) -> PathBuf {
        vault_root
            .join("promptibrary")
            .join("runs")
            .join(format!("{:04}", started_at.year()))
            .join(format!("{:02}", started_at.month()))
            .join(format!("{run_id}.md"))
    }

    /// Open a fresh transcript file at the canonical path. Errors if
    /// the file already exists.
    pub async fn open(
        vault_root: &Path,
        run_id: &str,
        started_at: DateTime<Utc>,
    ) -> Result<Self> {
        let path = Self::transcript_path(vault_root, run_id, started_at);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(AppError::from)?;
        }
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .await
            .map_err(AppError::from)?;
        Ok(Self {
            path,
            file,
            bytes_written: 0,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    pub async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        self.file.write_all(bytes).await.map_err(AppError::from)?;
        self.bytes_written = self.bytes_written.saturating_add(bytes.len() as u64);
        Ok(())
    }

    pub async fn flush(&mut self) -> Result<()> {
        self.file.flush().await.map_err(AppError::from)?;
        self.file.sync_all().await.map_err(AppError::from)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn transcript_path_uses_year_month_layout() {
        let vault = Path::new("/v");
        let when = Utc.with_ymd_and_hms(2026, 5, 19, 20, 0, 0).unwrap();
        let p = TranscriptWriter::transcript_path(vault, "01R1", when);
        assert_eq!(p, Path::new("/v/promptibrary/runs/2026/05/01R1.md"));
    }

    #[tokio::test]
    async fn open_creates_parent_dirs_and_file() {
        let tmp = tempfile::tempdir().unwrap();
        let when = Utc.with_ymd_and_hms(2026, 5, 19, 0, 0, 0).unwrap();
        let mut tw = TranscriptWriter::open(tmp.path(), "01R", when).await.unwrap();
        tw.write(b"hello").await.unwrap();
        tw.write(b" world\n").await.unwrap();
        tw.flush().await.unwrap();
        assert_eq!(tw.bytes_written(), 12);
        let read = tokio::fs::read_to_string(tw.path()).await.unwrap();
        assert_eq!(read, "hello world\n");
    }

    #[tokio::test]
    async fn open_refuses_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let when = Utc.with_ymd_and_hms(2026, 5, 19, 0, 0, 0).unwrap();
        let _t1 = TranscriptWriter::open(tmp.path(), "01R", when).await.unwrap();
        let t2 = TranscriptWriter::open(tmp.path(), "01R", when).await;
        assert!(t2.is_err(), "second open should fail with create_new");
    }
}
