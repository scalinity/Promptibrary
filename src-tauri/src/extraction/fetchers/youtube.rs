//! YouTube transcript fetcher (spec §6 *YouTube fetcher*).
//!
//! Shells out to `yt-dlp` for metadata + subtitle file, parses the
//! resulting json3 or vtt transcript into ordered `SourceChunk`s with
//! `timestampSeconds`. Adjacent duplicate fragments (auto-caption noise)
//! collapse to a single chunk. No English transcript → TranscriptUnavailable.
//!
//! `YtDlpRunner` abstracts subprocess execution so tests parse fixture
//! transcripts without needing yt-dlp installed.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use tokio::process::Command;

use crate::domain::source::{Source, YouTubeSource};
use crate::error::Result;
use crate::extraction::types::{
    ExtractionFailure, FetchedSourceContent, SourceChunk, SourceChunkKind,
};

#[derive(Debug)]
pub struct YtDlpMetadata {
    pub id: String,
    pub title: Option<String>,
    pub uploader: Option<String>,
    pub channel: Option<String>,
    pub duration_seconds: Option<u32>,
    pub description: Option<String>,
}

#[async_trait]
pub trait YtDlpRunner: Send + Sync {
    async fn version_probe(&self) -> Result<std::result::Result<String, ExtractionFailure>>;
    async fn dump_metadata(
        &self,
        url: &str,
    ) -> Result<std::result::Result<YtDlpMetadata, ExtractionFailure>>;
    /// Download subtitles to `temp_dir`. Returns the file path that was
    /// created (json3 preferred, then vtt), or `Err(TranscriptUnavailable)`
    /// if no English transcript exists.
    async fn download_subs(
        &self,
        url: &str,
        video_id: &str,
        temp_dir: &Path,
    ) -> Result<std::result::Result<PathBuf, ExtractionFailure>>;
}

pub struct RealYtDlpRunner;

#[async_trait]
impl YtDlpRunner for RealYtDlpRunner {
    async fn version_probe(&self) -> Result<std::result::Result<String, ExtractionFailure>> {
        let out = Command::new("yt-dlp").arg("--version").output().await;
        match out {
            Ok(o) if o.status.success() => Ok(Ok(String::from_utf8_lossy(&o.stdout).trim().into())),
            Ok(_) | Err(_) => Ok(Err(ExtractionFailure::DependencyMissing {
                name: "yt-dlp".into(),
                install_hint: install_hint_for_current_os(),
            })),
        }
    }

    async fn dump_metadata(
        &self,
        url: &str,
    ) -> Result<std::result::Result<YtDlpMetadata, ExtractionFailure>> {
        let out = Command::new("yt-dlp")
            .args(["--skip-download", "--dump-json", "--no-warnings"])
            .arg(url)
            .output()
            .await;
        match out {
            Ok(o) if o.status.success() => match parse_metadata(&o.stdout) {
                Ok(m) => Ok(Ok(m)),
                Err(e) => Ok(Err(ExtractionFailure::ExtractionFailed {
                    reason: format!("yt-dlp metadata parse failed: {e}"),
                })),
            },
            Ok(o) => Ok(Err(ExtractionFailure::ExtractionFailed {
                reason: format!(
                    "yt-dlp metadata exited with status {}: {}",
                    o.status,
                    String::from_utf8_lossy(&o.stderr)
                ),
            })),
            Err(e) => Ok(Err(ExtractionFailure::DependencyMissing {
                name: "yt-dlp".into(),
                install_hint: format!("{}\nUnderlying error: {e}", install_hint_for_current_os()),
            })),
        }
    }

