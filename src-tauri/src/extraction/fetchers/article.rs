//! Generic web-article fetcher (spec §6, patched).
//!
//! Deterministic, no Mozilla-Readability scoring. The LLM is the actual
//! extractor; this Rust pipeline strips noise + picks the densest subtree
//! so the article fits in context.
//!
//! Subtree selection: `<article>` → `<main>` → `[role=main]` → highest
//! text-to-tag-ratio descendant (single linear pass, no recursion, no
//! weighting) → `<body>` minus stripped tags.

use chrono::Utc;
use once_cell::sync::Lazy;
use scraper::{ElementRef, Html, Node, Selector};

// Static selectors — parsed once at first access, reused for every
// `parse_html_article` call. The dynamic `meta[property/name]` selectors
// stay inline because they interpolate field names at call time.
static SEL_TITLE: Lazy<Selector> = Lazy::new(|| Selector::parse("title").expect("title"));
static SEL_ARTICLE: Lazy<Selector> = Lazy::new(|| Selector::parse("article").expect("article"));
static SEL_MAIN: Lazy<Selector> = Lazy::new(|| Selector::parse("main").expect("main"));
static SEL_ROLE_MAIN: Lazy<Selector> =
    Lazy::new(|| Selector::parse("[role=\"main\"]").expect("[role=main]"));
static SEL_BODY: Lazy<Selector> = Lazy::new(|| Selector::parse("body").expect("body"));
static SEL_JSONLD: Lazy<Selector> = Lazy::new(|| {
    Selector::parse("script[type=\"application/ld+json\"]").expect("script[type=application/ld+json]")
});

use crate::domain::source::{ArticleSource, Source};
use crate::error::{AppError, AppErrorKind, Result};
use crate::extraction::types::{
    ExtractionFailure, FetchedSourceContent, SourceChunk, SourceChunkKind,
};

const NOISE_TAGS: &[&str] = &[
    "script", "style", "noscript", "svg", "iframe", "nav", "footer", "aside", "header",
];

const MAX_BODY_BYTES: usize = 10 * 1024 * 1024; // 10 MB
const PAYWALL_TEXT_THRESHOLD: usize = 1000;
const SUBTREE_MIN_TEXT: usize = 500;
const USER_AGENT: &str = "Promptibrary/1.0 (+local desktop importer)";
const PAYWALL_MARKERS: &[&str] = &[
    "paywall",
    "subscribe to read",
    "members-only",
    "members only",
    "tp-modal",
    "tp-meter",
    "this is a subscriber-only",
    "to continue reading",
];

#[derive(Debug)]
pub struct ParsedArticle {
    pub title: Option<String>,
    pub author: Option<String>,
    pub site_name: Option<String>,
    pub published_at_rfc3339: Option<String>,
    pub text: String,
}

/// Fetch + parse `url`. Recoverable issues (paywall, oversize, network
/// blip) come back as `Err(ExtractionFailure)` so the caller can render a
/// tailored UI panel; hard transport faults become `AppError`.
pub async fn fetch_article(
    url: &str,
    canonical_url: &str,
    http: &reqwest::Client,
) -> Result<std::result::Result<FetchedSourceContent, ExtractionFailure>> {
    let resp = match http
        .get(url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) if e.is_timeout() || e.is_connect() => {
            return Ok(Err(ExtractionFailure::NetworkUnavailable {
                message: format!("{e}"),
            }))
        }
        Err(e) => {
            return Err(AppError::new(
                AppErrorKind::ExtractionFailed,
                format!("article fetch failed: {e}"),
            ))
        }
    };

    if !resp.status().is_success() {
        return Ok(Err(ExtractionFailure::ExtractionFailed {
            reason: format!("article fetch returned HTTP {}", resp.status()),
        }));
    }

    let body = resp.bytes().await?;
    if body.len() > MAX_BODY_BYTES {
        return Ok(Err(ExtractionFailure::ExtractionFailed {
            reason: format!("body too large: {} bytes (max {})", body.len(), MAX_BODY_BYTES),
        }));
    }
    let html_text = String::from_utf8_lossy(&body).into_owned();

    let parsed = parse_html_article(&html_text);

    if parsed.text.chars().count() < PAYWALL_TEXT_THRESHOLD && looks_paywalled(&html_text) {
        return Ok(Err(ExtractionFailure::PaywallLikely {
            preview: parsed.text.chars().take(400).collect(),
        }));
    }

    Ok(Ok(into_fetched(parsed, canonical_url)))
}

