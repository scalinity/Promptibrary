// ⌘K command palette — backed by the cmdk_search IPC (L5/SCA-729).
//
// Sections, in the order returned by the backend:
//   - prompts  : top 8 via the hybrid search backend (FTS5 + recency + usage)
//   - runs     : top 5 matching prompt_title via the runs table
//   - actions  : static catalog of in-app commands
//   - routes   : static catalog of every spec §12 route
//
// The component groups results by `kind` and renders one Command.Group
// per non-empty section. Group order is fixed (prompts → runs →
// actions → routes) regardless of relative scores — the spec calls for
// section-grouping rather than a single ranked list.

import { Command } from "cmdk";
import { useNavigate } from "react-router-dom";
import { useHotkeys } from "react-hotkeys-hook";
// eslint-disable-next-line no-restricted-imports -- SCA-723: pre-CLAUDE.md useEffect, refactor in follow-up cleanup pass
import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";

import { cmdkSearch, type CmdkResult } from "@/shared/api/ipc";
import { asPromptId } from "@/shared/types/ids";
import { searchKeys } from "@/shared/api/queryKeys";
import { useSearchStore } from "@/features/search/stores/search-store";
import { useLibraryStore } from "@/features/library/stores/library-store";

/// SCA-742: render FTS5 snippet `<mark>…</mark>` markers as real
/// `<mark>` elements. Pre-fix the JSX rendered the snippet as a text
/// node (correct XSS posture), so users saw literal `<mark>` and
/// `</mark>` strings in the subtitle area instead of highlighted hits.
///
/// We split on the two marker tokens and wrap the captured ranges in
/// real `<mark>` JSX elements. Everything else stays a text node, so
/// XSS surface is unchanged — the FTS5 snippet payload can contain
/// any content from the prompt body, but text-content rendering is
/// safe by construction.
function renderSnippet(snippet: string): React.ReactNode {
  const parts = snippet.split(/(<mark>|<\/mark>)/g);
  const out: React.ReactNode[] = [];
  let inMark = false;
  let key = 0;
  for (const part of parts) {
    if (part === "<mark>") {
      inMark = true;
      continue;
    }
    if (part === "</mark>") {
      inMark = false;
      continue;
    }
    if (part === "") continue;
    if (inMark) {
      out.push(<mark key={key++}>{part}</mark>);
    } else {
      out.push(part);
    }
  }
  return out;
}

// Map static-action ids → handlers. Routes/prompts/runs handle
// navigation generically; actions need bespoke side effects (routed to
// the closest existing surface for now — the real action hooks land
// alongside the diagnostics + repair-orphans tickets).
//
// SCA-738: every target here is a route that actually exists in
// src/router.tsx — the pre-fix design referenced /compose/new and
// /settings/* nested routes that bounced to / via the wildcard.
const ACTION_ROUTES: Record<string, string> = {
  "new-prompt": "/",
  "import-url": "/import",
  "rebuild-index": "/settings",
  "run-diagnostics": "/settings",
  "reveal-vault": "/settings",
  "repair-orphans": "/settings",
};

