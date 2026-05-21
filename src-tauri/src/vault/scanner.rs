//! Cold scans and incremental scans of the vault.
//!
//! Spec §3 module contract. `scan_vault` walks `<vault>/promptibrary/prompts/`,
//! parses each `*.md` into a `Prompt`, upserts into SQLite, and deletes
//! index rows whose vault_path no longer exists on disk.
//!
//! Progress: callers can pass a `ScanProgress` callback. The scanner emits
//! every 50 files or every 200ms whichever comes first. The IPC layer
//! adapts this to the `index://progress` event in L1.

use std::time::{Duration, Instant};

use sqlx::SqlitePool;

use crate::domain::prompt::Prompt;
use crate::error::{AppError, Result};
use crate::ids::PromptId;
use crate::index::prompts_repo;
use crate::vault::frontmatter::parse_prompt_frontmatter;
use crate::vault::markdown::parse_markdown_document;
use crate::vault::paths::VaultPaths;

#[derive(Debug, Clone, Default)]
pub struct ScanSummary {
    pub scanned_files: usize,
    pub indexed_prompts: usize,
    pub malformed_files: usize,
    pub deleted_rows: usize,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, Copy)]
pub struct ScanProgress {
    pub scanned: usize,
    pub indexed: usize,
    pub malformed: usize,
}

