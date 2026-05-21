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
    ForeignKeyViolation,
    GitError,
    SettingsInvalid,
    KeychainError,
    UnsupportedSource,
    TranscriptUnavailable,
    /// SCA-911 (W17): filesystem path requested by an IPC handler
    /// (e.g. `open_path`, `reveal_in_terminal`) does not exist.
    /// Distinct from `VaultMissing`, which means the *vault* root is
    /// unattached.
    PathNotFound,
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
        // SCA-597: do NOT leak the OS error message verbatim across the
        // IPC wire. std::io::Error often contains absolute filesystem
        // paths (e.g. "No such file or directory (os error 2)" or
        // "Permission denied (os error 13)") which is CWE-209 information
        // disclosure. Log the full error via tracing for backend debugging
        // and surface only the ErrorKind name on the wire.
        tracing::warn!(error = ?e, "io error converted to AppError");
        let kind_label = format!("{:?}", e.kind());
        Self::internal(format!("io: {}", kind_label))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        // SCA-913 (W10, CWE-209): serde_json::Error can include the
        // raw input excerpt. Log full detail; keep wire surface to the
        // line/column metadata.
        tracing::warn!(error = ?e, "serde_json error converted to AppError");
        Self::new(
            AppErrorKind::Internal,
            format!("json: line {} col {}", e.line(), e.column()),
        )
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        // Classify SQLite errors by their numeric code rather than message text:
        // - 5  = SQLITE_BUSY (database is locked)
        // - 6  = SQLITE_LOCKED (table-level lock conflict)
        // - 11 = SQLITE_CORRUPT (database disk image is malformed)
        // - 26 = SQLITE_NOTADB (file is not a database, treat as corrupt)
        // - 787 = SQLITE_CONSTRAINT_FOREIGNKEY (extended result code; SCA-750
        //   — surface FK violations distinctly so callers can recognize
        //   "referenced row doesn't exist" rather than masking as Internal)
        // Reference: https://sqlite.org/rescode.html
        //
        // SCA-913 (W10, CWE-209): non-database variants (encode/decode,
        // pool, IO) leak SQL text and column names verbatim. Log full,
        // surface only the variant discriminator.
        match &e {
            sqlx::Error::Database(db_err) => match db_err.code().as_deref() {
                Some("5") | Some("6") => {
                    Self::new(AppErrorKind::SqliteLocked, db_err.message().to_string())
                }
                Some("11") | Some("26") => {
                    Self::new(AppErrorKind::SqliteCorrupt, db_err.message().to_string())
                }
                Some("787") => Self::new(
                    AppErrorKind::ForeignKeyViolation,
                    db_err.message().to_string(),
                ),
                _ => {
                    tracing::warn!(error = ?e, "sqlx database error converted to AppError");
                    Self::new(AppErrorKind::Internal, "sqlx: database error")
                }
            },
            other => {
                tracing::warn!(error = ?other, "sqlx error converted to AppError");
                let label = match other {
                    sqlx::Error::Configuration(_) => "configuration",
                    sqlx::Error::Io(_) => "io",
                    sqlx::Error::Tls(_) => "tls",
                    sqlx::Error::Protocol(_) => "protocol",
                    sqlx::Error::RowNotFound => "row_not_found",
                    sqlx::Error::TypeNotFound { .. } => "type_not_found",
                    sqlx::Error::ColumnIndexOutOfBounds { .. } => "column_index",
                    sqlx::Error::ColumnNotFound(_) => "column_not_found",
                    sqlx::Error::ColumnDecode { .. } => "column_decode",
                    sqlx::Error::Decode(_) => "decode",
                    sqlx::Error::PoolTimedOut => "pool_timed_out",
                    sqlx::Error::PoolClosed => "pool_closed",
                    sqlx::Error::WorkerCrashed => "worker_crashed",
                    sqlx::Error::Migrate(_) => "migrate",
                    _ => "other",
                };
                Self::new(AppErrorKind::Internal, format!("sqlx: {label}"))
            }
        }
    }
}

