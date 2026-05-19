//! Time helpers.
//!
//! `now_iso8601` is the standard timestamp source used in transcript frontmatter,
//! `Run.started_at`, and Prompt `created_at`/`updated_at` fields.

use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};

pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

pub fn to_iso8601(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn now_iso8601() -> String {
    to_iso8601(now_utc())
}

/// SCA-747: parse a timestamp string that the DB might have stored in
/// either RFC3339 (`2026-05-19T01:00:00Z`) or SQLite-default
/// (`2026-05-19 01:00:00`, no tz) form.
///
/// Pre-fix call sites used `DateTime::parse_from_rfc3339(&s).ok()` and
/// silently returned None when the row was stored with the SQLite
/// default format. That made recency_boost evaluate to 0 for the row
/// and the bug was invisible until a user asked "why did my recently-
/// used prompt drop in rank."
///
/// We try RFC3339 first (the canonical form Promptibrary writes via
/// `to_iso8601`), then fall back to the SQLite default. If both fail
/// we log a warning with the offending string and return None — the
/// log entry is what makes the previously-silent failure debuggable.
pub fn parse_db_timestamp(s: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    // SQLite default with optional fractional seconds.
    for fmt in &[
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
    ] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
        }
    }
    tracing::warn!(timestamp = %s, "parse_db_timestamp failed — unrecognized format");
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rfc3339_with_z() {
        let dt = parse_db_timestamp("2026-05-19T01:00:00Z").unwrap();
        // Round-trip through UTC RFC3339 — chrono normalizes "Z" to
        // "+00:00" so we compare against that form.
        assert_eq!(dt.to_rfc3339(), "2026-05-19T01:00:00+00:00");
    }

    #[test]
    fn parses_sqlite_default_format() {
        let dt = parse_db_timestamp("2026-05-19 01:00:00").unwrap();
        assert_eq!(dt.to_rfc3339(), "2026-05-19T01:00:00+00:00");
    }

    #[test]
    fn parses_sqlite_with_fractional_seconds() {
        assert!(parse_db_timestamp("2026-05-19 01:00:00.123").is_some());
    }

    #[test]
    fn returns_none_on_garbage() {
        assert!(parse_db_timestamp("not-a-date").is_none());
    }

    /// SCA-747 invariant: a row written via to_iso8601 must round-trip
    /// cleanly back through parse_db_timestamp.
    #[test]
    fn round_trip_through_to_iso8601() {
        let now = now_utc();
        let s = to_iso8601(now);
        let parsed = parse_db_timestamp(&s).unwrap();
        // Compare unix seconds — to_iso8601 truncates fractional seconds.
        assert_eq!(parsed.timestamp(), now.timestamp());
    }
}
