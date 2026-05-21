//! Promptibrary application library.
//!
//! Mirrors the Tauri 2 convention: `main.rs` is a thin entry that calls
//! `promptibrary_lib::run()`; everything else lives here so Tauri's mobile
//! entry-point macro can attach to `run` directly.

pub mod app_state;
pub mod error;
pub mod ids;
pub mod time;

pub mod assistant;
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
pub mod anthropic_common;

use std::sync::Arc;

use app_state::AppServices;

/// Compile-time check that the updater pubkey was populated in
/// tauri.conf.json before this binary was built. tauri.conf.json is
/// the canonical source of truth; we don't re-read it at runtime to
/// avoid divergent paths. The strict `"pubkey": ""` literal indicates
/// the keypair was never generated.
fn updater_pubkey_is_set() -> bool {
    // SCA-919: include the literal config bytes so a release build
    // with an unpopulated pubkey hard-warns instead of silently
    // shipping an unsigned updater path. include_str! resolves at
    // compile time; the boolean is the trivial substring check.
    const TAURI_CONF: &str = include_str!("../tauri.conf.json");
    // Empty-pubkey marker covers both formatted variations the user
    // might encounter (`"pubkey": ""` and `"pubkey":""`).
    !TAURI_CONF.contains("\"pubkey\": \"\"") && !TAURI_CONF.contains("\"pubkey\":\"\"")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let services = Arc::new(AppServices::new());

    // SCA-919 (C3): wire the updater plugin only when a real pubkey is
    // configured. tauri-plugin-updater 2.x reads `pubkey` from the
    // bundled tauri.conf.json at runtime and refuses to install
    // unsigned bundles when set — that's the fail-closed posture spec
    // §16 mandates. Until the user generates the Tauri signer keypair
    // (interactive: `tauri signer generate` + paste pubkey here +
    // TAURI_SIGNING_PRIVATE_KEY GH secret), skip the plugin entirely
    // so the app doesn't ship a non-functional updater path. A loud
    // warning at startup documents the deferred state.
    let updater_pubkey_present = updater_pubkey_is_set();
    if !updater_pubkey_present {
        tracing::warn!(
            "SCA-919: tauri.conf.json updater.pubkey is empty — updater plugin NOT wired. \
             Run `tauri signer generate` and populate pubkey before shipping a release."
        );
    }

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init());
    if updater_pubkey_present {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }
    builder
        .manage(services)
        // SCA-894 / SCA-895 — replace Tauri's default macOS menu, and own
        // ⌘N via a menu item. ⌘N is one of WKWebView's reserved shortcuts;
        // even with no menu binding the keystroke is intercepted at the
        // AppKit / responder-chain layer and never reaches JS keydown
        // handlers. The canonical macOS fix is to give the shortcut to a
        // menu item we control, then emit a Tauri event to the frontend on
        // activation. App + Edit submenus keep ⌘Q and ⌘C/⌘V/⌘X/⌘A working
        // inside native inputs + CodeMirror.
        .setup(|app| {
            use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
            use tauri::{Emitter, Manager};

            let app_submenu = SubmenuBuilder::new(app, "Promptibrary")
                .about(None)
                .separator()
                .services()
                .separator()
                .hide()
                .hide_others()
                .show_all()
                .separator()
                .quit()
                .build()?;

            let new_prompt = MenuItemBuilder::with_id("new_prompt", "New prompt")
                .accelerator("CmdOrCtrl+N")
                .build(app)?;
            let file_submenu = SubmenuBuilder::new(app, "File")
                .item(&new_prompt)
                .build()?;

            let edit_submenu = SubmenuBuilder::new(app, "Edit")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;

            let menu = MenuBuilder::new(app)
                .items(&[&app_submenu, &file_submenu, &edit_submenu])
                .build()?;
            app.set_menu(menu)?;

            app.on_menu_event(|app, event| {
                if event.id().as_ref() == "new_prompt" {
                    let _ = app.emit("menu:new-prompt", ());
                }
            });

            // SCA-900 — re-attach a previously-selected vault on startup
            // so the user doesn't have to re-pick it every launch. Reads
            // settings.json synchronously to find the path; the actual
            // attach (DB open + migrations) runs in a spawned task so
            // setup() returns fast and the window can render while the
            // vault initializes. Failures log + continue — the user
            // lands on the no-vault state and can re-select.
            let services_for_attach: app_state::ManagedState =
                app.state::<app_state::ManagedState>().inner().clone();
            let local = commands::settings::load_persisted_local(&services_for_attach.app_data_dir);
            if let Some(vault_root) = local.vault_path {
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = commands::vault::attach_vault(&services_for_attach, vault_root.clone()).await {
                        tracing::warn!(
                            path = %vault_root.display(),
                            error = ?e,
                            "auto-attach of persisted vault failed; starting with no vault",
                        );
                    }
                });
            }

            Ok(())
        })
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
            commands::settings::clear_telemetry_cache,
            commands::settings::delete_all_run_history,
            // commands::git
            commands::git::get_prompt_history,
            commands::git::get_prompt_diff,
            commands::git::revert_prompt_to_commit,
            // commands::system
            commands::system::probe_dependencies,
            commands::system::reveal_in_terminal,
            commands::system::open_path,
            // commands::assistant
            commands::assistant::assistant_stream_turn,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
