#!/usr/bin/env bash
# Creates the src/ scaffold per spec §3.
# Idempotent — only writes a file when missing, so re-runs won't clobber real content.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

write() {
  local path="$1"
  local content="$2"
  if [[ ! -f "$path" ]]; then
    mkdir -p "$(dirname "$path")"
    printf "%s" "$content" >"$path"
    echo "wrote $path"
  fi
}

stub_ts() {
  local path="$1"
  local role="$2"
  write "$path" "// ${role}
//
// L0 scaffold — module stub for the src/ tree from spec §3.
// Real implementation lands in a later layer; this file exists so the tree
// matches the spec and TypeScript can resolve future imports.

export {};
"
}

stub_tsx_component() {
  local path="$1"
  local role="$2"
  local name="$3"
  write "$path" "// ${role}
//
// L0 scaffold — component stub. Real implementation in a later layer.

export function ${name}(): null {
  return null;
}
"
}

stub_tsx_route() {
  local path="$1"
  local role="$2"
  local name="$3"
  write "$path" "// ${role}
//
// L0 scaffold — route stub. Real implementation in a later layer.

export function ${name}(): null {
  return null;
}
"
}

stub_ts_hook() {
  local path="$1"
  local role="$2"
  local name="$3"
  write "$path" "// ${role}
//
// L0 scaffold — hook stub. Real implementation in a later layer.

export function ${name}(): never {
  throw new Error('${name} is not implemented yet (L0 scaffold).');
}
"
}

# --- top-level ---
stub_ts src/vite-env.d.ts "Vite client + Tauri ambient types."

# --- shared/api/ ---
stub_ts src/shared/api/ipc.ts "Typed wrapper around @tauri-apps/api/core invoke for IPC commands."
stub_ts src/shared/api/events.ts "Tauri event subscriptions (run://output, vault://changed, index://progress, extract://progress)."
stub_ts src/shared/api/errors.ts "Frontend mirror of Rust AppError / AppErrorKind."
stub_ts src/shared/api/queryKeys.ts "TanStack Query key factory."

# --- shared/types/ are populated in L0.7 ---

# --- shared/ui/ (wrapper layer; shadcn/ is empty until L0.8) ---
mkdir -p src/shared/ui/shadcn
stub_tsx_component src/shared/ui/app-shell.tsx "Top-level app chrome wrapper (sidebar + topbar + outlet)." AppShell
stub_tsx_component src/shared/ui/sidebar.tsx "Left navigation sidebar." Sidebar
stub_tsx_component src/shared/ui/topbar.tsx "Top toolbar (vault status + global search trigger)." Topbar
stub_tsx_component src/shared/ui/keyboard-shortcut.tsx "Renders ⌘/⇧/⌥/⌃ + key chord glyphs." KeyboardShortcut
stub_tsx_component src/shared/ui/status-pill.tsx "Status badge pill (running, succeeded, failed, etc.)." StatusPill
stub_tsx_component src/shared/ui/empty-state.tsx "Empty-state placeholder block." EmptyState
stub_tsx_component src/shared/ui/error-callout.tsx "Error callout banner mapping AppErrorKind → message." ErrorCallout
stub_tsx_component src/shared/ui/loading-spinner.tsx "Minimal indeterminate spinner." LoadingSpinner
stub_tsx_component src/shared/ui/tag-chip.tsx "Single tag chip with optional remove affordance." TagChip
stub_tsx_component src/shared/ui/confirm-dialog.tsx "Typed-confirmation dialog (used for destructive actions)." ConfirmDialog
stub_tsx_component src/shared/ui/file-path-field.tsx "Read-only path field with reveal-in-Finder action." FilePathField

# --- shared/lib/ ---
stub_ts src/shared/lib/cn.ts "Tailwind class-merger; thin wrapper over clsx + tailwind-merge."
stub_ts src/shared/lib/dates.ts "ISO-datetime helpers for display formatting."
stub_ts src/shared/lib/paths.ts "Frontend-side path utilities (vault-relative joining, slug derivation)."
stub_ts src/shared/lib/ansi.ts "ANSI sequence helpers for transcript rendering."
stub_ts src/shared/lib/shortcuts.ts "Keyboard shortcut registration helpers."
stub_ts src/shared/lib/validation.ts "Frontend-side validation helpers mirroring backend types."

