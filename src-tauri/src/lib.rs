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
        .invoke_handler(tauri::generate_handler![
            // commands::vault
            commands::vault::select_vault,
            commands::vault::validate_vault,
            commands::vault::scan_vault_cmd,
            commands::vault::rebuild_index,
            commands::vault::get_vault_status,
            // commands::prompts
            commands::prompts::list_prompts,
            commands::prompts::get_prompt,
            commands::prompts::create_prompt,
            commands::prompts::update_prompt,
            commands::prompts::archive_prompt_cmd,
            commands::prompts::delete_prompt,
            commands::prompts::export_prompt,
            // commands::variables
            commands::variables::parse_variables,
            commands::variables::validate_launch_inputs,
            commands::variables::render_prompt_preview,
            // commands::search
            commands::search::search_prompts,
            commands::search::suggest_tags,
            commands::search::cmdk_search,
            // commands::launches
            commands::launches::start_launch,
            commands::launches::stop_run,
            commands::launches::send_terminal_input,
            commands::launches::resize_terminal,
            // commands::runs
            commands::runs::list_runs,
            commands::runs::get_run,
            commands::runs::get_prompt_runs,
            commands::runs::fetch_transcript,
            commands::runs::repair_orphaned_transcripts,
            // commands::extraction
            commands::extraction::detect_source,
            commands::extraction::fetch_source_preview,
            commands::extraction::extract_prompt_candidates,
            commands::extraction::save_extracted_prompt,
            // commands::settings
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::set_secret,
            commands::settings::clear_secret,
            commands::settings::get_secret_status,
            // commands::git
            commands::git::get_prompt_history,
            commands::git::get_prompt_diff,
            commands::git::revert_prompt_to_commit,
            // commands::system
            commands::system::probe_dependencies,
            commands::system::reveal_in_terminal,
            commands::system::open_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
