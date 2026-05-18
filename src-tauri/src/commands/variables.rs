//! `commands::variables` per spec §11.
//!
//! TODO(L1): implement against `variables::parser` + `variables::renderer` + `variables::validation`.

use serde_json::Value;

use crate::error::{AppError, Result};

#[tauri::command]
pub async fn parse_variables(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn validate_launch_inputs(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn render_prompt_preview(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}