    async fn download_subs(
        &self,
        url: &str,
        video_id: &str,
        temp_dir: &Path,
    ) -> Result<std::result::Result<PathBuf, ExtractionFailure>> {
        let _ = tokio::fs::create_dir_all(temp_dir).await;
        let path_arg = format!("temp:{}", temp_dir.display());
        let out = Command::new("yt-dlp")
            .args([
                "--skip-download",
                "--write-subs",
                "--write-auto-subs",
                "--sub-langs",
                "en.*,en",
                "--sub-format",
                "json3/vtt/best",
                "--paths",
                &path_arg,
                "--output",
                "%(id)s.%(ext)s",
            ])
            .arg(url)
            .output()
            .await
            .map_err(|e| crate::error::AppError::new(
                crate::error::AppErrorKind::ExtractionFailed,
                format!("yt-dlp subs failed: {e}"),
            ))?;

        if !out.status.success() {
            return Ok(Err(ExtractionFailure::ExtractionFailed {
                reason: format!(
                    "yt-dlp subs exited with status {}: {}",
                    out.status,
                    String::from_utf8_lossy(&out.stderr)
                ),
            }));
        }

        // Walk the temp dir for the produced file. Prefer json3, fall back
        // to vtt. Manual subs (e.g. `<id>.en.json3`) precede auto
        // (`<id>.en.json3` is the same filename — yt-dlp picks the best
        // single track per `--sub-format` arg).
        let candidates = find_transcript_files(temp_dir, video_id)?;
        if let Some(path) = candidates.into_iter().next() {
            Ok(Ok(path))
        } else {
            Ok(Err(ExtractionFailure::TranscriptUnavailable))
        }
    }
}

fn install_hint_for_current_os() -> String {
    if cfg!(target_os = "macos") {
        "brew install yt-dlp".into()
    } else if cfg!(target_os = "linux") {
        "pipx install yt-dlp (or use your distro package)".into()
    } else {
        "Install yt-dlp from https://github.com/yt-dlp/yt-dlp".into()
    }
}

fn find_transcript_files(temp_dir: &Path, video_id: &str) -> Result<Vec<PathBuf>> {
    let mut json3: Vec<PathBuf> = vec![];
    let mut vtt: Vec<PathBuf> = vec![];
    let entries = match std::fs::read_dir(temp_dir) {
        Ok(e) => e,
        Err(_) => return Ok(vec![]),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.starts_with(video_id) {
            continue;
        }
        if name.ends_with(".json3") {
            json3.push(path);
        } else if name.ends_with(".vtt") {
            vtt.push(path);
        }
    }
    // Manual subtitle files don't have `.auto` infix; auto have `.auto.<lang>.<ext>`
    // pattern depending on yt-dlp version. As a simple "prefer manual"
    // heuristic, sort so files NOT containing ".auto." come first.
    json3.sort_by_key(|p| {
        p.to_string_lossy().contains(".auto.") as u8
    });
    vtt.sort_by_key(|p| p.to_string_lossy().contains(".auto.") as u8);
    Ok(json3.into_iter().chain(vtt).collect())
}

// ─── Transcript parsers ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct Json3 {
    events: Option<Vec<Json3Event>>,
}

#[derive(Debug, Deserialize)]
struct Json3Event {
    #[serde(rename = "tStartMs")]
    t_start_ms: Option<i64>,
    segs: Option<Vec<Json3Seg>>,
}

#[derive(Debug, Deserialize)]
struct Json3Seg {
    utf8: Option<String>,
}

pub fn parse_json3(raw: &str) -> Vec<SourceChunk> {
    let parsed: Json3 = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let mut entries: Vec<(f64, String)> = Vec::new();
    for ev in parsed.events.unwrap_or_default() {
        let ts = ev.t_start_ms.unwrap_or(0) as f64 / 1000.0;
        let mut text = String::new();
        for seg in ev.segs.unwrap_or_default() {
            if let Some(s) = seg.utf8 {
                text.push_str(&s);
            }
        }
        let cleaned = text.trim().to_string();
        if !cleaned.is_empty() {
            entries.push((ts, cleaned));
        }
    }
    entries.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    collapse_adjacent(entries)
}

pub fn parse_vtt(raw: &str) -> Vec<SourceChunk> {
    let mut entries: Vec<(f64, String)> = Vec::new();
    let mut current_ts: Option<f64> = None;
    let mut current_text = String::new();
    for line in raw.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            if let Some(ts) = current_ts.take() {
                let cleaned = current_text.trim().to_string();
                if !cleaned.is_empty() {
                    entries.push((ts, cleaned));
                }
                current_text.clear();
            }
            continue;
        }
        if let Some(ts) = parse_vtt_timestamp_line(line) {
            current_ts = Some(ts);
        } else if current_ts.is_some() {
            if !current_text.is_empty() {
                current_text.push(' ');
            }
            current_text.push_str(&strip_vtt_tags(line));
        }
    }
    if let Some(ts) = current_ts.take() {
        let cleaned = current_text.trim().to_string();
        if !cleaned.is_empty() {
            entries.push((ts, cleaned));
        }
    }
    entries.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    collapse_adjacent(entries)
}

