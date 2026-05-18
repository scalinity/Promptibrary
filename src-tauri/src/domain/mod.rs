//! Domain types — Rust mirror of `src/shared/types/*` per spec §4.
//!
//! L0 populates the submodules below in `L0.11`. They live here so other
//! modules can import them by path even while bodies are stubs.

pub mod prompt;
pub mod variable;
pub mod launch;
pub mod run;
pub mod tag;
pub mod source;
pub mod settings;
pub mod git;
pub mod search;
pub mod transcript;

#[cfg(test)]
mod tests {
    use super::prompt::{ClaudeModelId, ClaudePermissionMode};
    use super::source::{Source, YouTubeSource};
    use super::variable::{BoolVariable, Variable, VariableSource};

    #[test]
    fn claude_model_id_serializes_kebab_case() {
        let json = serde_json::to_string(&ClaudeModelId::ClaudeSonnet46).unwrap();
        assert_eq!(json, "\"claude-sonnet-4-6\"");
        let parsed: ClaudeModelId = serde_json::from_str("\"claude-opus-4-7\"").unwrap();
        assert_eq!(parsed, ClaudeModelId::ClaudeOpus47);
    }

    #[test]
    fn permission_mode_serializes_camel_case() {
        let json = serde_json::to_string(&ClaudePermissionMode::AcceptEdits).unwrap();
        assert_eq!(json, "\"acceptEdits\"");
        let parsed: ClaudePermissionMode = serde_json::from_str("\"bypassPermissions\"").unwrap();
        assert_eq!(parsed, ClaudePermissionMode::BypassPermissions);
    }

    #[test]
    fn variable_uses_type_discriminator_snake_case() {
        let var = Variable::Bool(BoolVariable {
            key: "write_tests".into(),
            label: "Write missing tests".into(),
            description: None,
            required: false,
            default_value: Some(true),
            order: 5,
            source: VariableSource::Frontmatter,
            render_true: "yes".into(),
            render_false: "no".into(),
        });
        let json = serde_json::to_value(&var).unwrap();
        assert_eq!(json["type"], "bool");
        assert_eq!(json["renderTrue"], "yes");
        assert_eq!(json["renderFalse"], "no");
        assert_eq!(json["defaultValue"], true);
        // Round trip.
        let back: Variable = serde_json::from_value(json).unwrap();
        match back {
            Variable::Bool(b) => assert_eq!(b.render_true, "yes"),
            _ => panic!("expected Bool variant"),
        }
    }

    #[test]
    fn source_uses_kind_discriminator_snake_case() {
        let s = Source::Youtube(YouTubeSource {
            origin_url: Some("https://youtube.com/x".into()),
            title: None,
            author: None,
            fetched_at: None,
            content_hash: None,
            video_id: "abc".into(),
            channel_name: None,
            transcript_language: None,
            duration_seconds: None,
        });
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["kind"], "youtube");
        assert_eq!(json["videoId"], "abc");
        assert_eq!(json["originUrl"], "https://youtube.com/x");
        let _back: Source = serde_json::from_value(json).unwrap();
    }
}
