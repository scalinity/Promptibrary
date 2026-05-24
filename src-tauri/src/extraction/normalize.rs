//! Normalize `FetchedSourceContent` into an `ExtractionInput` per spec §6.
//!
//! Steps:
//! 1. Collapse intra-line whitespace runs (preserve code-fence content
//!    verbatim, preserve paragraph breaks).
//! 2. Cap text at the user-configured `source_cap_bytes` (default 60k
//!    standard, 160k deep — both editable via Settings since SCA-906).
//!    SCA-916 (W3): the cap is measured in BYTES, not characters. Spec
//!    §6 originally said "chars"; the implementation has always used
//!    `String::len` (byte length). On byte-heavy sources (CJK, emoji)
//!    the effective character count is lower than the byte budget —
//!    intentional, because the cap exists to protect the model's
//!    token budget, not to give predictable character counts.
//! 3. If over cap, summarize structurally: preserve titles/headings,
//!    numbered step lists (workflows), fenced code, lines that look like
//!    constraints/warnings; drop low-density prose first.

use super::types::{ExtractionInput, ExtractionMode, FetchedSourceContent};

pub fn normalize_for_extraction(
    content: FetchedSourceContent,
    mode: ExtractionMode,
    cap_bytes: usize,
    model_id: String,
) -> ExtractionInput {
    let normalized = normalize_text(&content.text);
    let capped = if normalized.len() > cap_bytes {
        summarize_structurally(&normalized, cap_bytes)
    } else {
        normalized
    };

    let max_candidate_count = match mode {
        ExtractionMode::Standard => 4,
        ExtractionMode::Deep => 8,
    };

    ExtractionInput {
        source: content.source,
        title: content.title,
        author: content.author,
        url: content.canonical_url,
        text: capped,
        chunks: content.chunks,
        images: content.images,
        max_candidate_count,
        extraction_mode: mode,
        model_id,
    }
}

/// Collapse whitespace runs while preserving code fences and paragraph
/// breaks (two-or-more newlines collapse to exactly two).
pub fn normalize_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_fence = false;
    let mut blank_run = 0usize;

    for raw_line in input.split('\n') {
        let line = raw_line.trim_end_matches('\r');
        let trimmed_trailing = line.trim_end();

        let starts_fence = trimmed_trailing.trim_start().starts_with("```");
        if starts_fence {
            // Toggle fence state and emit the fence line verbatim.
            if blank_run > 0 {
                out.push_str("\n\n");
                blank_run = 0;
            } else if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(trimmed_trailing);
            in_fence = !in_fence;
            continue;
        }

        if in_fence {
            // Inside a fence we preserve content verbatim (only strip trailing CR).
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
            continue;
        }

        if trimmed_trailing.is_empty() {
            blank_run += 1;
            continue;
        }

        // Collapse intra-line whitespace runs.
        let collapsed: String = collapse_intra_line(trimmed_trailing);
        if blank_run > 0 {
            // Two newlines for paragraph break, regardless of how many blanks.
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            blank_run = 0;
        } else if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&collapsed);
    }

    out
}