# --- features/library ---
stub_tsx_route src/features/library/routes/library-route.tsx "Route: /  Prompt library home." LibraryRoute
stub_tsx_component src/features/library/components/prompt-list.tsx "Renders the indexed prompt list." PromptList
stub_tsx_component src/features/library/components/prompt-card.tsx "Single prompt card with launch CTA." PromptCard
stub_tsx_component src/features/library/components/tag-filter-bar.tsx "Tag filter chip row." TagFilterBar
stub_tsx_component src/features/library/components/library-toolbar.tsx "Library toolbar: search, sort, new prompt." LibraryToolbar
stub_tsx_component src/features/library/components/telemetry-mini-stats.tsx "Mini stats row at top of library." TelemetryMiniStats
stub_ts_hook src/features/library/hooks/use-prompts.ts "Query hook fetching prompt list via list_prompts." usePrompts
stub_ts_hook src/features/library/hooks/use-library-filters.ts "Computes filtered/sorted prompt list given store state." useLibraryFilters
stub_ts src/features/library/stores/library-store.ts "Zustand store for library-local filter/sort/layout state."

# --- features/prompt ---
stub_tsx_route src/features/prompt/routes/prompt-route.tsx "Route: /prompt/:id  Prompt editor." PromptRoute
stub_tsx_component src/features/prompt/components/prompt-editor.tsx "Container for the prompt editor view." PromptEditor
stub_tsx_component src/features/prompt/components/prompt-frontmatter-panel.tsx "Side panel for title/tags/source/launch defaults." PromptFrontmatterPanel
stub_tsx_component src/features/prompt/components/prompt-body-editor.tsx "CodeMirror 6 body editor with variable-ref decorations." PromptBodyEditor
stub_tsx_component src/features/prompt/components/variable-reference-list.tsx "Right rail listing parsed variable references." VariableReferenceList
stub_tsx_component src/features/prompt/components/launch-profile-panel.tsx "Inline launch-profile editor strip." LaunchProfilePanel
stub_tsx_component src/features/prompt/components/prompt-history-panel.tsx "Git history panel for the prompt file." PromptHistoryPanel
stub_tsx_component src/features/prompt/components/prompt-diff-view.tsx "Read-only diff between two prompt versions." PromptDiffView
stub_tsx_component src/features/prompt/components/export-menu.tsx "Export-as menu (markdown, JSON)." ExportMenu
stub_ts_hook src/features/prompt/hooks/use-prompt.ts "Query hook for a single prompt by ID." usePrompt
stub_ts_hook src/features/prompt/hooks/use-prompt-save.ts "Mutation hook for atomic prompt save." usePromptSave
stub_ts_hook src/features/prompt/hooks/use-variable-parse.ts "Hook that calls parse_variables for the current body." useVariableParse
stub_ts_hook src/features/prompt/hooks/use-git-history.ts "Hook fetching Git history for the prompt file." useGitHistory
stub_ts src/features/prompt/stores/prompt-editor-store.ts "Zustand store for unsaved prompt drafts."

# --- features/launch ---
stub_tsx_component src/features/launch/components/launch-drawer.tsx "Right-drawer launch composer." LaunchDrawer
stub_tsx_component src/features/launch/components/variable-form.tsx "Form rendering one VariableControl per parsed variable." VariableForm
stub_tsx_component src/features/launch/components/variable-control.tsx "Type-specific control for a single Variable." VariableControl
stub_tsx_component src/features/launch/components/inline-tweak-editor.tsx "Ephemeral 'tweak before launch' body editor." InlineTweakEditor
stub_tsx_component src/features/launch/components/working-dir-picker.tsx "Working directory picker with native dialog." WorkingDirPicker
stub_tsx_component src/features/launch/components/permission-flags-editor.tsx "Editor for ClaudePermissionRule allowed/disallowed lists." PermissionFlagsEditor
stub_tsx_component src/features/launch/components/mcp-config-editor.tsx "Editor for MCP config paths." McpConfigEditor
stub_tsx_component src/features/launch/components/launch-button.tsx "Big launch CTA with validation state." LaunchButton
stub_ts_hook src/features/launch/hooks/use-launch-draft.ts "Hook bridging launchDraftStore + parsed variables." useLaunchDraft
stub_ts_hook src/features/launch/hooks/use-launch-validation.ts "Hook calling validate_launch_inputs." useLaunchValidation
stub_ts_hook src/features/launch/hooks/use-start-launch.ts "Mutation hook calling start_launch and navigating to /run/:id." useStartLaunch
stub_ts src/features/launch/stores/launch-draft-store.ts "Zustand store for per-prompt ephemeral launch state."