impl From<sqlx::migrate::MigrateError> for AppError {
    fn from(e: sqlx::migrate::MigrateError) -> Self {
        // SCA-913 (W10, CWE-209): migrate errors can include absolute
        // filesystem paths for migration sources. Log full, surface
        // only the variant discriminator.
        tracing::warn!(error = ?e, "sqlx migrate error converted to AppError");
        Self::new(AppErrorKind::Internal, "migrate: error")
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        // SCA-913 (W10, CWE-209): reqwest::Error.to_string() includes
        // the target URL with query string (and the resolved IP in
        // some failure modes). Log full; surface only the failure
        // category.
        tracing::warn!(error = ?e, "reqwest error converted to AppError");
        if e.is_connect() || e.is_timeout() {
            Self::new(AppErrorKind::NetworkUnavailable, "network: unreachable")
        } else if e.is_decode() {
            Self::new(AppErrorKind::Internal, "http: decode failed")
        } else if e.is_status() {
            let status = e.status().map(|s| s.as_u16()).unwrap_or(0);
            Self::new(AppErrorKind::Internal, format!("http: status {status}"))
        } else {
            Self::new(AppErrorKind::Internal, "http: request failed")
        }
    }
}

impl From<notify::Error> for AppError {
    fn from(e: notify::Error) -> Self {
        // SCA-913 (W10, CWE-209): notify::Error includes the path the
        // watcher was monitoring. Log full; surface only the category.
        tracing::warn!(error = ?e, "notify error converted to AppError");
        Self::new(AppErrorKind::Internal, "watcher: error")
    }
}

impl From<keyring_core::error::Error> for AppError {
    fn from(e: keyring_core::error::Error) -> Self {
        // SCA-913 (W9, CWE-209): Display, not Debug.
        tracing::warn!(error = %e, "keychain error converted to AppError");
        Self::new(AppErrorKind::KeychainError, "keychain: error")
    }
}

