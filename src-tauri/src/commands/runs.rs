//! `commands::runs` per spec §11.
//!
//! TODO(L3): implement against `index::runs_repo` + `vault::repair`.

use serde_json::Value;

use crate::error::{AppError, Result};

#[tauri::command]
pub async fn list_runs(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn get_run(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn get_prompt_runs(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn fetch_transcript(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn repair_orphaned_transcripts() -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}
