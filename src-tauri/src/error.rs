//! Typed application error per spec §11.
//!
//! L0 scaffold — exhaustive `AppErrorKind` plus a serializable `AppError` /
//! `AppErrorDto`. Later layers extend the `From` conversions and use the
//! per-variant constructors at the call sites.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Exhaustive list of error variants surfaced to the frontend. Stays in lockstep
/// with `src/shared/types/ipc.ts::AppErrorKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppErrorKind {
    VaultMissing,
    VaultInvalid,
    VaultNotGitRepo,
    PromptNotFound,
    PromptMalformed,
    YamlMalformed,
    VariableParseFailed,
    VariableValidationFailed,
    WorkingDirectoryInvalid,
    DependencyMissing,
    PtySpawnFailed,
    RunNotFound,
    RunNotActive,
    TooManyActiveRuns,
    ClaudeCliMissing,
    ClaudeCliFailed,
    AnthropicKeyMissing,
    AnthropicAuthInvalid,
    NetworkUnavailable,
    RateLimited,
    ExtractionFailed,
    MalformedModelOutput,
    SqliteLocked,
    SqliteCorrupt,
    GitError,
    SettingsInvalid,
    KeychainError,
    UnsupportedSource,
    TranscriptUnavailable,
    Internal,
}

/// Detail bag values forwarded to the frontend. Keep keys snake_case.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DetailValue {
    String(String),
    Integer(i64),
    Bool(bool),
    Null,
}

impl From<&str> for DetailValue {
    fn from(s: &str) -> Self {
        DetailValue::String(s.to_string())
    }
}

impl From<String> for DetailValue {
    fn from(s: String) -> Self {
        DetailValue::String(s)
    }
}

impl From<i64> for DetailValue {
    fn from(n: i64) -> Self {
        DetailValue::Integer(n)
    }
}

impl From<bool> for DetailValue {
    fn from(b: bool) -> Self {
        DetailValue::Bool(b)
    }
}

/// Internal app error type. Carries the typed kind, a human message, and an
/// optional detail bag that the frontend renders verbatim.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{kind:?}: {message}")]
pub struct AppError {
    pub kind: AppErrorKind,
    pub message: String,
    pub details: HashMap<String, DetailValue>,
}

impl AppError {
    pub fn new(kind: AppErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            details: HashMap::new(),
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<DetailValue>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::Internal, message)
    }
}

/// Wire-format error sent across the IPC boundary. Mirrors the TS
/// `AppErrorDto` in `src/shared/types/ipc.ts` exactly. Lives in the error
/// module rather than the domain layer because it's a *wire* type, not a
/// business concept — domain modules that need to carry an error reference
/// import it for serialization parity, not for layering coupling.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppErrorDto {
    pub kind: AppErrorKind,
    pub message: String,
    pub details: HashMap<String, DetailValue>,
}

impl From<AppError> for AppErrorDto {
    fn from(e: AppError) -> Self {
        Self {
            kind: e.kind,
            message: e.message,
            details: e.details,
        }
    }
}

// Tauri serializes returned errors via this `Serialize` impl. The fully-qualified
// `std::result::Result` avoids the `crate::error::Result` alias defined below.
impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        AppErrorDto::from(self.clone()).serialize(s)
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

// --- `From` impls so internal code can use `?` ---

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::internal(format!("io: {}", e))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::new(AppErrorKind::Internal, format!("json: {}", e))
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        // Classify SQLite errors by their numeric code rather than message text:
        // - 5  = SQLITE_BUSY (database is locked)
        // - 6  = SQLITE_LOCKED (table-level lock conflict)
        // - 11 = SQLITE_CORRUPT (database disk image is malformed)
        // - 26 = SQLITE_NOTADB (file is not a database, treat as corrupt)
        // Reference: https://sqlite.org/rescode.html
        match &e {
            sqlx::Error::Database(db_err) => match db_err.code().as_deref() {
                Some("5") | Some("6") => {
                    Self::new(AppErrorKind::SqliteLocked, db_err.message().to_string())
                }
                Some("11") | Some("26") => {
                    Self::new(AppErrorKind::SqliteCorrupt, db_err.message().to_string())
                }
                _ => Self::new(AppErrorKind::Internal, format!("sqlx: {}", e)),
            },
            _ => Self::new(AppErrorKind::Internal, format!("sqlx: {}", e)),
        }
    }
}

impl From<sqlx::migrate::MigrateError> for AppError {
    fn from(e: sqlx::migrate::MigrateError) -> Self {
        Self::new(AppErrorKind::Internal, format!("migrate: {}", e))
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_connect() || e.is_timeout() {
            Self::new(AppErrorKind::NetworkUnavailable, format!("network: {}", e))
        } else {
            Self::new(AppErrorKind::Internal, format!("http: {}", e))
        }
    }
}

impl From<notify::Error> for AppError {
    fn from(e: notify::Error) -> Self {
        Self::new(AppErrorKind::Internal, format!("watcher: {}", e))
    }
}

impl From<keyring_core::error::Error> for AppError {
    fn from(e: keyring_core::error::Error) -> Self {
        Self::new(AppErrorKind::KeychainError, format!("keychain: {}", e))
    }
}

impl From<serde_yaml::Error> for AppError {
    fn from(e: serde_yaml::Error) -> Self {
        Self::new(AppErrorKind::YamlMalformed, format!("yaml: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_helper_sets_kind() {
        let err = AppError::internal("boom");
        assert_eq!(err.kind, AppErrorKind::Internal);
        assert_eq!(err.message, "boom");
        assert!(err.details.is_empty());
    }

    #[test]
    fn with_detail_stores_typed_values() {
        let err = AppError::new(AppErrorKind::PromptNotFound, "missing")
            .with_detail("prompt_id", "01JZ7M1K6M8D4E9SZ7P1Q9KT4A")
            .with_detail("attempt", 3i64)
            .with_detail("retryable", true);
        assert_eq!(err.details.len(), 3);
        assert!(matches!(err.details["prompt_id"], DetailValue::String(_)));
        assert!(matches!(err.details["attempt"], DetailValue::Integer(3)));
        assert!(matches!(err.details["retryable"], DetailValue::Bool(true)));
    }

    #[test]
    fn ipc_error_serializes_with_camelcase() {
        let err = AppError::new(AppErrorKind::VaultMissing, "no vault");
        let wire = serde_json::to_value(&err).unwrap();
        assert_eq!(wire["kind"], "VaultMissing");
        assert_eq!(wire["message"], "no vault");
        assert!(wire["details"].is_object());
    }

    #[test]
    fn io_error_collapses_to_internal() {
        let io = std::io::Error::other("disk gone");
        let err: AppError = io.into();
        assert_eq!(err.kind, AppErrorKind::Internal);
        assert!(err.message.contains("disk gone"));
    }
}