pub async fn scan_vault<F: FnMut(ScanProgress)>(
    vault: &VaultPaths,
    db: &SqlitePool,
    mut on_progress: F,
) -> Result<ScanSummary> {
    let started = Instant::now();
    let prompts_dir = vault.prompts_dir();
    if !prompts_dir.exists() {
        return Ok(ScanSummary {
            duration_ms: started.elapsed().as_millis(),
            ..Default::default()
        });
    }

    // Collect candidates first so the borrow into the DB happens cleanly.
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&prompts_dir).map_err(AppError::from)? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("md") {
            paths.push(p);
        }
    }

    let mut summary = ScanSummary::default();
    // `on_disk_paths` — every vault_path the walk *touched*, regardless of
    // parse outcome. SCA-592: a transiently-malformed file must NOT cause
    // its index row to be deleted; the file is still on disk and the row
    // is still authoritative until the file is fixed.
    let mut on_disk_paths: Vec<String> = Vec::with_capacity(paths.len());
    let mut last_emit = Instant::now();
    let mut last_emit_count = 0usize;

    for path in paths {
        summary.scanned_files += 1;
        // Record the path as on-disk *before* any parse attempt, so the
        // stale-delete pass below doesn't drop the row on a transient
        // YAML error mid-edit.
        if let Some(rel) = vault.to_relative(&path) {
            on_disk_paths.push(rel);
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => {
                summary.malformed_files += 1;
                continue;
            }
        };
        let parsed = match parse_markdown_document(&content) {
            Ok(p) => p,
            Err(_) => {
                summary.malformed_files += 1;
                continue;
            }
        };
        let fm = match parse_prompt_frontmatter(&parsed.frontmatter_yaml) {
            Ok(f) => f,
            Err(_) => {
                summary.malformed_files += 1;
                continue;
            }
        };

        let vault_path = match vault.to_relative(&path) {
            Some(rel) => rel,
            None => {
                summary.malformed_files += 1;
                continue;
            }
        };
        // Already recorded above for the on-disk set; nothing more to do here.

        let body = parsed.body.clone();
        let prompt = Prompt {
            id: PromptId(fm.id),
            title: fm.title,
            slug: fm.slug,
            summary: fm.summary,
            body: body.clone(),
            vault_path: vault_path.clone(),
            created_at: fm.created_at,
            updated_at: fm.updated_at,
            archived_at: fm.archived_at,
            tags: fm.tags,
            source: fm.source,
            variables: fm.variables,
            launch_defaults: fm.launch_defaults.unwrap_or_default(),
            telemetry: crate::domain::prompt::PromptTelemetrySummary {
                launch_count: 0,
                last_used_at: None,
                success_rate: None,
                avg_run_seconds: None,
                avg_token_count: None,
            },
            checksum_sha256: format!("sha256:{}", crate::util::hash::sha256_hex(&content)),
        };

        if let Err(e) = prompts_repo::upsert_prompt(db, &prompt).await {
            // Convert SQLite errors to malformed-file count — the file is
            // ok on disk but we couldn't persist it. Surface the count
            // back up; specific row-level errors are logged via tracing.
            tracing::warn!(error = ?e, vault_path = %vault_path, "upsert failed during scan");
            summary.malformed_files += 1;
            continue;
        }
        summary.indexed_prompts += 1;
        let _ = vault_path;

        // Throttled progress emission.
        let elapsed_since = last_emit.elapsed();
        let new_files = summary.scanned_files - last_emit_count;
        if new_files >= 50 || elapsed_since >= Duration::from_millis(200) {
            on_progress(ScanProgress {
                scanned: summary.scanned_files,
                indexed: summary.indexed_prompts,
                malformed: summary.malformed_files,
            });
            last_emit = Instant::now();
            last_emit_count = summary.scanned_files;
        }
    }

    // Delete index rows whose vault_path is absent from disk. We compare
    // against `on_disk_paths` (every file the walk touched) — NOT only the
    // successfully-parsed files (SCA-592). A single DELETE by vault_path
    // suffices; FK CASCADE handles prompt_tags (SCA-593, removes the prior
    // SELECT id → DELETE id round-trip and its inter-statement race).
    //
    // SCA-922 (W23): build a HashSet for O(1) membership lookup (was
    // O(N·M) linear-scan over on_disk_paths per existing row), and
    // batch the DELETE into chunks of 500 inside a single transaction
    // (was N individual round-trips fighting the WAL writer).
    let existing_paths: Vec<String> =
        sqlx::query_scalar("SELECT vault_path FROM prompts WHERE vault_path LIKE ?")
            .bind("promptibrary/prompts/%")
            .fetch_all(db)
            .await
            .map_err(AppError::from)?;
    let disk_set: std::collections::HashSet<&str> =
        on_disk_paths.iter().map(String::as_str).collect();
    let stale: Vec<&str> = existing_paths
        .iter()
        .filter(|p| !disk_set.contains(p.as_str()))
        .map(String::as_str)
        .collect();
    if !stale.is_empty() {
        let mut tx = db.begin().await.map_err(AppError::from)?;
        for chunk in stale.chunks(500) {
            let placeholders = std::iter::repeat("?")
                .take(chunk.len())
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "DELETE FROM prompts WHERE vault_path IN ({placeholders})"
            );
            let mut q = sqlx::query(&sql);
            for path in chunk {
                q = q.bind(*path);
            }
            let result = q.execute(&mut *tx).await.map_err(AppError::from)?;
            summary.deleted_rows += result.rows_affected() as usize;
        }
        tx.commit().await.map_err(AppError::from)?;
    }

    summary.duration_ms = started.elapsed().as_millis();
    // Emit a final progress tick.
    on_progress(ScanProgress {
        scanned: summary.scanned_files,
        indexed: summary.indexed_prompts,
        malformed: summary.malformed_files,
    });
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::db::in_memory_connect_options;
    use crate::index::migrations::run_migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn temp_pool() -> SqlitePool {
        // SCA-598: in-memory SQLite (was tempfile + mem::forget, which
        // leaked a dir per test run on CI).
        let opts = in_memory_connect_options();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        run_migrations(&pool).await.unwrap();
        pool
    }

    fn write_prompt_file(vault: &VaultPaths, slug: &str, body: &str, archived: bool, id: &str) {
        std::fs::create_dir_all(vault.prompts_dir()).unwrap();
        let path = vault.absolute(&format!("promptibrary/prompts/{slug}.md")).unwrap();
        let archived_line = if archived {
            "archived_at: 2026-05-18T15:00:00Z\n"
        } else {
            "archived_at:\n"
        };
        let yaml = format!(
            "promptibrary_schema: 1\n\
             id: {id}\n\
             title: {slug}\n\
             slug: {slug}\n\
             summary: x\n\
             created_at: 2026-05-18T14:00:00Z\n\
             updated_at: 2026-05-18T14:00:00Z\n\
             {archived_line}\
             tags: []\n\
             source:\n  kind: manual\n  origin_url:\n  title:\n  author:\n  fetched_at:\n  content_hash:\n\
             variables: []\n",
            slug = slug,
            archived_line = archived_line,
        );
        let content = format!("---\n{yaml}---\n{body}");
        std::fs::write(path, content).unwrap();
    }

    #[tokio::test]
    async fn scan_indexes_valid_files_and_counts_malformed() {
        let pool = temp_pool().await;
        let dir = tempfile::tempdir().unwrap();
        let v = VaultPaths::new(dir.path());
        write_prompt_file(&v, "alpha", "body\n", false, "01JZ7M1K6M8D4E9SZ7P1Q9KT4A");
        write_prompt_file(&v, "beta", "body\n", true, "01JZ7M1K6M8D4E9SZ7P1Q9KT4B");
        // Malformed file.
        std::fs::write(
            v.absolute("promptibrary/prompts/junk.md").unwrap(),
            "not yaml at all\n",
        )
        .unwrap();

        let summary = scan_vault(&v, &pool, |_| {}).await.unwrap();
        assert_eq!(summary.scanned_files, 3);
        assert_eq!(summary.indexed_prompts, 2);
        assert_eq!(summary.malformed_files, 1);
    }

    #[tokio::test]
    async fn scan_deletes_rows_for_missing_files() {
        let pool = temp_pool().await;
        let dir = tempfile::tempdir().unwrap();
        let v = VaultPaths::new(dir.path());
        write_prompt_file(&v, "alpha", "body\n", false, "01JZ7M1K6M8D4E9SZ7P1Q9KT4A");
        scan_vault(&v, &pool, |_| {}).await.unwrap();
        std::fs::remove_file(v.absolute("promptibrary/prompts/alpha.md").unwrap()).unwrap();
        let summary = scan_vault(&v, &pool, |_| {}).await.unwrap();
        assert_eq!(summary.deleted_rows, 1);
    }
}
