#!/usr/bin/env bash
# Creates the src-tauri/src/ scaffold per spec §3.
# Each leaf .rs file is a doc-commented module stub describing its role.
# Idempotent: only writes files when missing.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

S="src-tauri/src"

write() {
  local path="$1"
  local content="$2"
  if [[ ! -f "$path" ]]; then
    mkdir -p "$(dirname "$path")"
    printf "%s" "$content" >"$path"
    echo "wrote $path"
  fi
}

stub_rs() {
  local path="$1"
  local role="$2"
  write "$path" "//! ${role}
//!
//! L0 scaffold — module stub. Real implementation lands in a later layer.
"
}

# --- domain/ (real types land in L0.11) ---

# --- vault/ ---
stub_rs $S/vault/paths.rs "Canonical vault-relative paths; resolves prompt + run file paths."
stub_rs $S/vault/scanner.rs "Cold scans and incremental scans of the vault."
stub_rs $S/vault/markdown.rs "Markdown body/frontmatter split."
stub_rs $S/vault/frontmatter.rs "Frontmatter parse/validation for prompts and run transcripts."
stub_rs $S/vault/writer.rs "Atomic writes to the vault: prompts, run transcripts, archived files."
stub_rs $S/vault/watcher.rs "\`notify\` watcher lifecycle and debounced rescans."
stub_rs $S/vault/repair.rs "Orphan and spool transcript recovery."
write $S/vault/mod.rs "//! Vault — Markdown vault layout, scans, atomic writes, and the file watcher.
//!
//! L0 scaffold — submodules are stubs until L1.

pub mod paths;
pub mod scanner;
pub mod markdown;
pub mod frontmatter;
pub mod writer;
pub mod watcher;
pub mod repair;
"

# --- index/ ---
stub_rs $S/index/db.rs "SQLite pool + PRAGMA configuration."
stub_rs $S/index/prompts_repo.rs "Prompt index row CRUD."
stub_rs $S/index/runs_repo.rs "Run row CRUD."
stub_rs $S/index/telemetry_repo.rs "Launch telemetry aggregates."
stub_rs $S/index/fts.rs "FTS5 indexing and search."
stub_rs $S/index/embeddings.rs "Local embedding generation and vector search."
stub_rs $S/index/reindex.rs "Cold and incremental reindex orchestration."
# migrations.rs is written by L0.12 with real content.

# --- variables/ ---
stub_rs $S/variables/lexer.rs "Tokenizes \`{{...}}\` per spec §5 EBNF."
stub_rs $S/variables/parser.rs "Parses typed variable references; computes byte and UTF-16 offsets in one pass."
stub_rs $S/variables/renderer.rs "Renders the final prompt by replacing end-to-start to preserve offsets."
stub_rs $S/variables/validation.rs "Per-type validation rules for resolved variable values."
stub_rs $S/variables/tests.rs "Golden tests for the §15 variable matrix."
write $S/variables/mod.rs "//! Variables — variable syntax lexer, parser, renderer, validation.
//!
//! L0 scaffold — submodules are stubs until L1.

pub mod lexer;
pub mod parser;
pub mod renderer;
pub mod validation;
#[cfg(test)]
mod tests;
"

# --- extraction/ ---
stub_rs $S/extraction/detect.rs "Source URL classification (manual, youtube, x_twitter, article)."
stub_rs $S/extraction/normalize.rs "Converts fetched source content to extraction input."
stub_rs $S/extraction/anthropic.rs "Typed Anthropic Messages API client."
stub_rs $S/extraction/prompts.rs "Exact LLM prompt templates (byte-equal extraction system prompt)."
stub_rs $S/extraction/response.rs "Validates LLM JSON output."
stub_rs $S/extraction/cache.rs "Source fetch and extraction cache."
stub_rs $S/extraction/rate_limit.rs "Per-provider rate limiter."
write $S/extraction/mod.rs "//! Extraction — source detection, fetching, normalization, Anthropic call, response validation.
//!
//! L0 scaffold — submodules are stubs until L4.

pub mod detect;
pub mod fetchers;
pub mod normalize;
pub mod anthropic;
pub mod prompts;
pub mod response;
pub mod cache;
pub mod rate_limit;
"
stub_rs $S/extraction/fetchers/youtube.rs "YouTube transcript fetcher (yt-dlp shell)."
stub_rs $S/extraction/fetchers/x_twitter.rs "X/Twitter thread fetcher."
stub_rs $S/extraction/fetchers/article.rs "Generic web article fetcher (deterministic subtree selection only)."
write $S/extraction/fetchers/mod.rs "//! Source-specific fetchers.

pub mod youtube;
pub mod x_twitter;
pub mod article;
"

