//! Domain types — Rust mirror of `src/shared/types/*` per spec §4.
//!
//! L0 populates the submodules below in `L0.11`. They live here so other
//! modules can import them by path even while bodies are stubs.

pub mod prompt;
pub mod variable;
pub mod launch;
pub mod run;
pub mod tag;
pub mod source;
pub mod settings;
pub mod git;
pub mod search;
pub mod transcript;
