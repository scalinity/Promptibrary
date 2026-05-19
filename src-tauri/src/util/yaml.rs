//! YAML helpers shared by frontmatter parsing.
//!
//! Spec §3 contract for `util::yaml`: typed serde wrappers that surface
//! friendly error messages. Anything that goes through `from_str_friendly`
//! produces a `YamlMalformed` error with a human-readable summary.

use serde::{de::DeserializeOwned, Serialize};

use crate::error::{AppError, AppErrorKind, Result};

pub fn from_str_friendly<T: DeserializeOwned>(s: &str) -> Result<T> {
    serde_yaml::from_str::<T>(s).map_err(|e| {
        AppError::new(
            AppErrorKind::YamlMalformed,
            format!("yaml parse failed: {}", e),
        )
        .with_detail("source_excerpt", excerpt(s, 200))
    })
}

pub fn to_string<T: Serialize>(value: &T) -> Result<String> {
    serde_yaml::to_string(value).map_err(AppError::from)
}

fn excerpt(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    // SCA-609: walk char boundaries so a multi-byte char crossing the
    // `max` byte index doesn't panic with "byte index is not a char
    // boundary". Take the largest char-boundary slice <= max bytes.
    let cut = s
        .char_indices()
        .take_while(|(i, _)| *i <= max)
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0);
    format!("{}…", &s[..cut])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Sample {
        name: String,
        count: u32,
    }

    #[test]
    fn parses_valid_yaml() {
        let s: Sample = from_str_friendly("name: hello\ncount: 7\n").unwrap();
        assert_eq!(
            s,
            Sample {
                name: "hello".into(),
                count: 7
            }
        );
    }

    #[test]
    fn malformed_yaml_returns_yaml_malformed_kind() {
        let err = from_str_friendly::<Sample>("not\nyaml:: @ malformed").unwrap_err();
        assert_eq!(err.kind, AppErrorKind::YamlMalformed);
    }

    #[test]
    fn type_mismatch_returns_yaml_malformed_kind() {
        let err = from_str_friendly::<Sample>("name: hi\ncount: not-a-number\n").unwrap_err();
        assert_eq!(err.kind, AppErrorKind::YamlMalformed);
    }

    #[test]
    fn excerpt_handles_multibyte_at_boundary() {
        // SCA-609 regression: emoji crossing the cut boundary would panic
        // with "byte index N is not a char boundary".
        let s: String = "x".repeat(198) + "🚀rest";
        let err = from_str_friendly::<Sample>(&s).unwrap_err();
        assert_eq!(err.kind, AppErrorKind::YamlMalformed);
    }
}
