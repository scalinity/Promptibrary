//! Parse + validate `ExtractionResponse` from the Anthropic Messages API
//! per spec §6 *Validation rules*.
//!
//! The repair-loop (one shot only) lives in the Anthropic client, which
//! calls back into this module after each attempt. Validation is *all*
//! collected — never short-circuit on the first error — so the model gets
//! the full error set in the single repair request.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::types::{CandidatePrompt, ExtractionResponse};
use crate::variables::parser::{
    parse_template_variables, ParseVariablesInput, VariableParseError,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValidationError {
    JsonParseFailed { error: String },
    SchemaVersionMismatch { got: u32, expected: u32 },
    CandidatesOutOfRange { got: usize, min: usize, max: usize },
    TitleLengthOutOfRange { index: usize, len: usize, min: usize, max: usize },
    BodyLengthOutOfRange { index: usize, len: usize, min: usize, max: usize },
    TagFormatInvalid { index: usize, tag: String },
    InvalidVariableInBody { index: usize, errors: Vec<String> },
    DestinationNotAllowed { index: usize, got: String, allowed: String },
    SecretLeak { index: usize, marker: String },
}

const TITLE_MIN: usize = 5;
const TITLE_MAX: usize = 120;
const BODY_MIN: usize = 200;
const BODY_MAX: usize = 60_000;
const CANDIDATES_MIN: usize = 1;
const CANDIDATES_MAX: usize = 8;
const ALLOWED_DESTINATION: &str = "claude_code_cli";

static TAG_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9][a-z0-9-]{0,39}$").expect("tag regex compiles"));

// Per spec §6 *Validation rules → Reject candidate if it embeds secrets*.
// The patterns target the common API-key prefixes attackers tend to leak in
// source material, plus a generic "long alphanum string near `key|token|secret`"
// sweep.
static SECRET_PATTERNS: Lazy<Vec<(&'static str, Regex)>> = Lazy::new(|| {
    vec![
        (
            "anthropic_key",
            Regex::new(r"sk-ant-[A-Za-z0-9_-]{16,}").unwrap(),
        ),
        (
            "openai_key",
            Regex::new(r"sk-[A-Za-z0-9]{20,}").unwrap(),
        ),
        ("aws_key", Regex::new(r"AKIA[0-9A-Z]{16}").unwrap()),
        ("slack_bot_token", Regex::new(r"xox[baprs]-[A-Za-z0-9-]{10,}").unwrap()),
        ("github_token", Regex::new(r"gh[pousr]_[A-Za-z0-9]{30,}").unwrap()),
        // SCA-715: narrow with `\b` word boundaries so the keyword can't
        // match inside `mytoken_name`, `apikey_id`, etc. — those phrases
        // appear constantly in valid auth-API docs. We still reject any
        // unambiguous `secret: <32+ char blob>` shape that's almost
        // certainly a credential pasted into source material.
        (
            "generic_kv",
            Regex::new(
                r#"(?i)\b(?:api[_-]?key|access[_-]?token|secret[_-]?key|bearer[_-]?token|client[_-]?secret)\b\s*[:=]\s*['"]?[A-Za-z0-9+/=_-]{32,}"#,
            )
            .unwrap(),
        ),
    ]
});

/// Parse + validate the raw model output. On failure, returns the typed
/// error set so the repair request can echo each one back to the model.
pub fn parse_and_validate(raw: &str) -> Result<ExtractionResponse, Vec<ValidationError>> {
    let parsed: ExtractionResponse = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(e) => {
            return Err(vec![ValidationError::JsonParseFailed {
                error: e.to_string(),
            }])
        }
    };

    let mut errors = Vec::new();

    if parsed.schema_version != 1 {
        errors.push(ValidationError::SchemaVersionMismatch {
            got: parsed.schema_version,
            expected: 1,
        });
    }

    if parsed.candidates.len() < CANDIDATES_MIN || parsed.candidates.len() > CANDIDATES_MAX {
        errors.push(ValidationError::CandidatesOutOfRange {
            got: parsed.candidates.len(),
            min: CANDIDATES_MIN,
            max: CANDIDATES_MAX,
        });
    }

    for (idx, c) in parsed.candidates.iter().enumerate() {
        validate_candidate(idx, c, &mut errors);
    }

    if errors.is_empty() {
        Ok(parsed)
    } else {
        Err(errors)
    }
}

