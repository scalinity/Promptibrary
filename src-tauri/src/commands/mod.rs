//! IPC command modules per spec §11.
//!
//! Each module corresponds to one of the 10 IPC namespaces from the spec.
//! 42 commands total are registered in `lib.rs::run`'s
//! `tauri::generate_handler!` block, distributed across these modules:
//!
//! | Module     | Commands |
//! |------------|---------:|
//! | vault      | 5        |
//! | prompts    | 7        |
//! | variables  | 3        |
//! | search     | 3        |
//! | launches   | 4        |
//! | runs       | 5        |
//! | extraction | 4        |
//! | settings   | 5        |
//! | git        | 3        |
//! | system     | 3        |
//! | **Total**  | **42**   |
//!
//! L0 stubs return `AppError::internal("not_yet_implemented")`; real
//! implementations land progressively in L1–L5. The dispatcher registration
//! makes every command callable (with a typed error response) from day one,
//! so the frontend never hits "command not found".

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