fn parse_vtt_timestamp_line(line: &str) -> Option<f64> {
    // Expected: "HH:MM:SS.mmm --> HH:MM:SS.mmm [optional position settings]"
    let arrow = line.find("-->")?;
    let start = line[..arrow].trim();
    parse_vtt_clock(start)
}

fn parse_vtt_clock(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }
    let mut iter = parts.iter().rev();
    let secs_part = iter.next()?;
    let mins_part = iter.next()?;
    let hours_part = iter.next().copied().unwrap_or("0");
    let secs: f64 = secs_part.replace(',', ".").parse().ok()?;
    let mins: f64 = mins_part.parse().ok()?;
    let hours: f64 = hours_part.parse().ok()?;
    Some(hours * 3600.0 + mins * 60.0 + secs)
}

fn strip_vtt_tags(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_tag = false;
    for c in line.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(c);
        }
    }
    out
}

fn collapse_adjacent(entries: Vec<(f64, String)>) -> Vec<SourceChunk> {
    let mut out: Vec<SourceChunk> = Vec::new();
    let mut prev_text: Option<String> = None;
    for (i, (ts, text)) in entries.into_iter().enumerate() {
        if prev_text.as_deref() == Some(text.as_str()) {
            continue;
        }
        out.push(SourceChunk {
            kind: SourceChunkKind::Transcript,
            order: i as u32,
            text: text.clone(),
            url: None,
            timestamp_seconds: Some(ts),
        });
        prev_text = Some(text);
    }
    // Re-number order densely after collapse.
    for (i, chunk) in out.iter_mut().enumerate() {
        chunk.order = i as u32;
    }
    out
}

#[derive(Debug)]
pub enum MetadataParseError {
    Json(serde_json::Error),
    MissingId,
}

impl std::fmt::Display for MetadataParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(e) => write!(f, "{e}"),
            Self::MissingId => write!(f, "yt-dlp metadata is missing video id"),
        }
    }
}

impl From<serde_json::Error> for MetadataParseError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

fn parse_metadata(stdout: &[u8]) -> std::result::Result<YtDlpMetadata, MetadataParseError> {
    let v: serde_json::Value = serde_json::from_slice(stdout)?;
    // SCA-712: reject empty / missing id explicitly. An empty `video_id`
    // would later cause `find_transcript_files` to do `starts_with("")`,
    // matching every file in the temp dir — including transcripts from a
    // concurrent fetch (in V1, rate-limited away; but fragile contract).
    let id = v["id"].as_str().unwrap_or("").trim().to_string();
    if id.is_empty() {
        return Err(MetadataParseError::MissingId);
    }
    Ok(YtDlpMetadata {
        id,
        title: v["title"].as_str().map(str::to_string),
        uploader: v["uploader"].as_str().map(str::to_string),
        channel: v["channel"].as_str().map(str::to_string),
        duration_seconds: v["duration"].as_f64().map(|f| f as u32),
        description: v["description"].as_str().map(str::to_string),
    })
}

// ─── Orchestration ───────────────────────────────────────────────────────────

