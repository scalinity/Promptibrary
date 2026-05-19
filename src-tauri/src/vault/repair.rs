//! Orphan and spool transcript recovery.
//!
//! Spec §3 module contract for `vault::repair`. Two responsibilities:
//!   - `repair_orphaned_transcripts` — move files from the app-support
//!     orphan spool path back into `<vault>/promptibrary/runs/...` when
//!     the vault is available again.
//!   - `repair_missing_dirs` — re-create the canonical four
//!     `<vault>/promptibrary/{prompts,runs,exports,extraction-cache}`
//!     directories if any are missing.

use std::path::PathBuf;

use crate::error::Result;
use crate::vault::paths::VaultPaths;

#[derive(Debug, Clone)]
pub struct RepairSummary {
    pub orphans_moved: usize,
    pub dirs_created: usize,
}

pub fn repair_missing_dirs(vault: &VaultPaths) -> Result<RepairSummary> {
    let mut created = 0usize;
    for dir in [
        vault.prompts_dir(),
        vault.runs_dir(),
        vault.exports_dir(),
        vault.extraction_cache_dir(),
    ] {
        if !dir.exists() {
            std::fs::create_dir_all(&dir).map_err(crate::error::AppError::from)?;
            created += 1;
        }
    }
    Ok(RepairSummary {
        orphans_moved: 0,
        dirs_created: created,
    })
}

/// Walk `orphan_root` for `<run_id>.md` files and move each into the
/// `<vault>/promptibrary/runs/YYYY/MM/DD/<run_id>.md` slot, using the
/// transcript's `started_at` frontmatter date. Files we can't classify
/// (no frontmatter, malformed) stay in the orphan path so a human can
/// inspect them.
pub fn repair_orphaned_transcripts(
    vault: &VaultPaths,
    orphan_root: &PathBuf,
) -> Result<RepairSummary> {
    let mut moved = 0usize;
    if !orphan_root.exists() {
        return Ok(RepairSummary {
            orphans_moved: 0,
            dirs_created: 0,
        });
    }
    for entry in std::fs::read_dir(orphan_root).map_err(crate::error::AppError::from)? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let parsed = match crate::vault::markdown::parse_markdown_document(&content) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let fm = match crate::vault::frontmatter::parse_run_frontmatter(&parsed.frontmatter_yaml) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let rel = crate::vault::paths::run_path_for_date(&fm.id, fm.started_at);
        let dest = vault.absolute(&rel);
        if let Some(parent) = dest.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(crate::error::AppError::from)?;
            }
        }
        // Use atomic rename when on same filesystem; fall back to copy + delete.
        if std::fs::rename(&path, &dest).is_err() {
            std::fs::copy(&path, &dest).map_err(crate::error::AppError::from)?;
            let _ = std::fs::remove_file(&path);
        }
        moved += 1;
    }
    Ok(RepairSummary {
        orphans_moved: moved,
        dirs_created: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_missing_dirs_creates_all_four() {
        let dir = tempfile::tempdir().unwrap();
        let v = VaultPaths::new(dir.path());
        let summary = repair_missing_dirs(&v).unwrap();
        assert_eq!(summary.dirs_created, 4);
        assert!(v.prompts_dir().exists());
        assert!(v.runs_dir().exists());
        assert!(v.exports_dir().exists());
        assert!(v.extraction_cache_dir().exists());

        // Second call is a no-op.
        let summary2 = repair_missing_dirs(&v).unwrap();
        assert_eq!(summary2.dirs_created, 0);
    }

    #[test]
    fn repair_orphaned_transcripts_moves_classified_files() {
        let vault_dir = tempfile::tempdir().unwrap();
        let orphan_dir = tempfile::tempdir().unwrap();
        let v = VaultPaths::new(vault_dir.path());
        repair_missing_dirs(&v).unwrap();

        let orphan_file = orphan_dir.path().join("01RUN.md");
        let content = "---\n\
            promptibrary_schema: 1\n\
            id: 01RUN\n\
            prompt_id: 01PROMPT\n\
            started_at: 2026-05-18T14:00:00Z\n\
            ---\n\
            transcript body\n";
        std::fs::write(&orphan_file, content).unwrap();

        let summary = repair_orphaned_transcripts(&v, &orphan_dir.path().to_path_buf()).unwrap();
        assert_eq!(summary.orphans_moved, 1);
        assert!(!orphan_file.exists());
        assert!(v
            .absolute("promptibrary/runs/2026/05/18/01RUN.md")
            .exists());
    }
}
