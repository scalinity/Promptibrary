//! Source URL classification per spec §6 *Source detection*.
//!
//! Maps a raw URL string to a typed `SourceDetection`. Detection is
//! deterministic, depends only on the URL itself, and never performs a
//! network round-trip — that's the fetcher's job.

use url::Url;

use super::types::{SourceDetection, UnsupportedSourceReason};

/// Classify `url` against the spec §6 detection table. Returns one of:
/// - `Youtube { canonical_url, video_id }`
/// - `XTwitter { canonical_url, post_id, username }`
/// - `Article { canonical_url, hostname }`
/// - `Unsupported { reason }` for invalid URLs or non-http(s) schemes
pub fn detect_source(input: &str) -> SourceDetection {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return SourceDetection::Unsupported {
            reason: UnsupportedSourceReason::InvalidUrl,
        };
    }

    let parsed = match Url::parse(trimmed) {
        Ok(u) => u,
        Err(_) => {
            return SourceDetection::Unsupported {
                reason: UnsupportedSourceReason::InvalidUrl,
            }
        }
    };

    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return SourceDetection::Unsupported {
            reason: UnsupportedSourceReason::UnsupportedScheme,
        };
    }

    let host = match parsed.host_str() {
        Some(h) => h.to_ascii_lowercase(),
        None => {
            return SourceDetection::Unsupported {
                reason: UnsupportedSourceReason::InvalidUrl,
            }
        }
    };

    // Normalize host: strip leading "www.".
    let normalized_host = host.strip_prefix("www.").unwrap_or(&host).to_string();

    if let Some(d) = detect_youtube(&parsed, &normalized_host) {
        return d;
    }

    if let Some(d) = detect_x_twitter(&parsed, &normalized_host) {
        return d;
    }

    SourceDetection::Article {
        canonical_url: canonical_article(&parsed, &normalized_host),
        hostname: normalized_host,
    }
}

fn detect_youtube(url: &Url, host: &str) -> Option<SourceDetection> {
    let video_id = match host {
        "youtube.com" | "m.youtube.com" => {
            if url.path() == "/watch" {
                url.query_pairs()
                    .find(|(k, _)| k == "v")
                    .map(|(_, v)| v.into_owned())
                    .filter(|id| is_youtube_id(id))?
            } else {
                let segments: Vec<&str> =
                    url.path_segments()?.filter(|s| !s.is_empty()).collect();
                match segments.as_slice() {
                    ["shorts", id, ..] | ["embed", id, ..] | ["live", id, ..]
                        if is_youtube_id(id) =>
                    {
                        (*id).to_string()
                    }
                    _ => return None,
                }
            }
        }
        "youtu.be" => url
            .path_segments()
            .and_then(|mut s| s.next())
            .filter(|id| is_youtube_id(id))
            .map(str::to_string)?,
        _ => return None,
    };

    Some(SourceDetection::Youtube {
        canonical_url: format!("https://www.youtube.com/watch?v={video_id}"),
        video_id,
    })
}

fn detect_x_twitter(url: &Url, host: &str) -> Option<SourceDetection> {
    let matches_host = matches!(host, "x.com" | "twitter.com" | "mobile.twitter.com");
    if !matches_host {
        return None;
    }

    let segments: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    let (username, post_id) = match segments.as_slice() {
        [user, "status", id, ..] | [user, "statuses", id, ..] if is_tweet_id(id) => {
            (Some((*user).to_string()), (*id).to_string())
        }
        ["i", "web", "status", id, ..] if is_tweet_id(id) => (None, (*id).to_string()),
        _ => return None,
    };

    Some(SourceDetection::XTwitter {
        canonical_url: match &username {
            Some(u) => format!("https://x.com/{u}/status/{post_id}"),
            None => format!("https://x.com/i/web/status/{post_id}"),
        },
        post_id,
        username,
    })
}

fn is_youtube_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 20
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn is_tweet_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 25 && id.chars().all(|c| c.is_ascii_digit())
}

