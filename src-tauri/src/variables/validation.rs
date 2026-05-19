//! Per-type validation rules for resolved variable values.
//!
//! Spec §5 *Validation rules per type*. The validator is the only module in
//! `variables/` that touches the filesystem — the parser stays pure so it can
//! be tested deterministically.
//!
//! The validator takes a `Variable` (declared metadata + constraints) plus a
//! raw input value, and returns either:
//!   - `Ok(ResolvedVariableValue)` — canonical form ready for the renderer
//!     (e.g. file/folder paths are expanded and canonicalized; text is
//!     trimmed and re-bounded; number is parsed once).
//!   - `Err(Vec<ValidationIssue>)` — one or more issues describing what
//!     failed. Issues are surface-grade messages designed for the
//!     variable-control UI, not raw stack traces.

use std::path::PathBuf;
use std::process;

use serde::{Deserialize, Serialize};

use crate::domain::variable::{
    BoolVariable, FileVariable, FolderVariable, MultilineVariable, NumberVariable, SelectVariable,
    TextVariable, Variable,
};
use crate::variables::renderer::ResolvedVariableValue;

// ---------- Public types ----------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateValueInput {
    pub variable: Variable,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum ValidationIssue {
    Required { key: String },
    MinLength { key: String, min: u32, actual: u32 },
    MaxLength { key: String, max: u32, actual: u32 },
    PatternMismatch { key: String, pattern: String },
    NotFinite { key: String },
    BelowMin { key: String, min: f64 },
    AboveMax { key: String, max: f64 },
    NotInteger { key: String },
    StepMismatch { key: String, step: f64 },
    FileMissing { key: String, path: String },
    FileNotRegular { key: String, path: String },
    FileExtensionNotAllowed { key: String, ext: String, allowed: Vec<String> },
    FolderMissing { key: String, path: String },
    FolderNotDirectory { key: String, path: String },
    FolderNotWritable { key: String, path: String },
    SelectValueNotInOptions { key: String, value: String, allowed: Vec<String> },
    TypeMismatch { key: String },
}

// ---------- Entrypoint ------------------------------------------------------

pub fn validate_value(
    input: ValidateValueInput,
) -> Result<ResolvedVariableValue, Vec<ValidationIssue>> {
    match input.variable {
        Variable::File(v) => validate_file(v, input.value),
        Variable::Folder(v) => validate_folder(v, input.value),
        Variable::Text(v) => validate_text(v, input.value),
        Variable::Multiline(v) => validate_multiline(v, input.value),
        Variable::Select(v) => validate_select(v, input.value),
        Variable::Bool(v) => validate_bool(v, input.value),
        Variable::Number(v) => validate_number(v, input.value),
    }
}

// ---------- file -----------------------------------------------------------

fn validate_file(
    v: FileVariable,
    value: serde_json::Value,
) -> Result<ResolvedVariableValue, Vec<ValidationIssue>> {
    let raw = match value.as_str() {
        Some(s) => s.trim(),
        None => {
            return Err(vec![ValidationIssue::TypeMismatch { key: v.key }]);
        }
    };
    if raw.is_empty() {
        if v.required {
            return Err(vec![ValidationIssue::Required { key: v.key }]);
        } else {
            return Ok(ResolvedVariableValue::File {
                key: v.key,
                value: PathBuf::new(),
            });
        }
    }

    let expanded = expand_tilde(raw);
    let mut issues = Vec::new();

    if v.must_exist {
        match std::fs::metadata(&expanded) {
            Ok(meta) => {
                if !meta.is_file() {
                    issues.push(ValidationIssue::FileNotRegular {
                        key: v.key.clone(),
                        path: expanded.to_string_lossy().to_string(),
                    });
                }
            }
            Err(_) => {
                issues.push(ValidationIssue::FileMissing {
                    key: v.key.clone(),
                    path: expanded.to_string_lossy().to_string(),
                });
            }
        }
    }

    if !v.allowed_extensions.is_empty() {
        let ext_ok = expanded
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| {
                let elc = e.to_lowercase();
                v.allowed_extensions
                    .iter()
                    .any(|a| a.trim_start_matches('.').to_lowercase() == elc)
            })
            .unwrap_or(false);
        if !ext_ok {
            issues.push(ValidationIssue::FileExtensionNotAllowed {
                key: v.key.clone(),
                ext: expanded
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string(),
                allowed: v.allowed_extensions.clone(),
            });
        }
    }

    if !issues.is_empty() {
        return Err(issues);
    }

    let canonical = match std::fs::canonicalize(&expanded) {
        Ok(p) => p,
        Err(_) => expanded,
    };
    Ok(ResolvedVariableValue::File {
        key: v.key,
        value: canonical,
    })
}

