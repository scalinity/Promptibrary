//! `commands::search` per spec §11.
//!
//! TODO(L5): implement against `index::fts` + `index::embeddings`.

use serde_json::Value;

use crate::error::{AppError, Result};

#[tauri::command]
pub async fn search_prompts(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn suggest_tags(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn cmdk_search(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}
