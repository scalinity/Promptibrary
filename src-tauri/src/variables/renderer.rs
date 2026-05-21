//! Renders the final prompt by replacing variable refs end-to-start to
//! preserve byte offsets across the rewrite.
//!
//! Spec §5 *Renderer contract*.
//!
//! Rules implemented:
//!   1. Replace from the end of the template to preserve offsets.
//!   2. `file` / `folder` render as absolute paths (the validator
//!      canonicalizes; the renderer just stringifies).
//!   3. `text` / `multiline` render the validated string verbatim.
//!   4. `select` renders the option's `value` (not its `label`).
//!   5. `bool` renders `renderTrue` / `renderFalse` when mode is
//!      `frontmatter_strings`; otherwise the literal "true"/"false".
//!   6. `number` renders canonical decimal: no thousands separators,
//!      integer numbers without `.0`.
//!   7. No shell escaping. The resolved prompt goes to PTY stdin via
//!      bracketed paste — not into a shell command.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::domain::variable::{Variable, VariableType};
use crate::variables::parser::VariableRef;

// ---------- Public types -----------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BoolRenderMode {
    Literal,
    FrontmatterStrings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResolvedVariableValue {
    File { key: String, value: PathBuf },
    Folder { key: String, value: PathBuf },
    Text { key: String, value: String },
    Multiline { key: String, value: String },
    Select { key: String, value: String },
    Bool { key: String, value: bool },
    Number { key: String, value: f64 },
}

impl ResolvedVariableValue {
    pub fn key(&self) -> &str {
        match self {
            Self::File { key, .. }
            | Self::Folder { key, .. }
            | Self::Text { key, .. }
            | Self::Multiline { key, .. }
            | Self::Select { key, .. }
            | Self::Bool { key, .. }
            | Self::Number { key, .. } => key,
        }
    }