// ---------- folder ---------------------------------------------------------

fn validate_folder(
    v: FolderVariable,
    value: serde_json::Value,
) -> Result<ResolvedVariableValue, Vec<ValidationIssue>> {
    let raw = match value.as_str() {
        Some(s) => s.trim(),
        None => return Err(vec![ValidationIssue::TypeMismatch { key: v.key }]),
    };
    if raw.is_empty() {
        if v.required {
            return Err(vec![ValidationIssue::Required { key: v.key }]);
        } else {
            return Ok(ResolvedVariableValue::Folder {
                key: v.key,
                value: PathBuf::new(),
            });
        }
    }
    let expanded = expand_tilde(raw);
    let mut issues = Vec::new();
    if v.must_exist {
        match std::fs::metadata(&expanded) {
            Ok(meta) => {
                if !meta.is_dir() {
                    issues.push(ValidationIssue::FolderNotDirectory {
                        key: v.key.clone(),
                        path: expanded.to_string_lossy().to_string(),
                    });
                }
            }
            Err(_) => {
                issues.push(ValidationIssue::FolderMissing {
                    key: v.key.clone(),
                    path: expanded.to_string_lossy().to_string(),
                });
            }
        }
    }
    if v.must_be_writable && issues.is_empty() {
        // Write probe — create a unique sentinel file, then delete it.
        let probe = expanded.join(format!(".promptibrary-write-test-{}", process::id()));
        match std::fs::write(&probe, b"") {
            Ok(_) => {
                let _ = std::fs::remove_file(&probe);
            }
            Err(_) => {
                issues.push(ValidationIssue::FolderNotWritable {
                    key: v.key.clone(),
                    path: expanded.to_string_lossy().to_string(),
                });
            }
        }
    }
    if !issues.is_empty() {
        return Err(issues);
    }
    let canonical = std::fs::canonicalize(&expanded).unwrap_or(expanded);
    Ok(ResolvedVariableValue::Folder {
        key: v.key,
        value: canonical,
    })
}

// ---------- text -----------------------------------------------------------

fn validate_text(
    v: TextVariable,
    value: serde_json::Value,
) -> Result<ResolvedVariableValue, Vec<ValidationIssue>> {
    let raw = match value.as_str() {
        Some(s) => s,
        None => return Err(vec![ValidationIssue::TypeMismatch { key: v.key }]),
    };
    let value = if v.trim {
        raw.trim().to_string()
    } else {
        raw.to_string()
    };
    let mut issues = Vec::new();
    if v.required && value.is_empty() {
        issues.push(ValidationIssue::Required { key: v.key.clone() });
    }
    // UTF-8 scalar length (chars), not bytes.
    let scalar_len = value.chars().count() as u32;
    if let Some(min) = v.min_length {
        if scalar_len < min {
            issues.push(ValidationIssue::MinLength {
                key: v.key.clone(),
                min,
                actual: scalar_len,
            });
        }
    }
    if let Some(max) = v.max_length {
        if scalar_len > max {
            issues.push(ValidationIssue::MaxLength {
                key: v.key.clone(),
                max,
                actual: scalar_len,
            });
        }
    }
    if let Some(pat) = &v.pattern {
        // Compile via the cache (SCA-591). The cache compiles on demand
        // if the parser didn't pre-warm it (e.g. frontmatter-only
        // patterns); compile errors collapse into PatternMismatch since
        // the user can't fix a compile error from the value field.
        if !crate::variables::regex_cache::is_match(pat, &value) {
            issues.push(ValidationIssue::PatternMismatch {
                key: v.key.clone(),
                pattern: pat.clone(),
            });
        }
    }
    if !issues.is_empty() {
        return Err(issues);
    }
    Ok(ResolvedVariableValue::Text { key: v.key, value })
}

