//! Filesystem helpers scoped to the vault.

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppErrorKind, Result};

pub fn canonicalize_existing(path: &Path) -> Result<PathBuf> {
    std::fs::canonicalize(path).map_err(|e| {
        AppError::new(
            AppErrorKind::Internal,
            format!("canonicalize {}: {}", path.display(), e),
        )
    })
}

pub fn is_within(parent: &Path, child: &Path) -> bool {
    let p: Vec<_> = parent.components().collect();
    let c: Vec<_> = child.components().collect();
    if c.len() < p.len() {
        return false;
    }
    p.iter().zip(c.iter()).all(|(a, b)| a == b)
}

pub fn read_dir_tolerant(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        if let Ok(e) = entry {
            out.push(e.path());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_within_same_path_is_true() {
        let p = Path::new("/Users/x/vault");
        assert!(is_within(p, p));
    }

    #[test]
    fn is_within_descendant_is_true() {
        let parent = Path::new("/Users/x/vault");
        let child = Path::new("/Users/x/vault/promptibrary/prompts/a.md");
        assert!(is_within(parent, child));
    }

    #[test]
    fn is_within_sibling_is_false() {
        let parent = Path::new("/Users/x/vault");
        let other = Path::new("/Users/x/other/prompts/a.md");
        assert!(!is_within(parent, other));
    }

    #[test]
    fn is_within_partial_string_match_is_false() {
        let parent = Path::new("/Users/x/vault");
        let other = Path::new("/Users/x/vault-evil/prompts/a.md");
        assert!(!is_within(parent, other));
    }

    #[test]
    fn read_dir_tolerant_returns_existing_entries() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "x").unwrap();
        std::fs::write(dir.path().join("b.md"), "y").unwrap();
        let entries = read_dir_tolerant(dir.path()).unwrap();
        assert_eq!(entries.len(), 2);
    }
}
