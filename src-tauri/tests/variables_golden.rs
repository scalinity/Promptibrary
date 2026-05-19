//! §15 *Variable parser* test matrix — golden integration test.
//!
//! Each `.template.txt` under `tests/golden/variables/` is parsed and the
//! ParseVariablesOutput is asserted against a checked-in JSON schema in
//! this file. Spec §15:
//!
//!   | Dimension | Cases |
//!   |---|---|
//!   | Type | all 7 variants |
//!   | Naming | shorthand, named, select named, duplicate keys |
//!   | Constraints | valid, unknown, invalid numeric |
//!   | Unicode | emoji before/inside/after refs, multi-byte offsets |
//!   | Malformed | unclosed, empty, invalid key, unknown type |
//!   | Frontmatter merge | compatible override, incompatible conflict, stale |
//!
//! Frontmatter merge cells are covered by parser unit tests; this file
//! exercises the *template-side* dimensions end-to-end against the public
//! `parse_template_variables` API.

use promptibrary_lib::variables::parser::{
    parse_template_variables, ParseVariablesInput, VariableParseError,
};

fn load(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/variables")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {}", path.display(), e))
}

fn parse(name: &str) -> promptibrary_lib::variables::parser::ParseVariablesOutput {
    let template = load(name);
    parse_template_variables(ParseVariablesInput {
        template,
        frontmatter_variables: vec![],
    })
}

#[test]
fn golden_01_all_types_parses_seven_refs() {
    let out = parse("01_all_types.template.txt");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.refs.len(), 7);

    use promptibrary_lib::domain::variable::VariableType;
    let types: Vec<VariableType> = out.refs.iter().map(|r| r.var_type).collect();
    assert_eq!(
        types,
        vec![
            VariableType::File,
            VariableType::Folder,
            VariableType::Text,
            VariableType::Select,
            VariableType::Bool,
            VariableType::Number,
            VariableType::Multiline,
        ]
    );

    // Constraints on folder + text + number flow through.
    let folder = &out.refs[1];
    assert_eq!(
        folder.constraints.get("mustBeWritable"),
        Some(&"true".to_string())
    );
    let text = &out.refs[2];
    assert_eq!(text.constraints.get("maxLength"), Some(&"120".to_string()));
    let number = &out.refs[5];
    assert_eq!(number.constraints.get("min"), Some(&"1".to_string()));
    assert_eq!(number.constraints.get("max"), Some(&"12".to_string()));
    assert_eq!(number.constraints.get("integer"), Some(&"true".to_string()));

    // Select options.
    let select = &out.refs[3];
    let opts = select.options.as_ref().unwrap();
    assert_eq!(opts.len(), 3);
    assert_eq!(opts[0].value, "minimal");
    assert_eq!(opts[2].value, "deep");
}

#[test]
fn golden_02_unicode_emoji_offsets_are_utf16() {
    let out = parse("02_unicode_emoji.template.txt");
    assert!(out.errors.is_empty());
    assert_eq!(out.refs.len(), 1);
    let r = &out.refs[0];
    // 🚀 (1 char, 2 UTF-16 code units, 4 bytes) + " ship " (6 chars, 6 utf16, 6 bytes)
    // start_utf16 = 2 + 6 = 8
    // start_byte = 4 + 6 = 10
    assert_eq!(r.start_utf16, 8);
    assert_eq!(r.start_byte, 10);
}

#[test]
fn golden_03_malformed_emits_each_error_class() {
    let out = parse("03_malformed.template.txt");
    let has = |pred: fn(&VariableParseError) -> bool| out.errors.iter().any(pred);
    assert!(
        has(|e| matches!(e, VariableParseError::UnclosedVariableRef { .. })),
        "missing UnclosedVariableRef"
    );
    assert!(
        has(|e| matches!(e, VariableParseError::UnknownVariableType { .. })),
        "missing UnknownVariableType"
    );
    assert!(
        has(|e| matches!(e, VariableParseError::InvalidVariableKey { .. })),
        "missing InvalidVariableKey"
    );
    assert!(
        has(|e| matches!(e, VariableParseError::SelectOptionsRequired { .. })),
        "missing SelectOptionsRequired"
    );
    assert!(
        has(|e| matches!(e, VariableParseError::InvalidConstraint { .. })),
        "missing InvalidConstraint"
    );
    assert!(
        has(|e| matches!(e, VariableParseError::DuplicateSelectOption { .. })),
        "missing DuplicateSelectOption"
    );
}

#[test]
fn golden_04_duplicates_merge_and_conflict() {
    let out = parse("04_duplicates.template.txt");
    // 3 refs (text:name × 2 + file:name) — types collide.
    assert_eq!(out.refs.len(), 3);
    assert!(out.errors.iter().any(|e| matches!(
        e,
        VariableParseError::ConflictingVariableDefinitions { .. }
    )));
}

#[test]
fn golden_05_shorthand_increments_per_type() {
    let out = parse("05_shorthand.template.txt");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.refs.len(), 3);
    assert_eq!(out.refs[0].key, "file");
    assert_eq!(out.refs[1].key, "file_2");
    assert_eq!(out.refs[2].key, "folder");
}
