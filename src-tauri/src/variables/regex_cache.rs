//! Process-wide cache of compiled regex patterns for `text` variables.
//!
//! Spec §5 lets prompt authors declare a `pattern` constraint that the
//! validator enforces against user input. Compiling a regex on every
//! validate call wastes work (most patterns are static per-prompt) and
//! exposes the app to ReDoS-via-pattern-compilation (CWE-1333).
//!
//! This module pre-compiles patterns at parse time (`ensure_compiled`) and
//! exposes a fast lookup (`is_match`) for the validator. The cache lives
//! for the process lifetime — the working-set is bounded by the number of
//! distinct patterns in the user's vault, which is small.

use std::collections::HashMap;
use std::sync::RwLock;

use once_cell::sync::Lazy;
use regex::Regex;

static CACHE: Lazy<RwLock<HashMap<String, Regex>>> = Lazy::new(|| RwLock::new(HashMap::new()));

/// Compile `pat` if not already cached. Returns the compile error so
/// the parser can surface `InvalidConstraint` at parse time.
pub fn ensure_compiled(pat: &str) -> Result<(), regex::Error> {
    if let Ok(guard) = CACHE.read() {
        if guard.contains_key(pat) {
            return Ok(());
        }
    }
    let re = Regex::new(pat)?;
    if let Ok(mut guard) = CACHE.write() {
        guard.insert(pat.to_string(), re);
    }
    Ok(())
}

/// Match `value` against `pat`, compiling on demand if the parser didn't
/// pre-warm the cache (e.g. patterns assembled directly via frontmatter).
/// Returns `false` for any compile error — the validator treats both
/// "doesn't match" and "couldn't compile" as `PatternMismatch`.
pub fn is_match(pat: &str, value: &str) -> bool {
    if let Ok(guard) = CACHE.read() {
        if let Some(re) = guard.get(pat) {
            return re.is_match(value);
        }
    }
    let re = match Regex::new(pat) {
        Ok(r) => r,
        Err(_) => return false,
    };
    let m = re.is_match(value);
    if let Ok(mut guard) = CACHE.write() {
        guard.insert(pat.to_string(), re);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn good_pattern_compiles_and_matches() {
        ensure_compiled(r"^[a-z]+$").unwrap();
        assert!(is_match(r"^[a-z]+$", "abc"));
        assert!(!is_match(r"^[a-z]+$", "ABC1"));
    }

    #[test]
    fn bad_pattern_errors_at_ensure() {
        let err = ensure_compiled(r"[unclosed").err();
        assert!(err.is_some(), "expected regex compile error");
    }

    #[test]
    fn repeated_ensure_uses_cache() {
        // Just verifies no panic on repeated insert.
        ensure_compiled(r"^x$").unwrap();
        ensure_compiled(r"^x$").unwrap();
    }
}
