//! Revert a prompt file to a selected commit blob per spec §10.
//!
//! Read the blob at `<commit_sha>:<vault_relative_path>`, validate
//! that the contents parse as a Promptibrary prompt (frontmatter +
//! body), then atomic-write the blob back to the current file path
//! via `vault::writer`. DO NOT auto-commit — per spec §10 the user
//! decides when to commit the reverted state.
//!
//! Validation matters: if the blob at the commit pre-dates a schema
//! change, parsing fails and we return the validation error rather
//! than overwriting the user's current file with un-parseable data.

use std::path::{Path, PathBuf};

use git2::{Oid, Repository};

use crate::error::{AppError, AppErrorKind, Result};
use crate::util::atomic_write::atomic_write_bytes;

/// Read the blob for `vault_relative_path` at `commit_sha`, validate
/// it parses as a Promptibrary prompt, then atomic-write to
/// `target_path` (typically `<vault_root>/<vault_relative_path>`).
/// Returns the raw blob bytes so the caller can re-index immediately.
///
/// `validate_fn` is injected by the caller so this module doesn't
/// need a hard dep on the vault parser — the L1 vault parser will
/// be passed in by `commands::git::revert_prompt_to_commit`.
pub fn revert_file_to_commit<V>(
    repo: &Repository,
    commit_sha: &str,
    vault_relative_path: &Path,
    target_path: &Path,
    validate_fn: V,
) -> Result<Vec<u8>>
where
    V: FnOnce(&[u8]) -> Result<()>,
{
    let oid = Oid::from_str(commit_sha).map_err(|e| {
        AppError::new(
            AppErrorKind::GitError,
            format!("invalid commit_sha {commit_sha}: {}", e.message()),
        )
    })?;
    let commit = repo.find_commit(oid).map_err(git_err)?;
    let tree = commit.tree().map_err(git_err)?;
    let entry = tree.get_path(vault_relative_path).map_err(|e| {
        AppError::new(
            AppErrorKind::GitError,
            format!(
                "blob not found at {} in commit {commit_sha}: {}",
                vault_relative_path.display(),
                e.message()
            ),
        )
    })?;
    let blob = repo.find_blob(entry.id()).map_err(git_err)?;
    let bytes = blob.content().to_vec();

    // Validate BEFORE writing — refuses to clobber the live file with
    // an un-parseable older blob.
    validate_fn(&bytes)?;

    atomic_write_bytes(target_path, &bytes)?;
    Ok(bytes)
}

fn git_err(e: git2::Error) -> AppError {
    AppError::new(AppErrorKind::GitError, format!("git: {}", e.message()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::repo::tests::init_test_repo;
    use git2::Signature;
    use std::fs;

    fn commit_file(
        repo_path: &Path,
        rel_path: &str,
        contents: &str,
        message: &str,
    ) -> String {
        let repo = Repository::open(repo_path).unwrap();
        let full = repo_path.join(rel_path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full, contents).unwrap();
        let mut idx = repo.index().unwrap();
        idx.add_path(Path::new(rel_path)).unwrap();
        let tree_id = idx.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let oid = repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])
            .unwrap();
        idx.write().unwrap();
        oid.to_string()
    }

    #[test]
    fn revert_restores_old_blob_to_current_path() {
        let (_dir, path) = init_test_repo();
        let sha_a = commit_file(&path, "prompt.md", "old content\n", "v1");
        commit_file(&path, "prompt.md", "new content\n", "v2");

        // Current file holds the new content.
        let target = path.join("prompt.md");
        assert_eq!(fs::read_to_string(&target).unwrap(), "new content\n");

        let repo = Repository::open(&path).unwrap();
        let bytes = revert_file_to_commit(
            &repo,
            &sha_a,
            Path::new("prompt.md"),
            &target,
            |_blob| Ok(()),
        )
        .unwrap();
        assert_eq!(bytes, b"old content\n");
        // File on disk now matches the old blob.
        assert_eq!(fs::read_to_string(&target).unwrap(), "old content\n");
    }

    #[test]
    fn revert_refuses_to_overwrite_on_validation_failure() {
        let (_dir, path) = init_test_repo();
        let sha_a = commit_file(&path, "prompt.md", "old content\n", "v1");
        commit_file(&path, "prompt.md", "new content\n", "v2");

        let target = path.join("prompt.md");
        let repo = Repository::open(&path).unwrap();
        let err = revert_file_to_commit(
            &repo,
            &sha_a,
            Path::new("prompt.md"),
            &target,
            |_blob| {
                Err(AppError::new(
                    AppErrorKind::PromptMalformed,
                    "simulated parse failure",
                ))
            },
        )
        .unwrap_err();
        assert_eq!(err.kind, AppErrorKind::PromptMalformed);
        // Live file NOT overwritten.
        assert_eq!(fs::read_to_string(&target).unwrap(), "new content\n");
    }

    #[test]
    fn revert_unknown_commit_errors() {
        let (_dir, path) = init_test_repo();
        commit_file(&path, "p.md", "x", "init prompt");
        let repo = Repository::open(&path).unwrap();
        let err = revert_file_to_commit(
            &repo,
            "deadbeef0000000000000000000000000000beef",
            Path::new("p.md"),
            &path.join("p.md"),
            |_| Ok(()),
        )
        .unwrap_err();
        assert_eq!(err.kind, AppErrorKind::GitError);
    }
}
