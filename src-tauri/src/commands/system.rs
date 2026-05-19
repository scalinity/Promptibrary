//! `commands::system` per spec §11.
//!
//! TODO(L5): implement against `system::diagnostics` + `system::os` + `launch::process_probe`.

use serde_json::Value;

use crate::commands::not_yet_implemented_stub;
use crate::error::Result;

#[tauri::command]
pub async fn probe_dependencies() -> Result<Value> {
    not_yet_implemented_stub("commands::system::probe_dependencies")
}

#[tauri::command]
pub async fn reveal_in_terminal(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::system::reveal_in_terminal")
}

#[tauri::command]
pub async fn open_path(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::system::open_path")
}
