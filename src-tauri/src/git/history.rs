//! Per-file commit history with rename detection per spec §10.
//!
//! Walks commits that touched a given path, following renames within
//! a bounded window (`versionHistory.renameDetectionWindow`, default
//! 200 per the patched §13). The window cap is mandatory: libgit2's
//! rename detection is `O(n²)` over the diffed file set, so an
//! unbounded walk on a long-lived vault is a real perf cliff.

use std::path::Path;

use git2::{DiffFindOptions, DiffOptions, Repository, Sort};

use crate::domain::git::{PromptHistoryEntry, ShortStat};
use crate::error::{AppError, AppErrorKind, Result};

/// Walk commits that touched `path` (with rename detection) and
/// return up to `window_size` entries newest-first. The window
/// applies both to the commit walk and to libgit2's rename-detection
/// scope.
pub fn get_file_history(
    repo: &Repository,
    path: &Path,
    window_size: usize,
) -> Result<Vec<PromptHistoryEntry>> {
    let mut revwalk = repo.revwalk().map_err(git_err)?;
    // Sort::TIME alone is non-deterministic when commits share a
    // second (real-world rare, test-suite common). Sort::TOPOLOGICAL
    // forces a child-before-parent walk so the rename-tracking
    // current_path update is consistent. The combined flag yields
    // newest-first topo order.
    revwalk
        .set_sorting(Sort::TIME | Sort::TOPOLOGICAL)
        .map_err(git_err)?;
    revwalk.push_head().map_err(git_err)?;

    let mut out: Vec<PromptHistoryEntry> = Vec::new();
    let mut current_path = path.to_path_buf();
    let mut visited = 0usize;

    for oid_result in revwalk {
        if visited >= window_size {
            break;
        }
        visited += 1;

        let oid = oid_result.map_err(git_err)?;
        let commit = repo.find_commit(oid).map_err(git_err)?;

        if commit.parent_count() > 1 {
            continue;
        }
        let parent_tree = if commit.parent_count() == 0 {
            None
        } else {
            Some(commit.parent(0).map_err(git_err)?.tree().map_err(git_err)?)
        };
        let this_tree = commit.tree().map_err(git_err)?;

        let mut diff = repo
            .diff_tree_to_tree(
                parent_tree.as_ref(),
                Some(&this_tree),
                Some(&mut DiffOptions::new()),
            )
            .map_err(git_err)?;

        let mut find_opts = DiffFindOptions::new();
        find_opts.renames(true).renames_from_rewrites(true);
        diff.find_similar(Some(&mut find_opts)).map_err(git_err)?;

        let mut hit = false;
        let mut next_path = current_path.clone();
        for delta in diff.deltas() {
            let new_p = delta.new_file().path();
            let old_p = delta.old_file().path();

            if new_p.map(|p| p == current_path).unwrap_or(false) {
                hit = true;
                if let Some(op) = old_p {
                    if op != current_path {
                        next_path = op.to_path_buf();
                    }
                }
                break;
            }
            if old_p.map(|p| p == current_path).unwrap_or(false) {
                hit = true;
                break;
            }
        }

        if !hit {
            continue;
        }

        let short_stat = compute_short_stat(&diff);
        let author = commit.author();
        let authored_at = chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
            .unwrap_or_else(chrono::Utc::now);
        let message = commit.message().unwrap_or("").to_string();

        out.push(PromptHistoryEntry {
            commit_sha: oid.to_string(),
            author_name: author.name().unwrap_or("").to_string(),
            author_email: author.email().unwrap_or("").to_string(),
            authored_at,
            message,
            short_stat,
        });

        current_path = next_path;
    }
    Ok(out)
}

fn compute_short_stat(diff: &git2::Diff) -> ShortStat {
    let stats = diff.stats().ok();
    match stats {
        Some(s) => ShortStat {
            files_changed: s.files_changed() as u32,
            insertions: s.insertions() as u32,
            deletions: s.deletions() as u32,
        },
        None => ShortStat {
            files_changed: 0,
            insertions: 0,
            deletions: 0,
        },
    }
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

    fn rename_and_commit(
        repo_path: &std::path::Path,
        from_rel: &str,
        to_rel: &str,
        message: &str,
    ) -> String {
        let repo = Repository::open(repo_path).unwrap();
        let from = repo_path.join(from_rel);
        let to = repo_path.join(to_rel);
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::rename(&from, &to).unwrap();
        let mut idx = repo.index().unwrap();
        idx.remove_path(std::path::Path::new(from_rel)).unwrap();
        idx.add_path(std::path::Path::new(to_rel)).unwrap();
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
    fn three_commits_to_same_file_yield_three_entries() {
        let (_dir, path) = init_test_repo();
        commit_file(&path, "prompt.md", "v1", "add");
        commit_file(&path, "prompt.md", "v2", "update");
        commit_file(&path, "prompt.md", "v3", "another update");

        let repo = Repository::open(&path).unwrap();
        let history = get_file_history(&repo, std::path::Path::new("prompt.md"), 200).unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].message.trim(), "another update");
        assert_eq!(history[2].message.trim(), "add");
    }

    #[test]
    fn rename_is_detected_within_window() {
        let (_dir, path) = init_test_repo();
        commit_file(&path, "old-name.md", "v1", "add under old name");
        rename_and_commit(&path, "old-name.md", "new-name.md", "rename");
        commit_file(&path, "new-name.md", "v2", "post-rename edit");

        let repo = Repository::open(&path).unwrap();
        let history = get_file_history(&repo, std::path::Path::new("new-name.md"), 200).unwrap();
        assert_eq!(history.len(), 3, "expected 3 entries, got {history:#?}");
    }

    #[test]
    fn window_cap_truncates_walk() {
        let (_dir, path) = init_test_repo();
        for i in 0..5 {
            commit_file(&path, "prompt.md", &format!("v{i}"), &format!("commit {i}"));
        }
        let repo = Repository::open(&path).unwrap();
        let history = get_file_history(&repo, std::path::Path::new("prompt.md"), 3).unwrap();
        assert!(history.len() <= 3);
    }
}