fn validate_candidate(idx: usize, c: &CandidatePrompt, errors: &mut Vec<ValidationError>) {
    let title_len = c.title.chars().count();
    if title_len < TITLE_MIN || title_len > TITLE_MAX {
        errors.push(ValidationError::TitleLengthOutOfRange {
            index: idx,
            len: title_len,
            min: TITLE_MIN,
            max: TITLE_MAX,
        });
    }

    let body_len = c.body.chars().count();
    if body_len < BODY_MIN || body_len > BODY_MAX {
        errors.push(ValidationError::BodyLengthOutOfRange {
            index: idx,
            len: body_len,
            min: BODY_MIN,
            max: BODY_MAX,
        });
    }

    for tag in &c.tags {
        if !TAG_RE.is_match(tag) {
            errors.push(ValidationError::TagFormatInvalid {
                index: idx,
                tag: tag.clone(),
            });
        }
    }

    let parse_out = parse_template_variables(ParseVariablesInput {
        template: c.body.clone(),
        frontmatter_variables: c.variables.clone(),
    });
    if !parse_out.errors.is_empty() {
        errors.push(ValidationError::InvalidVariableInBody {
            index: idx,
            errors: parse_out
                .errors
                .iter()
                .map(format_variable_error)
                .collect(),
        });
    }

    if let Some(dest) = &c.launch_defaults_patch.destination {
        if dest != ALLOWED_DESTINATION {
            errors.push(ValidationError::DestinationNotAllowed {
                index: idx,
                got: dest.clone(),
                allowed: ALLOWED_DESTINATION.to_string(),
            });
        }
    }

    if let Some(marker) = scan_for_secrets(&c.body) {
        errors.push(ValidationError::SecretLeak {
            index: idx,
            marker: marker.to_string(),
        });
    }
}

fn scan_for_secrets(text: &str) -> Option<&'static str> {
    for (name, re) in SECRET_PATTERNS.iter() {
        if re.is_match(text) {
            return Some(*name);
        }
    }
    None
}

fn format_variable_error(e: &VariableParseError) -> String {
    match e {
        VariableParseError::UnclosedVariableRef { start_utf16 } => {
            format!("unclosed variable ref at offset {start_utf16}")
        }
        VariableParseError::UnknownVariableType { raw_type, .. } => {
            format!("unknown variable type `{raw_type}`")
        }
        VariableParseError::InvalidVariableKey { key, .. } => {
            format!("invalid variable key `{key}`")
        }
        VariableParseError::SelectOptionsRequired { .. } => {
            "select variable missing options".to_string()
        }
        _ => format!("{e:?}"),
    }
}

