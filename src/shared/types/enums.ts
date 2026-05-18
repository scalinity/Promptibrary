// Shared enums per spec §4. Casing here is the wire casing — Rust derives
// `#[serde(rename_all = ...)]` to match.

export type LaunchDestination = "claude_code_cli";

export type ClaudeModelId =
  | "claude-sonnet-4-6"
  | "claude-opus-4-7"
  | "sonnet"
  | "opus";

export type ExtractionMode = "standard" | "deep";

export type ClaudePermissionMode =
  | "default"
  | "acceptEdits"
  | "plan"
  | "auto"
  | "dontAsk"
  | "bypassPermissions";

export type VerifierMode = "off" | "manual_ultrareview_after_run";

export type TagColorSlug =
  | "gray"
  | "red"
  | "orange"
  | "yellow"
  | "green"
  | "blue"
  | "cyan"
  | "purple"
  | "pink";

export type RunStatus =
  | "created"
  | "validating"
  | "spawning"
  | "running"
  | "stopping"
  | "succeeded"
  | "failed"
  | "canceled"
  | "vault_unavailable"
  | "transcript_spooled";

export type SourceKind = "manual" | "youtube" | "x_twitter" | "article";
