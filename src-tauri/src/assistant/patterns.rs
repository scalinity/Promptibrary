//! Bundled Fabric AI system prompts for the in-app assistant.
//!
//! Three patterns ship with V1. **Improve Prompt** is the default and is used
//! when the user opens the assistant drawer without an explicit selection.
//!
//! ## Byte-equal contract
//!
//! Each pattern's text lives in `docs/spec-snippets/fabric-*.md` and is
//! pulled into the binary via `include_str!`. That makes drift a build error
//! rather than a runtime surprise — the same approach used by
//! `extraction::prompts::EXTRACTION_SYSTEM_PROMPT`. The sha256 hashes are
//! recorded next to each constant so an upstream change (or local edit to
//! the snippet file) is auditable.
//!
//! ## Provenance
//!
//! - `improve_prompt` — upstream `danielmiessler/Fabric@main`, path
//!   `data/patterns/improve_prompt/system.md`.
//! - `improve_writing` — upstream `danielmiessler/Fabric@main`, path
//!   `data/patterns/improve_writing/system.md`.
//! - `improve_prompt_xml` — **user-authored**, not in upstream Fabric.
//!   Sourced from the locally-installed copy at
//!   `~/.config/fabric/patterns/improve_prompt_xml/system.md` at the time
//!   the snippet was committed. Treat the bundled file as canonical going
//!   forward; an upstream Fabric version, if it ever lands, does not
//!   automatically supersede.

use serde::{Deserialize, Serialize};

/// Improve Prompt — upstream Fabric pattern. Default.
///
/// sha256: `81a1d238b0087d62a2e0b3cfb74a9fc9f62bd8021c4ce8762ba7432371f46886`
pub const IMPROVE_PROMPT_SYSTEM: &str =
    include_str!("../../../docs/spec-snippets/fabric-improve-prompt.md");

/// Improve Prompt XML — user-authored variant of Improve Prompt that
/// restructures the output around XML tags instead of Markdown headings.
///
/// sha256: `059830ad5aa74f1f6d2dcaadeea8fad8c4d09507c0c4ac04507f8352d9e536b0`
pub const IMPROVE_PROMPT_XML_SYSTEM: &str =
    include_str!("../../../docs/spec-snippets/fabric-improve-prompt-xml.md");

/// Improve Writing — upstream Fabric pattern. Refines clarity, coherence,
/// grammar, and style of the input text without changing meaning.
///
/// sha256: `98288d12a0aa175b91582dc7d750378a3a219330a0ee7b2719556dd3263c87e8`
pub const IMPROVE_WRITING_SYSTEM: &str =
    include_str!("../../../docs/spec-snippets/fabric-improve-writing.md");

/// The three V1 assistant patterns. Wire format mirrors the
/// snake_case pattern IDs Fabric uses on disk so frontend ↔ Rust ↔ user-
/// installed pattern paths line up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantPattern {
    /// Default. Improves a prompt into a stronger version.
    ImprovePrompt,
    /// Improves a prompt and restructures it with XML tags (user-authored).
    ImprovePromptXml,
    /// Improves writing quality (clarity, coherence, grammar, style).
    ImproveWriting,
}

impl Default for AssistantPattern {
    fn default() -> Self {
        Self::ImprovePrompt
    }
}

