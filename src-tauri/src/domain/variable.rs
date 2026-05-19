//! Typed variable system per spec §4 "Variable".

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VariableSource {
    Parsed,
    Frontmatter,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VariableType {
    File,
    Folder,
    Text,
    Multiline,
    Select,
    Bool,
    Number,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Variable {
    File(FileVariable),
    Folder(FolderVariable),
    Text(TextVariable),
    Multiline(MultilineVariable),
    Select(SelectVariable),
    Bool(BoolVariable),
    Number(NumberVariable),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileVariable {
    pub key: String,
    pub label: String,
    pub description: Option<String>,
    pub required: bool,
    pub default_value: Option<PathBuf>,
    pub order: u32,
    pub source: VariableSource,
    pub must_exist: bool,
    pub allowed_extensions: Vec<String>,
    // V1 is single-file-only per spec §5. If multi-file support is ever added,
    // re-introduce as an `allow_multiple: bool` field on both sides.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderVariable {
    pub key: String,
    pub label: String,
    pub description: Option<String>,
    pub required: bool,
    pub default_value: Option<PathBuf>,
    pub order: u32,
    pub source: VariableSource,
    pub must_exist: bool,
    pub must_be_writable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextVariable {
    pub key: String,
    pub label: String,
    pub description: Option<String>,
    pub required: bool,
    pub default_value: Option<String>,
    pub order: u32,
    pub source: VariableSource,
    pub min_length: Option<u32>,
    pub max_length: Option<u32>,
    pub pattern: Option<String>,
    pub trim: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultilineVariable {
    pub key: String,
    pub label: String,
    pub description: Option<String>,
    pub required: bool,
    pub default_value: Option<String>,
    pub order: u32,
    pub source: VariableSource,
    pub min_length: Option<u32>,
    pub max_length: Option<u32>,
    pub trim_trailing_whitespace: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectVariable {
    pub key: String,
    pub label: String,
    pub description: Option<String>,
    pub required: bool,
    pub default_value: Option<String>,
    pub order: u32,
    pub source: VariableSource,
    pub options: Vec<SelectOption>,
    // V1 selects are option-only per spec §5. Custom-value support would
    // re-introduce `allow_custom: bool` on both sides.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoolVariable {
    pub key: String,
    pub label: String,
    pub description: Option<String>,
    pub required: bool,
    pub default_value: Option<bool>,
    pub order: u32,
    pub source: VariableSource,
    pub render_true: String,
    pub render_false: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumberVariable {
    pub key: String,
    pub label: String,
    pub description: Option<String>,
    pub required: bool,
    pub default_value: Option<f64>,
    pub order: u32,
    pub source: VariableSource,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub integer: bool,
}
