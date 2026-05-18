//! `commands::vault` per spec §11.
//!
//! TODO(L1): implement against `vault::scanner` + `vault::repair` + `index::db`.

use serde_json::Value;

use crate::error::{AppError, Result};

#[tauri::command]
pub async fn select_vault() -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn validate_vault(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn scan_vault(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn rebuild_index() -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn get_vault_status() -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}