export function CmdKPalette(): React.JSX.Element {
  const isOpen = useSearchStore((s) => s.isOpen);
  const query = useSearchStore((s) => s.query);
  const setQuery = useSearchStore((s) => s.setQuery);
  const close = useSearchStore((s) => s.close);
  const semantic = useSearchStore((s) => s.semantic);
  const toggleSemantic = useSearchStore((s) => s.toggleSemantic);

  const navigate = useNavigate();
  const select = useLibraryStore((s) => s.select);

  // Restore focus to the previously-focused element on close (SCA-655).
  const previousActive = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (isOpen) {
      const active = document.activeElement;
      previousActive.current =
        active instanceof HTMLElement ? active : null;
    } else if (previousActive.current != null) {
      previousActive.current.focus();
      previousActive.current = null;
    }
  }, [isOpen]);

  useHotkeys(
    "escape",
    (e) => {
      if (isOpen) {
        e.preventDefault();
        close();
      }
    },
    { enableOnFormTags: true },
  );

  // Clear the query when the dialog closes so reopening starts fresh.
  // useEffect is the right fit here: bridging a Zustand boolean to a
  // Zustand string is exactly the cross-store sync useEffect is for.
  useEffect(() => {
    if (!isOpen) setQuery("");
  }, [isOpen, setQuery]);

  const search = useQuery({
    queryKey: searchKeys.cmdk(query),
    queryFn: () => cmdkSearch(query),
    enabled: isOpen,
    staleTime: 1_000,
    placeholderData: (previous) => previous,
  });

  if (!isOpen) return <></>;

  const results: CmdkResult[] = search.data ?? [];
  const byKind = {
    prompt: results.filter((r) => r.kind === "prompt"),
    run: results.filter((r) => r.kind === "run"),
    action: results.filter((r) => r.kind === "action"),
    route: results.filter((r) => r.kind === "route"),
  };

  const handleSelect = (item: CmdkResult): void => {
    switch (item.kind) {
      case "prompt":
        select(asPromptId(item.id));
        navigate("/");
        break;
      case "run":
        navigate(`/run/${item.id}`);
        break;
      case "route":
        // subtitle holds the route path
        if (item.subtitle) navigate(item.subtitle);
        break;
      case "action": {
        const target = ACTION_ROUTES[item.id];
        if (target) navigate(target);
        break;
      }
    }
    close();
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
      onClick={close}
      style={{
        position: "fixed",
        inset: 0,
        background: "var(--bg-overlay)",
        backdropFilter: "blur(4px)",
        display: "grid",
        placeItems: "start center",
        paddingTop: "12vh",
        zIndex: 100,
      }}
    >
      <Command
        onClick={(e) => e.stopPropagation()}
        style={{
          width: "min(640px, calc(100vw - 48px))",
          background: "var(--bg-raised)",
          border: "1px solid var(--border-mid)",
          borderRadius: "var(--r-md)",
          overflow: "hidden",
          boxShadow:
            "0 24px 64px -16px var(--shadow-modal), 0 0 0 1px var(--border-subtle)",
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            padding: "var(--sp-3) var(--sp-4)",
            borderBottom: "var(--hairline)",
            gap: 12,
          }}
        >
          <span
            aria-hidden="true"
            style={{
              fontFamily: "var(--font-mono)",
              fontSize: 14,
              color: "var(--accent-dim)",
            }}
          >
            ⌕
          </span>
          <Command.Input
            value={query}
            onValueChange={setQuery}
            placeholder="search prompts, runs, actions…"
            autoFocus
            style={{
              flex: 1,
              background: "transparent",
              border: "none",
              outline: "none",
              fontFamily: "var(--font-mono)",
              fontSize: 13,
              color: "var(--ink-primary)",
            }}
          />
          <button
            type="button"
            onClick={toggleSemantic}
            disabled
            title="Semantic search lands with the embedding model (V2-deferred per docs/notes/L5-observations.md) — Cmd-K is text-only for V1"
            aria-pressed={semantic}
            style={{
              fontFamily: "var(--font-mono)",
              fontSize: 10.5,
              letterSpacing: "0.04em",
              textTransform: "uppercase",
              color: semantic ? "var(--accent)" : "var(--ink-dim)",
              background: "transparent",
              border: "1px solid var(--border-subtle)",
              borderRadius: "var(--r-xs)",
              padding: "2px 8px",
              cursor: "not-allowed",
              opacity: 0.55,
            }}
          >
            sem
          </button>
        </div>
        <Command.List
          style={{
            maxHeight: 360,
            overflow: "auto",
            padding: "var(--sp-2) 0",
          }}
        >
          <Command.Empty
            style={{
              padding: "var(--sp-4)",
              fontFamily: "var(--font-mono)",
              fontSize: 11.5,
              color: "var(--ink-tertiary)",
              textAlign: "center",
            }}
          >
            nothing matches
          </Command.Empty>

          {byKind.prompt.length > 0 && (
            <Command.Group heading="prompts">
              {byKind.prompt.map((p) => (
                <Command.Item
                  key={`prompt:${p.id}`}
                  value={`prompt:${p.id}:${p.title}`}
                  onSelect={() => handleSelect(p)}
                  style={cmdkItemStyle}
                >
                  <span style={{ color: "var(--ink-primary)" }}>{p.title}</span>
                  {p.subtitle != null && p.subtitle !== "" && (
                    <span style={cmdkSubStyle}>{renderSnippet(p.subtitle)}</span>
                  )}
                </Command.Item>
              ))}
            </Command.Group>
          )}

          {byKind.run.length > 0 && (
            <Command.Group heading="runs">
              {byKind.run.map((r) => (
                <Command.Item
                  key={`run:${r.id}`}
                  value={`run:${r.id}:${r.title}`}
                  onSelect={() => handleSelect(r)}
                  style={cmdkItemStyle}
                >
                  <span style={{ color: "var(--ink-primary)" }}>{r.title}</span>
                  {r.subtitle != null && r.subtitle !== "" && (
                    <span style={cmdkSubStyle}>{r.subtitle}</span>
                  )}
                </Command.Item>
              ))}
            </Command.Group>
          )}

          {byKind.action.length > 0 && (
            <Command.Group heading="actions">
              {byKind.action.map((a) => (
                <Command.Item
                  key={`action:${a.id}`}
                  value={`action:${a.id}:${a.title}`}
                  onSelect={() => handleSelect(a)}
                  style={cmdkItemStyle}
                >
                  <span style={{ color: "var(--ink-primary)" }}>{a.title}</span>
                  {a.subtitle != null && a.subtitle !== "" && (
                    <span style={cmdkSubStyle}>{a.subtitle}</span>
                  )}
                </Command.Item>
              ))}
            </Command.Group>
          )}

          {byKind.route.length > 0 && (
            <Command.Group heading="go to">
              {byKind.route.map((r) => (
                <Command.Item
                  key={`route:${r.id}`}
                  value={`route:${r.id}:${r.title}`}
                  onSelect={() => handleSelect(r)}
                  style={cmdkItemStyle}
                >
                  <span style={{ color: "var(--ink-primary)" }}>{r.title}</span>
                  {r.subtitle != null && r.subtitle !== "" && (
                    <span style={cmdkSubStyle}>{r.subtitle}</span>
                  )}
                </Command.Item>
              ))}
            </Command.Group>
          )}
        </Command.List>
        <footer
          style={{
            borderTop: "var(--hairline)",
            padding: "var(--sp-2) var(--sp-4)",
            fontFamily: "var(--font-mono)",
            fontSize: 10.5,
            color: "var(--ink-tertiary)",
            letterSpacing: "0.04em",
            display: "flex",
            justifyContent: "space-between",
          }}
        >
          <span>↑ ↓ navigate · ↵ open · esc close</span>
          <span>
            {search.isFetching
              ? "…"
              : `${results.length} result${results.length === 1 ? "" : "s"}`}
          </span>
        </footer>
      </Command>
    </div>
  );
}

const cmdkItemStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 12,
  padding: "var(--sp-3) var(--sp-4)",
  fontFamily: "var(--font-ui)",
  fontSize: 13,
  cursor: "pointer",
  color: "var(--ink-secondary)",
};

const cmdkSubStyle: React.CSSProperties = {
  color: "var(--ink-tertiary)",
  fontSize: 11.5,
  marginLeft: "auto",
  fontFamily: "var(--font-mono)",
  letterSpacing: "0.01em",
  maxWidth: "55%",
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};
