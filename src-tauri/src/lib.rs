//! Promptibrary application library.
//!
//! Mirrors the Tauri 2 convention: `main.rs` is a thin entry that calls
//! `promptibrary_lib::run()`; everything else lives here so Tauri's mobile
//! entry-point macro can attach to `run` directly.

pub mod app_state;
pub mod error;
pub mod ids;
pub mod time;

pub mod commands;
pub mod domain;

pub mod vault;
pub mod index;
pub mod variables;
pub mod extraction;
pub mod launch;
pub mod terminal;
pub mod git;
pub mod settings;
pub mod system;
pub mod util;

use std::sync::Arc;

use app_state::AppServices;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let services = Arc::new(AppServices::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(services)
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