/// Pure HTML → article parse. Tested in isolation; no I/O.
pub fn parse_html_article(html_text: &str) -> ParsedArticle {
    let doc = Html::parse_document(html_text);

    let title_tag = doc
        .select(&SEL_TITLE)
        .next()
        .map(|t| collect_text(t).trim().to_string())
        .filter(|s| !s.is_empty());
    let og_title = meta_property(&doc, "og:title");
    let title = og_title.or(title_tag);

    let site_name = meta_property(&doc, "og:site_name");
    let author = meta_property(&doc, "article:author")
        .or_else(|| meta_name(&doc, "author"))
        .or_else(|| jsonld_author(&doc));
    let published_at_rfc3339 = meta_property(&doc, "article:published_time")
        .or_else(|| meta_property(&doc, "og:article:published_time"))
        .or_else(|| jsonld_date_published(&doc));

    let subtree = select_subtree(&doc);
    let text = match subtree {
        Some(el) => serialize_subtree(el),
        None => serialize_body_fallback(&doc),
    };

    ParsedArticle {
        title,
        author,
        site_name,
        published_at_rfc3339,
        text,
    }
}

fn into_fetched(parsed: ParsedArticle, canonical_url: &str) -> FetchedSourceContent {
    let now = Utc::now();
    let published_at = parsed
        .published_at_rfc3339
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc));

    let source = Source::Article(ArticleSource {
        origin_url: Some(canonical_url.to_string()),
        title: parsed.title.clone(),
        author: parsed.author.clone(),
        fetched_at: Some(now),
        content_hash: None,
        site_name: parsed.site_name.clone(),
        byline: parsed.author.clone(),
        published_at,
    });

    let chunks = vec![SourceChunk {
        kind: SourceChunkKind::Article,
        order: 0,
        text: parsed.text.clone(),
        url: Some(canonical_url.to_string()),
        timestamp_seconds: None,
    }];

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(parsed.text.as_bytes());
    let content_hash = format!("{:x}", hasher.finalize());

    FetchedSourceContent {
        source,
        canonical_url: canonical_url.to_string(),
        fetched_at: now,
        title: parsed.title,
        author: parsed.author,
        text: parsed.text,
        chunks,
        raw_metadata: Default::default(),
        content_hash,
        cached: false,
    }
}

// ─── Subtree selection ───────────────────────────────────────────────────────

fn select_subtree(doc: &Html) -> Option<ElementRef<'_>> {
    if let Some(el) = doc.select(&SEL_ARTICLE).next() {
        return Some(el);
    }
    if let Some(el) = doc.select(&SEL_MAIN).next() {
        return Some(el);
    }
    if let Some(el) = doc.select(&SEL_ROLE_MAIN).next() {
        return Some(el);
    }
    densest_descendant(doc)
}

/// Single linear pass over all elements. Pick the element with the highest
/// plain-text-to-tag ratio whose plain-text length is at least
/// `SUBTREE_MIN_TEXT` chars. No recursion into scored children, no per-class
/// weighting — this is intentionally NOT Readability.
fn densest_descendant(doc: &Html) -> Option<ElementRef<'_>> {
    let mut best: Option<(ElementRef<'_>, f64)> = None;
    let root = doc.root_element();
    for el in root.descendants().filter_map(ElementRef::wrap) {
        if is_noise_element(el) {
            continue;
        }
        let text = collect_text(el);
        let text_len = text.chars().count();
        if text_len < SUBTREE_MIN_TEXT {
            continue;
        }
        let tag_count = count_descendant_elements(el);
        let ratio = text_len as f64 / (tag_count as f64 + 1.0);
        match best {
            None => best = Some((el, ratio)),
            Some((_, br)) if ratio > br => best = Some((el, ratio)),
            _ => {}
        }
    }
    best.map(|(el, _)| el)
}

fn count_descendant_elements(el: ElementRef<'_>) -> usize {
    el.descendants().filter(|n| n.value().is_element()).count()
}

fn is_noise_element(el: ElementRef<'_>) -> bool {
    NOISE_TAGS.contains(&el.value().name())
}

// ─── Serialization ───────────────────────────────────────────────────────────