impl AssistantPattern {
    /// The system-prompt body the assistant uses to prime Claude.
    pub fn system_prompt(self) -> &'static str {
        match self {
            Self::ImprovePrompt => IMPROVE_PROMPT_SYSTEM,
            Self::ImprovePromptXml => IMPROVE_PROMPT_XML_SYSTEM,
            Self::ImproveWriting => IMPROVE_WRITING_SYSTEM,
        }
    }

    /// Human-readable label shown in the pattern selector.
    pub fn label(self) -> &'static str {
        match self {
            Self::ImprovePrompt => "Improve Prompt",
            Self::ImprovePromptXml => "Improve Prompt XML",
            Self::ImproveWriting => "Improve Writing",
        }
    }

    /// Snake-case pattern ID. Matches the wire serialization and Fabric's
    /// on-disk directory name.
    pub fn id(self) -> &'static str {
        match self {
            Self::ImprovePrompt => "improve_prompt",
            Self::ImprovePromptXml => "improve_prompt_xml",
            Self::ImproveWriting => "improve_writing",
        }
    }

    /// All variants in display order — drives the frontend dropdown.
    pub fn all() -> [AssistantPattern; 3] {
        [
            Self::ImprovePrompt,
            Self::ImprovePromptXml,
            Self::ImproveWriting,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `include_str!` already guarantees compile-time byte-equality between
    /// each constant and its source file. These guards catch accidental
    /// truncation or wholesale replacement of a snippet — for example, an
    /// editor stripping CRLF / trailing newlines or someone replacing the
    /// file with the wrong pattern's content.
    #[test]
    fn improve_prompt_sentinel_and_length() {
        assert!(IMPROVE_PROMPT_SYSTEM.starts_with("# IDENTITY and PURPOSE\n"));
        assert!(IMPROVE_PROMPT_SYSTEM.contains("expert LLM prompt writing service"));
        assert!(IMPROVE_PROMPT_SYSTEM.contains("START PROMPT WRITING KNOWLEDGE"));
        // Upstream is ~34.7 KB; guard against silent truncation.
        assert!(
            IMPROVE_PROMPT_SYSTEM.len() > 30_000,
            "improve_prompt looks truncated: {} bytes",
            IMPROVE_PROMPT_SYSTEM.len()
        );
    }

    #[test]
    fn improve_prompt_xml_sentinel_and_length() {
        assert!(IMPROVE_PROMPT_XML_SYSTEM.starts_with("# IDENTITY and PURPOSE\n"));
        assert!(IMPROVE_PROMPT_XML_SYSTEM.contains("XML tags"));
        assert!(IMPROVE_PROMPT_XML_SYSTEM.contains("ALLOWED TAG VOCABULARY"));
        assert!(
            IMPROVE_PROMPT_XML_SYSTEM.len() > 7_000,
            "improve_prompt_xml looks truncated: {} bytes",
            IMPROVE_PROMPT_XML_SYSTEM.len()
        );
    }

    #[test]
    fn improve_writing_sentinel_and_length() {
        assert!(IMPROVE_WRITING_SYSTEM.starts_with("# IDENTITY and PURPOSE\n"));
        assert!(IMPROVE_WRITING_SYSTEM.contains("writing expert"));
        assert!(
            IMPROVE_WRITING_SYSTEM.len() > 500,
            "improve_writing looks truncated: {} bytes",
            IMPROVE_WRITING_SYSTEM.len()
        );
    }

    #[test]
    fn pattern_accessors_round_trip() {
        for p in AssistantPattern::all() {
            assert!(!p.system_prompt().is_empty());
            assert!(!p.label().is_empty());
            assert!(!p.id().is_empty());
        }
    }

    #[test]
    fn pattern_default_is_improve_prompt() {
        assert_eq!(AssistantPattern::default(), AssistantPattern::ImprovePrompt);
    }

    #[test]
    fn pattern_ids_are_snake_case() {
        assert_eq!(AssistantPattern::ImprovePrompt.id(), "improve_prompt");
        assert_eq!(
            AssistantPattern::ImprovePromptXml.id(),
            "improve_prompt_xml"
        );
        assert_eq!(AssistantPattern::ImproveWriting.id(), "improve_writing");
    }

    #[test]
    fn pattern_wire_serialization_matches_id() {
        for p in AssistantPattern::all() {
            let json = serde_json::to_string(&p).expect("serialize");
            // Wire form: a JSON string like `"improve_prompt"`.
            assert_eq!(json, format!("\"{}\"", p.id()));
            let back: AssistantPattern = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, p);
        }
    }
}
