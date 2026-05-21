//! Parses typed variable references; computes byte and UTF-16 offsets in one
//! pass. Pure — performs no filesystem validation.
//!
//! Spec §5 *Parser contract*. Outputs:
//!   - `refs`: lexical-order list of `VariableRef`
//!   - `variables`: deduplicated `Variable` records (parser + frontmatter merge)
//!   - `errors`: collected non-fatal errors

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::domain::variable::{
    BoolVariable, FileVariable, FolderVariable, MultilineVariable, NumberVariable, SelectOption,
    SelectVariable, TextVariable, Variable, VariableSource, VariableType,
};
use crate::variables::lexer::{lex_template, OffsetMap, Token, TokenKind};

// ---------- Public types -----------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseVariablesInput {
    pub template: String,
    #[serde(default)]
    pub frontmatter_variables: Vec<Variable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseVariablesOutput {
    pub refs: Vec<VariableRef>,
    pub variables: Vec<Variable>,
    pub errors: Vec<VariableParseError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariableRef {
    pub ref_id: String,
    pub raw: String,
    pub key: String,
    #[serde(rename = "type")]
    pub var_type: VariableType,
    pub start_utf16: usize,
    pub end_utf16: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub options: Option<Vec<SelectOption>>,
    pub constraints: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum VariableParseError {
    UnclosedVariableRef {
        #[serde(rename = "startUtf16")]
        start_utf16: usize,
    },
    UnknownVariableType {
        #[serde(rename = "rawType")]
        raw_type: String,
        #[serde(rename = "startUtf16")]
        start_utf16: usize,
        #[serde(rename = "endUtf16")]
        end_utf16: usize,
    },
    InvalidVariableKey {
        key: String,
        #[serde(rename = "startUtf16")]
        start_utf16: usize,
        #[serde(rename = "endUtf16")]
        end_utf16: usize,
    },
    SelectOptionsRequired {
        #[serde(rename = "startUtf16")]
        start_utf16: usize,
        #[serde(rename = "endUtf16")]
        end_utf16: usize,
    },
    DuplicateSelectOption {
        option: String,
        #[serde(rename = "startUtf16")]
        start_utf16: usize,
        #[serde(rename = "endUtf16")]
        end_utf16: usize,
    },
    InvalidConstraint {
        key: String,
        value: String,
        reason: String,
    },
    ConflictingVariableDefinitions {
        key: String,
        #[serde(rename = "firstType")]
        first_type: VariableType,
        #[serde(rename = "secondType")]
        second_type: VariableType,
    },
}

// ---------- Entrypoint -------------------------------------------------------

pub fn parse_template_variables(input: ParseVariablesInput) -> ParseVariablesOutput {
    let offsets = OffsetMap::build(&input.template);
    let (tokens, lex_errs) = lex_template(&input.template, &offsets);

    let mut errors: Vec<VariableParseError> = lex_errs
        .into_iter()
        .map(|e| VariableParseError::UnclosedVariableRef {
            start_utf16: e.start_utf16,
        })
        .collect();

    let mut refs: Vec<VariableRef> = Vec::new();
    // Per-type shorthand counter. Vec instead of HashMap so VariableType
    // doesn't need a Hash derive on the domain type.
    let mut shorthand_counters: Vec<(VariableType, usize)> = Vec::new();

    for tok in &tokens {
        if tok.kind != TokenKind::VariableBlockRaw {
            continue;
        }
        let (maybe_ref, block_errs) = parse_block(tok, &mut shorthand_counters);
        errors.extend(block_errs);
        if let Some(r) = maybe_ref {
            refs.push(r);
        }
    }

    let mut variables: Vec<Variable> = Vec::new();
    let mut seen_keys: Vec<(String, VariableType)> = Vec::new();

    for r in &refs {
        if let Some((_, existing_type)) = seen_keys.iter().find(|(k, _)| k == &r.key) {
            if *existing_type != r.var_type {
                errors.push(VariableParseError::ConflictingVariableDefinitions {
                    key: r.key.clone(),
                    first_type: *existing_type,
                    second_type: r.var_type,
                });
            }
            continue;
        }
        seen_keys.push((r.key.clone(), r.var_type));
        variables.push(build_default_variable(r, variables.len() as u32));
    }

    for fm in &input.frontmatter_variables {
        let key = variable_key(fm);
        let fm_type = variable_type(fm);
        if let Some(existing) = variables.iter_mut().find(|v| variable_key(v) == key) {
            let existing_type = variable_type(existing);
            if existing_type != fm_type {
                errors.push(VariableParseError::ConflictingVariableDefinitions {
                    key: key.clone(),
                    first_type: existing_type,
                    second_type: fm_type,
                });
                continue;
            }
            *existing = merge_frontmatter(existing.clone(), fm.clone());
        } else if !variable_required(fm) {
            let mut clone = fm.clone();
            set_source(&mut clone, VariableSource::Frontmatter);
            variables.push(clone);
        }
    }

    ParseVariablesOutput {
        refs,
        variables,
        errors,
    }
}

// ---------- Block parsing ----------------------------------------------------

fn parse_block(
    tok: &Token,
    shorthand: &mut Vec<(VariableType, usize)>,
) -> (Option<VariableRef>, Vec<VariableParseError>) {
    let inner_raw = &tok.raw[2..tok.raw.len() - 2];
    let inner = inner_raw.trim();
    if inner.is_empty() {
        return (
            None,
            vec![VariableParseError::UnknownVariableType {
                raw_type: String::new(),
                start_utf16: tok.start_utf16,
                end_utf16: tok.end_utf16,
            }],
        );
    }

    let (head, constraint_str) = match inner.find('?') {
        Some(i) => (&inner[..i], Some(inner[i + 1..].trim())),
        None => (inner, None),
    };
    let head = head.trim();

    let (type_name, payload) = match head.split_once(':') {
        Some((t, p)) => (t.trim(), Some(p.trim())),
        None => (head, None),
    };

    let var_type = match type_name {
        "file" => VariableType::File,
        "folder" => VariableType::Folder,
        "text" => VariableType::Text,
        "multiline" => VariableType::Multiline,
        "select" => VariableType::Select,
        "bool" => VariableType::Bool,
        "number" => VariableType::Number,
        other => {
            return (
                None,
                vec![VariableParseError::UnknownVariableType {
                    raw_type: other.to_string(),
                    start_utf16: tok.start_utf16,
                    end_utf16: tok.end_utf16,
                }],
            );
        }
    };

    let mut errors: Vec<VariableParseError> = Vec::new();

    let (key, options) = match var_type {
        VariableType::Select => {
            let p = payload.unwrap_or("").trim();
            if p.is_empty() {
                errors.push(VariableParseError::SelectOptionsRequired {
                    start_utf16: tok.start_utf16,
                    end_utf16: tok.end_utf16,
                });
                let key = shorthand_key(VariableType::Select, shorthand);
                (key, Some(Vec::new()))
            } else {
                let (key, options_part) = if let Some(eq) = p.find('=') {
                    let k = p[..eq].trim();
                    let opt = p[eq + 1..].trim();
                    if !is_valid_key(k) {
                        errors.push(VariableParseError::InvalidVariableKey {
                            key: k.to_string(),
                            start_utf16: tok.start_utf16,
                            end_utf16: tok.end_utf16,
                        });
                    }
                    (k.to_string(), opt)
                } else {
                    (shorthand_key(VariableType::Select, shorthand), p)
                };
                let parsed_opts = parse_select_options(
                    options_part,
                    tok.start_utf16,
                    tok.end_utf16,
                    &mut errors,
                );
                if parsed_opts.is_empty() {
                    errors.push(VariableParseError::SelectOptionsRequired {
                        start_utf16: tok.start_utf16,
                        end_utf16: tok.end_utf16,
                    });
                }
                (key, Some(parsed_opts))
            }
        }
        _ => {
            let key = match payload {
                Some(p) if !p.is_empty() => {
                    if !is_valid_key(p) {
                        errors.push(VariableParseError::InvalidVariableKey {
                            key: p.to_string(),
                            start_utf16: tok.start_utf16,
                            end_utf16: tok.end_utf16,
                        });
                    }
                    p.to_string()
                }
                _ => shorthand_key(var_type, shorthand),
            };
            (key, None)
        }
    };

    let constraints = match constraint_str {
        Some(s) if !s.is_empty() => parse_constraints(s),
        _ => HashMap::new(),
    };

    // SCA-612: f64::parse accepts "NaN" / "inf" / "-inf"; NaN comparisons
    // also return false for `>` so a {{number:n?min=NaN,max=10}} would
    // sneak past the range check below. Reject non-finite values up
    // front, then perform the range comparison only when both sides are
    // finite.
    let min_parsed = constraints.get("min").and_then(|s| s.parse::<f64>().ok());
    let max_parsed = constraints.get("max").and_then(|s| s.parse::<f64>().ok());
    for (key, val) in [("min", min_parsed), ("max", max_parsed)] {
        if let Some(v) = val {
            if !v.is_finite() {
                errors.push(VariableParseError::InvalidConstraint {
                    key: key.into(),
                    value: format!("{}", v),
                    reason: format!("{key}={v} is not a finite number"),
                });
            }
        }
    }
    if let (Some(min), Some(max)) = (min_parsed, max_parsed) {
        if min.is_finite() && max.is_finite() && min > max {
            errors.push(VariableParseError::InvalidConstraint {
                key: "min".into(),
                value: format!("{}", min),
                reason: format!("min={} exceeds max={}", min, max),
            });
        }
    }

    // Text pattern: compile at parse time so a malformed regex surfaces
    // immediately and the compiled form is warm in the cache. SCA-591.
    if matches!(var_type, VariableType::Text) {
        if let Some(pat) = constraints.get("pattern") {
            if let Err(e) = crate::variables::regex_cache::ensure_compiled(pat) {
                errors.push(VariableParseError::InvalidConstraint {
                    key: "pattern".into(),
                    value: pat.clone(),
                    reason: format!("invalid regex: {}", e),
                });
            }
        }
    }

    let ref_id = compute_ref_id(&tok.raw, tok.start_byte, tok.end_byte);

    let r = VariableRef {
        ref_id,
        raw: tok.raw.clone(),
        key,
        var_type,
        start_utf16: tok.start_utf16,
        end_utf16: tok.end_utf16,
        start_byte: tok.start_byte,
        end_byte: tok.end_byte,
        options,
        constraints,
    };

    (Some(r), errors)
}

// ---------- Helpers ----------------------------------------------------------

fn shorthand_key(t: VariableType, shorthand: &mut Vec<(VariableType, usize)>) -> String {
    let n = if let Some(pos) = shorthand.iter().position(|(ty, _)| *ty == t) {
        shorthand[pos].1 += 1;
        shorthand[pos].1
    } else {
        shorthand.push((t, 1));
        1
    };
    let base = match t {
        VariableType::File => "file",
        VariableType::Folder => "folder",
        VariableType::Text => "text",
        VariableType::Multiline => "multiline",
        VariableType::Select => "select",
        VariableType::Bool => "bool",
        VariableType::Number => "number",
    };
    if n == 1 {
        base.to_string()
    } else {
        format!("{}_{}", base, n)
    }
}

pub fn is_valid_key(s: &str) -> bool {
    let mut chars = s.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return false;
        }
    }
    true
}

