//! Index — SQLite-backed prompt + run + telemetry storage and full-text/embedding search.
//!
//! L0 scaffold: real implementation lives in L1+; `migrations.rs` carries the
//! migration set written in L0.12.

pub mod db;
pub mod migrations;
pub mod prompts_repo;
pub mod runs_repo;
pub mod telemetry_repo;
pub mod fts;
pub mod embeddings;
pub mod reindex;