pub async fn fetch_youtube(
    canonical_url: &str,
    video_id: &str,
    runner: &dyn YtDlpRunner,
    temp_dir: &Path,
) -> Result<std::result::Result<FetchedSourceContent, ExtractionFailure>> {
    if let Err(f) = runner.version_probe().await? {
        return Ok(Err(f));
    }

    let metadata = match runner.dump_metadata(canonical_url).await? {
        Ok(m) => m,
        Err(f) => return Ok(Err(f)),
    };

    let sub_path = match runner.download_subs(canonical_url, video_id, temp_dir).await? {
        Ok(p) => p,
        Err(f) => return Ok(Err(f)),
    };

    let raw = tokio::fs::read_to_string(&sub_path).await.map_err(|e| {
        crate::error::AppError::new(
            crate::error::AppErrorKind::ExtractionFailed,
            format!("read transcript: {e}"),
        )
    })?;
    let chunks = if sub_path.extension().and_then(|s| s.to_str()) == Some("json3") {
        parse_json3(&raw)
    } else {
        parse_vtt(&raw)
    };
    // Best-effort cleanup; ignore failure.
    let _ = tokio::fs::remove_file(&sub_path).await;

    if chunks.is_empty() {
        return Ok(Err(ExtractionFailure::TranscriptUnavailable));
    }

    let title = metadata.title.clone();
    let author = metadata.uploader.clone().or_else(|| metadata.channel.clone());
    let now = Utc::now();

    let source = Source::Youtube(YouTubeSource {
        origin_url: Some(canonical_url.to_string()),
        title: title.clone(),
        author: author.clone(),
        fetched_at: Some(now),
        content_hash: None,
        video_id: metadata.id.clone(),
        channel_name: metadata.channel.clone(),
        transcript_language: Some("en".into()),
        duration_seconds: metadata.duration_seconds,
    });

    let text = chunks
        .iter()
        .map(|c| c.text.clone())
        .collect::<Vec<_>>()
        .join("\n");

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let content_hash = format!("{:x}", hasher.finalize());

    Ok(Ok(FetchedSourceContent {
        source,
        canonical_url: canonical_url.to_string(),
        fetched_at: now,
        title,
        author,
        text,
        chunks,
        raw_metadata: Default::default(),
        content_hash,
        cached: false,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_empty_id_rejected() {
        let raw = br#"{"id": "", "title": "x"}"#;
        let err = parse_metadata(raw).unwrap_err();
        assert!(matches!(err, MetadataParseError::MissingId));
    }

    #[test]
    fn metadata_missing_id_rejected() {
        let raw = br#"{"title": "no id key"}"#;
        let err = parse_metadata(raw).unwrap_err();
        assert!(matches!(err, MetadataParseError::MissingId));
    }

    #[test]
    fn metadata_valid_id_parses() {
        let raw = br#"{"id": "abc123", "title": "x", "duration": 42.0}"#;
        let m = parse_metadata(raw).unwrap();
        assert_eq!(m.id, "abc123");
        assert_eq!(m.duration_seconds, Some(42));
    }

    #[test]
    fn json3_parses_events_in_timestamp_order() {
        let raw = r#"{"events":[
            {"tStartMs": 1500, "segs":[{"utf8":"world"}]},
            {"tStartMs": 0,    "segs":[{"utf8":"hello"}]}
        ]}"#;
        let chunks = parse_json3(raw);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "hello");
        assert_eq!(chunks[0].timestamp_seconds, Some(0.0));
        assert_eq!(chunks[1].text, "world");
        assert_eq!(chunks[1].timestamp_seconds, Some(1.5));
    }

    #[test]
    fn json3_collapses_adjacent_duplicates() {
        let raw = r#"{"events":[
            {"tStartMs":0,    "segs":[{"utf8":"the same"}]},
            {"tStartMs":1000, "segs":[{"utf8":"the same"}]},
            {"tStartMs":2000, "segs":[{"utf8":"different"}]}
        ]}"#;
        let chunks = parse_json3(raw);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "the same");
        assert_eq!(chunks[1].text, "different");
        // Order densely renumbered.
        assert_eq!(chunks[0].order, 0);
        assert_eq!(chunks[1].order, 1);
    }

    #[test]
    fn json3_skips_empty_segments() {
        let raw = r#"{"events":[
            {"tStartMs":0,    "segs":[{"utf8":"  "}]},
            {"tStartMs":1000, "segs":[{"utf8":"keep"}]}
        ]}"#;
        let chunks = parse_json3(raw);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "keep");
    }

    #[test]
    fn vtt_parses_basic_cues() {
        let raw = "WEBVTT\n\n00:00:00.000 --> 00:00:02.000\nhello world\n\n00:00:02.500 --> 00:00:04.000\nsecond line\n";
        let chunks = parse_vtt(raw);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "hello world");
        assert_eq!(chunks[0].timestamp_seconds, Some(0.0));
        assert_eq!(chunks[1].text, "second line");
        assert_eq!(chunks[1].timestamp_seconds, Some(2.5));
    }

    #[test]
    fn vtt_strips_inline_tags() {
        let raw = "WEBVTT\n\n00:00:00.000 --> 00:00:01.000\n<c.color>colored</c>\n";
        let chunks = parse_vtt(raw);
        assert_eq!(chunks[0].text, "colored");
    }

    #[test]
    fn vtt_clock_supports_hours_or_no_hours() {
        assert_eq!(parse_vtt_clock("00:00:30.500"), Some(30.5));
        assert_eq!(parse_vtt_clock("01:30.000"), Some(90.0));
        assert_eq!(parse_vtt_clock("1:02:03.456"), Some(3723.456));
    }

    // Minimal mock runner — verifies the orchestration calls each method
    // exactly once and assembles the final FetchedSourceContent correctly.
    struct StaticRunner {
        json3_path: PathBuf,
    }

    #[async_trait]
    impl YtDlpRunner for StaticRunner {
        async fn version_probe(
            &self,
        ) -> Result<std::result::Result<String, ExtractionFailure>> {
            Ok(Ok("yt-dlp 2026.01.01".into()))
        }
        async fn dump_metadata(
            &self,
            _url: &str,
        ) -> Result<std::result::Result<YtDlpMetadata, ExtractionFailure>> {
            Ok(Ok(YtDlpMetadata {
                id: "abc".into(),
                title: Some("Demo".into()),
                uploader: Some("Up".into()),
                channel: Some("Ch".into()),
                duration_seconds: Some(120),
                description: None,
            }))
        }
        async fn download_subs(
            &self,
            _url: &str,
            _video_id: &str,
            _temp_dir: &Path,
        ) -> Result<std::result::Result<PathBuf, ExtractionFailure>> {
            Ok(Ok(self.json3_path.clone()))
        }
    }

    #[tokio::test]
    async fn orchestration_assembles_youtube_source() {
        let tmp = tempfile::tempdir().unwrap();
        let json3_path = tmp.path().join("abc.en.json3");
        tokio::fs::write(
            &json3_path,
            r#"{"events":[{"tStartMs":0,"segs":[{"utf8":"hi"}]}]}"#,
        )
        .await
        .unwrap();

        let runner = StaticRunner { json3_path };
        let result = fetch_youtube(
            "https://www.youtube.com/watch?v=abc",
            "abc",
            &runner,
            tmp.path(),
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(result.title.as_deref(), Some("Demo"));
        assert_eq!(result.author.as_deref(), Some("Up"));
        match result.source {
            Source::Youtube(yt) => {
                assert_eq!(yt.video_id, "abc");
                assert_eq!(yt.duration_seconds, Some(120));
            }
            _ => panic!("expected Source::Youtube"),
        }
        assert_eq!(result.chunks.len(), 1);
        assert_eq!(result.chunks[0].text, "hi");
    }

    struct DepMissingRunner;
    #[async_trait]
    impl YtDlpRunner for DepMissingRunner {
        async fn version_probe(
            &self,
        ) -> Result<std::result::Result<String, ExtractionFailure>> {
            Ok(Err(ExtractionFailure::DependencyMissing {
                name: "yt-dlp".into(),
                install_hint: "brew install yt-dlp".into(),
            }))
        }
        async fn dump_metadata(
            &self,
            _: &str,
        ) -> Result<std::result::Result<YtDlpMetadata, ExtractionFailure>> {
            unreachable!()
        }
        async fn download_subs(
            &self,
            _: &str,
            _: &str,
            _: &Path,
        ) -> Result<std::result::Result<PathBuf, ExtractionFailure>> {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn missing_yt_dlp_short_circuits_to_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let runner = DepMissingRunner;
        let result = fetch_youtube("u", "v", &runner, tmp.path()).await.unwrap();
        match result.unwrap_err() {
            ExtractionFailure::DependencyMissing { name, .. } => assert_eq!(name, "yt-dlp"),
            other => panic!("expected DependencyMissing, got {other:?}"),
        }
    }
}
