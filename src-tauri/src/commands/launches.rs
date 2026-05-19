//! `commands::launches` per spec §11.
//!
//! TODO(L3): implement against `launch::pty_pool` + `launch::claude_cli`.

use serde_json::Value;

use crate::commands::not_yet_implemented_stub;
use crate::error::Result;

#[tauri::command]
pub async fn start_launch(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::launches::start_launch")
}

#[tauri::command]
pub async fn stop_run(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::launches::stop_run")
}

#[tauri::command]
pub async fn send_terminal_input(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::launches::send_terminal_input")
}

#[tauri::command]
pub async fn resize_terminal(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::launches::resize_terminal")
}