# --- features/terminal ---
stub_tsx_route src/features/terminal/routes/run-route.tsx "Route: /run/:id  Live terminal or read-only transcript." RunRoute
stub_tsx_component src/features/terminal/components/terminal-pane.tsx "xterm.js host element attaching to the live run." TerminalPane
stub_tsx_component src/features/terminal/components/run-header.tsx "Run header with prompt title, status pill, and actions." RunHeader
stub_tsx_component src/features/terminal/components/run-status-bar.tsx "Bottom status bar: working dir, model, permission mode." RunStatusBar
stub_tsx_component src/features/terminal/components/transcript-viewer.tsx "Read-only transcript renderer for completed runs." TranscriptViewer
stub_tsx_component src/features/terminal/components/ansi-transcript.tsx "ANSI-aware transcript section renderer." AnsiTranscript
stub_tsx_component src/features/terminal/components/terminal-toolbar.tsx "Top toolbar: stop, search, copy, reveal." TerminalToolbar
stub_ts_hook src/features/terminal/hooks/use-xterm.ts "Hook owning the xterm.js Terminal instance lifecycle." useXterm
stub_ts_hook src/features/terminal/hooks/use-terminal-events.ts "Subscribes to run://output and run://status events." useTerminalEvents
stub_ts_hook src/features/terminal/hooks/use-run.ts "Query hook for run metadata." useRun
stub_ts_hook src/features/terminal/hooks/use-transcript.ts "Query hook for completed-run transcript content." useTranscript
stub_ts src/features/terminal/stores/terminal-store.ts "Zustand store for active terminal attachment state."

# --- features/import ---
stub_tsx_route src/features/import/routes/import-route.tsx "Route: /import  Source detection → preview → extraction." ImportRoute
stub_tsx_component src/features/import/components/source-url-form.tsx "Source URL input with detection." SourceUrlForm
stub_tsx_component src/features/import/components/source-preview.tsx "Preview of normalized source content." SourcePreview
stub_tsx_component src/features/import/components/extraction-mode-toggle.tsx "Standard / Deep mode toggle." ExtractionModeToggle
stub_tsx_component src/features/import/components/candidate-list.tsx "List of extracted candidate prompts." CandidateList
stub_tsx_component src/features/import/components/candidate-editor.tsx "Editor for a single extraction candidate." CandidateEditor
stub_tsx_component src/features/import/components/save-candidate-dialog.tsx "Confirm dialog before saving a candidate." SaveCandidateDialog
stub_tsx_component src/features/import/components/import-failure-panel.tsx "Failure panel with retry/abort actions." ImportFailurePanel
stub_ts_hook src/features/import/hooks/use-source-detection.ts "Hook calling detect_source for an input URL." useSourceDetection
stub_ts_hook src/features/import/hooks/use-source-preview.ts "Hook fetching normalized source content." useSourcePreview
stub_ts_hook src/features/import/hooks/use-extraction.ts "Hook calling extract_prompt_candidates." useExtraction
stub_ts src/features/import/stores/import-store.ts "Zustand store for the current import session state machine."

# --- features/search ---
stub_tsx_component src/features/search/components/cmdk-palette.tsx "⌘K command palette overlay." CmdKPalette
stub_tsx_component src/features/search/components/search-results.tsx "Search result list grouped by kind." SearchResults
stub_tsx_component src/features/search/components/semantic-toggle.tsx "Hybrid / semantic-only toggle." SemanticToggle
stub_ts_hook src/features/search/hooks/use-search.ts "Hook calling search_prompts with debounce." useSearch
stub_ts_hook src/features/search/hooks/use-cmdk.ts "Hook owning the palette open state and shortcuts." useCmdK
stub_ts src/features/search/stores/search-store.ts "Zustand store for the search palette."

# --- features/settings ---
stub_tsx_route src/features/settings/routes/settings-route.tsx "Route: /settings  Settings page." SettingsRoute
stub_tsx_component src/features/settings/components/vault-settings.tsx "Vault path + validation card." VaultSettings
stub_tsx_component src/features/settings/components/defaults-settings.tsx "Default model / permission mode / verifier mode card." DefaultsSettings
stub_tsx_component src/features/settings/components/extraction-settings.tsx "Extraction model + candidate counts card." ExtractionSettings
stub_tsx_component src/features/settings/components/secrets-settings.tsx "Anthropic key + X bearer secret status." SecretsSettings
stub_tsx_component src/features/settings/components/telemetry-settings.tsx "Telemetry on/off + two destructive actions." TelemetrySettings
stub_tsx_component src/features/settings/components/diagnostics-panel.tsx "Dependency probe diagnostics." DiagnosticsPanel
stub_tsx_component src/features/settings/components/updater-settings.tsx "Updater status + manual check." UpdaterSettings
stub_ts_hook src/features/settings/hooks/use-settings.ts "Hook fetching merged AppSettings." useSettings
stub_ts_hook src/features/settings/hooks/use-dependency-probes.ts "Hook calling probe_dependencies on mount." useDependencyProbes
stub_ts src/features/settings/stores/settings-store.ts "Zustand store for dirty local settings edits."

echo "scaffold complete."