fn canonical_article(url: &Url, host: &str) -> String {
    let mut canonical = format!("{}://{}", url.scheme(), host);
    if let Some(port) = url.port() {
        canonical.push_str(&format!(":{port}"));
    }
    let path = url.path();
    if path == "/" {
        // canonical stays at scheme://host
    } else {
        canonical.push_str(path.trim_end_matches('/'));
    }
    if let Some(q) = url.query() {
        if !q.is_empty() {
            canonical.push('?');
            canonical.push_str(q);
        }
    }
    canonical
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detect(s: &str) -> SourceDetection {
        detect_source(s)
    }

    #[test]
    fn spec_table_youtube_watch() {
        match detect("https://www.youtube.com/watch?v=dQw4w9WgXcQ") {
            SourceDetection::Youtube { video_id, .. } => assert_eq!(video_id, "dQw4w9WgXcQ"),
            _ => panic!("expected youtube"),
        }
    }

    #[test]
    fn spec_table_youtube_short_url() {
        match detect("https://youtu.be/dQw4w9WgXcQ") {
            SourceDetection::Youtube {
                video_id,
                canonical_url,
            } => {
                assert_eq!(video_id, "dQw4w9WgXcQ");
                assert_eq!(canonical_url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
            }
            _ => panic!("expected youtube"),
        }
    }

    #[test]
    fn spec_table_youtube_shorts() {
        match detect("https://www.youtube.com/shorts/abc123_XYZ") {
            SourceDetection::Youtube { video_id, .. } => assert_eq!(video_id, "abc123_XYZ"),
            _ => panic!("expected youtube"),
        }
    }

    #[test]
    fn youtube_extra_query_params_ignored() {
        match detect("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=42s&list=PLfoo") {
            SourceDetection::Youtube { video_id, .. } => assert_eq!(video_id, "dQw4w9WgXcQ"),
            _ => panic!("expected youtube"),
        }
    }

    #[test]
    fn youtube_missing_v_param_falls_through_to_article() {
        match detect("https://www.youtube.com/watch") {
            SourceDetection::Article { hostname, .. } => assert_eq!(hostname, "youtube.com"),
            _ => panic!("expected article fallback"),
        }
    }

    #[test]
    fn spec_table_x_com() {
        match detect("https://x.com/foo/status/1234567890") {
            SourceDetection::XTwitter {
                post_id, username, ..
            } => {
                assert_eq!(post_id, "1234567890");
                assert_eq!(username.as_deref(), Some("foo"));
            }
            _ => panic!("expected x_twitter"),
        }
    }

    #[test]
    fn spec_table_twitter_com() {
        match detect("https://twitter.com/foo/status/9999") {
            SourceDetection::XTwitter {
                post_id, username, ..
            } => {
                assert_eq!(post_id, "9999");
                assert_eq!(username.as_deref(), Some("foo"));
            }
            _ => panic!("expected x_twitter"),
        }
    }

    #[test]
    fn spec_table_mobile_twitter_com() {
        match detect("https://mobile.twitter.com/foo/status/9999") {
            SourceDetection::XTwitter { post_id, .. } => assert_eq!(post_id, "9999"),
            _ => panic!("expected x_twitter"),
        }
    }

    #[test]
    fn x_twitter_i_web_status_falls_back_to_anonymous() {
        match detect("https://x.com/i/web/status/1234567890") {
            SourceDetection::XTwitter {
                post_id, username, ..
            } => {
                assert_eq!(post_id, "1234567890");
                assert_eq!(username, None);
            }
            _ => panic!("expected x_twitter"),
        }
    }

    #[test]
    fn spec_table_arbitrary_article() {
        match detect("https://example.com/blog/post-1") {
            SourceDetection::Article {
                hostname,
                canonical_url,
            } => {
                assert_eq!(hostname, "example.com");
                assert_eq!(canonical_url, "https://example.com/blog/post-1");
            }
            _ => panic!("expected article"),
        }
    }

    #[test]
    fn article_strips_www_prefix() {
        match detect("https://www.example.com/x") {
            SourceDetection::Article { hostname, .. } => assert_eq!(hostname, "example.com"),
            _ => panic!("expected article"),
        }
    }

    #[test]
    fn article_lowercases_host() {
        match detect("https://EXAMPLE.com/Path") {
            SourceDetection::Article {
                hostname,
                canonical_url,
            } => {
                assert_eq!(hostname, "example.com");
                assert_eq!(canonical_url, "https://example.com/Path");
            }
            _ => panic!("expected article"),
        }
    }

    #[test]
    fn article_drops_trailing_slash() {
        match detect("https://example.com/x/") {
            SourceDetection::Article { canonical_url, .. } => {
                assert_eq!(canonical_url, "https://example.com/x");
            }
            _ => panic!("expected article"),
        }
    }

    #[test]
    fn ftp_scheme_unsupported() {
        match detect("ftp://example.com/x") {
            SourceDetection::Unsupported {
                reason: UnsupportedSourceReason::UnsupportedScheme,
            } => {}
            _ => panic!("expected unsupported_scheme"),
        }
    }

    #[test]
    fn empty_or_malformed_url_invalid() {
        match detect("") {
            SourceDetection::Unsupported {
                reason: UnsupportedSourceReason::InvalidUrl,
            } => {}
            _ => panic!("expected invalid_url"),
        }
        match detect("not-a-url") {
            SourceDetection::Unsupported {
                reason: UnsupportedSourceReason::InvalidUrl,
            } => {}
            _ => panic!("expected invalid_url"),
        }
    }

    #[test]
    fn non_http_scheme_unsupported() {
        match detect("file:///etc/passwd") {
            SourceDetection::Unsupported {
                reason: UnsupportedSourceReason::UnsupportedScheme,
            } => {}
            _ => panic!("expected unsupported_scheme"),
        }
    }

    #[test]
    fn whitespace_trimmed() {
        match detect("  https://example.com/x  ") {
            SourceDetection::Article { hostname, .. } => assert_eq!(hostname, "example.com"),
            _ => panic!("expected article"),
        }
    }
}
