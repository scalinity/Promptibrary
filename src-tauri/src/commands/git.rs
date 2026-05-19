//! `commands::git` per spec §11.
//!
//! TODO(L5): implement against `git::history` + `git::diff` + `git::revert`.

use serde_json::Value;

use crate::commands::not_yet_implemented_stub;
use crate::error::Result;

#[tauri::command]
pub async fn get_prompt_history(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::git::get_prompt_history")
}

#[tauri::command]
pub async fn get_prompt_diff(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::git::get_prompt_diff")
}

#[tauri::command]
pub async fn revert_prompt_to_commit(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::git::revert_prompt_to_commit")
}
