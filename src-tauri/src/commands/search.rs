//! `commands::search` per spec §11.
//!
//! TODO(L5): implement against `index::fts` + `index::embeddings`.

use serde_json::Value;

use crate::commands::not_yet_implemented_stub;
use crate::error::Result;

#[tauri::command]
pub async fn search_prompts(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::search::search_prompts")
}

#[tauri::command]
pub async fn suggest_tags(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::search::suggest_tags")
}

#[tauri::command]
pub async fn cmdk_search(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::search::cmdk_search")
}
