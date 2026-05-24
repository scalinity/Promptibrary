// Route: `/prompt/:promptId` — full editor composition.
//
// Layout: frontmatter panel (left) + body editor (center) + variable
// reference list (right). Launch drawer is overlaid when active.

// eslint-disable-next-line no-restricted-imports -- SCA-723: pre-CLAUDE.md useEffect, refactor in follow-up cleanup pass
import { Suspense, lazy, useEffect } from "react";
import { useParams } from "react-router-dom";
import { useHotkeys } from "react-hotkeys-hook";

import { usePrompt } from "@/features/prompt/hooks/use-prompt";
import { PromptFrontmatterPanel } from "@/features/prompt/components/prompt-frontmatter-panel";
import { PromptBodyEditor } from "@/features/prompt/components/prompt-body-editor";
import { VariableReferenceList } from "@/features/prompt/components/variable-reference-list";
import { LaunchProfilePanel } from "@/features/prompt/components/launch-profile-panel";
import { PromptHistoryPanel } from "@/features/prompt/components/prompt-history-panel";
import { ExportMenu } from "@/features/prompt/components/export-menu";
import { EditableTitle } from "@/features/prompt/components/editable-title";
import { usePromptEditorStore } from "@/features/prompt/stores/prompt-editor-store";
import { useLaunchDraftStore } from "@/features/launch/stores/launch-draft-store";
import { useAssistantStore } from "@/features/assistant/store/assistant-store";
import { asPromptId } from "@/shared/types/ids";
import { EmptyState } from "@/shared/ui/empty-state";
import { LoadingSpinner } from "@/shared/ui/loading-spinner";

const LaunchDrawer = lazy(() =>
  import("@/features/launch/components/launch-drawer").then((m) => ({
    default: m.LaunchDrawer,
  })),
);

const AssistantPanel = lazy(() =>
  import("@/features/assistant/components/assistant-panel").then((m) => ({
    default: m.AssistantPanel,
  })),
);

export function PromptRoute(): React.JSX.Element {
  const { promptId } = useParams<{ promptId: string }>();
  const id = promptId != null ? asPromptId(promptId) : null;
  const promptQuery = usePrompt(id);
  const editorBody = usePromptEditorStore((s) => s.body);
  const setBody = usePromptEditorStore((s) => s.setBody);
  const showHistory = usePromptEditorStore((s) => s.showHistory);
  const toggleHistory = usePromptEditorStore((s) => s.toggleHistory);
  const clearDraft = usePromptEditorStore((s) => s.clear);
  const openLaunch = useLaunchDraftStore((s) => s.open);
  const drawerOpen = useLaunchDraftStore((s) => s.isOpen);
  const toggleAssistant = useAssistantStore((s) => s.toggleOpen);
  const assistantOpen = useAssistantStore((s) => s.isOpen);

  // SCA-635 — drop this prompt's editor draft on unmount so the store
  // doesn't grow unboundedly across the session. Pure cleanup; nothing
  // declarative bridges a route-unmount → store-mutation flow.
  useEffect(() => {
    if (id == null) return;
    return () => {
      clearDraft(id);
    };
  }, [id, clearDraft]);

  useHotkeys(
    "meta+shift+h, ctrl+shift+h",
    (e) => {
      e.preventDefault();
      toggleHistory();
    },
    { enableOnFormTags: true },
  );

  useHotkeys(
    "meta+enter, ctrl+enter",
    (e) => {
      if (promptQuery.data != null) {
        e.preventDefault();
        openLaunch(promptQuery.data);
      }
    },
    { enableOnFormTags: true },
  );

  useHotkeys(
    "meta+i, ctrl+i",
    (e) => {
      e.preventDefault();
      toggleAssistant();
    },
    { enableOnFormTags: true },
  );

  if (promptQuery.isLoading) {
    return (
      <section className="detail-pane">
        <div style={{ display: "grid", placeItems: "center", height: "100%" }}>
          <LoadingSpinner label="loading prompt…" />
        </div>
      </section>
    );
  }
  if (promptQuery.data == null) {
    return (
      <section className="detail-pane">
        <EmptyState
          glyph="§"
          title="Prompt not found"
          body="The prompt may have been archived or deleted from the vault."
        />
      </section>
    );
  }

  const prompt = promptQuery.data;
  const body = editorBody[prompt.id] ?? prompt.body;

  return (
    <>
      <aside
        className="list-pane"
        aria-label="Prompt frontmatter"
        style={{ overflow: "auto" }}
      >
        <PromptFrontmatterPanel prompt={prompt} />
        <LaunchProfilePanel prompt={prompt} />
        <ExportMenu prompt={prompt} />
      </aside>
      <section className="detail-pane" aria-label="Prompt body" style={{ overflow: "auto" }}>
        <header
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            padding: "var(--sp-4) var(--sp-5)",
            borderBottom: "var(--hairline)",
          }}
        >
          <div>
            <div
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 10.5,
                color: "var(--ink-tertiary)",
                letterSpacing: "0.04em",
                textTransform: "uppercase",
              }}
            >
              editor
            </div>
            <EditableTitle key={prompt.id} prompt={prompt} />
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <button
              type="button"
              className="btn"
              onClick={toggleHistory}
              aria-pressed={showHistory}
            >
              history <span className="kbd">⌘⇧H</span>
            </button>
            <button
              type="button"
              className="btn-launch"
              onClick={() => openLaunch(prompt)}
            >
              launch <span className="kbd-inline">⌘↵</span>
            </button>
          </div>
        </header>
        <div style={{ padding: "var(--sp-5)" }}>
          <PromptBodyEditor
            promptId={prompt.id}
            value={body}
            onChange={(v) => setBody(prompt.id, v)}
          />
        </div>
        {showHistory && (
          <div style={{ padding: "0 var(--sp-5) var(--sp-5)" }}>
            <PromptHistoryPanel prompt={prompt} />
          </div>
        )}
      </section>
      <aside
        className="detail-pane"
        aria-label="Variables"
        style={{
          maxWidth: 340,
          borderLeft: "var(--hairline)",
          overflow: "auto",
        }}
      >
        <VariableReferenceList prompt={prompt} body={body} />
      </aside>
      <Suspense fallback={null}>
        {drawerOpen && <LaunchDrawer prompt={prompt} />}
      </Suspense>
      <Suspense fallback={null}>
        {assistantOpen && <AssistantPanel promptId={prompt.id} />}
      </Suspense>
    </>
  );
}
