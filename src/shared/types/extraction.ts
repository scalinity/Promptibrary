// L4 extraction wire types — mirror of `src-tauri/src/extraction/types.rs`.
//
// Spec §6. Discriminated unions use `kind` to mirror Rust's `#[serde(tag = "kind")]`.

import type { IsoDateTime } from "./ids";
import type {
  ClaudeModelId,
  ClaudePermissionMode,
  ExtractionMode,
  SourceKind,
  VerifierMode,
} from "./enums";
import type { Source } from "./source";
import type { Variable } from "./variable";

export type { ExtractionMode };

// ─── Source detection ────────────────────────────────────────────────────────

export type UnsupportedSourceReason =
  | "invalid_url"
  | "unsupported_scheme"
  | "unsupported_host";

export type SourceDetection =
  | { kind: "youtube"; canonicalUrl: string; videoId: string }
  | {
      kind: "x_twitter";
      canonicalUrl: string;
      postId: string;
      username: string | null;
    }
  | { kind: "article"; canonicalUrl: string; hostname: string }
  | { kind: "unsupported"; reason: UnsupportedSourceReason };

// ─── Fetched source content ──────────────────────────────────────────────────

export type SourceChunkKind =
  | "title"
  | "metadata"
  | "transcript"
  | "post"
  | "article"
  | "code"
  | "quote";

export interface SourceChunk {
  kind: SourceChunkKind;
  order: number;
  text: string;
  url: string | null;
  timestampSeconds: number | null;
}

/**
 * Image URL discovered alongside the textual content (OG meta tags, post
 * media, inline `<img>` tags). Forwarded to the LLM as Anthropic
 * `image` content blocks so prompts embedded in screenshots are
 * extractable. SCA-967.
 */
export interface SourceImage {
  url: string;
  alt: string | null;
}

export interface FetchedSourceContent {
  source: Source;
  canonicalUrl: string;
  fetchedAt: IsoDateTime;
  title: string | null;
  author: string | null;
  text: string;
  chunks: SourceChunk[];
  /**
   * Image URLs attached to this source. May be omitted on cache rows
   * created before SCA-967 — treat absent / undefined as `[]`.
   */
  images?: SourceImage[];
  rawMetadata: Record<string, string | number | boolean | null>;
  contentHash: string;
  /** True when this preview came from the SQLite cache rather than a fresh fetch. */
  cached?: boolean;
}

// ─── LLM input + output ──────────────────────────────────────────────────────

export interface ExtractionInput {
  source: Source;
  title: string | null;
  author: string | null;
  url: string;
  text: string;
  chunks: SourceChunk[];
  images?: SourceImage[];
  maxCandidateCount: number;
  extractionMode: ExtractionMode;
}

export type CandidateConfidence = "low" | "medium" | "high";

export interface SourceAnchor {
  chunkOrder: number;
  quote: string;
  reason: string;
}

export interface LaunchDefaultsPatch {
  destination?: "claude_code_cli" | null;
  model?: ClaudeModelId | null;
  verifierMode?: VerifierMode | null;
  permissionMode?: ClaudePermissionMode | null;
  allowedTools?: string[] | null;
  disallowedTools?: string[] | null;
  mcpConfigPaths?: string[] | null;
  strictMcpConfig?: boolean | null;
  appendSystemPrompt?: string | null;
  maxTurns?: number | null;
}

export interface CandidatePrompt {
  title: string;
  summary: string;
  body: string;
  tags: string[];
  variables: Variable[];
  launchDefaultsPatch: LaunchDefaultsPatch;
  confidence: CandidateConfidence;
  rationale: string;
  sourceAnchors: SourceAnchor[];
}

export interface ExtractionResponse {
  schemaVersion: 1;
  candidates: CandidatePrompt[];
}

// ─── Failures ────────────────────────────────────────────────────────────────

export type ExtractionFailure =
  | { kind: "network_unavailable"; message: string }
  | { kind: "unsupported_source"; reason: UnsupportedSourceReason }
  | { kind: "dependency_missing"; name: string; installHint: string }
  | { kind: "transcript_unavailable" }
  | { kind: "paywall_likely"; preview: string }
  | { kind: "rate_limited"; provider: string; resetAt: IsoDateTime | null }
  | { kind: "anthropic_key_missing" }
  | { kind: "anthropic_auth_invalid" }
  | { kind: "llm_refusal"; excerpt: string }
  | { kind: "malformed_model_output"; raw: string; errors: string[] }
  | { kind: "extraction_failed"; reason: string };

// Re-exported so consumers don't have to remember which file `SourceKind` lives in.
export type { SourceKind };
