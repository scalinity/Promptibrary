//! Markdown body/frontmatter split.
//!
//! Spec §3 module contract for `vault::markdown`. We split on the first
//! `---` opening line and the first `---` closing line. Everything between
//! is the YAML frontmatter; everything after is the body.
//!
//! Newlines are normalized to LF on serialization. Both UNIX (LF) and
//! Windows (CRLF) inputs parse correctly.

use crate::error::{AppError, AppErrorKind, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownDocument {
    pub frontmatter_yaml: String,
    pub body: String,
}

pub fn parse_markdown_document(input: &str) -> Result<MarkdownDocument> {
    let normalized = input.replace("\r\n", "\n");
    let mut lines = normalized.split_inclusive('\n');

    // Find the opening `---`.
    let first = match lines.next() {
        Some(l) => l,
        None => {
            return Ok(MarkdownDocument {
                frontmatter_yaml: String::new(),
                body: String::new(),
            });
        }
    };
    if first.trim_end() != "---" {
        // No frontmatter — the whole input is the body.
        return Ok(MarkdownDocument {
            frontmatter_yaml: String::new(),
            body: normalized,
        });
    }

    let mut yaml = String::new();
    let mut found_close = false;
    let mut body = String::new();
    while let Some(line) = lines.next() {
        if line.trim_end() == "---" {
            found_close = true;
            break;
        }
        yaml.push_str(line);
    }
    if !found_close {
        return Err(AppError::new(
            AppErrorKind::PromptMalformed,
            "frontmatter block was opened but never closed",
        ));
    }
    for line in lines {
        body.push_str(line);
    }
    // Trim a single leading newline from the body — many editors put a
    // blank line after the closing `---`.
    if let Some(stripped) = body.strip_prefix('\n') {
        body = stripped.to_string();
    }

    Ok(MarkdownDocument {
        frontmatter_yaml: yaml,
        body,
    })
}

pub fn serialize_markdown_document(doc: &MarkdownDocument) -> String {
    let mut out = String::with_capacity(doc.frontmatter_yaml.len() + doc.body.len() + 16);
    out.push_str("---\n");
    out.push_str(&doc.frontmatter_yaml);
    if !doc.frontmatter_yaml.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("---\n");
    out.push_str(&doc.body);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_body() {
        let input = "---\ntitle: hello\n---\nbody here\n";
        let d = parse_markdown_document(input).unwrap();
        assert_eq!(d.frontmatter_yaml, "title: hello\n");
        assert_eq!(d.body, "body here\n");
    }

    #[test]
    fn round_trip_preserves_payload() {
        let original = "---\ntitle: hello\n---\nbody\n";
        let parsed = parse_markdown_document(original).unwrap();
        let re = serialize_markdown_document(&parsed);
        assert_eq!(re, original);
    }

    #[test]
    fn missing_close_errors() {
        let input = "---\ntitle: hello\nbody...";
        let err = parse_markdown_document(input).unwrap_err();
        assert_eq!(err.kind, AppErrorKind::PromptMalformed);
    }

    #[test]
    fn input_without_frontmatter_is_all_body() {
        let input = "plain markdown\nno frontmatter\n";
        let d = parse_markdown_document(input).unwrap();
        assert!(d.frontmatter_yaml.is_empty());
        assert_eq!(d.body, input);
    }

    #[test]
    fn crlf_normalized_to_lf() {
        let input = "---\r\ntitle: hi\r\n---\r\nbody\r\n";
        let d = parse_markdown_document(input).unwrap();
        assert_eq!(d.body, "body\n");
    }
}
