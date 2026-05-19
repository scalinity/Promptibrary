//! Canonical vault-relative paths. Resolves prompt + run file paths in the
//! layout from spec §4 *Vault and app-support layout*.
//!
//! Every path produced here is **relative** to the vault root and uses
//! forward slashes. Absolute paths are reconstructed by callers via
//! `VaultPaths::absolute(...)`.

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
    pub fn absolute(&self, relative: &str) -> PathBuf {
        let mut p = self.vault_root.clone();
        for part in relative.split('/').filter(|s| !s.is_empty()) {
            p.push(part);
        }
        p
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
        let prompt_abs = v.absolute("promptibrary/prompts/hi.md");
        assert!(prompt_abs.starts_with(dir.path()));
        let back = v.to_relative(&prompt_abs).unwrap();
        assert_eq!(back, "promptibrary/prompts/hi.md");
    }
}
