// Topbar — brand mark, global search field (Cmd-K trigger), action buttons.
//
// Mirrors `.topbar` from the canonical app.css. Search field is a *trigger*
// for the cmdk palette; clicking it (or pressing ⌘K / `/`) opens the dialog.

// eslint-disable-next-line no-restricted-imports -- SCA-895: external sync with Tauri menu-event bus, no declarative alternative
import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { useHotkeys } from "react-hotkeys-hook";
import { listen } from "@tauri-apps/api/event";

import { useCreatePrompt } from "@/features/library/hooks/use-create-prompt";
import { useSearchStore } from "@/features/search/stores/search-store";

export function Topbar(): React.JSX.Element {
  const navigate = useNavigate();
  const openPalette = useSearchStore((s) => s.open);
  const createPromptMutation = useCreatePrompt();
  const { mutate: createPromptMutate } = createPromptMutation;

  const handleNewPrompt = () => {
    if (createPromptMutation.isPending) return;
    createPromptMutate(undefined);
  };

  // SCA-895 — listen for the `menu:new-prompt` event the Rust side emits
  // when the ⌘N menu item fires. WKWebView swallows ⌘N before keydown
  // reaches JS, so the menu has to own the shortcut; this hook bridges the
  // OS-level activation back into the React mutation. `mutate` is a stable
  // reference per TanStack Query so the listener doesn't churn.
  useEffect(() => {
    const unlistenPromise = listen("menu:new-prompt", () => {
      createPromptMutate(undefined);
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [createPromptMutate]);

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
  );

  useHotkeys(
    "meta+comma, ctrl+comma",
    (e) => {
      e.preventDefault();
      navigate("/settings");
    },
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
          onClick={handleNewPrompt}
          disabled={createPromptMutation.isPending}
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