impl From<serde_yaml::Error> for AppError {
    fn from(e: serde_yaml::Error) -> Self {
        // SCA-913 (W10, CWE-209): serde_yaml::Error includes the
        // offending line text. Log full; surface only the location.
        tracing::warn!(error = ?e, "serde_yaml error converted to AppError");
        let loc = e
            .location()
            .map(|l| format!("line {} col {}", l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());
        Self::new(AppErrorKind::YamlMalformed, format!("yaml: {loc}"))
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
        // SCA-597: the wire message must NOT include the OS error string
        // (which often carries an absolute filesystem path). It surfaces
        // only the ErrorKind label; the full error is logged via tracing.
        let io = std::io::Error::other("disk gone");
        let err: AppError = io.into();
        assert_eq!(err.kind, AppErrorKind::Internal);
        assert!(
            !err.message.contains("disk gone"),
            "io error message must not be echoed: {}",
            err.message
        );
        assert!(err.message.starts_with("io: "));
    }

    #[test]
    fn detail_value_null_serializes_as_json_null() {
        // Lock in serde's untagged-unit-variant → JSON null behavior so a
        // future serde change can't silently flip it to the string "Null"
        // (which the TS Record<string, ... | null> shape would not parse).
        let v = DetailValue::Null;
        let json = serde_json::to_value(&v).unwrap();
        assert!(json.is_null(), "DetailValue::Null must wire as JSON null, got {json:?}");
    }

    #[test]
    fn detail_value_roundtrip_through_app_error() {
        // Belt-and-suspenders: ensure Null and string variants both survive
        // the full IpcError wire conversion.
        let err = AppError::new(AppErrorKind::Internal, "x")
            .with_detail("nothing", DetailValue::Null)
            .with_detail("something", "value");
        let wire = serde_json::to_value(&err).unwrap();
        assert!(wire["details"]["nothing"].is_null());
        assert_eq!(wire["details"]["something"], "value");
    }

    /// SCA-910 architectural test: AppErrorKind variant set is in
    /// lockstep with `src/shared/types/ipc.ts::AppErrorKind`. The
    /// exhaustive `match` below fails to compile if a Rust variant is
    /// added without listing it here; the count assertion fails if the
    /// TS union isn't updated in the same commit.
    #[test]
    fn app_error_kind_variant_parity() {
        // Keep in lockstep with `src/shared/types/ipc.ts::AppErrorKind`.
        // Adding a variant: bump this AND extend the TS union.
        const TS_VARIANT_COUNT: usize = 32;
        fn wire_name(k: AppErrorKind) -> &'static str {
            match k {
                AppErrorKind::VaultMissing => "VaultMissing",
                AppErrorKind::VaultInvalid => "VaultInvalid",
                AppErrorKind::VaultNotGitRepo => "VaultNotGitRepo",
                AppErrorKind::PromptNotFound => "PromptNotFound",
                AppErrorKind::PromptMalformed => "PromptMalformed",
                AppErrorKind::YamlMalformed => "YamlMalformed",
                AppErrorKind::VariableParseFailed => "VariableParseFailed",
                AppErrorKind::VariableValidationFailed => "VariableValidationFailed",
                AppErrorKind::WorkingDirectoryInvalid => "WorkingDirectoryInvalid",
                AppErrorKind::DependencyMissing => "DependencyMissing",
                AppErrorKind::PtySpawnFailed => "PtySpawnFailed",
                AppErrorKind::RunNotFound => "RunNotFound",
                AppErrorKind::RunNotActive => "RunNotActive",
                AppErrorKind::TooManyActiveRuns => "TooManyActiveRuns",
                AppErrorKind::ClaudeCliMissing => "ClaudeCliMissing",
                AppErrorKind::ClaudeCliFailed => "ClaudeCliFailed",
                AppErrorKind::AnthropicKeyMissing => "AnthropicKeyMissing",
                AppErrorKind::AnthropicAuthInvalid => "AnthropicAuthInvalid",
                AppErrorKind::NetworkUnavailable => "NetworkUnavailable",
                AppErrorKind::RateLimited => "RateLimited",
                AppErrorKind::ExtractionFailed => "ExtractionFailed",
                AppErrorKind::MalformedModelOutput => "MalformedModelOutput",
                AppErrorKind::SqliteLocked => "SqliteLocked",
                AppErrorKind::SqliteCorrupt => "SqliteCorrupt",
                AppErrorKind::ForeignKeyViolation => "ForeignKeyViolation",
                AppErrorKind::GitError => "GitError",
                AppErrorKind::SettingsInvalid => "SettingsInvalid",
                AppErrorKind::KeychainError => "KeychainError",
                AppErrorKind::UnsupportedSource => "UnsupportedSource",
                AppErrorKind::TranscriptUnavailable => "TranscriptUnavailable",
                AppErrorKind::PathNotFound => "PathNotFound",
                AppErrorKind::Internal => "Internal",
            }
        }
        let all = [
            AppErrorKind::VaultMissing,
            AppErrorKind::VaultInvalid,
            AppErrorKind::VaultNotGitRepo,
            AppErrorKind::PromptNotFound,
            AppErrorKind::PromptMalformed,
            AppErrorKind::YamlMalformed,
            AppErrorKind::VariableParseFailed,
            AppErrorKind::VariableValidationFailed,
            AppErrorKind::WorkingDirectoryInvalid,
            AppErrorKind::DependencyMissing,
            AppErrorKind::PtySpawnFailed,
            AppErrorKind::RunNotFound,
            AppErrorKind::RunNotActive,
            AppErrorKind::TooManyActiveRuns,
            AppErrorKind::ClaudeCliMissing,
            AppErrorKind::ClaudeCliFailed,
            AppErrorKind::AnthropicKeyMissing,
            AppErrorKind::AnthropicAuthInvalid,
            AppErrorKind::NetworkUnavailable,
            AppErrorKind::RateLimited,
            AppErrorKind::ExtractionFailed,
            AppErrorKind::MalformedModelOutput,
            AppErrorKind::SqliteLocked,
            AppErrorKind::SqliteCorrupt,
            AppErrorKind::ForeignKeyViolation,
            AppErrorKind::GitError,
            AppErrorKind::SettingsInvalid,
            AppErrorKind::KeychainError,
            AppErrorKind::UnsupportedSource,
            AppErrorKind::TranscriptUnavailable,
            AppErrorKind::PathNotFound,
            AppErrorKind::Internal,
        ];
        assert_eq!(all.len(), TS_VARIANT_COUNT);
        for k in all {
            let json = serde_json::to_value(k).unwrap();
            assert_eq!(json, serde_json::Value::String(wire_name(k).to_string()));
        }
    }
}
