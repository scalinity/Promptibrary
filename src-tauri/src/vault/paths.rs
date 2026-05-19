//! Canonical vault-relative paths. Resolves prompt + run file paths in the
//! layout from spec §4 *Vault and app-support layout*.
//!
//! Every path produced here is **relative** to the vault root and uses
//! forward slashes. Absolute paths are reconstructed by callers via
//! `VaultPaths::absolute(...)`.
//!
//! Path-traversal posture (SCA-588):
//!
//! `absolute()` returns `Option<PathBuf>` and refuses to escape the vault.
//! `..` segments are filtered from the input, then the candidate is checked
//! against `is_within(vault_root, candidate)` — a None return means the
//! input attempted to traverse out of the vault (e.g. a poisoned SQLite
//! `vault_path` value injected by an attacker with local write access to
//! the index DB). Callers MUST handle None as `PromptMalformed`.
//!
//! `to_relative()` and `absolute()` are now symmetric: both filter to
//! `Component::Normal` only and reject anything else.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Datelike, Utc};

#[derive(Debug, Clone)]
pub struct VaultPaths {
    pub vault_root: PathBuf,
}

impl VaultPaths {
    pub fn new(vault_root: impl Into<PathBuf>) -> Self {
        Self {
            vault_root: vault_root.into(),
        }
    }

    pub fn promptibrary_root(&self) -> PathBuf {
        self.vault_root.join("promptibrary")
    }

    pub fn prompts_dir(&self) -> PathBuf {
        self.promptibrary_root().join("prompts")
    }

    pub fn runs_dir(&self) -> PathBuf {
        self.promptibrary_root().join("runs")
    }

    pub fn exports_dir(&self) -> PathBuf {
        self.promptibrary_root().join("exports")
    }

    pub fn extraction_cache_dir(&self) -> PathBuf {
        self.promptibrary_root().join("extraction-cache")
    }

    pub fn settings_yml(&self) -> PathBuf {
        self.promptibrary_root().join("settings.yml")
    }

    /// Convert an absolute vault path to its vault-relative form (forward
    /// slashes, no leading slash). Returns `None` if the path is not within
    /// the vault.
    ///
    /// Symmetric with [`Self::absolute`]: both keep `Component::Normal`
    /// only and drop `CurDir` / `ParentDir` / `RootDir` / `Prefix`.
    pub fn to_relative(&self, abs: &Path) -> Option<String> {
        let rel = abs.strip_prefix(&self.vault_root).ok()?;
        Some(
            rel.components()
                .filter_map(|c| match c {
                    std::path::Component::Normal(s) => Some(s.to_string_lossy().to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("/"),
        )
    }

    /// Resolve an absolute path from a vault-relative path string.
    ///
    /// Returns `None` if the relative path tries to traverse outside the
    /// vault (any `..` segment) or would otherwise resolve outside the
    /// vault root. Callers MUST handle the `None` case — typically by
    /// returning `AppErrorKind::PromptMalformed` with the offending
    /// `vault_path` in `details`.
    ///
    /// This is the trust boundary for `vault_path` values that may have
    /// come from attacker-controlled SQLite rows (CWE-22 / SCA-588).
    pub fn absolute(&self, relative: &str) -> Option<PathBuf> {
        let mut p = self.vault_root.clone();
        for part in relative.split('/') {
            if part.is_empty() {
                continue;
            }
            // Reject any traversal or root-altering segment outright. Both
            // forward-slash splits ("../foo") and platform Component::ParentDir
            // (".." or platform-specific) land in this branch.
            if part == ".." || part == "." {
                return None;
            }
            p.push(part);
        }
        if crate::util::fs::is_within(&self.vault_root, &p) {
            Some(p)
        } else {
            None
        }
    }
}

/// Vault-relative path string. We don't bother with a newtype here — usage
/// is local and the spec models this as just `string` on the wire.
pub fn prompt_path_for_slug(slug: &str) -> String {
    format!("promptibrary/prompts/{slug}.md")
}

pub fn run_path_for_date(run_id: &str, dt: DateTime<Utc>) -> String {
    format!(
        "promptibrary/runs/{year}/{month:02}/{day:02}/{run_id}.md",
        year = dt.year(),
        month = dt.month(),
        day = dt.day(),
        run_id = run_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn prompt_path_uses_slug() {
        assert_eq!(
            prompt_path_for_slug("ship-it"),
            "promptibrary/prompts/ship-it.md"
        );
    }

    #[test]
    fn run_path_pads_month_and_day() {
        let dt = Utc.with_ymd_and_hms(2026, 5, 8, 12, 0, 0).unwrap();
        assert_eq!(
            run_path_for_date("01ABC", dt),
            "promptibrary/runs/2026/05/08/01ABC.md"
        );
    }

    #[test]
    fn vault_paths_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let v = VaultPaths::new(dir.path());
        let prompt_abs = v.absolute("promptibrary/prompts/hi.md").unwrap();
        assert!(prompt_abs.starts_with(dir.path()));
        let back = v.to_relative(&prompt_abs).unwrap();
        assert_eq!(back, "promptibrary/prompts/hi.md");
    }

    #[test]
    fn absolute_rejects_dotdot_traversal() {
        let v = VaultPaths::new("/vault");
        // Classic traversal attempt
        assert!(v.absolute("../etc/passwd").is_none());
        assert!(v.absolute("../../etc/passwd").is_none());
        // Embedded in the middle is also rejected.
        assert!(v.absolute("promptibrary/../../etc/passwd").is_none());
        // CurDir is also rejected for symmetry — encoded vault_paths must
        // be in canonical form.
        assert!(v.absolute("./promptibrary/prompts/x.md").is_none());
    }

    #[test]
    fn absolute_accepts_well_formed_vault_path() {
        let v = VaultPaths::new("/vault");
        let p = v.absolute("promptibrary/prompts/hello.md").unwrap();
        assert_eq!(p, PathBuf::from("/vault/promptibrary/prompts/hello.md"));
    }

    #[test]
    fn absolute_skips_empty_segments() {
        let v = VaultPaths::new("/vault");
        // Doubled slashes still resolve cleanly inside the vault.
        let p = v.absolute("promptibrary//prompts//x.md").unwrap();
        assert_eq!(p, PathBuf::from("/vault/promptibrary/prompts/x.md"));
    }
}
