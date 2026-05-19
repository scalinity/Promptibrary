//! SQLite pool + PRAGMA configuration per CLAUDE.md backend invariants.
//!
//! Every Promptibrary SQLite pool MUST be created with `connect_options` so
//! the three PRAGMAs (`foreign_keys=ON`, `journal_mode=WAL`, `busy_timeout=5000`)
//! are applied on every connection in the pool. SQLite defaults `foreign_keys`
//! to OFF, which would silently disable every `ON DELETE CASCADE` declared in
//! the schema — the cascade declarations live in migrations 0001/0003/0005 but
//! they are inert without this PRAGMA on the connection.

use std::path::Path;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;

use crate::error::Result;

/// Build the canonical Promptibrary `SqliteConnectOptions` for a database
/// path. Always uses the three PRAGMAs required by CLAUDE.md.
pub fn connect_options(path: &Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_millis(5000))
}

/// Build the canonical in-memory variant — same PRAGMAs, used by tests.
/// `journal_mode=WAL` is ignored for `:memory:` databases (SQLite always uses
/// MEMORY for in-memory DBs); we still request it for parity with prod.
pub fn in_memory_connect_options() -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .in_memory(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_millis(5000))
}

/// Open a production pool at `path`. The migration runner is **not** called
/// here — `AppServices` decides when to apply migrations after startup
/// validation (L1+).
#[allow(dead_code)] // wired up by AppServices in L1
pub async fn connect(path: &Path) -> Result<SqlitePool> {
    let opts = connect_options(path);
    let pool = SqlitePoolOptions::new().connect_with(opts).await?;
    Ok(pool)
}
