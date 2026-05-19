//! Typed JSON helpers.

use serde::{de::DeserializeOwned, Serialize};

use crate::error::{AppError, AppErrorKind, Result};

pub fn to_string<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(AppError::from)
}

pub fn from_str<T: DeserializeOwned>(s: &str) -> Result<T> {
    serde_json::from_str(s).map_err(|e| {
        AppError::new(
            AppErrorKind::Internal,
            format!("json parse failed: {}", e),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S {
        name: String,
        count: u32,
    }

    #[test]
    fn round_trip_struct() {
        let s = S {
            name: "x".into(),
            count: 3,
        };
        let raw = to_string(&s).unwrap();
        let back: S = from_str(&raw).unwrap();
        assert_eq!(s, back);
    }
}
