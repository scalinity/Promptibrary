//! `commands::system` per spec §11.
//!
//! TODO(L5): implement against `system::diagnostics` + `system::os` + `launch::process_probe`.

use serde_json::Value;

use crate::error::{AppError, Result};

#[tauri::command]
pub async fn probe_dependencies() -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn reveal_in_terminal(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn open_path(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}
