//! Strong ID newtype wrappers and ULID generation per spec §4.
//!
//! These match the TypeScript branded ids exactly — `#[serde(transparent)]`
//! makes them serialize/deserialize as bare strings on the wire while keeping
//! Rust-side type safety.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PromptId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceId(pub String);

impl PromptId {
    pub fn new() -> Self {
        Self(new_ulid())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl RunId {
    pub fn new() -> Self {
        Self(new_ulid())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl SourceId {
    pub fn new() -> Self {
        Self(new_ulid())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Deliberately no `Default` impls: `..Default::default()` on a struct that
// contains a PromptId / RunId / SourceId would silently allocate a fresh
// ULID via the CSPRNG, which is surprising behavior for a trait that is
// conventionally zero-cost. Callers must invoke `::new()` explicitly to
// signal that ID generation is happening.

/// Generates a fresh Crockford-Base32 ULID. 26 ASCII chars, uppercase.
pub fn new_ulid() -> String {
    ulid::Ulid::new().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ulid_is_26_chars_uppercase() {
        let id = new_ulid();
        assert_eq!(id.len(), 26);
        assert!(id.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
    }

    #[test]
    fn distinct_ulids_are_unique() {
        let mut ids = std::collections::HashSet::new();
        for _ in 0..1000 {
            assert!(ids.insert(new_ulid()), "duplicate ULID");
        }
    }

    #[test]
    fn prompt_id_serializes_transparent() {
        let id = PromptId("01JZ7M1K6M8D4E9SZ7P1Q9KT4A".into());
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"01JZ7M1K6M8D4E9SZ7P1Q9KT4A\"");
        let parsed: PromptId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn ids_are_not_mixable() {
        // Compile-time check: assigning across types would not compile.
        let p = PromptId::new();
        let r = RunId::new();
        assert_ne!(p.as_str(), r.as_str());
    }
}
