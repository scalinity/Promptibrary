//! `commands::extraction` per spec §11.
//!
//! TODO(L4): implement against `extraction::detect` + `extraction::fetchers` + `extraction::anthropic`.

use serde_json::Value;

use crate::error::{AppError, Result};

#[tauri::command]
pub async fn detect_source(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn fetch_source_preview(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn extract_prompt_candidates(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn save_extracted_prompt(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}