fn collapse_intra_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut prev_ws = false;
    for c in line.chars() {
        if c.is_whitespace() {
            if !prev_ws {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    out
}

/// Score-and-trim summarization: walks the text paragraph-by-paragraph,
/// keeps paragraphs that look structural (headings, fenced code, numbered
/// step lists, warning/note lines), drops low-density prose until we fit
/// under `cap_bytes`.
pub fn summarize_structurally(text: &str, cap_bytes: usize) -> String {
    if text.len() <= cap_bytes {
        return text.to_string();
    }

    let paragraphs: Vec<Paragraph<'_>> = collect_paragraphs(text);
    // Sort by drop priority — keep highest priority paragraphs first.
    let mut priorities: Vec<(usize, u8)> = paragraphs
        .iter()
        .enumerate()
        .map(|(i, p)| (i, paragraph_priority(p)))
        .collect();

    // Greedy: start with empty, add in priority order until cap.
    priorities.sort_by(|a, b| b.1.cmp(&a.1));
    let mut kept = vec![false; paragraphs.len()];
    let mut size: usize = 0;
    for (idx, _prio) in priorities {
        let p = &paragraphs[idx];
        let added = p.text.len() + 2; // paragraph separator
        if size + added <= cap_bytes {
            kept[idx] = true;
            size += added;
        }
    }

    let mut out = String::with_capacity(size);
    for (i, p) in paragraphs.iter().enumerate() {
        if kept[i] {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(p.text);
        }
    }
    out
}

struct Paragraph<'a> {
    text: &'a str,
}

fn collect_paragraphs(text: &str) -> Vec<Paragraph<'_>> {
    // Split on blank lines while preserving fenced code blocks intact (a
    // fence's interior may contain blank lines — those must NOT split).
    let mut out: Vec<Paragraph<'_>> = Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();
    let mut in_fence = false;
    let mut fence_start_segment: Option<usize> = None;
    let mut line_start = 0usize;

    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            let line = &text[line_start..i];
            if line.trim_start().starts_with("```") {
                if !in_fence {
                    // Remember where the fence opened so we can recover
                    // gracefully if EOF arrives without a closing fence
                    // (SCA-717).
                    fence_start_segment = Some(start);
                } else {
                    fence_start_segment = None;
                }
                in_fence = !in_fence;
            } else if !in_fence && line.trim().is_empty() {
                let para = text[start..i].trim();
                if !para.is_empty() {
                    out.push(Paragraph { text: para });
                }
                start = i + 1;
            }
            line_start = i + 1;
        }
        i += 1;
    }

    if in_fence {
        // EOF inside an unclosed fence (SCA-717). Re-split the tail
        // from the open fence onward as normal blank-line-separated
        // paragraphs so the priority scorer doesn't see one giant
        // low-priority prose blob.
        let unclosed_start = fence_start_segment.unwrap_or(start);
        for para in text[unclosed_start..].split("\n\n") {
            let trimmed = para.trim();
            if !trimmed.is_empty() {
                out.push(Paragraph { text: trimmed });
            }
        }
    } else {
        // Trailing paragraph.
        let tail = text[start..].trim();
        if !tail.is_empty() {
            out.push(Paragraph { text: tail });
        }
    }
    out
}

/// Higher = keep first. Per spec §6 *Normalization before LLM*:
/// preserve title/headings, workflows, commands/code, constraints/warnings.
fn paragraph_priority(p: &Paragraph<'_>) -> u8 {
    let text = p.text;
    let first_line = text.lines().next().unwrap_or("").trim_start();

    if first_line.starts_with('#') {
        return 9; // Headings
    }
    if first_line.starts_with("```") {
        return 8; // Fenced code blocks
    }
    if looks_like_step_list(text) {
        return 7; // Workflows / numbered step lists
    }
    if looks_like_warning(first_line) {
        return 6; // Notes / warnings / constraints
    }
    if first_line.starts_with('-') || first_line.starts_with('*') {
        return 4; // Bulleted lists — moderately structural
    }
    // Default: low-density prose. Boost very short paragraphs slightly
    // since they're more likely to be section intros / pull quotes than
    // filler walls of text.
    if text.len() < 240 {
        3
    } else {
        2
    }
}

fn looks_like_step_list(text: &str) -> bool {
    let mut numbered_lines = 0;
    let mut total_lines = 0;
    for line in text.lines() {
        total_lines += 1;
        let t = line.trim_start();
        let mut chars = t.chars();
        let first = chars.next().unwrap_or(' ');
        if first.is_ascii_digit() {
            // "1." / "1)" / "01."
            let rest: String = chars.take_while(|c| c.is_ascii_digit()).collect();
            let after = t
                .chars()
                .nth(1 + rest.len())
                .map(|c| c == '.' || c == ')')
                .unwrap_or(false);
            if after {
                numbered_lines += 1;
            }
        }
    }
    total_lines >= 2 && numbered_lines * 2 >= total_lines
}

