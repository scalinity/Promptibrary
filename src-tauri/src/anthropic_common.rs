//! Shared helpers for any module that talks to Anthropic's Messages API.
//!
//! Both `extraction::anthropic` (one-shot JSON response) and
//! `assistant::transport` (streaming SSE) parse the rate-limit reset
//! headers the same way. SCA-960 lifts that helper out of the two
//! transport modules so they share one implementation.
//!
//! Future shared helpers belong here too (request-id extraction, common
//! auth header builders, etc.); the module starts intentionally small.

/// Parse the reset-at timestamp from Anthropic's rate-limit headers.
///
/// Header contract (Anthropic Messages API, as of 2026-05):
/// - `anthropic-ratelimit-requests-reset` — RFC3339 absolute timestamp
///   (e.g. `2026-05-19T12:00:00Z`). Preferred when present because the
///   value is wall-clock, immune to clock-skew on the client.
/// - `retry-after` — RFC7231 seconds-until-reset. Fallback when the
///   Anthropic-specific header is absent.
///
/// Other Anthropic rate-limit headers are emitted too
/// (`anthropic-ratelimit-tokens-{remaining,reset}`,
/// `anthropic-ratelimit-input-tokens-*`, etc.). We only care about the
/// "you can retry at X" answer here; the others are dashboarding signals.
///
/// Reference: https://docs.anthropic.com/en/api/rate-limits — keep this
/// pointer current when adjusting parsing. If Anthropic renames the
/// preferred header, the `RateLimited` failure silently degrades to the
/// `retry-after` path, so update both the constant strings below AND
/// the relevant transport tests when the header schema changes.
pub fn parse_rate_limit_reset(
    headers: &reqwest::header::HeaderMap,
) -> Option<chrono::DateTime<chrono::Utc>> {
    if let Some(reset) = headers
        .get("anthropic-ratelimit-requests-reset")
        .and_then(|v| v.to_str().ok())
    {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(reset) {
            return Some(dt.with_timezone(&chrono::Utc));
        }
    }
    if let Some(retry_after) = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
    {
        if let Ok(secs) = retry_after.parse::<i64>() {
            return Some(chrono::Utc::now() + chrono::Duration::seconds(secs));
        }
    }
    None
}
