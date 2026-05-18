//! IPC command modules per spec §11.
//!
//! Every command is a thin `#[tauri::command]` wrapper. L0 stubs return
//! `AppError::internal("not_yet_implemented")`; the dispatcher in
//! `lib.rs::run` registers each one so the frontend can call any command and
//! get a typed error rather than "command not found".

pub mod prompts;
pub mod variables;
pub mod vault;
pub mod search;
pub mod launches;
pub mod runs;
pub mod extraction;
pub mod settings;
pub mod git;
pub mod system;
