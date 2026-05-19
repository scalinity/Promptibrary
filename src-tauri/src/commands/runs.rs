//! `commands::runs` per spec §11.
//!
//! TODO(L3): implement against `index::runs_repo` + `vault::repair`.

use serde_json::Value;

use crate::commands::not_yet_implemented_stub;
use crate::error::Result;

#[tauri::command]
pub async fn list_runs(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::runs::list_runs")
}

#[tauri::command]
pub async fn get_run(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::runs::get_run")
}

#[tauri::command]
pub async fn get_prompt_runs(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::runs::get_prompt_runs")
}

#[tauri::command]
pub async fn fetch_transcript(_input: Value) -> Result<Value> {
    not_yet_implemented_stub("commands::runs::fetch_transcript")
}

#[tauri::command]
pub async fn repair_orphaned_transcripts() -> Result<Value> {
    not_yet_implemented_stub("commands::runs::repair_orphaned_transcripts")
}
