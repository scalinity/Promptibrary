//! `commands::settings` per spec §11.
//!
//! TODO(L5): implement against `settings::local_store` + `settings::vault_store` + `settings::keychain`.

use serde_json::Value;

use crate::error::{AppError, Result};

#[tauri::command]
pub async fn get_settings() -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn update_settings(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn set_secret(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn clear_secret(_input: Value) -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}

#[tauri::command]
pub async fn get_secret_status() -> Result<Value> {
    Err(AppError::internal("not_yet_implemented"))
}
