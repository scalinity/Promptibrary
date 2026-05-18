//! `commands::git` per spec §11.
//!
//! TODO(L5): implement against `git::history` + `git::diff` + `git::revert`.

use serde_json::Value;

use crate::error::{AppError, Result};

#[tauri::command]
pub async fn get_prompt_history(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn get_prompt_diff(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn revert_prompt_to_commit(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}
