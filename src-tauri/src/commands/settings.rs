//! `commands::settings` per spec §11.
//!
//! TODO(L5): implement against `settings::local_store` + `settings::vault_store` + `settings::keychain`.

use serde_json::Value;

use crate::commands::not_yet_implemented_stub;
use crate::error::Result;

#[tauri::command]
pub async fn get_settings() -> Result<Value> {
    not_yet_implemented_stub("commands::settings::get_settings")
}

#[tauri::command]
pub async fn update_settings(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::settings::update_settings")
}

#[tauri::command]
pub async fn set_secret(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::settings::set_secret")
}

#[tauri::command]
pub async fn clear_secret(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::settings::clear_secret")
}

#[tauri::command]
pub async fn get_secret_status() -> Result<Value> {
    not_yet_implemented_stub("commands::settings::get_secret_status")
}