// ---------- multiline ------------------------------------------------------

fn validate_multiline(
    v: MultilineVariable,
    value: serde_json::Value,
) -> Result<ResolvedVariableValue, Vec<ValidationIssue>> {
    let raw = match value.as_str() {
        Some(s) => s.to_string(),
        None => return Err(vec![ValidationIssue::TypeMismatch { key: v.key }]),
    };
    let value = if v.trim_trailing_whitespace {
        raw.lines()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        raw
    };
    let mut issues = Vec::new();
    if v.required && !value.chars().any(|c| !c.is_whitespace()) {
        issues.push(ValidationIssue::Required { key: v.key.clone() });
    }
    let scalar_len = value.chars().count() as u32;
    if let Some(min) = v.min_length {
        if scalar_len < min {
            issues.push(ValidationIssue::MinLength {
                key: v.key.clone(),
                min,
                actual: scalar_len,
            });
        }
    }
    if let Some(max) = v.max_length {
        if scalar_len > max {
            issues.push(ValidationIssue::MaxLength {
                key: v.key.clone(),
                max,
                actual: scalar_len,
            });
        }
    }
    if !issues.is_empty() {
        return Err(issues);
    }
    Ok(ResolvedVariableValue::Multiline { key: v.key, value })
}

// ---------- select ---------------------------------------------------------

fn validate_select(
    v: SelectVariable,
    value: serde_json::Value,
) -> Result<ResolvedVariableValue, Vec<ValidationIssue>> {
    let raw = match value.as_str() {
        Some(s) => s.to_string(),
        None => return Err(vec![ValidationIssue::TypeMismatch { key: v.key }]),
    };
    if raw.is_empty() {
        if v.required {
            return Err(vec![ValidationIssue::Required { key: v.key }]);
        }
    }
    let allowed: Vec<String> = v.options.iter().map(|o| o.value.clone()).collect();
    if !allowed.iter().any(|a| a == &raw) {
        return Err(vec![ValidationIssue::SelectValueNotInOptions {
            key: v.key,
            value: raw,
            allowed,
        }]);
    }
    Ok(ResolvedVariableValue::Select { key: v.key, value: raw })
}

// ---------- bool -----------------------------------------------------------

fn validate_bool(
    v: BoolVariable,
    value: serde_json::Value,
) -> Result<ResolvedVariableValue, Vec<ValidationIssue>> {
    let actual = match value {
        serde_json::Value::Bool(b) => b,
        serde_json::Value::Null => v.default_value.unwrap_or(false),
        _ => return Err(vec![ValidationIssue::TypeMismatch { key: v.key }]),
    };
    Ok(ResolvedVariableValue::Bool { key: v.key, value: actual })
}

// ---------- number ---------------------------------------------------------

fn validate_number(
    v: NumberVariable,
    value: serde_json::Value,
) -> Result<ResolvedVariableValue, Vec<ValidationIssue>> {
    let n = match value {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::Null => v.default_value,
        _ => None,
    };
    let n = match n {
        Some(n) => n,
        None => {
            return Err(vec![ValidationIssue::TypeMismatch { key: v.key }]);
        }
    };

    let mut issues = Vec::new();
    if !n.is_finite() {
        issues.push(ValidationIssue::NotFinite { key: v.key.clone() });
        return Err(issues);
    }
    if v.integer && n.fract() != 0.0 {
        issues.push(ValidationIssue::NotInteger { key: v.key.clone() });
    }
    if let Some(min) = v.min {
        if n < min {
            issues.push(ValidationIssue::BelowMin {
                key: v.key.clone(),
                min,
            });
        }
    }
    if let Some(max) = v.max {
        if n > max {
            issues.push(ValidationIssue::AboveMax {
                key: v.key.clone(),
                max,
            });
        }
    }
    if let Some(step) = v.step {
        if step > 0.0 {
            let base = v.min.unwrap_or(0.0);
            let rem = ((n - base) / step).round() * step + base - n;
            if rem.abs() > 1e-9 {
                issues.push(ValidationIssue::StepMismatch {
                    key: v.key.clone(),
                    step,
                });
            }
        }
    }
    if !issues.is_empty() {
        return Err(issues);
    }
    Ok(ResolvedVariableValue::Number { key: v.key, value: n })
}

