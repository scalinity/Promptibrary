// Topbar — brand mark, global search field (Cmd-K trigger), action buttons.
//
// Mirrors `.topbar` from the canonical app.css. Search field is a *trigger*
// for the cmdk palette; clicking it (or pressing ⌘K / `/`) opens the dialog.

import { useNavigate } from "react-router-dom";
import { useHotkeys } from "react-hotkeys-hook";

import { useSearchStore } from "@/features/search/stores/search-store";

export function Topbar(): React.JSX.Element {
  const navigate = useNavigate();
  const openPalette = useSearchStore((s) => s.open);

  useHotkeys(
    "meta+k, ctrl+k",
    (e) => {
      e.preventDefault();
      openPalette("");
    },
    { enableOnFormTags: true },
  );

  useHotkeys(
    "/",
    (e) => {
      e.preventDefault();
      openPalette("");
    },
    { enableOnFormTags: false },
  );

  useHotkeys(
    "meta+i, ctrl+i",
    (e) => {
      e.preventDefault();
      navigate("/import");
    },
    { enableOnFormTags: true },
  );

  useHotkeys(
    "meta+comma, ctrl+comma",
    (e) => {
      e.preventDefault();
      navigate("/settings");
    },
    { enableOnFormTags: true },
  );

  return (
    <div className="topbar">
      <div className="brand">
        <span className="brand-name">
          Promptibrary<span className="dot">.</span>
        </span>
        <span className="brand-tag">// agentic operator console</span>
      </div>
      <div className="center">
        <button
          type="button"
          className="search"
          onClick={() => openPalette("")}
          aria-label="Search prompts, tags, history"
        >
          <span className="glyph">⌕</span>
          <span
            style={{
              flex: 1,
              textAlign: "left",
              fontFamily: "var(--font-mono)",
              fontSize: "12px",
              color: "var(--ink-dim)",
            }}
          >
            search prompts, tags, history…
          </span>
          <span className="kbd">⌘K</span>
        </button>
      </div>
      <div className="actions">
        <button
          type="button"
          className="btn"
          onClick={() => navigate("/import")}
        >
          import <span className="kbd">⌘I</span>
        </button>
        <button
          type="button"
          className="btn"
          onClick={() => navigate("/")}
          title="New prompt"
        >
          new <span className="kbd">⌘N</span>
        </button>
        <button
          type="button"
          className="icon-btn"
          aria-label="Settings"
          onClick={() => navigate("/settings")}
        >
          ⚙
        </button>
      </div>
    </div>
  );
}