fn parse_select_options(
    s: &str,
    start_utf16: usize,
    end_utf16: usize,
    errors: &mut Vec<VariableParseError>,
) -> Vec<SelectOption> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<SelectOption> = Vec::new();
    for raw in s.split('|') {
        let val = raw.trim().to_string();
        if val.is_empty() {
            continue;
        }
        if !seen.insert(val.clone()) {
            errors.push(VariableParseError::DuplicateSelectOption {
                option: val.clone(),
                start_utf16,
                end_utf16,
            });
            continue;
        }
        let label = humanize_label(&val);
        out.push(SelectOption { value: val, label });
    }
    out
}

fn humanize_label(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut new_word = true;
    for ch in value.chars() {
        if ch == '_' || ch == '-' {
            out.push(' ');
            new_word = true;
        } else if new_word {
            for u in ch.to_uppercase() {
                out.push(u);
            }
            new_word = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Parse the constraint section of a variable ref.
///
/// Splits on `,` for pairs and `=` for key/value, with no escape mechanism.
/// **Known limitation (SCA-610):** a `pattern` constraint that contains a
/// literal `,` or `=` will be truncated, since those chars are the
/// delimiters. Example: `?pattern=^[a,b]+$` parses as `pattern=^[a` plus
/// a junk-key segment `b]+$` (which is then dropped because `is_valid_key`
/// rejects keys starting with `[`).
///
/// Workarounds today:
///   1. Author the pattern without `,` or `=` (most regexes don't need
///      them outside character classes).
///   2. Move the pattern into the `pattern` field of frontmatter
///      `variables[].pattern`, which bypasses constraint-string parsing
///      entirely.
///
/// V2-candidate: introduce a quoting mechanism (e.g. backslash-escape or
/// single-quoted values). Not in scope for L1 — the spec §5 grammar
/// hard-codes the unescaped split.
fn parse_constraints(s: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for part in s.split(',') {
        let part = part.trim();
        if let Some(eq) = part.find('=') {
            let k = part[..eq].trim();
            let v = part[eq + 1..].trim();
            if is_valid_key(k) && !v.is_empty() {
                out.insert(k.to_string(), v.to_string());
            }
        }
    }
    out
}

fn compute_ref_id(raw: &str, start: usize, end: usize) -> String {
    let mut h = Sha256::new();
    h.update(raw.as_bytes());
    h.update(start.to_le_bytes());
    h.update(end.to_le_bytes());
    let digest = h.finalize();
    // SCA-928 (B16): single hex-encoding helper in util::hex.
    crate::util::hex::encode_lower(&digest[..8])
}

// ---------- Default-variable construction -----------------------------------

fn build_default_variable(r: &VariableRef, order: u32) -> Variable {
    let key = r.key.clone();
    let label = humanize_label(&key);
    match r.var_type {
        VariableType::File => {
            let allowed_extensions = r
                .constraints
                .get("allowedExtensions")
                .map(|s| {
                    s.split('|')
                        .map(|x| x.trim().trim_start_matches('.').to_string())
                        .filter(|x| !x.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            Variable::File(FileVariable {
                key,
                label,
                description: None,
                required: bool_constraint(&r.constraints, "required", true),
                default_value: None,
                order,
                source: VariableSource::Parsed,
                must_exist: bool_constraint(&r.constraints, "mustExist", true),
                allowed_extensions,
            })
        }
        VariableType::Folder => Variable::Folder(FolderVariable {
            key,
            label,
            description: None,
            required: bool_constraint(&r.constraints, "required", true),
            default_value: None,
            order,
            source: VariableSource::Parsed,
            must_exist: bool_constraint(&r.constraints, "mustExist", true),
            must_be_writable: bool_constraint(&r.constraints, "mustBeWritable", false),
        }),
        VariableType::Text => Variable::Text(TextVariable {
            key,
            label,
            description: None,
            required: bool_constraint(&r.constraints, "required", true),
            default_value: None,
            order,
            source: VariableSource::Parsed,
            min_length: u32_constraint(&r.constraints, "minLength"),
            max_length: u32_constraint(&r.constraints, "maxLength").or(Some(4000)),
            pattern: r.constraints.get("pattern").cloned(),
            trim: bool_constraint(&r.constraints, "trim", true),
        }),
        VariableType::Multiline => Variable::Multiline(MultilineVariable {
            key,
            label,
            description: None,
            required: bool_constraint(&r.constraints, "required", true),
            default_value: None,
            order,
            source: VariableSource::Parsed,
            min_length: u32_constraint(&r.constraints, "minLength"),
            max_length: u32_constraint(&r.constraints, "maxLength").or(Some(60_000)),
            trim_trailing_whitespace: bool_constraint(
                &r.constraints,
                "trimTrailingWhitespace",
                false,
            ),
        }),
        VariableType::Select => {
            let options = r.options.clone().unwrap_or_default();
            let default_value = options.first().map(|o| o.value.clone());
            Variable::Select(SelectVariable {
                key,
                label,
                description: None,
                required: bool_constraint(&r.constraints, "required", true),
                default_value,
                order,
                source: VariableSource::Parsed,
                options,
            })
        }
        VariableType::Bool => Variable::Bool(BoolVariable {
            key,
            label,
            description: None,
            required: bool_constraint(&r.constraints, "required", false),
            default_value: Some(false),
            order,
            source: VariableSource::Parsed,
            render_true: r
                .constraints
                .get("renderTrue")
                .cloned()
                .unwrap_or_else(|| "true".into()),
            render_false: r
                .constraints
                .get("renderFalse")
                .cloned()
                .unwrap_or_else(|| "false".into()),
        }),
        VariableType::Number => Variable::Number(NumberVariable {
            key,
            label,
            description: None,
            required: bool_constraint(&r.constraints, "required", true),
            default_value: None,
            order,
            source: VariableSource::Parsed,
            min: f64_constraint(&r.constraints, "min"),
            max: f64_constraint(&r.constraints, "max"),
            step: f64_constraint(&r.constraints, "step").or(Some(1.0)),
            integer: bool_constraint(&r.constraints, "integer", false),
        }),
    }
}

fn bool_constraint(c: &HashMap<String, String>, key: &str, default: bool) -> bool {
    match c.get(key).map(|s| s.as_str()) {
        Some("true") | Some("True") | Some("TRUE") | Some("1") => true,
        Some("false") | Some("False") | Some("FALSE") | Some("0") => false,
        _ => default,
    }
}

fn u32_constraint(c: &HashMap<String, String>, key: &str) -> Option<u32> {
    c.get(key).and_then(|s| s.parse::<u32>().ok())
}

fn f64_constraint(c: &HashMap<String, String>, key: &str) -> Option<f64> {
    c.get(key).and_then(|s| s.parse::<f64>().ok())
}

// ---------- Frontmatter-merge helpers ---------------------------------------

pub fn variable_key(v: &Variable) -> String {
    match v {
        Variable::File(x) => x.key.clone(),
        Variable::Folder(x) => x.key.clone(),
        Variable::Text(x) => x.key.clone(),
        Variable::Multiline(x) => x.key.clone(),
        Variable::Select(x) => x.key.clone(),
        Variable::Bool(x) => x.key.clone(),
        Variable::Number(x) => x.key.clone(),
    }
}

pub fn variable_type(v: &Variable) -> VariableType {
    match v {
        Variable::File(_) => VariableType::File,
        Variable::Folder(_) => VariableType::Folder,
        Variable::Text(_) => VariableType::Text,
        Variable::Multiline(_) => VariableType::Multiline,
        Variable::Select(_) => VariableType::Select,
        Variable::Bool(_) => VariableType::Bool,
        Variable::Number(_) => VariableType::Number,
    }
}

pub fn variable_required(v: &Variable) -> bool {
    match v {
        Variable::File(x) => x.required,
        Variable::Folder(x) => x.required,
        Variable::Text(x) => x.required,
        Variable::Multiline(x) => x.required,
        Variable::Select(x) => x.required,
        Variable::Bool(x) => x.required,
        Variable::Number(x) => x.required,
    }
}

pub fn set_source(v: &mut Variable, s: VariableSource) {
    match v {
        Variable::File(x) => x.source = s,
        Variable::Folder(x) => x.source = s,
        Variable::Text(x) => x.source = s,
        Variable::Multiline(x) => x.source = s,
        Variable::Select(x) => x.source = s,
        Variable::Bool(x) => x.source = s,
        Variable::Number(x) => x.source = s,
    }
}

/// Merge a parser-discovered Variable record with its frontmatter
/// declaration.
///
/// **Order semantics (SCA-613):** the merged variable takes the
/// frontmatter's `order` field, not the parser's lexical position. This
/// is intentional per spec §5 — the frontmatter is the authoritative
/// source for UI ordering so the prompt author can sort their form
/// independently of how the variable refs happen to appear in the body
/// (e.g. "show `target_file` first even though it's referenced last in
/// the prompt").
///
/// If a frontmatter variable lacks an explicit `order`, serde defaults
/// the field to `0`, which collapses all such variables to the top of
/// the UI. Authors who want lexical ordering should omit the variable
/// entry from frontmatter; the parser-discovered Variable (with
/// `source = Parsed` and lexical `order`) is then used directly.
fn merge_frontmatter(parsed: Variable, fm: Variable) -> Variable {
    match (parsed, fm) {
        (Variable::File(_), Variable::File(fm)) => Variable::File(FileVariable {
            source: VariableSource::Frontmatter,
            ..fm
        }),
        (Variable::Folder(_), Variable::Folder(fm)) => Variable::Folder(FolderVariable {
            source: VariableSource::Frontmatter,
            ..fm
        }),
        (Variable::Text(_), Variable::Text(fm)) => Variable::Text(TextVariable {
            source: VariableSource::Frontmatter,
            ..fm
        }),
        (Variable::Multiline(_), Variable::Multiline(fm)) => Variable::Multiline(MultilineVariable {
            source: VariableSource::Frontmatter,
            ..fm
        }),
        (Variable::Select(p), Variable::Select(fm)) => {
            let options = if !fm.options.is_empty() {
                fm.options
            } else {
                p.options
            };
            Variable::Select(SelectVariable {
                source: VariableSource::Frontmatter,
                options,
                ..fm
            })
        }
        (Variable::Bool(_), Variable::Bool(fm)) => Variable::Bool(BoolVariable {
            source: VariableSource::Frontmatter,
            ..fm
        }),
        (Variable::Number(_), Variable::Number(fm)) => Variable::Number(NumberVariable {
            source: VariableSource::Frontmatter,
            ..fm
        }),
        (_, other) => other,
    }
}

// ---------- Tests ------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> ParseVariablesOutput {
        parse_template_variables(ParseVariablesInput {
            template: s.into(),
            frontmatter_variables: vec![],
        })
    }

    #[test]
    fn single_file_with_key_is_parsed() {
        let out = parse("{{file:target_file}}");
        assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
        assert_eq!(out.refs.len(), 1);
        assert_eq!(out.refs[0].key, "target_file");
    }

    #[test]
    fn shorthand_file_uses_shorthand_key() {
        let out = parse("{{file}}");
        assert_eq!(out.refs[0].key, "file");
    }

    #[test]
    fn multiple_shorthand_files_increment() {
        let out = parse("{{file}} {{file}}");
        assert_eq!(out.refs[0].key, "file");
        assert_eq!(out.refs[1].key, "file_2");
    }

    #[test]
    fn select_with_options() {
        let out = parse("{{select:depth=minimal|moderate|deep}}");
        assert_eq!(out.refs[0].key, "depth");
        let opts = out.refs[0].options.as_ref().unwrap();
        assert_eq!(opts.len(), 3);
    }

    #[test]
    fn select_without_key_uses_shorthand() {
        let out = parse("{{select:a|b|c}}");
        assert_eq!(out.refs[0].key, "select");
    }

    #[test]
    fn select_without_options_errors() {
        let out = parse("{{select}}");
        assert!(out.errors.iter().any(|e| matches!(
            e,
            VariableParseError::SelectOptionsRequired { .. }
        )));
    }

    #[test]
    fn unknown_type_errors() {
        let out = parse("{{unknown}}");
        assert!(out.errors.iter().any(|e| matches!(
            e,
            VariableParseError::UnknownVariableType { .. }
        )));
    }

    #[test]
    fn invalid_key_errors() {
        let out = parse("{{text:9bad}}");
        assert!(out.errors.iter().any(|e| matches!(
            e,
            VariableParseError::InvalidVariableKey { .. }
        )));
    }

    #[test]
    fn unclosed_ref_errors() {
        let out = parse("prefix {{file:foo");
        assert!(out.errors.iter().any(|e| matches!(
            e,
            VariableParseError::UnclosedVariableRef { .. }
        )));
    }

    #[test]
    fn duplicate_keys_merge_when_same_type() {
        let out = parse("{{text:name}} {{text:name}}");
        assert_eq!(out.refs.len(), 2);
        assert_eq!(out.variables.len(), 1);
    }

    #[test]
    fn duplicate_keys_with_different_types_conflict() {
        let out = parse("{{text:x}} {{file:x}}");
        assert!(out.errors.iter().any(|e| matches!(
            e,
            VariableParseError::ConflictingVariableDefinitions { .. }
        )));
    }

    #[test]
    fn whitespace_inside_braces_is_tolerated() {
        let out = parse("{{ file : target }}");
        assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
        assert_eq!(out.refs[0].key, "target");
    }

    #[test]
    fn constraint_pairs_are_captured() {
        let out = parse("{{number:n?min=1,max=12,integer=true}}");
        let r = &out.refs[0];
        assert_eq!(r.constraints.get("min"), Some(&"1".to_string()));
    }

    #[test]
    fn invalid_numeric_range_emits_error() {
        let out = parse("{{number:n?min=10,max=1}}");
        assert!(out.errors.iter().any(|e| matches!(
            e,
            VariableParseError::InvalidConstraint { .. }
        )));
    }

    #[test]
    fn ref_id_is_16_hex_chars() {
        let out = parse("{{file:a}}");
        assert_eq!(out.refs[0].ref_id.len(), 16);
    }

    #[test]
    fn emoji_offsets_track_utf16() {
        let out = parse("🚀 {{file:a}} done");
        let r = &out.refs[0];
        assert_eq!(r.start_byte, 5);
        assert_eq!(r.start_utf16, 3);
    }

    #[test]
    fn select_with_duplicate_options_emits_error() {
        let out = parse("{{select:a|a|b}}");
        assert!(out.errors.iter().any(|e| matches!(
            e,
            VariableParseError::DuplicateSelectOption { .. }
        )));
    }

    #[test]
    fn default_variables_apply_variant_defaults() {
        let out = parse("{{text:name}} {{number:n}}");
        let text = out
            .variables
            .iter()
            .find(|v| variable_key(v) == "name")
            .unwrap();
        if let Variable::Text(t) = text {
            assert_eq!(t.max_length, Some(4000));
            assert!(t.trim);
        } else {
            panic!("expected text");
        }
    }

    #[test]
    fn frontmatter_compatible_override_applies() {
        let frontmatter = vec![Variable::Text(TextVariable {
            key: "name".into(),
            label: "Module name".into(),
            description: Some("desc".into()),
            required: true,
            default_value: None,
            order: 2,
            source: VariableSource::Frontmatter,
            min_length: Some(1),
            max_length: Some(120),
            pattern: None,
            trim: true,
        })];
        let out = parse_template_variables(ParseVariablesInput {
            template: "{{text:name}}".into(),
            frontmatter_variables: frontmatter,
        });
        assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
        if let Variable::Text(t) = &out.variables[0] {
            assert_eq!(t.label, "Module name");
            assert_eq!(t.max_length, Some(120));
        } else {
            panic!("expected text");
        }
    }

    #[test]
    fn frontmatter_type_mismatch_emits_conflict() {
        let frontmatter = vec![Variable::Number(NumberVariable {
            key: "name".into(),
            label: "n".into(),
            description: None,
            required: true,
            default_value: None,
            order: 0,
            source: VariableSource::Frontmatter,
            min: None,
            max: None,
            step: Some(1.0),
            integer: false,
        })];
        let out = parse_template_variables(ParseVariablesInput {
            template: "{{text:name}}".into(),
            frontmatter_variables: frontmatter,
        });
        assert!(out.errors.iter().any(|e| matches!(
            e,
            VariableParseError::ConflictingVariableDefinitions { .. }
        )));
    }

    #[test]
    fn frontmatter_stale_optional_kept() {
        let frontmatter = vec![Variable::Text(TextVariable {
            key: "removed".into(),
            label: "Removed".into(),
            description: None,
            required: false,
            default_value: None,
            order: 0,
            source: VariableSource::Frontmatter,
            min_length: None,
            max_length: None,
            pattern: None,
            trim: true,
        })];
        let out = parse_template_variables(ParseVariablesInput {
            template: "{{text:other}}".into(),
            frontmatter_variables: frontmatter,
        });
        let keys: Vec<String> = out.variables.iter().map(variable_key).collect();
        assert!(keys.contains(&"other".to_string()));
        assert!(keys.contains(&"removed".to_string()));
    }
}
