//! `commands::variables` per spec §11.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::variable::Variable;
use crate::error::{AppError, AppErrorKind, Result};
use crate::variables::parser::{
    parse_template_variables, ParseVariablesInput, ParseVariablesOutput, VariableRef,
};
use crate::variables::renderer::{
    render_prompt, BoolRenderMode, RenderPromptInput, RenderPromptOutput, ResolvedVariableValue,
};
use crate::variables::validation::{validate_value, ValidateValueInput, ValidationIssue};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateLaunchInputsInput {
    pub variables: Vec<Variable>,
    pub values: std::collections::HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateLaunchInputsOutput {
    pub resolved: Vec<ResolvedVariableValue>,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPromptPreviewInput {
    pub template: String,
    pub refs: Vec<VariableRef>,
    pub values: Vec<ResolvedVariableValue>,
    #[serde(default = "default_bool_mode")]
    pub bool_render_mode: BoolRenderMode,
    #[serde(default)]
    pub variables: Vec<Variable>,
}

fn default_bool_mode() -> BoolRenderMode {
    BoolRenderMode::FrontmatterStrings
}

#[tauri::command]
pub async fn parse_variables(input: ParseVariablesInput) -> Result<ParseVariablesOutput> {
    Ok(parse_template_variables(input))
}

#[tauri::command]
pub async fn validate_launch_inputs(
    input: ValidateLaunchInputsInput,
) -> Result<ValidateLaunchInputsOutput> {
    let mut resolved = Vec::new();
    let mut issues = Vec::new();
    for variable in input.variables {
        let key = match &variable {
            Variable::File(v) => v.key.clone(),
            Variable::Folder(v) => v.key.clone(),
            Variable::Text(v) => v.key.clone(),
            Variable::Multiline(v) => v.key.clone(),
            Variable::Select(v) => v.key.clone(),
            Variable::Bool(v) => v.key.clone(),
            Variable::Number(v) => v.key.clone(),
        };
        let value = input.values.get(&key).cloned().unwrap_or(Value::Null);
        match validate_value(ValidateValueInput { variable, value }) {
            Ok(v) => resolved.push(v),
            Err(mut es) => issues.append(&mut es),
        }
    }
    if !issues.is_empty() {
        let payload = serde_json::to_string(&issues).unwrap_or_default();
        return Err(
            AppError::new(AppErrorKind::VariableValidationFailed, "validation failed")
                .with_detail("issues_json", payload),
        );
    }
    Ok(ValidateLaunchInputsOutput { resolved, issues })
}

#[tauri::command]
pub async fn render_prompt_preview(input: RenderPromptPreviewInput) -> Result<RenderPromptOutput> {
    Ok(render_prompt(RenderPromptInput {
        template: input.template,
        refs: input.refs,
        values: input.values,
        bool_render_mode: input.bool_render_mode,
        variables: input.variables,
    }))
}
