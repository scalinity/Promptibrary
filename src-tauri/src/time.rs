//! Time helpers.
//!
//! `now_iso8601` is the standard timestamp source used in transcript frontmatter,
//! `Run.started_at`, and Prompt `created_at`/`updated_at` fields.

use chrono::{DateTime, SecondsFormat, Utc};

pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

pub fn to_iso8601(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn now_iso8601() -> String {
    to_iso8601(now_utc())
}
