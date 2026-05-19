//! `commands::extraction` per spec §11.
//!
//! TODO(L4): implement against `extraction::detect` + `extraction::fetchers` + `extraction::anthropic`.

use serde_json::Value;

use crate::commands::not_yet_implemented_stub;
use crate::error::Result;

#[tauri::command]
pub async fn detect_source(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::extraction::detect_source")
}

#[tauri::command]
pub async fn fetch_source_preview(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::extraction::fetch_source_preview")
}

#[tauri::command]
pub async fn extract_prompt_candidates(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::extraction::extract_prompt_candidates")
}

#[tauri::command]
pub async fn save_extracted_prompt(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::extraction::save_extracted_prompt")
}