fn serialize_subtree(el: ElementRef<'_>) -> String {
    let mut out = String::new();
    let mut state = SerState::default();
    walk(el, &mut out, &mut state);
    out.trim().to_string()
}

fn serialize_body_fallback(doc: &Html) -> String {
    if let Some(body) = doc.select(&SEL_BODY).next() {
        return serialize_subtree(body);
    }
    String::new()
}

#[derive(Default)]
struct SerState {
    list_stack: Vec<ListKind>,
    ordered_counter: Vec<u32>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum ListKind {
    Ul,
    Ol,
}

fn walk(el: ElementRef<'_>, out: &mut String, state: &mut SerState) {
    if is_noise_element(el) {
        return;
    }
    let name = el.value().name();
    match name {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level: usize = name[1..].parse().unwrap_or(1);
            ensure_blank_line(out);
            out.push_str(&"#".repeat(level));
            out.push(' ');
            out.push_str(collect_text(el).trim());
            out.push('\n');
        }
        "pre" => {
            ensure_blank_line(out);
            let lang = code_lang_from_child(el).unwrap_or_default();
            out.push_str(&format!("```{lang}\n"));
            out.push_str(&collect_text(el));
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("```\n");
        }
        "code"
            if !matches!(
                el.parent()
                    .and_then(ElementRef::wrap)
                    .map(|p| p.value().name()),
                Some("pre")
            ) =>
        {
            out.push('`');
            out.push_str(collect_text(el).trim());
            out.push('`');
        }
        "ul" | "ol" => {
            ensure_newline(out);
            let kind = if name == "ul" { ListKind::Ul } else { ListKind::Ol };
            state.list_stack.push(kind);
            if kind == ListKind::Ol {
                state.ordered_counter.push(0);
            }
            for child in el.children().filter_map(ElementRef::wrap) {
                walk(child, out, state);
            }
            if kind == ListKind::Ol {
                state.ordered_counter.pop();
            }
            state.list_stack.pop();
        }
        "li" => {
            ensure_newline(out);
            match state.list_stack.last().copied().unwrap_or(ListKind::Ul) {
                ListKind::Ul => out.push_str("- "),
                ListKind::Ol => {
                    if let Some(counter) = state.ordered_counter.last_mut() {
                        *counter += 1;
                        out.push_str(&format!("{counter}. "));
                    } else {
                        out.push_str("1. ");
                    }
                }
            }
            out.push_str(&inline_text(el));
            out.push('\n');
        }
        "p" => {
            ensure_blank_line(out);
            out.push_str(&inline_text(el));
            out.push('\n');
        }
        "br" => out.push('\n'),
        "a" => {
            let text = collect_text(el).trim().to_string();
            let href = el.value().attr("href").unwrap_or("");
            if href.is_empty() || link_text_equals_href(&text, href) {
                out.push_str(&text);
            } else if text.is_empty() {
                out.push_str(href);
            } else {
                out.push_str(&text);
                out.push_str(" (");
                out.push_str(href);
                out.push(')');
            }
        }
        _ => {
            for child in el.children() {
                match child.value() {
                    Node::Element(_) => {
                        if let Some(child_el) = ElementRef::wrap(child) {
                            walk(child_el, out, state);
                        }
                    }
                    Node::Text(t) => {
                        let s = t.trim();
                        if !s.is_empty() {
                            if !out.is_empty() && !out.ends_with(' ') && !out.ends_with('\n') {
                                out.push(' ');
                            }
                            out.push_str(s);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn inline_text(el: ElementRef<'_>) -> String {
    let mut s = String::new();
    inline_walk(el, &mut s);
    collapse_ws(&s)
}

fn inline_walk(el: ElementRef<'_>, out: &mut String) {
    for child in el.children() {
        match child.value() {
            Node::Text(t) => out.push_str(t),
            Node::Element(_) => {
                if let Some(child_el) = ElementRef::wrap(child) {
                    if is_noise_element(child_el) {
                        continue;
                    }
                    let name = child_el.value().name();
                    match name {
                        "br" => out.push('\n'),
                        "a" => {
                            let mut inner = String::new();
                            inline_walk(child_el, &mut inner);
                            let text = inner.trim().to_string();
                            let href = child_el.value().attr("href").unwrap_or("").trim();
                            if href.is_empty() || link_text_equals_href(&text, href) {
                                out.push_str(&text);
                            } else if text.is_empty() {
                                out.push_str(href);
                            } else {
                                out.push_str(&text);
                                out.push_str(" (");
                                out.push_str(href);
                                out.push(')');
                            }
                        }
                        "code" => {
                            out.push('`');
                            let mut inner = String::new();
                            inline_walk(child_el, &mut inner);
                            out.push_str(inner.trim());
                            out.push('`');
                        }
                        _ => inline_walk(child_el, out),
                    }
                }
            }
            _ => {}
        }
    }
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for c in s.chars() {
        if c == '\n' {
            out.push('\n');
            prev_ws = false;
        } else if c.is_whitespace() {
            if !prev_ws {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    out.trim().to_string()
}

fn collect_text(el: ElementRef<'_>) -> String {
    // `descendants()` returns a flat sequence; a `continue` on a noise
    // element only skips that element, NOT its text-node children which
    // appear later in the same flat iteration. We need a real recursive
    // walk that prunes the entire subtree under any noise CHILD — this
    // matters for `densest_descendant` scoring, where unaccounted-for
    // `<script>` text would inflate the text-to-tag ratio (SCA-707).
    //
    // Caller is allowed to ask for text inside an explicit noise element
    // (the JSON-LD path passes a `<script>` directly) — the root is
    // never pruned; only noise *descendants* are. That matches the
    // semantics the previous flat-descendants code accidentally gave us.
    let mut out = String::new();
    collect_text_children(el, &mut out);
    out
}

fn collect_text_children(el: ElementRef<'_>, out: &mut String) {
    for child in el.children() {
        match child.value() {
            Node::Text(t) => out.push_str(t),
            Node::Element(_) => {
                if let Some(child_el) = ElementRef::wrap(child) {
                    if is_noise_element(child_el) {
                        continue;
                    }
                    collect_text_children(child_el, out);
                }
            }
            _ => {}
        }
    }
}

fn code_lang_from_child(pre: ElementRef<'_>) -> Option<String> {
    for child in pre.descendants().filter_map(ElementRef::wrap) {
        if child.value().name() == "code" {
            if let Some(class) = child.value().attr("class") {
                for tok in class.split_ascii_whitespace() {
                    if let Some(rest) = tok.strip_prefix("language-") {
                        return Some(rest.to_string());
                    }
                }
            }
        }
    }
    None
}

fn link_text_equals_href(text: &str, href: &str) -> bool {
    let t = text.trim().trim_end_matches('/');
    let h = href.trim().trim_end_matches('/');
    t == h || format!("https://{t}") == h || format!("http://{t}") == h
}

fn ensure_blank_line(out: &mut String) {
    if out.is_empty() {
        return;
    }
    if !out.ends_with("\n\n") {
        if out.ends_with('\n') {
            out.push('\n');
        } else {
            out.push_str("\n\n");
        }
    }
}

fn ensure_newline(out: &mut String) {
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
}

// ─── Metadata ────────────────────────────────────────────────────────────────

fn meta_property(doc: &Html, prop: &str) -> Option<String> {
    doc.select(&selector(&format!("meta[property=\"{prop}\"]")))
        .next()
        .and_then(|el| el.value().attr("content").map(str::to_string))
        .filter(|s| !s.is_empty())
}

fn meta_name(doc: &Html, name: &str) -> Option<String> {
    doc.select(&selector(&format!("meta[name=\"{name}\"]")))
        .next()
        .and_then(|el| el.value().attr("content").map(str::to_string))
        .filter(|s| !s.is_empty())
}

fn jsonld_author(doc: &Html) -> Option<String> {
    extract_jsonld_string(doc, &["author", "name"])
        .or_else(|| extract_jsonld_string(doc, &["author"]))
}

fn jsonld_date_published(doc: &Html) -> Option<String> {
    extract_jsonld_string(doc, &["datePublished"])
}

fn extract_jsonld_string(doc: &Html, path: &[&str]) -> Option<String> {
    for script in doc.select(&SEL_JSONLD) {
        let raw = collect_text(script);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(s) = traverse_path(&v, path) {
                return Some(s);
            }
        }
    }
    None
}

fn traverse_path(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for &p in path {
        current = current.get(p)?;
    }
    match current {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(items) => items.first().and_then(|v| match v {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Object(_) => v
                .get(*path.last().unwrap_or(&""))
                .and_then(|v| v.as_str().map(str::to_string)),
            _ => None,
        }),
        _ => None,
    }
}

// ─── Paywall heuristic ───────────────────────────────────────────────────────

/// Paywall markers always appear in the page chrome (subscribe banners,
/// modal containers, vendor classes) — scanning the full body would
/// allocate up to 10 MB of lowercase string for no payoff. Limit to the
/// first 64 KB which captures the entire `<head>` and the above-the-fold
/// `<body>` content for any real paywalled page.
const PAYWALL_SCAN_BYTES: usize = 65_536;

fn looks_paywalled(html_text: &str) -> bool {
    let limit = html_text.len().min(PAYWALL_SCAN_BYTES);
    // Truncate at a char boundary so the slice is valid UTF-8.
    let mut cut = limit;
    while cut > 0 && !html_text.is_char_boundary(cut) {
        cut -= 1;
    }
    let lower = html_text[..cut].to_ascii_lowercase();
    PAYWALL_MARKERS.iter().any(|m| lower.contains(m))
}

// ─── Selector helper ─────────────────────────────────────────────────────────

fn selector(s: &str) -> Selector {
    // Parse failures here are programming errors — the selector strings are
    // string literals, not user input. Panicking is the right behavior so
    // the test suite catches it loudly.
    Selector::parse(s).expect("static selector parses")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(html: &str) -> ParsedArticle {
        parse_html_article(html)
    }

    #[test]
    fn picks_article_tag_when_present() {
        let html = r#"
            <!doctype html><html><body>
              <header><h1>Site Header</h1></header>
              <article>
                <h1>Real Title</h1>
                <p>This is the real body of the article that we should extract.</p>
              </article>
              <footer>Footer noise</footer>
            </body></html>
        "#;
        let p = parse(html);
        assert!(p.text.contains("Real Title"));
        assert!(p.text.contains("This is the real body"));
        assert!(!p.text.contains("Site Header"));
        assert!(!p.text.contains("Footer noise"));
    }

    #[test]
    fn picks_main_tag_when_no_article() {
        let html = r#"<html><body><main><p>Main body content here.</p></main><nav>nope</nav></body></html>"#;
        let p = parse(html);
        assert!(p.text.contains("Main body content here."));
        assert!(!p.text.contains("nope"));
    }

    #[test]
    fn falls_back_to_densest_descendant() {
        let body_para = "x".repeat(800);
        let html = format!(
            "<html><body><div class='sidebar'>tiny</div><div class='content'><p>{body_para}</p></div></body></html>"
        );
        let p = parse(&html);
        assert!(p.text.contains(&body_para));
        assert!(!p.text.contains("tiny"));
    }

    #[test]
    fn strips_noise_tags() {
        let html = r#"
            <html><body>
              <article>
                <script>alert('xss')</script>
                <style>.x{color:red}</style>
                <nav>menu</nav>
                <p>Keep this paragraph.</p>
                <footer>drop me</footer>
              </article>
            </body></html>
        "#;
        let p = parse(html);
        assert!(p.text.contains("Keep this paragraph"));
        assert!(!p.text.contains("alert"));
        assert!(!p.text.contains("color:red"));
        assert!(!p.text.contains("menu"));
        assert!(!p.text.contains("drop me"));
    }

    #[test]
    fn preserves_headings_with_pound_markers() {
        let html = r#"<article><h1>H1</h1><h2>H2</h2><h3>H3</h3><p>body</p></article>"#;
        let p = parse(html);
        assert!(p.text.contains("# H1"));
        assert!(p.text.contains("## H2"));
        assert!(p.text.contains("### H3"));
    }

    #[test]
    fn preserves_code_fences_with_language_hint() {
        let html =
            r#"<article><pre><code class="language-rust">fn main() {}</code></pre></article>"#;
        let p = parse(html);
        assert!(p.text.contains("```rust"));
        assert!(p.text.contains("fn main() {}"));
    }

    #[test]
    fn preserves_lists_ordered_and_unordered() {
        let html = r#"<article><p>intro</p><ul><li>a</li><li>b</li></ul><ol><li>one</li><li>two</li></ol></article>"#;
        let p = parse(html);
        assert!(p.text.contains("- a"));
        assert!(p.text.contains("- b"));
        assert!(p.text.contains("1. one"));
        assert!(p.text.contains("2. two"));
    }

    #[test]
    fn links_only_rendered_when_text_differs_from_href() {
        let html = r#"<article><p><a href="https://example.com/foo">read more</a> and <a href="https://example.com/foo">https://example.com/foo</a></p></article>"#;
        let p = parse(html);
        assert!(p.text.contains("read more (https://example.com/foo)"));
        assert!(!p
            .text
            .contains("https://example.com/foo (https://example.com/foo)"));
    }

    #[test]
    fn extracts_og_title_over_title_tag() {
        let html = r#"
            <html><head>
              <title>Tab Title</title>
              <meta property="og:title" content="OG Title"/>
              <meta property="og:site_name" content="Site"/>
            </head><body><article><p>body body body</p></article></body></html>
        "#;
        let p = parse(html);
        assert_eq!(p.title.as_deref(), Some("OG Title"));
        assert_eq!(p.site_name.as_deref(), Some("Site"));
    }

    #[test]
    fn extracts_published_at_from_meta() {
        let html = r#"
            <html><head>
              <meta property="article:published_time" content="2026-05-19T10:00:00Z"/>
            </head><body><article><p>x</p></article></body></html>
        "#;
        let p = parse(html);
        assert_eq!(
            p.published_at_rfc3339.as_deref(),
            Some("2026-05-19T10:00:00Z")
        );
    }

    #[test]
    fn extracts_jsonld_author_and_date() {
        let html = r#"
            <html><head>
              <script type="application/ld+json">
              {"@type":"NewsArticle","author":{"name":"Alice"},"datePublished":"2026-05-19T10:00:00Z"}
              </script>
            </head><body><article><p>x</p></article></body></html>
        "#;
        let p = parse(html);
        assert_eq!(p.author.as_deref(), Some("Alice"));
        assert_eq!(
            p.published_at_rfc3339.as_deref(),
            Some("2026-05-19T10:00:00Z")
        );
    }

    #[test]
    fn paywall_detection_triggers_on_short_text() {
        let html = r#"<html><body><article><p>Subscribe to read.</p></article><div class="tp-modal"></div></body></html>"#;
        assert!(looks_paywalled(html));
        let p = parse(html);
        assert!(p.text.len() < PAYWALL_TEXT_THRESHOLD);
    }

    #[test]
    fn densest_descendant_ignores_inline_script_text() {
        // SCA-707: previously, `collect_text` used `descendants()` and
        // `continue`d on noise elements, but the script's child text node
        // was a separate iteration and slipped through — inflating the
        // text-to-tag ratio for sidebar containers that wrap analytics
        // blobs. After the fix the script contents are skipped entirely.
        let real_paragraph = "x".repeat(800);
        let inline_blob = "y".repeat(2_000);
        // The sidebar div is small (one short word of visible text) but
        // contains a long inline script. Pre-fix, the script's text
        // would push it past the 500-char threshold and outrank the
        // dense content div.
        let html = format!(
            r#"<html><body>
              <div class='sidebar'>tiny<script>{inline_blob}</script></div>
              <div class='content'><p>{real_paragraph}</p></div>
            </body></html>"#
        );
        let p = parse(&html);
        assert!(p.text.contains(&real_paragraph));
        assert!(!p.text.contains("yyyy"), "inline script text must not leak into selected subtree");
    }

    #[test]
    fn no_readability_terms_in_production_section() {
        // Spec §6 patched rules — make sure nobody resurrects a scoring
        // heuristic by accident. Walk the production section only (above
        // `#[cfg(test)]`) so the forbidden-words listed below this test
        // body don't trip the guard themselves.
        let src = include_str!("article.rs");
        let prod = src
            .split_once("#[cfg(test)]")
            .map(|(prod, _)| prod)
            .unwrap_or(src);
        // Identifier-shaped tokens only (snake_case). Free-text mentions
        // of "Readability" in comments are fine (and used above to explain
        // why we DON'T do that).
        for forbidden in &[
            "density_score",
            "weight_boost",
            "node_score",
            "class_weight",
            "fn score(",
            "fn weight(",
            "fn boost(",
        ] {
            assert!(
                !prod.to_ascii_lowercase().contains(forbidden),
                "forbidden Readability-style identifier `{forbidden}` reappeared in production code"
            );
        }
    }
}
