// Topbar — brand mark, global search field (Cmd-K trigger), action buttons.
//
// Mirrors `.topbar` from the canonical app.css. Search field is a *trigger*
// for the cmdk palette; clicking it (or pressing ⌘K / `/`) opens the dialog.

// eslint-disable-next-line no-restricted-imports -- SCA-895: external sync with Tauri menu-event bus, no declarative alternative
import { useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { useHotkeys } from "react-hotkeys-hook";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

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

  // SCA-930 — explicit OS drag invocation. Tauri 2's data-tauri-drag-region
  // shim should do this for us, but the attribute-based path wasn't firing
  // in our setup. Calling startDragging() directly from a mousedown handler
  // is bulletproof: it goes through the same IPC + permission as the shim
  // but without the attribute-walk indirection. Skip the call when the
  // user clicked an interactive control so button/search clicks still
  // register.
  const handleTopbarMouseDown = (e: React.MouseEvent<HTMLDivElement>): void => {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement | null;
    if (
      target == null ||
      target.closest(
        'button, input, a, select, textarea, [role="button"], [role="textbox"], [role="combobox"]',
      ) != null
    ) {
      return;
    }
    void getCurrentWindow().startDragging().catch(() => {
      // E2E mock mode runs in a plain browser where @tauri-apps/api is
      // a stub — calls reject. Silent so we don't spam the console.
    });
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
    // SCA-930 — Tauri 2 window-drag handle. The runtime injects a
    // document-level mousedown listener that walks up the DOM looking
    // for data-tauri-drag-region; anywhere this attribute is present
    // (and no closer ancestor opted out via data-tauri-drag-region="false")
    // calls startDragging() instead of selecting text. CSS app-region
    // is the Electron/Chromium convention and is silently ignored by
    // WKWebView, which is why the previous CSS-only attempt did
    // nothing.
    <div
      className="topbar"
      data-tauri-drag-region
      onMouseDown={handleTopbarMouseDown}
    >
      <div className="brand" data-tauri-drag-region>
        <span className="brand-name" data-tauri-drag-region>
          Promptibrary<span className="dot">.</span>
        </span>
        <span className="brand-tag" data-tauri-drag-region>// agentic operator console</span>
      </div>
      <div className="center" data-tauri-drag-region>
        <button
          type="button"
          className="search"
          onClick={() => openPalette("")}
          aria-label="Search prompts, tags, history"
          data-tauri-drag-region="false"
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
      <div className="actions" data-tauri-drag-region>
        <button
          type="button"
          className="btn"
          onClick={() => navigate("/import")}
          data-tauri-drag-region="false"
        >
          import <span className="kbd">⌘I</span>
        </button>
        <button
          type="button"
          className="btn"
          onClick={handleNewPrompt}
          disabled={createPromptMutation.isPending}
          title="New prompt"
          data-tauri-drag-region="false"
        >
          new <span className="kbd">⌘N</span>
        </button>
        <button
          type="button"
          className="icon-btn"
          aria-label="Settings"
          onClick={() => navigate("/settings")}
          data-tauri-drag-region="false"
        >
          ⚙
        </button>
      </div>
    </div>
  );
}
