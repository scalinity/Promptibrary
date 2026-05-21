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
import { useDeferredValue, useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";

import { cmdkSearch, type CmdkResult } from "@/shared/api/ipc";
import { asPromptId } from "@/shared/types/ids";
import { searchKeys } from "@/shared/api/queryKeys";
import { useSearchStore } from "@/features/search/stores/search-store";
import { useLibraryStore } from "@/features/library/stores/library-store";

import "@/features/search/cmdk-palette.css";

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

  // SCA-922 (W26): debounce the IPC trigger via useDeferredValue so a
  // 12-character query fires ~2 round-trips during the keystroke burst
  // instead of 12. Keystroke input stays responsive (uses `query`);
  // the IPC + results layer reads `deferredQuery`.
  const deferredQuery = useDeferredValue(query);
  const search = useQuery({
    queryKey: searchKeys.cmdk(deferredQuery),
    queryFn: () => cmdkSearch(deferredQuery),
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
      className="cmdk-overlay"
    >
      <Command
        onClick={(e) => e.stopPropagation()}
        className="cmdk-dialog"
      >
        <div className="cmdk-input-row">
          <span aria-hidden="true" className="cmdk-input-glyph">
            ⌕
          </span>
          <Command.Input
            value={query}
            onValueChange={setQuery}
            placeholder="search prompts, runs, actions…"
            autoFocus
            className="cmdk-input"
          />
          <button
            type="button"
            onClick={toggleSemantic}
            disabled
            title="Semantic search lands with the embedding model (V2-deferred per docs/notes/L5-observations.md) — Cmd-K is text-only for V1"
            aria-pressed={semantic}
            className="cmdk-sem-toggle"
          >
            sem
          </button>
        </div>
        <Command.List className="cmdk-list">
          <Command.Empty className="cmdk-empty">
            nothing matches
          </Command.Empty>

          {byKind.prompt.length > 0 && (
            <Command.Group heading="prompts">
              {byKind.prompt.map((p) => (
                <Command.Item
                  key={`prompt:${p.id}`}
                  value={`prompt:${p.id}:${p.title}`}
                  onSelect={() => handleSelect(p)}
                  className="cmdk-item"
                >
                  <span className="cmdk-item-title">{p.title}</span>
                  {p.subtitle != null && p.subtitle !== "" && (
                    <span className="cmdk-item-sub">{renderSnippet(p.subtitle)}</span>
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
                  className="cmdk-item"
                >
                  <span className="cmdk-item-title">{r.title}</span>
                  {r.subtitle != null && r.subtitle !== "" && (
                    <span className="cmdk-item-sub">{r.subtitle}</span>
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
                  className="cmdk-item"
                >
                  <span className="cmdk-item-title">{a.title}</span>
                  {a.subtitle != null && a.subtitle !== "" && (
                    <span className="cmdk-item-sub">{a.subtitle}</span>
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
                  className="cmdk-item"
                >
                  <span className="cmdk-item-title">{r.title}</span>
                  {r.subtitle != null && r.subtitle !== "" && (
                    <span className="cmdk-item-sub">{r.subtitle}</span>
                  )}
                </Command.Item>
              ))}
            </Command.Group>
          )}
        </Command.List>
        <footer className="cmdk-footer">
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