    pub fn variable_type(&self) -> VariableType {
        match self {
            Self::File { .. } => VariableType::File,
            Self::Folder { .. } => VariableType::Folder,
            Self::Text { .. } => VariableType::Text,
            Self::Multiline { .. } => VariableType::Multiline,
            Self::Select { .. } => VariableType::Select,
            Self::Bool { .. } => VariableType::Bool,
            Self::Number { .. } => VariableType::Number,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPromptInput {
    pub template: String,
    pub refs: Vec<VariableRef>,
    pub values: Vec<ResolvedVariableValue>,
    pub bool_render_mode: BoolRenderMode,
    /// Frontmatter `variables` list — needed for bool renderTrue/renderFalse
    /// strings and for `select` value validation against declared options.
    #[serde(default)]
    pub variables: Vec<Variable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPromptOutput {
    pub rendered: String,
    pub replacements: Vec<RenderReplacement>,
    pub errors: Vec<RenderError>,
    pub rendered_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderReplacement {
    pub key: String,
    #[serde(rename = "type")]
    pub var_type: VariableType,
    pub start_utf16: usize,
    pub end_utf16: usize,
    pub rendered_value_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum RenderError {
    MissingValue {
        key: String,
    },
    TypeMismatch {
        key: String,
        expected: VariableType,
        actual: VariableType,
    },
    InvalidSelectValue {
        key: String,
        value: String,
        allowed: Vec<String>,
    },
}

// ---------- Entrypoint ------------------------------------------------------

pub fn render_prompt(input: RenderPromptInput) -> RenderPromptOutput {
    let mut errors: Vec<RenderError> = Vec::new();
    let mut replacements: Vec<RenderReplacement> = Vec::new();

    // Resolve each ref to a string. We walk the refs in lexical order to
    // build replacements, then apply them tail-first.
    let mut resolved: Vec<(usize, usize, usize, usize, VariableType, String, String)> = Vec::new();
    // (start_byte, end_byte, start_utf16, end_utf16, var_type, key, rendered_value)

    for r in &input.refs {
        let value = input.values.iter().find(|v| v.key() == r.key);
        let value = match value {
            Some(v) => v,
            None => {
                errors.push(RenderError::MissingValue {
                    key: r.key.clone(),
                });
                continue;
            }
        };
        if value.variable_type() != r.var_type {
            errors.push(RenderError::TypeMismatch {
                key: r.key.clone(),
                expected: r.var_type,
                actual: value.variable_type(),
            });
            continue;
        }

        let rendered = match value {
            ResolvedVariableValue::File { value, .. } => value.to_string_lossy().to_string(),
            ResolvedVariableValue::Folder { value, .. } => value.to_string_lossy().to_string(),
            ResolvedVariableValue::Text { value, .. } => value.clone(),
            ResolvedVariableValue::Multiline { value, .. } => value.clone(),
            ResolvedVariableValue::Select { key, value } => {
                // Validate against declared options when we know them. Prefer
                // the ref's parsed options; fall back to the matching
                // Variable in input.variables.
                let allowed: Option<Vec<String>> = if let Some(opts) = &r.options {
                    if opts.is_empty() {
                        None
                    } else {
                        Some(opts.iter().map(|o| o.value.clone()).collect())
                    }
                } else {
                    input
                        .variables
                        .iter()
                        .find_map(|v| match v {
                            Variable::Select(s) if &s.key == key => {
                                Some(s.options.iter().map(|o| o.value.clone()).collect())
                            }
                            _ => None,
                        })
                };
                if let Some(allowed) = allowed {
                    if !allowed.iter().any(|o| o == value) {
                        errors.push(RenderError::InvalidSelectValue {
                            key: key.clone(),
                            value: value.clone(),
                            allowed,
                        });
                        continue;
                    }
                }
                value.clone()
            }
            ResolvedVariableValue::Bool { key, value } => match input.bool_render_mode {
                BoolRenderMode::Literal => {
                    if *value {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }
                }
                BoolRenderMode::FrontmatterStrings => {
                    let (t, f) = input
                        .variables
                        .iter()
                        .find_map(|v| match v {
                            Variable::Bool(b) if &b.key == key => {
                                Some((b.render_true.clone(), b.render_false.clone()))
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| ("true".into(), "false".into()));
                    if *value {
                        t
                    } else {
                        f
                    }
                }
            },
            ResolvedVariableValue::Number { value, .. } => format_number(*value),
        };

        replacements.push(RenderReplacement {
            key: r.key.clone(),
            var_type: r.var_type,
            start_utf16: r.start_utf16,
            end_utf16: r.end_utf16,
            rendered_value_preview: truncate_preview(&rendered, 80),
        });
        resolved.push((
            r.start_byte,
            r.end_byte,
            r.start_utf16,
            r.end_utf16,
            r.var_type,
            r.key.clone(),
            rendered,
        ));
    }

    // Apply replacements from end → start.
    let mut rendered = input.template.clone();
    resolved.sort_by(|a, b| b.0.cmp(&a.0));
    for (start_byte, end_byte, _su, _eu, _t, _k, value) in resolved {
        rendered.replace_range(start_byte..end_byte, &value);
    }

    let mut h = Sha256::new();
    h.update(rendered.as_bytes());
    let rendered_sha256 = format!("sha256:{}", hex::encode_lower(h.finalize()));

    RenderPromptOutput {
        rendered,
        replacements,
        errors,
        rendered_sha256,
    }
}

fn format_number(v: f64) -> String {
    if v.is_nan() || v.is_infinite() {
        // The validator should have rejected these; render a stable string.
        return format!("{}", v);
    }
    if v.fract() == 0.0 && v.abs() < 1e16 {
        format!("{}", v as i64)
    } else {
        // Default Rust display avoids thousands separators and uses the
        // shortest round-tripping representation.
        format!("{}", v)
    }
}

fn truncate_preview(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    }
}

// SCA-928 (B16): hex encoding now delegates to the canonical util::hex.
// Local module retained as a thin alias for the existing callsite.
mod hex {
    pub fn encode_lower<T: AsRef<[u8]>>(bytes: T) -> String {
        crate::util::hex::encode_lower(bytes.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::variables::parser::{parse_template_variables, ParseVariablesInput};

    fn parse(t: &str) -> Vec<VariableRef> {
        parse_template_variables(ParseVariablesInput {
            template: t.into(),
            frontmatter_variables: vec![],
        })
        .refs
    }

    #[test]
    fn renders_simple_text_replacement() {
        let template = "Hello {{text:name}}!";
        let refs = parse(template);
        let out = render_prompt(RenderPromptInput {
            template: template.into(),
            refs,
            values: vec![ResolvedVariableValue::Text {
                key: "name".into(),
                value: "world".into(),
            }],
            bool_render_mode: BoolRenderMode::Literal,
            variables: vec![],
        });
        assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
        assert_eq!(out.rendered, "Hello world!");
    }

    #[test]
    fn replaces_end_first_to_preserve_offsets() {
        // The first ref expands much larger than its source. If we replaced
        // start-first, the second ref's byte offsets would no longer point
        // into the original locations.
        let template = "{{text:a}}=A,{{text:b}}=B";
        let refs = parse(template);
        let out = render_prompt(RenderPromptInput {
            template: template.into(),
            refs,
            values: vec![
                ResolvedVariableValue::Text {
                    key: "a".into(),
                    value: "first-much-longer".into(),
                },
                ResolvedVariableValue::Text {
                    key: "b".into(),
                    value: "second".into(),
                },
            ],
            bool_render_mode: BoolRenderMode::Literal,
            variables: vec![],
        });
        assert_eq!(out.rendered, "first-much-longer=A,second=B");
    }

    #[test]
    fn missing_value_emits_error() {
        let template = "Hello {{text:name}}";
        let refs = parse(template);
        let out = render_prompt(RenderPromptInput {
            template: template.into(),
            refs,
            values: vec![],
            bool_render_mode: BoolRenderMode::Literal,
            variables: vec![],
        });
        assert!(out
            .errors
            .iter()
            .any(|e| matches!(e, RenderError::MissingValue { .. })));
        // The unresolved ref stays in the output.
        assert!(out.rendered.contains("{{text:name}}"));
    }

    #[test]
    fn type_mismatch_emits_error() {
        let template = "{{text:x}}";
        let refs = parse(template);
        let out = render_prompt(RenderPromptInput {
            template: template.into(),
            refs,
            values: vec![ResolvedVariableValue::Number {
                key: "x".into(),
                value: 3.0,
            }],
            bool_render_mode: BoolRenderMode::Literal,
            variables: vec![],
        });
        assert!(out
            .errors
            .iter()
            .any(|e| matches!(e, RenderError::TypeMismatch { .. })));
    }

    #[test]
    fn select_renders_value_not_label() {
        let template = "{{select:depth=a|b|c}}";
        let refs = parse(template);
        let out = render_prompt(RenderPromptInput {
            template: template.into(),
            refs,
            values: vec![ResolvedVariableValue::Select {
                key: "depth".into(),
                value: "b".into(),
            }],
            bool_render_mode: BoolRenderMode::Literal,
            variables: vec![],
        });
        assert_eq!(out.rendered, "b");
        assert!(out.errors.is_empty());
    }

    #[test]
    fn select_with_invalid_value_errors() {
        let template = "{{select:depth=a|b|c}}";
        let refs = parse(template);
        let out = render_prompt(RenderPromptInput {
            template: template.into(),
            refs,
            values: vec![ResolvedVariableValue::Select {
                key: "depth".into(),
                value: "zzz".into(),
            }],
            bool_render_mode: BoolRenderMode::Literal,
            variables: vec![],
        });
        assert!(out
            .errors
            .iter()
            .any(|e| matches!(e, RenderError::InvalidSelectValue { .. })));
    }

    #[test]
    fn bool_literal_mode_renders_true_false_keyword() {
        let template = "{{bool:flag}}";
        let refs = parse(template);
        let out = render_prompt(RenderPromptInput {
            template: template.into(),
            refs,
            values: vec![ResolvedVariableValue::Bool {
                key: "flag".into(),
                value: true,
            }],
            bool_render_mode: BoolRenderMode::Literal,
            variables: vec![],
        });
        assert_eq!(out.rendered, "true");
    }

    #[test]
    fn bool_frontmatter_strings_renders_configured_strings() {
        use crate::domain::variable::{BoolVariable, VariableSource};
        let template = "{{bool:flag}}";
        let refs = parse(template);
        let variables = vec![Variable::Bool(BoolVariable {
            key: "flag".into(),
            label: "Flag".into(),
            description: None,
            required: false,
            default_value: Some(false),
            order: 0,
            source: VariableSource::Frontmatter,
            render_true: "do the thing".into(),
            render_false: "skip the thing".into(),
        })];
        let out = render_prompt(RenderPromptInput {
            template: template.into(),
            refs,
            values: vec![ResolvedVariableValue::Bool {
                key: "flag".into(),
                value: true,
            }],
            bool_render_mode: BoolRenderMode::FrontmatterStrings,
            variables,
        });
        assert_eq!(out.rendered, "do the thing");
    }

    #[test]
    fn number_renders_canonical_decimal() {
        let template = "{{number:n}}";
        let refs = parse(template);
        let cases = [(3.0_f64, "3"), (3.5, "3.5"), (1000.0, "1000"), (-7.0, "-7")];
        for (v, expected) in cases {
            let out = render_prompt(RenderPromptInput {
                template: template.into(),
                refs: refs.clone(),
                values: vec![ResolvedVariableValue::Number {
                    key: "n".into(),
                    value: v,
                }],
                bool_render_mode: BoolRenderMode::Literal,
                variables: vec![],
            });
            assert_eq!(out.rendered, expected, "value {v}");
        }
    }

    #[test]
    fn rendered_sha256_changes_with_payload() {
        let template = "{{text:x}}";
        let refs = parse(template);
        let a = render_prompt(RenderPromptInput {
            template: template.into(),
            refs: refs.clone(),
            values: vec![ResolvedVariableValue::Text {
                key: "x".into(),
                value: "alpha".into(),
            }],
            bool_render_mode: BoolRenderMode::Literal,
            variables: vec![],
        });
        let b = render_prompt(RenderPromptInput {
            template: template.into(),
            refs,
            values: vec![ResolvedVariableValue::Text {
                key: "x".into(),
                value: "beta".into(),
            }],
            bool_render_mode: BoolRenderMode::Literal,
            variables: vec![],
        });
        assert_ne!(a.rendered_sha256, b.rendered_sha256);
        assert!(a.rendered_sha256.starts_with("sha256:"));
    }

    #[test]
    fn unused_value_variable_key_ignored() {
        // values with no matching ref should not error.
        let template = "Hello {{text:a}}";
        let refs = parse(template);
        let out = render_prompt(RenderPromptInput {
            template: template.into(),
            refs,
            values: vec![
                ResolvedVariableValue::Text {
                    key: "a".into(),
                    value: "Alice".into(),
                },
                ResolvedVariableValue::Text {
                    key: "unused".into(),
                    value: "noop".into(),
                },
            ],
            bool_render_mode: BoolRenderMode::Literal,
            variables: vec![],
        });
        assert!(out.errors.is_empty());
        assert_eq!(out.rendered, "Hello Alice");
    }
}

