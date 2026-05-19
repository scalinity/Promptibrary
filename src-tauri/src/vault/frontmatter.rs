//! Frontmatter parse/validation for prompts and run transcripts.
//!
//! Spec §3 module contract. We deserialize YAML into typed structs
//! (`PromptFrontmatter`, `RunFrontmatter`), apply spec rules
//! (`promptibrary_schema == 1`, ULID validation on `id`), and return
//! typed `YamlMalformed` / `PromptMalformed` errors.
//!
//! Notable serde decisions:
//!   - Use snake_case keys to match the spec YAML examples.
//!   - Optional fields default via `#[serde(default)]` so partial
//!     frontmatter doesn't fail open.
//!   - `archived_at:` with no value parses as `None`, matching Obsidian
//!     conventions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::prompt::LaunchDefaults;
use crate::domain::source::Source;
use crate::domain::variable::Variable;
use crate::error::{AppError, AppErrorKind, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptFrontmatter {
    pub promptibrary_schema: u32,
    pub id: String,
    pub title: String,
    pub slug: String,
    #[serde(default)]
    pub summary: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub archived_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub source: Source,
    #[serde(default)]
    pub variables: Vec<Variable>,
    #[serde(default)]
    pub launch_defaults: Option<LaunchDefaults>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFrontmatter {
    pub promptibrary_schema: u32,
    pub id: String,
    pub prompt_id: String,
    pub started_at: DateTime<Utc>,
    #[serde(default)]
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub exit_code: Option<i32>,
}

pub fn parse_prompt_frontmatter(yaml: &str) -> Result<PromptFrontmatter> {
    let fm: PromptFrontmatter = serde_yaml::from_str(yaml).map_err(|e| {
        AppError::new(
            AppErrorKind::YamlMalformed,
            format!("yaml parse failed: {}", e),
        )
    })?;
    if fm.promptibrary_schema != 1 {
        return Err(AppError::new(
            AppErrorKind::PromptMalformed,
            format!(
                "promptibrary_schema must be 1, got {}",
                fm.promptibrary_schema
            ),
        ));
    }
    if !is_ulid(&fm.id) {
        return Err(AppError::new(
            AppErrorKind::PromptMalformed,
            format!("id is not a valid ULID: {}", fm.id),
        ));
    }
    Ok(fm)
}

pub fn parse_run_frontmatter(yaml: &str) -> Result<RunFrontmatter> {
    let fm: RunFrontmatter = serde_yaml::from_str(yaml).map_err(|e| {
        AppError::new(
            AppErrorKind::YamlMalformed,
            format!("yaml parse failed: {}", e),
        )
    })?;
    if fm.promptibrary_schema != 1 {
        return Err(AppError::new(
            AppErrorKind::PromptMalformed,
            "promptibrary_schema must be 1 in run transcript",
        ));
    }
    Ok(fm)
}

fn is_ulid(s: &str) -> bool {
    s.len() == 26
        && s.chars().all(|c| {
            c.is_ascii_uppercase() && c != 'I' && c != 'L' && c != 'O' && c != 'U'
                || c.is_ascii_digit()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_yaml() -> &'static str {
        "promptibrary_schema: 1\n\
         id: 01JZ7M1K6M8D4E9SZ7P1Q9KT4A\n\
         title: \"Test\"\n\
         slug: test\n\
         summary: A test prompt\n\
         created_at: 2026-05-18T14:30:00Z\n\
         updated_at: 2026-05-18T14:30:00Z\n\
         archived_at:\n\
         tags: []\n\
         source:\n  kind: manual\n  origin_url:\n  title:\n  author:\n  fetched_at:\n  content_hash:\n\
         variables: []\n"
    }

    #[test]
    fn parses_valid_prompt_frontmatter() {
        let fm = parse_prompt_frontmatter(sample_yaml()).unwrap();
        assert_eq!(fm.promptibrary_schema, 1);
        assert_eq!(fm.title, "Test");
        assert!(fm.archived_at.is_none());
    }

    #[test]
    fn rejects_wrong_schema_version() {
        let bad = sample_yaml().replace("promptibrary_schema: 1", "promptibrary_schema: 2");
        let err = parse_prompt_frontmatter(&bad).unwrap_err();
        assert_eq!(err.kind, AppErrorKind::PromptMalformed);
    }

    #[test]
    fn rejects_invalid_ulid() {
        let bad = sample_yaml().replace(
            "id: 01JZ7M1K6M8D4E9SZ7P1Q9KT4A",
            "id: not-a-ulid-at-all-12345678",
        );
        let err = parse_prompt_frontmatter(&bad).unwrap_err();
        assert_eq!(err.kind, AppErrorKind::PromptMalformed);
    }

    #[test]
    fn malformed_yaml_returns_yaml_malformed() {
        let err = parse_prompt_frontmatter("not: : valid: yaml :::").unwrap_err();
        assert_eq!(err.kind, AppErrorKind::YamlMalformed);
    }
}