# --- launch/ ---
stub_rs $S/launch/claude_cli.rs "Builds the \`claude\` CLI invocation (no shell wrap, no \`--max-turns\`)."
stub_rs $S/launch/pty_pool.rs "Active PTY registry keyed by RunId."
stub_rs $S/launch/pty_session.rs "Single-run PTY process lifecycle."
stub_rs $S/launch/prompt_injector.rs "Event-driven bracketed-paste prompt injection."
stub_rs $S/launch/transcript_writer.rs "Streaming transcript writes to vault + spool."
stub_rs $S/launch/process_probe.rs "Probe for \`claude\`, \`yt-dlp\`, \`git\` availability."
stub_rs $S/launch/signals.rs "Stop escalation: SIGINT → SIGTERM → SIGKILL."
write $S/launch/mod.rs "//! Launch — direct PTY spawn pipeline for the Claude Code CLI.
//!
//! L0 scaffold — submodules are stubs until L3.

pub mod claude_cli;
pub mod pty_pool;
pub mod pty_session;
pub mod prompt_injector;
pub mod transcript_writer;
pub mod process_probe;
pub mod signals;
"

# --- terminal/ ---
stub_rs $S/terminal/ansi.rs "ANSI normalization and 24-bit color preservation."
stub_rs $S/terminal/event_bridge.rs "Emits terminal/run events to the frontend."
stub_rs $S/terminal/resize.rs "PTY resize handling."
stub_rs $S/terminal/backpressure.rs "Output buffering and drop policy."
write $S/terminal/mod.rs "//! Terminal — ANSI normalization, event bridge, resize, backpressure.
//!
//! L0 scaffold — submodules are stubs until L3.

pub mod ansi;
pub mod event_bridge;
pub mod resize;
pub mod backpressure;
"

# --- git/ ---
stub_rs $S/git/repo.rs "Validates and opens the vault Git repo."
stub_rs $S/git/history.rs "File history."
stub_rs $S/git/diff.rs "Commit and file diffs."
stub_rs $S/git/revert.rs "Reverts a prompt file to a selected commit blob."
write $S/git/mod.rs "//! Git — vault Git repo introspection and per-file history/diff/revert.
//!
//! L0 scaffold — submodules are stubs until L5.

pub mod repo;
pub mod history;
pub mod diff;
pub mod revert;
"

# --- settings/ ---
stub_rs $S/settings/local_store.rs "Local JSON settings outside vault."
stub_rs $S/settings/vault_store.rs "Vault-local non-secret settings."
stub_rs $S/settings/keychain.rs "Secret storage (keyring 4)."
stub_rs $S/settings/validation.rs "Settings validation."
write $S/settings/mod.rs "//! Settings — local + vault settings stores, keychain, validation.
//!
//! L0 scaffold — submodules are stubs until L5.

pub mod local_store;
pub mod vault_store;
pub mod keychain;
pub mod validation;
"

# --- system/ ---
stub_rs $S/system/paths.rs "OS-specific app paths (app support, cache, runtime)."
stub_rs $S/system/shell.rs "Login shell and PATH probing."
stub_rs $S/system/os.rs "macOS / Linux helpers (reveal in Finder, open Terminal.app)."
stub_rs $S/system/diagnostics.rs "Dependency checks."
write $S/system/mod.rs "//! System — OS-specific helpers.
//!
//! L0 scaffold — submodules are stubs until L5.

pub mod paths;
pub mod shell;
pub mod os;
pub mod diagnostics;
"

# --- util/ ---
stub_rs $S/util/atomic_write.rs "Atomic write helper: tmp.<pid> → fsync → rename."
stub_rs $S/util/debounce.rs "Debouncing helper used by the watcher."
stub_rs $S/util/fs.rs "Filesystem helpers."
stub_rs $S/util/json.rs "Typed JSON helpers."
stub_rs $S/util/slug.rs "Slug derivation from prompt titles."
stub_rs $S/util/yaml.rs "YAML helpers shared by frontmatter."
write $S/util/mod.rs "//! Util — small reusable helpers.
//!
//! L0 scaffold — submodules are stubs until used.

pub mod atomic_write;
pub mod debounce;
pub mod fs;
pub mod json;
pub mod slug;
pub mod yaml;
"

# --- app_state.rs ---
write $S/app_state.rs "//! Shared Tauri state container.
//!
//! L0 scaffold — \`AppServices\` is the holder for indexes, settings, PTY pool,
//! etc. that subsequent layers populate. Today it holds nothing.

#[derive(Default)]
pub struct AppServices {
    // TODO(L1+): SQLite pool, vault watcher, settings store, secrets keychain
    // handle, PTY pool, embedding service, etc. all land here.
}

impl AppServices {
    pub fn new() -> Self {
        Self::default()
    }
}

pub type ManagedState = std::sync::Arc<AppServices>;
"

# --- time.rs ---
write $S/time.rs "//! Time helpers.
//!
//! \`now_iso8601\` is the standard timestamp source used in transcript frontmatter,
//! \`Run.started_at\`, and Prompt \`created_at\`/\`updated_at\` fields.

use chrono::{DateTime, SecondsFormat, Utc};

pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

pub fn to_iso8601(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn now_iso8601() -> String {
    to_iso8601(now_utc())
}
"

echo "rust scaffold complete."
