//! Open a vault Git repo per spec §10 *Versioning*.
//!
//! Returns `AppErrorKind::VaultNotGitRepo` if the path isn't a Git
//! working tree. Promptibrary writes prompts as Markdown files and
//! relies on the user's existing Git workflow for history; we never
//! initialise a repo on the user's behalf — if the vault isn't
//! already a Git repo, history/diff/revert features are simply
//! unavailable for that vault.

use std::path::Path;

use git2::Repository;

use crate::error::{AppError, AppErrorKind, Result};

/// Open the Git repo containing `vault_root`. The repo discovery walks
/// up the directory tree, so a vault stored inside a parent monorepo
/// also resolves cleanly.
pub fn open_repo(vault_root: &Path) -> Result<Repository> {
    Repository::discover(vault_root).map_err(|e| {
        AppError::new(
            AppErrorKind::VaultNotGitRepo,
            format!(
                "git open: {} is not inside a Git working tree: {}",
                vault_root.display(),
                e.message()
            ),
        )
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use git2::Signature;
    use std::path::PathBuf;

    /// Spin up a fresh real Git repo in a tempdir and return its
    /// working-tree root with an initial empty commit so HEAD exists.
    pub(crate) fn init_test_repo() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let repo = Repository::init(&path).unwrap();
        let sig = Signature::now("Test", "test@example.com").unwrap();
        let tree_id = {
            let mut idx = repo.index().unwrap();
            idx.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();
        (dir, path)
    }

    #[test]
    fn open_repo_on_real_dir_succeeds() {
        let (_dir, path) = init_test_repo();
        let _repo = open_repo(&path).unwrap();
    }

    #[test]
    fn open_repo_on_non_repo_dir_errors_with_kind() {
        let dir = tempfile::tempdir().unwrap();
        match open_repo(dir.path()) {
            Ok(_) => panic!("expected VaultNotGitRepo error on non-repo dir"),
            Err(e) => assert_eq!(e.kind, AppErrorKind::VaultNotGitRepo),
        }
    }

    #[test]
    fn open_repo_walks_up_to_find_root() {
        let (_dir, path) = init_test_repo();
        let nested = path.join("nested").join("deep");
        std::fs::create_dir_all(&nested).unwrap();
        let _repo = open_repo(&nested).unwrap();
    }
}