/// Find the first top-level `{...}` JSON object in `text`. Used when the
/// model wraps its JSON in markdown despite the system prompt's instructions.
/// Returns the substring including braces; the caller re-runs `parse_and_validate`.
pub fn salvage_first_json_object(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut start: Option<usize> = None;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;

    for (i, &b) in bytes.iter().enumerate() {
        if in_string {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = start {
                        return Some(&text[s..=i]);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good_candidate(title: &str, body: &str) -> String {
        let body_full = if body.is_empty() {
            "x".repeat(BODY_MIN + 10)
        } else {
            body.into()
        };
        format!(
            r#"{{
              "schemaVersion": 1,
              "candidates": [
                {{
                  "title": "{title}",
                  "summary": "Sum",
                  "body": "{body_full}",
                  "tags": ["one", "two-three"],
                  "variables": [],
                  "launchDefaultsPatch": {{}},
                  "confidence": "medium",
                  "rationale": "because",
                  "sourceAnchors": []
                }}
              ]
            }}"#
        )
    }

    #[test]
    fn valid_response_parses() {
        use super::super::types::CandidateConfidence;
        let raw = good_candidate("Refactor X for performance", "");
        let parsed = parse_and_validate(&raw).unwrap();
        assert_eq!(parsed.candidates.len(), 1);
        assert_eq!(parsed.candidates[0].confidence, CandidateConfidence::Medium);
    }

    #[test]
    fn json_parse_error_surfaces_typed() {
        let err = parse_and_validate("not json").unwrap_err();
        assert!(matches!(err[0], ValidationError::JsonParseFailed { .. }));
    }

    #[test]
    fn schema_version_mismatch_caught() {
        let raw = good_candidate("Refactor X", "").replace("\"schemaVersion\": 1", "\"schemaVersion\": 2");
        let err = parse_and_validate(&raw).unwrap_err();
        assert!(err.iter().any(|e| matches!(e, ValidationError::SchemaVersionMismatch { got: 2, .. })));
    }

    #[test]
    fn empty_candidates_rejected() {
        let raw = r#"{"schemaVersion":1,"candidates":[]}"#;
        let err = parse_and_validate(raw).unwrap_err();
        assert!(err
            .iter()
            .any(|e| matches!(e, ValidationError::CandidatesOutOfRange { got: 0, .. })));
    }

    #[test]
    fn title_too_short_rejected() {
        let raw = good_candidate("x", "");
        let err = parse_and_validate(&raw).unwrap_err();
        assert!(err
            .iter()
            .any(|e| matches!(e, ValidationError::TitleLengthOutOfRange { .. })));
    }

    #[test]
    fn body_too_short_rejected() {
        let raw = good_candidate("Refactor X", "tiny body");
        let err = parse_and_validate(&raw).unwrap_err();
        assert!(err
            .iter()
            .any(|e| matches!(e, ValidationError::BodyLengthOutOfRange { .. })));
    }

    #[test]
    fn bad_tag_rejected() {
        let mut raw = good_candidate("Refactor X for perf", "");
        raw = raw.replace("\"tags\": [\"one\", \"two-three\"]", "\"tags\": [\"BadTag\"]");
        let err = parse_and_validate(&raw).unwrap_err();
        assert!(err.iter().any(|e| matches!(e, ValidationError::TagFormatInvalid { tag, .. } if tag == "BadTag")));
    }

    #[test]
    fn non_claude_destination_rejected() {
        let mut raw = good_candidate("Refactor X for perf", "");
        raw = raw.replace(
            "\"launchDefaultsPatch\": {}",
            "\"launchDefaultsPatch\": {\"destination\": \"cursor\"}",
        );
        let err = parse_and_validate(&raw).unwrap_err();
        assert!(err
            .iter()
            .any(|e| matches!(e, ValidationError::DestinationNotAllowed { got, .. } if got == "cursor")));
    }

    #[test]
    fn anthropic_key_embedded_in_body_rejected() {
        let leak_body =
            format!("{} sk-ant-ABCDEFGH12345678IJKLMNOP", "padding ".repeat(40));
        let raw = good_candidate("Refactor X", &leak_body);
        let err = parse_and_validate(&raw).unwrap_err();
        assert!(err
            .iter()
            .any(|e| matches!(e, ValidationError::SecretLeak { marker, .. } if marker == "anthropic_key")));
    }

    #[test]
    fn aws_key_embedded_rejected() {
        let leak_body = format!("AKIA1234567890ABCDEF in {}", "x".repeat(BODY_MIN));
        let raw = good_candidate("Refactor X", &leak_body);
        let err = parse_and_validate(&raw).unwrap_err();
        assert!(err
            .iter()
            .any(|e| matches!(e, ValidationError::SecretLeak { marker, .. } if marker == "aws_key")));
    }

    #[test]
    fn generic_kv_near_key_word_rejected() {
        let leak_body = format!(
            "Configure api_key: AAAA1111BBBB2222CCCC3333DDDD4444 then {}",
            "x".repeat(BODY_MIN)
        );
        let raw = good_candidate("Refactor X for perf", &leak_body);
        let err = parse_and_validate(&raw).unwrap_err();
        assert!(err.iter().any(|e| matches!(e, ValidationError::SecretLeak { .. })));
    }

    #[test]
    fn generic_kv_does_not_false_positive_on_auth_api_prose() {
        // SCA-715 — prompts that mention `mytoken` or `api_key_name` in
        // prose must NOT be rejected. The pre-narrowing pattern would
        // hit on `(?i)(?:key|token|secret|bearer)\s*[:=]\s*…` which fires
        // on any place those words appeared with a long alphanum.
        let safe_body = format!(
            "Document the auth flow: the `mytoken` parameter accepts a string longer than 32 chars (like AAAA1111BBBB2222CCCC3333DDDD4444). The `secret_name` field stores the same. {}",
            "x".repeat(BODY_MIN)
        );
        let raw = good_candidate("Refactor X for perf", &safe_body);
        // Should NOT trip the secret scanner.
        let result = parse_and_validate(&raw);
        if let Err(errs) = &result {
            assert!(
                !errs.iter().any(|e| matches!(e, ValidationError::SecretLeak { .. })),
                "auth-API prose with no real credential must not trip the secret scanner"
            );
        }
    }

    #[test]
    fn salvage_extracts_first_object_from_markdown_wrap() {
        let wrap = "Here is the JSON:\n```json\n{\"schemaVersion\":1,\"candidates\":[]}\n```\nThat's it.";
        let salvaged = salvage_first_json_object(wrap).unwrap();
        assert_eq!(salvaged, "{\"schemaVersion\":1,\"candidates\":[]}");
    }

    #[test]
    fn salvage_handles_nested_braces_in_strings() {
        let wrap = r#"prelude {"a":"}","b":{"c":1}} epilogue"#;
        let salvaged = salvage_first_json_object(wrap).unwrap();
        assert_eq!(salvaged, r#"{"a":"}","b":{"c":1}}"#);
    }

    #[test]
    fn salvage_returns_none_when_no_object() {
        assert!(salvage_first_json_object("no object here").is_none());
    }
}
