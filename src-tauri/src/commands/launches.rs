//! `commands::launches` per spec §11.
//!
//! TODO(L3): implement against `launch::pty_pool` + `launch::claude_cli`.

use serde_json::Value;

use crate::error::{AppError, Result};

#[tauri::command]
pub async fn start_launch(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn stop_run(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn send_terminal_input(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn resize_terminal(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}