// ---------- Helpers --------------------------------------------------------

fn expand_tilde(raw: &str) -> PathBuf {
    if let Some(stripped) = raw.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(stripped);
        }
    } else if raw == "~" {
        if let Some(home) = home_dir() {
            return home;
        }
    }
    PathBuf::from(raw)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::variable::{SelectOption, VariableSource, VariableType};

    fn text_var(key: &str, min: Option<u32>, max: Option<u32>, pattern: Option<&str>) -> TextVariable {
        TextVariable {
            key: key.into(),
            label: key.into(),
            description: None,
            required: true,
            default_value: None,
            order: 0,
            source: VariableSource::Parsed,
            min_length: min,
            max_length: max,
            pattern: pattern.map(String::from),
            trim: true,
        }
    }

    #[test]
    fn text_required_rejects_empty() {
        let v = text_var("name", None, None, None);
        let r = validate_value(ValidateValueInput {
            variable: Variable::Text(v),
            value: serde_json::json!("  "),
        });
        assert!(matches!(r, Err(ref issues) if issues.iter().any(|i| matches!(i, ValidationIssue::Required { .. }))));
    }

    #[test]
    fn text_min_max_use_char_count() {
        // 'é' is 2 bytes but 1 scalar.
        let v = text_var("name", Some(3), Some(4), None);
        let ok = validate_value(ValidateValueInput {
            variable: Variable::Text(v.clone()),
            value: serde_json::json!("café"),
        });
        assert!(ok.is_ok(), "got {:?}", ok);
        let too_short = validate_value(ValidateValueInput {
            variable: Variable::Text(v),
            value: serde_json::json!("hi"),
        });
        assert!(matches!(too_short, Err(ref i) if i.iter().any(|x| matches!(x, ValidationIssue::MinLength { .. }))));
    }

    #[test]
    fn text_pattern_matches() {
        let v = text_var("name", None, None, Some("^[a-z]+$"));
        let ok = validate_value(ValidateValueInput {
            variable: Variable::Text(v.clone()),
            value: serde_json::json!("abc"),
        });
        assert!(ok.is_ok());
        let bad = validate_value(ValidateValueInput {
            variable: Variable::Text(v),
            value: serde_json::json!("Abc1"),
        });
        assert!(bad.is_err());
    }

    #[test]
    fn multiline_required_needs_non_whitespace() {
        let v = MultilineVariable {
            key: "body".into(),
            label: "Body".into(),
            description: None,
            required: true,
            default_value: None,
            order: 0,
            source: VariableSource::Parsed,
            min_length: None,
            max_length: None,
            trim_trailing_whitespace: false,
        };
        let bad = validate_value(ValidateValueInput {
            variable: Variable::Multiline(v),
            value: serde_json::json!("\n   \n\t\n"),
        });
        assert!(bad.is_err());
    }

    #[test]
    fn select_value_must_be_in_options() {
        let v = SelectVariable {
            key: "depth".into(),
            label: "Depth".into(),
            description: None,
            required: true,
            default_value: Some("a".into()),
            order: 0,
            source: VariableSource::Parsed,
            options: vec![
                SelectOption {
                    value: "a".into(),
                    label: "A".into(),
                },
                SelectOption {
                    value: "b".into(),
                    label: "B".into(),
                },
            ],
        };
        let ok = validate_value(ValidateValueInput {
            variable: Variable::Select(v.clone()),
            value: serde_json::json!("a"),
        });
        assert!(ok.is_ok());
        let bad = validate_value(ValidateValueInput {
            variable: Variable::Select(v),
            value: serde_json::json!("Z"),
        });
        assert!(matches!(bad, Err(ref i) if i.iter().any(|x| matches!(x, ValidationIssue::SelectValueNotInOptions { .. }))));
    }

    #[test]
    fn bool_uses_default_when_null() {
        let v = BoolVariable {
            key: "f".into(),
            label: "F".into(),
            description: None,
            required: false,
            default_value: Some(true),
            order: 0,
            source: VariableSource::Parsed,
            render_true: "yes".into(),
            render_false: "no".into(),
        };
        let r = validate_value(ValidateValueInput {
            variable: Variable::Bool(v),
            value: serde_json::Value::Null,
        });
        assert!(matches!(r, Ok(ResolvedVariableValue::Bool { value: true, .. })));
    }

    #[test]
    fn number_must_be_finite() {
        let v = NumberVariable {
            key: "n".into(),
            label: "N".into(),
            description: None,
            required: true,
            default_value: None,
            order: 0,
            source: VariableSource::Parsed,
            min: None,
            max: None,
            step: None,
            integer: false,
        };
        let nan = validate_value(ValidateValueInput {
            variable: Variable::Number(v.clone()),
            value: serde_json::json!(f64::NAN),
        });
        // serde_json doesn't actually serialize NaN — represented as null.
        assert!(nan.is_err());
    }

    #[test]
    fn number_step_respects_epsilon() {
        let v = NumberVariable {
            key: "n".into(),
            label: "N".into(),
            description: None,
            required: true,
            default_value: None,
            order: 0,
            source: VariableSource::Parsed,
            min: Some(0.0),
            max: Some(10.0),
            step: Some(1.0),
            integer: true,
        };
        let ok = validate_value(ValidateValueInput {
            variable: Variable::Number(v.clone()),
            value: serde_json::json!(3),
        });
        assert!(ok.is_ok(), "got {:?}", ok);
        let bad = validate_value(ValidateValueInput {
            variable: Variable::Number(v),
            value: serde_json::json!(3.5),
        });
        assert!(matches!(bad, Err(ref i) if i.iter().any(|x| matches!(x, ValidationIssue::NotInteger { .. }))));
    }

    #[test]
    fn file_required_rejects_empty_string() {
        let v = FileVariable {
            key: "f".into(),
            label: "F".into(),
            description: None,
            required: true,
            default_value: None,
            order: 0,
            source: VariableSource::Parsed,
            must_exist: false,
            allowed_extensions: vec![],
        };
        let r = validate_value(ValidateValueInput {
            variable: Variable::File(v),
            value: serde_json::json!(""),
        });
        assert!(matches!(r, Err(ref i) if i.iter().any(|x| matches!(x, ValidationIssue::Required { .. }))));
    }

    #[test]
    fn folder_writable_probe_succeeds_in_tempdir() {
        let dir = tempfile::tempdir().unwrap();
        let v = FolderVariable {
            key: "d".into(),
            label: "D".into(),
            description: None,
            required: true,
            default_value: None,
            order: 0,
            source: VariableSource::Parsed,
            must_exist: true,
            must_be_writable: true,
        };
        let r = validate_value(ValidateValueInput {
            variable: Variable::Folder(v),
            value: serde_json::Value::String(dir.path().to_string_lossy().to_string()),
        });
        assert!(r.is_ok(), "got {:?}", r);
        // Probe cleaned up.
        let probe = dir.path().join(format!(".promptibrary-write-test-{}", process::id()));
        assert!(!probe.exists());
    }

    #[test]
    fn file_extension_check_is_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hello.RS");
        std::fs::write(&path, b"x").unwrap();
        let v = FileVariable {
            key: "f".into(),
            label: "F".into(),
            description: None,
            required: true,
            default_value: None,
            order: 0,
            source: VariableSource::Parsed,
            must_exist: true,
            allowed_extensions: vec!["rs".into()],
        };
        let r = validate_value(ValidateValueInput {
            variable: Variable::File(v),
            value: serde_json::Value::String(path.to_string_lossy().to_string()),
        });
        assert!(r.is_ok(), "got {:?}", r);
    }

    // Touch the unused VariableType import so warnings stay clean.
    #[allow(dead_code)]
    fn _type_check(_t: VariableType) {}
}
