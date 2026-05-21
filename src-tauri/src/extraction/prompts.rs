//! Exact LLM prompt text — byte-equal to spec §6 *Exact extraction system prompt*.
//!
//! **DO NOT modify** `EXTRACTION_SYSTEM_PROMPT` without a matching spec patch.
//! The canonical text lives in `docs/spec-snippets/extraction-system-prompt.txt`
//! so the spec and the const stay in lockstep — `include_str!` makes drift
//! detection compile-time. Bumping the text MUST also bump
//! `EXTRACTION_PROMPT_VERSION` so the cache invalidates.

use super::types::{ExtractionInput, SourceChunk, SourceChunkKind};

/// Spec §6 *Exact extraction system prompt*. Loaded at compile time from
/// `docs/spec-snippets/extraction-system-prompt.txt` so a drift between
/// spec and code is a build error, not a runtime surprise.
///
/// Source: `docs/spec-snippets/extraction-system-prompt.txt`.
pub const EXTRACTION_SYSTEM_PROMPT: &str =
    include_str!("../../../docs/spec-snippets/extraction-system-prompt.txt");

/// Cache-busting version bumped whenever `EXTRACTION_SYSTEM_PROMPT` changes.
/// Mixed into the candidate-cache key (see `extraction::cache`) so candidates
/// produced under v1 don't get served when v2 lands.
pub const EXTRACTION_PROMPT_VERSION: u32 = 1;

/// Build the user message per spec §6 *User message template*.
pub fn build_user_payload(input: &ExtractionInput) -> String {
    let mut out = String::new();
    out.push_str("Source metadata:\n");
    out.push_str(&format!("- kind: {}\n", source_kind_label(input)));
    out.push_str(&format!("- url: {}\n", input.url));
    out.push_str(&format!("- title: {}\n", or_null(input.title.as_deref())));
    out.push_str(&format!("- author: {}\n", or_null(input.author.as_deref())));
    out.push_str(&format!(
        "- fetched_at: {}\n",
        fetched_at_label(&input.source)
    ));
    out.push_str(&format!(
        "- extraction_mode: {}\n",
        input.extraction_mode.as_wire()
    ));
    out.push_str(&format!(
        "- max_candidate_count: {}\n",
        input.max_candidate_count
    ));
    out.push_str("\nSource chunks:\n");
    for chunk in &input.chunks {
        out.push_str(&format_chunk_header(chunk));
        out.push('\n');
        out.push_str(&chunk.text);
        if !chunk.text.ends_with('\n') {
            out.push('\n');
        }
    }
    out.push_str("\nExtract Promptibrary launch profile candidates from this source.\n");
    out
}

fn source_kind_label(input: &ExtractionInput) -> &'static str {
    use crate::domain::source::Source;
    match input.source {
        Source::Manual(_) => "manual",
        Source::Youtube(_) => "youtube",
        Source::XTwitter(_) => "x_twitter",
        Source::Article(_) => "article",
    }
}

fn fetched_at_label(source: &crate::domain::source::Source) -> String {
    use crate::domain::source::Source;
    let dt = match source {
        Source::Manual(s) => s.fetched_at,
        Source::Youtube(s) => s.fetched_at,
        Source::XTwitter(s) => s.fetched_at,
        Source::Article(s) => s.fetched_at,
    };
    match dt {
        Some(d) => d.to_rfc3339(),
        None => "null".to_string(),
    }
}

fn or_null(opt: Option<&str>) -> String {
    opt.map(|s| s.to_string()).unwrap_or_else(|| "null".into())
}

fn format_chunk_header(chunk: &SourceChunk) -> String {
    let ts = match chunk.timestamp_seconds {
        Some(t) => format!("{t}"),
        None => "null".to_string(),
    };
    let url = chunk.url.as_deref().unwrap_or("null");
    format!(
        "[chunk {order} | {kind} | timestamp={ts} | url={url}]",
        order = chunk.order,
        kind = chunk_kind_label(chunk.kind),
    )
}

