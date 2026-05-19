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

/// ULID validator per the [ULID spec](https://github.com/ulid/spec):
/// 26 chars in Crockford Base32, alphabet `0-9A-Z` minus `I`, `L`, `O`, `U`.
/// SCA-617: the first character encodes the top 5 bits of a 48-bit
/// timestamp. Since 10 base32 chars hold 50 bits and the timestamp is 48,
/// the top 2 bits of the first char must be zero — so the first char is
/// limited to `0..=7`. The previous check accepted `Z...` as valid.
fn is_ulid(s: &str) -> bool {
    if s.len() != 26 {
        return false;
    }
    let mut chars = s.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };
    // First char must be in '0'..='7' (top 2 bits of the high byte are
    // unused per ULID spec).
    if !('0'..='7').contains(&first) {
        return false;
    }
    let valid_crockford = |c: char| {
        (c.is_ascii_uppercase() && c != 'I' && c != 'L' && c != 'O' && c != 'U')
            || c.is_ascii_digit()
    };
    if !valid_crockford(first) {
        return false;
    }
    chars.all(valid_crockford)
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

    #[test]
    fn is_ulid_rejects_high_first_char() {
        // SCA-617: ULID first char must be 0..=7 (top 2 bits unused).
        // 'Z' is otherwise a valid Crockford char but invalid as the
        // leading char of a ULID.
        assert!(!super::is_ulid("ZZZZZZZZZZZZZZZZZZZZZZZZZZ"));
        assert!(!super::is_ulid("8AAAAAAAAAAAAAAAAAAAAAAAAA"));
        assert!(!super::is_ulid("9AAAAAAAAAAAAAAAAAAAAAAAAA"));
    }

    #[test]
    fn is_ulid_accepts_canonical() {
        assert!(super::is_ulid("01JZ7M1K6M8D4E9SZ7P1Q9KT4A"));
        assert!(super::is_ulid("7ZZZZZZZZZZZZZZZZZZZZZZZZZ"));
    }

    #[test]
    fn is_ulid_rejects_disallowed_crockford_chars() {
        // I, L, O, U are excluded from Crockford Base32.
        assert!(!super::is_ulid("01JZ7M1K6M8D4E9SZ7P1Q9KTIA"));
        assert!(!super::is_ulid("01JZ7M1K6M8D4E9SZ7P1Q9KTLA"));
        assert!(!super::is_ulid("01JZ7M1K6M8D4E9SZ7P1Q9KTOA"));
        assert!(!super::is_ulid("01JZ7M1K6M8D4E9SZ7P1Q9KTUA"));
    }
}
