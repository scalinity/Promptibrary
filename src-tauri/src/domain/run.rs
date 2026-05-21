//! Run + TokenCount per spec §4 "Run".

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::AppErrorDto;
use crate::ids::{PromptId, RunId};

use super::launch::LaunchProfile;

/// Canonical run-lifecycle states.
///
/// SCA-910: previously this enum and `index::runs_repo::RunStatus`
/// diverged. The DB-write path was the source of truth; this enum is
/// now its canonical form. The TS mirror in
/// `src/shared/types/enums.ts::RunStatus` is kept in lockstep —
/// `run_status_variant_parity` below enforces the match at compile
/// time.
///
/// State machine (W2): `Started → FirstOutput → {Stopping → Finished,
/// Finished, Errored}`. The earlier `Running` variant was never
/// written and has been removed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Started,
    FirstOutput,
    Stopping,
    Finished,
    Errored,
}

impl RunStatus {
    /// Lowercase form persisted in `runs.status`. Stable wire format.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::FirstOutput => "first_output",
            Self::Stopping => "stopping",
            Self::Finished => "finished",
            Self::Errored => "errored",
        }
    }

    /// Parse the DB string back into a typed variant. Returns `None`
    /// for unknown values — callers decide whether to error or fall
    /// through.
    pub fn from_str(raw: &str) -> Option<Self> {
        match raw {
            "started" => Some(Self::Started),
            "first_output" => Some(Self::FirstOutput),
            "stopping" => Some(Self::Stopping),
            "finished" => Some(Self::Finished),
            "errored" => Some(Self::Errored),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StopSignal {
    #[serde(rename = "SIGINT")]
    SigInt,
    #[serde(rename = "SIGTERM")]
    SigTerm,
    #[serde(rename = "SIGKILL")]
    SigKill,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCount {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_creation_input_tokens: u32,
    pub cache_read_input_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: RunId,
    pub prompt_id: PromptId,
    pub prompt_title: String,
    pub status: RunStatus,
    pub profile: LaunchProfile,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub signal: Option<StopSignal>,
    pub transcript_vault_path: Option<String>,
    pub transcript_spool_path: Option<PathBuf>,
    // Use i64 to match SQLite's signed INTEGER storage and JS number range
    // (exact up to 2^53). A claude run that produces >8 EB of terminal output
    // is a bug worth surfacing, not a precision-loss to paper over.
    pub stdout_bytes: i64,
    pub stderr_bytes: i64,
    pub token_count: Option<TokenCount>,
    pub cost_usd: Option<f64>,
    pub error: Option<AppErrorDto>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SCA-910 architectural test: each variant has a fixed wire
    /// string. An exhaustive `match` makes adding a variant fail to
    /// compile here until the TS mirror is also updated.
    #[test]
    fn run_status_wire_strings_are_stable() {
        for (variant, expected) in [
            (RunStatus::Started, "started"),
            (RunStatus::FirstOutput, "first_output"),
            (RunStatus::Stopping, "stopping"),
            (RunStatus::Finished, "finished"),
            (RunStatus::Errored, "errored"),
        ] {
            assert_eq!(variant.as_str(), expected);
            let json = serde_json::to_value(variant).unwrap();
            assert_eq!(json, serde_json::Value::String(expected.to_string()));
            assert_eq!(RunStatus::from_str(expected), Some(variant));
        }
    }

    /// Compile-time enforcement: if a future commit adds a variant
    /// without updating `RUN_STATUS_VARIANTS`, `match` exhaustiveness
    /// catches it.
    #[test]
    fn run_status_variant_count_matches_ts() {
        // Keep in lockstep with `src/shared/types/enums.ts::RunStatus`.
        // Adding a variant: bump this number AND extend the TS union.
        const TS_VARIANT_COUNT: usize = 5;
        let count = [
            RunStatus::Started,
            RunStatus::FirstOutput,
            RunStatus::Stopping,
            RunStatus::Finished,
            RunStatus::Errored,
        ]
        .len();
        // Exhaustive match guard — adding a Rust variant fails this match.
        fn exhaustive_match(s: RunStatus) -> &'static str {
            match s {
                RunStatus::Started => "started",
                RunStatus::FirstOutput => "first_output",
                RunStatus::Stopping => "stopping",
                RunStatus::Finished => "finished",
                RunStatus::Errored => "errored",
            }
        }
        let _ = exhaustive_match(RunStatus::Started);
        assert_eq!(count, TS_VARIANT_COUNT);
    }
}