fn chunk_kind_label(kind: SourceChunkKind) -> &'static str {
    match kind {
        SourceChunkKind::Title => "title",
        SourceChunkKind::Metadata => "metadata",
        SourceChunkKind::Transcript => "transcript",
        SourceChunkKind::Post => "post",
        SourceChunkKind::Article => "article",
        SourceChunkKind::Code => "code",
        SourceChunkKind::Quote => "quote",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::source::{ArticleSource, Source};
    use crate::extraction::types::{ExtractionMode, SourceChunk, SourceChunkKind};

    #[test]
    fn system_prompt_present_and_starts_with_sentinel() {
        // `include_str!` already guarantees compile-time byte-equality between
        // the const and the file. This guard catches accidental empty/typo
        // edits to the snippet.
        assert!(EXTRACTION_SYSTEM_PROMPT.starts_with("You are Promptibrary's extraction engine."));
        assert!(EXTRACTION_SYSTEM_PROMPT.contains("Return ONLY valid JSON"));
        assert!(EXTRACTION_SYSTEM_PROMPT.contains("Output must be strict JSON."));
        assert!(
            EXTRACTION_SYSTEM_PROMPT.len() > 3500,
            "system prompt looks truncated: {} bytes",
            EXTRACTION_SYSTEM_PROMPT.len()
        );
    }

    #[test]
    fn prompt_version_starts_at_one() {
        assert_eq!(EXTRACTION_PROMPT_VERSION, 1);
    }

    fn input_with_chunks(chunks: Vec<SourceChunk>) -> ExtractionInput {
        ExtractionInput {
            source: Source::Article(ArticleSource {
                origin_url: Some("https://example.com/x".into()),
                title: Some("Title".into()),
                author: Some("Alice".into()),
                fetched_at: None,
                content_hash: None,
                site_name: None,
                byline: None,
                published_at: None,
            }),
            title: Some("Title".into()),
            author: Some("Alice".into()),
            url: "https://example.com/x".into(),
            text: "ignored".into(),
            chunks,
            max_candidate_count: 4,
            extraction_mode: ExtractionMode::Standard,
            model_id: "claude-sonnet-4-6".into(),
        }
    }

    #[test]
    fn user_payload_emits_metadata_and_chunks() {
        let payload = build_user_payload(&input_with_chunks(vec![SourceChunk {
            kind: SourceChunkKind::Article,
            order: 0,
            text: "Hello world".into(),
            url: None,
            timestamp_seconds: None,
        }]));
        assert!(payload.contains("kind: article"));
        assert!(payload.contains("url: https://example.com/x"));
        assert!(payload.contains("title: Title"));
        assert!(payload.contains("author: Alice"));
        assert!(payload.contains("extraction_mode: standard"));
        assert!(payload.contains("max_candidate_count: 4"));
        assert!(payload.contains("[chunk 0 | article | timestamp=null | url=null]"));
        assert!(payload.contains("Hello world"));
        assert!(payload
            .contains("Extract Promptibrary launch profile candidates from this source."));
    }

    #[test]
    fn user_payload_renders_null_for_missing_optionals() {
        let mut input = input_with_chunks(vec![]);
        input.title = None;
        input.author = None;
        let payload = build_user_payload(&input);
        assert!(payload.contains("title: null"));
        assert!(payload.contains("author: null"));
    }

    #[test]
    fn chunk_header_includes_timestamp_and_url() {
        let chunk = SourceChunk {
            kind: SourceChunkKind::Transcript,
            order: 3,
            text: "transcript line".into(),
            url: Some("https://youtu.be/x?t=42".into()),
            timestamp_seconds: Some(42.0),
        };
        let header = format_chunk_header(&chunk);
        assert_eq!(
            header,
            "[chunk 3 | transcript | timestamp=42 | url=https://youtu.be/x?t=42]"
        );
    }
}