fn looks_like_warning(first_line: &str) -> bool {
    let lower = first_line.to_ascii_lowercase();
    const MARKERS: &[&str] = &[
        "warning:",
        "caution:",
        "important:",
        "note:",
        "deprecated",
        "do not",
        "must not",
        "never ",
        "always ",
        "required",
    ];
    MARKERS.iter().any(|m| lower.starts_with(m) || lower.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::source::{ArticleSource, Source};
    use chrono::Utc;

    fn fetched(text: &str) -> FetchedSourceContent {
        FetchedSourceContent {
            source: Source::Article(ArticleSource {
                origin_url: None,
                title: None,
                author: None,
                fetched_at: None,
                content_hash: None,
                site_name: None,
                byline: None,
                published_at: None,
            }),
            canonical_url: "https://example.com/x".into(),
            fetched_at: Utc::now(),
            title: None,
            author: None,
            text: text.into(),
            chunks: vec![],
            images: vec![],
            raw_metadata: Default::default(),
            content_hash: "deadbeef".into(),
            cached: false,
        }
    }

    #[test]
    fn small_text_untouched_under_cap() {
        let input = fetched("Hello world.\n\nSecond paragraph.");
        let out = normalize_for_extraction(
            input,
            ExtractionMode::Standard,
            60_000,
            "claude-sonnet-4-6".to_string(),
        );
        assert_eq!(out.text, "Hello world.\n\nSecond paragraph.");
        assert_eq!(out.max_candidate_count, 4);
        assert_eq!(out.extraction_mode, ExtractionMode::Standard);
    }

    #[test]
    fn collapses_intra_line_whitespace() {
        let normalized = normalize_text("Hello    world\t\tagain");
        assert_eq!(normalized, "Hello world again");
    }

    #[test]
    fn collapses_blank_runs_to_one_break() {
        let normalized = normalize_text("a\n\n\n\nb\n\n\nc");
        assert_eq!(normalized, "a\n\nb\n\nc");
    }

    #[test]
    fn preserves_code_fence_content_verbatim() {
        let input = "Intro.\n\n```rust\nfn foo() {\n    bar();    baz();\n}\n```\n\nOutro.";
        let normalized = normalize_text(input);
        // Inside the fence, the double-space must survive.
        assert!(normalized.contains("    bar();    baz();"));
        // Fence delimiters intact.
        assert!(normalized.contains("```rust"));
        assert!(normalized.contains("```\n\nOutro"));
    }

    #[test]
    fn deep_mode_keeps_higher_cap() {
        let big = "x".repeat(120_000);
        let out = normalize_for_extraction(
            fetched(&big),
            ExtractionMode::Deep,
            160_000,
            "claude-opus-4-7".to_string(),
        );
        // Single paragraph of "x" * 120k fits in deep cap (160k) but
        // not in standard cap (60k).
        assert!(out.text.len() <= 160_000);
        assert!(out.text.len() >= 100_000);
    }

    #[test]
    fn unterminated_fence_at_eof_recovers_into_paragraphs() {
        // SCA-717: a code fence at EOF with no closing fence used to
        // leave the entire tail as one giant low-priority paragraph.
        // After the fix, the post-fence content is split on blank lines
        // and surfaces as individual paragraphs.
        let mut s = String::new();
        s.push_str("# Heading\n\n");
        s.push_str("```rust\nfn a() {}\n\n");
        // No closing ``` — simulates a truncated/malformed source.
        s.push_str("Second paragraph after the broken fence.\n\n");
        s.push_str("Third paragraph.");
        let paras = collect_paragraphs(&s);
        // Heading + at least one of the recovered tail paragraphs.
        assert!(paras.iter().any(|p| p.text.contains("Heading")));
        assert!(
            paras.iter().any(|p| p.text.contains("Second paragraph after the broken fence.")),
            "tail after unclosed fence must split on blank lines"
        );
        assert!(paras.iter().any(|p| p.text.contains("Third paragraph")));
    }

    #[test]
    fn structural_summarize_keeps_headings_and_code_under_cap() {
        let mut s = String::new();
        s.push_str("# Title\n\n");
        s.push_str("```sh\nrun-me\n```\n\n");
        s.push_str("Warning: dangerous step.\n\n");
        // Bloat the body with low-density paragraphs.
        for i in 0..200 {
            s.push_str(&format!(
                "Filler paragraph {i} — {}\n\n",
                "lorem ipsum dolor sit amet ".repeat(40)
            ));
        }
        let out = summarize_structurally(&s, 5_000);
        assert!(out.len() <= 5_000);
        assert!(out.contains("# Title"));
        assert!(out.contains("```sh"));
        assert!(out.contains("Warning: dangerous step."));
    }

    #[test]
    fn structural_summarize_keeps_numbered_workflows() {
        let mut s = String::new();
        s.push_str("Workflow:\n1. clone\n2. install\n3. run tests\n\n");
        for _ in 0..200 {
            s.push_str(&format!("Filler. {}\n\n", "x".repeat(80)));
        }
        let out = summarize_structurally(&s, 2_000);
        assert!(out.contains("1. clone"));
        assert!(out.contains("2. install"));
        assert!(out.contains("3. run tests"));
    }
}
