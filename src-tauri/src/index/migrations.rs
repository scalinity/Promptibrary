//! Migration runner.
//!
//! L0 scaffold — the actual SQL migration files in `src-tauri/migrations/`
//! land in L0.12. This module just wires `sqlx::migrate!` to them.

#[allow(dead_code)] // wired up by AppServices in L1
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[allow(dead_code)] // called by AppServices on startup in L1
pub async fn run_migrations(pool: &sqlx::SqlitePool) -> crate::error::Result<()> {
    MIGRATOR
        .run(pool)
        .await
        .map_err(crate::error::AppError::from)?;
    Ok(())
}
