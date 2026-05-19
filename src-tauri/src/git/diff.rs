//! Per-file diff between two commits per spec §10.
//!
//! Returns a unified-format diff string for a single path. The full
//! `PromptDiff` struct carries the prompt id, the optional `from_sha`
//! (None when comparing against the empty tree / first commit), and
//! the diff body.

use std::path::Path;

use git2::{DiffFormat, DiffOptions, Oid, Repository};

use crate::domain::git::PromptDiff;
use crate::error::{AppError, AppErrorKind, Result};
use crate::ids::PromptId;

/// Compute the unified diff for `path` between `from_sha` and
/// `to_sha`. If `from_sha` is None, diffs against the empty tree
/// (file added in `to_sha`).
pub fn get_file_diff(
    repo: &Repository,
    prompt_id: &PromptId,
    from_sha: Option<&str>,
    to_sha: &str,
    path: &Path,
) -> Result<PromptDiff> {
    let to_tree = {
        let oid = Oid::from_str(to_sha).map_err(|e| {
            AppError::new(
                AppErrorKind::GitError,
                format!("invalid to_sha {to_sha}: {}", e.message()),
            )
        })?;
        repo.find_commit(oid)
            .map_err(git_err)?
            .tree()
            .map_err(git_err)?
    };
    let from_tree = match from_sha {
        Some(sha) => {
            let oid = Oid::from_str(sha).map_err(|e| {
                AppError::new(
                    AppErrorKind::GitError,
                    format!("invalid from_sha {sha}: {}", e.message()),
                )
            })?;
            Some(
                repo.find_commit(oid)
                    .map_err(git_err)?
                    .tree()
                    .map_err(git_err)?,
            )
        }
        None => None,
    };

    let mut opts = DiffOptions::new();
    opts.pathspec(path);
    let diff = repo
        .diff_tree_to_tree(from_tree.as_ref(), Some(&to_tree), Some(&mut opts))
        .map_err(git_err)?;

    let mut buf = String::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        let origin = line.origin();
        match origin {
            '+' | '-' | ' ' => buf.push(origin),
            _ => {}
        }
        let content = std::str::from_utf8(line.content()).unwrap_or("");
        buf.push_str(content);
        true
    })
    .map_err(git_err)?;

    Ok(PromptDiff {
        prompt_id: prompt_id.clone(),
        from_sha: from_sha.map(|s| s.to_string()),
        to_sha: to_sha.to_string(),
        unified: buf,
    })
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
        repo_path: &std::path::Path,
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
        idx.add_path(std::path::Path::new(rel_path)).unwrap();
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
    fn diff_between_two_commits_contains_added_line() {
        let (_dir, path) = init_test_repo();
        let sha_a = commit_file(&path, "p.md", "hello\n", "v1");
        let sha_b = commit_file(&path, "p.md", "hello\nworld\n", "v2");

        let repo = Repository::open(&path).unwrap();
        let diff = get_file_diff(
            &repo,
            &PromptId("01TEST".into()),
            Some(&sha_a),
            &sha_b,
            std::path::Path::new("p.md"),
        )
        .unwrap();
        assert!(diff.unified.contains("+world"), "got: {}", diff.unified);
    }

    #[test]
    fn diff_against_empty_tree_shows_file_addition() {
        let (_dir, path) = init_test_repo();
        let sha = commit_file(&path, "new.md", "first\n", "add");
        let repo = Repository::open(&path).unwrap();
        let diff = get_file_diff(
            &repo,
            &PromptId("01NEW".into()),
            None,
            &sha,
            std::path::Path::new("new.md"),
        )
        .unwrap();
        assert!(diff.unified.contains("+first"));
        assert!(diff.from_sha.is_none());
    }

    #[test]
    fn invalid_sha_errors_with_git_kind() {
        let (_dir, path) = init_test_repo();
        let repo = Repository::open(&path).unwrap();
        let err = get_file_diff(
            &repo,
            &PromptId("01".into()),
            None,
            "not-a-sha",
            std::path::Path::new("nope.md"),
        )
        .unwrap_err();
        assert_eq!(err.kind, AppErrorKind::GitError);
    }
}
