// ⌘K command palette — uses cmdk under the hood, styled with design-system
// tokens. Groups: Prompts (real, via L1's listPrompts), Routes (static).
// Runs / Actions / Settings entries land in L3-L5.

import { Command } from "cmdk";
import { useNavigate } from "react-router-dom";
import { useHotkeys } from "react-hotkeys-hook";
import { useEffect } from "react";

import { useSearchStore } from "@/features/search/stores/search-store";
import { usePrompts } from "@/features/library/hooks/use-prompts";
import { useLibraryStore } from "@/features/library/stores/library-store";

const ROUTES = [
  { id: "lib", title: "Library", subtitle: "all prompts", path: "/" },
  { id: "imp", title: "Import", subtitle: "from URL", path: "/import" },
  { id: "set", title: "Settings", subtitle: "vault, defaults, secrets", path: "/settings" },
];

export function CmdKPalette(): React.JSX.Element {
  const isOpen = useSearchStore((s) => s.isOpen);
  const query = useSearchStore((s) => s.query);
  const setQuery = useSearchStore((s) => s.setQuery);
  const close = useSearchStore((s) => s.close);
  const semantic = useSearchStore((s) => s.semantic);
  const toggleSemantic = useSearchStore((s) => s.toggleSemantic);

  const navigate = useNavigate();
  const select = useLibraryStore((s) => s.select);
  const prompts = usePrompts();

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
  // This is a sync between a Zustand boolean and a Zustand string —
  // useEffect is the right fit; nothing declarative bridges the two.
  useEffect(() => {
    if (!isOpen) setQuery("");
  }, [isOpen, setQuery]);

  if (!isOpen) return <></>;

  const lc = query.toLowerCase();
  const promptMatches = (prompts.data ?? [])
    .filter((p) => p.title.toLowerCase().includes(lc))
    .slice(0, 8);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
      onClick={close}
      style={{
        position: "fixed",
        inset: 0,
        background: "oklch(0.08 0.01 220 / 0.5)",
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
            "0 24px 64px -16px oklch(0 0 0 / 0.6), 0 0 0 1px var(--border-subtle)",
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
            title="Semantic search activates in L5"
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
          {promptMatches.length > 0 && (
            <Command.Group heading="prompts">
              {promptMatches.map((p) => (
                <Command.Item
                  key={p.id}
                  value={`prompt:${p.id}:${p.title}`}
                  onSelect={() => {
                    select(p.id);
                    navigate("/");
                    close();
                  }}
                  style={cmdkItemStyle}
                >
                  <span style={{ color: "var(--ink-primary)" }}>{p.title}</span>
                  {p.summary && (
                    <span style={cmdkSubStyle}>{p.summary}</span>
                  )}
                </Command.Item>
              ))}
            </Command.Group>
          )}
          <Command.Group heading="go to">
            {ROUTES.map((r) => (
              <Command.Item
                key={r.id}
                value={`route:${r.path}:${r.title}`}
                onSelect={() => {
                  navigate(r.path);
                  close();
                }}
                style={cmdkItemStyle}
              >
                <span style={{ color: "var(--ink-primary)" }}>{r.title}</span>
                <span style={cmdkSubStyle}>{r.subtitle}</span>
              </Command.Item>
            ))}
          </Command.Group>
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
          <span>L2 surface — runs/actions land in L3+</span>
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
