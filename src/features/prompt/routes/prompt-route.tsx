// Route: `/prompt/:promptId` — full editor composition.
//
// Layout: frontmatter panel (left) + body editor (center) + variable
// reference list (right). Launch drawer is overlaid when active.

import { Suspense, lazy } from "react";
import { useParams } from "react-router-dom";
import { useHotkeys } from "react-hotkeys-hook";

import { usePrompt } from "@/features/prompt/hooks/use-prompt";
import { PromptFrontmatterPanel } from "@/features/prompt/components/prompt-frontmatter-panel";
import { PromptBodyEditor } from "@/features/prompt/components/prompt-body-editor";
import { VariableReferenceList } from "@/features/prompt/components/variable-reference-list";
import { LaunchProfilePanel } from "@/features/prompt/components/launch-profile-panel";
import { PromptHistoryPanel } from "@/features/prompt/components/prompt-history-panel";
import { ExportMenu } from "@/features/prompt/components/export-menu";
import { usePromptEditorStore } from "@/features/prompt/stores/prompt-editor-store";
import { useLaunchDraftStore } from "@/features/launch/stores/launch-draft-store";
import { asPromptId } from "@/shared/types/ids";
import { EmptyState } from "@/shared/ui/empty-state";
import { LoadingSpinner } from "@/shared/ui/loading-spinner";

const LaunchDrawer = lazy(() =>
  import("@/features/launch/components/launch-drawer").then((m) => ({
    default: m.LaunchDrawer,
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
  const openLaunch = useLaunchDraftStore((s) => s.open);
  const drawerOpen = useLaunchDraftStore((s) => s.isOpen);

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
      if (id != null) {
        e.preventDefault();
        openLaunch(id);
      }
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
            <h1
              style={{
                fontFamily: "var(--font-display)",
                fontWeight: 500,
                fontSize: 22,
                letterSpacing: "-0.02em",
                margin: "4px 0 0",
              }}
            >
              {prompt.title}
            </h1>
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
              onClick={() => openLaunch(prompt.id)}
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
    </>
  );
}
