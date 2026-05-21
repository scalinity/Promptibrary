//! Shared hashing primitives.
//!
//! SCA-600 consolidates the two copies of `hex_sha256` previously living
//! in `commands::prompts` and `vault::scanner`. Use this single helper
//! everywhere a prompt content / serialized-doc digest is needed.
//!
//! Convention: both the writer (post-serialization) and the scanner
//! (raw file contents) feed into `sha256_hex` and prepend `sha256:` at
//! the call site. The convention is documented inline at each call site
//! since the *input* differs (serialized doc vs raw file bytes); the
//! *format* is the same: `sha256:<64 hex chars>`.

use sha2::{Digest, Sha256};

/// Compute the SHA-256 of `s` and return the lowercase hex digest
/// (without any `sha256:` prefix). Callers prepend the prefix.
pub fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    // SCA-928 (B16): single hex-encoding implementation in util::hex.
    crate::util::hex::encode_lower(&h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_has_known_digest() {
        assert_eq!(
            sha256_hex(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn identical_inputs_produce_identical_digests() {
        assert_eq!(sha256_hex("abc"), sha256_hex("abc"));
    }

    #[test]
    fn distinct_inputs_produce_distinct_digests() {
        assert_ne!(sha256_hex("a"), sha256_hex("b"));
    }
}
