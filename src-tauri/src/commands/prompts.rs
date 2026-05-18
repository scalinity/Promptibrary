//! `commands::prompts` per spec §11.
//!
//! TODO(L1): implement against `vault::writer` + `vault::frontmatter` + `index::prompts_repo`.

use serde_json::Value;

use crate::error::{AppError, Result};

#[tauri::command]
pub async fn list_prompts(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn get_prompt(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn create_prompt(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn update_prompt(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn archive_prompt(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn delete_prompt(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn export_prompt(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}
