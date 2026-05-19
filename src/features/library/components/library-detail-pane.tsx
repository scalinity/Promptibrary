// Library detail pane — shows the selected prompt's header, variables, body,
// launch row, and the launch button. Re-uses the prompt-editor surfaces in
// read-mostly mode.

import { Suspense, lazy } from "react";

import { TagFilterBar } from "@/features/library/components/tag-filter-bar";
import { TelemetryMiniStats } from "@/features/library/components/telemetry-mini-stats";
import { usePrompt } from "@/features/prompt/hooks/use-prompt";
import { useLaunchDraftStore } from "@/features/launch/stores/launch-draft-store";
import { EmptyState } from "@/shared/ui/empty-state";
import type { PromptId } from "@/shared/types/ids";
import { LoadingSpinner } from "@/shared/ui/loading-spinner";

const PromptBodyEditor = lazy(() =>
  import("@/features/prompt/components/prompt-body-editor").then((m) => ({
    default: m.PromptBodyEditor,
  })),
);

const LaunchDrawer = lazy(() =>
  import("@/features/launch/components/launch-drawer").then((m) => ({
    default: m.LaunchDrawer,
  })),
);

interface LibraryDetailPaneProps {
  selectedId: PromptId | null;
}

export function LibraryDetailPane({
  selectedId,
}: LibraryDetailPaneProps): React.JSX.Element {
  const promptQuery = usePrompt(selectedId);
  const drawerOpen = useLaunchDraftStore((s) => s.isOpen);

  if (selectedId == null) {
    return (
      <section className="detail-pane" aria-label="Prompt detail">
        <TagFilterBar />
        <EmptyState
          glyph="⌖"
          title="Select a prompt"
          body="The library shows every prompt in the active vault. Pick one to see its variables, body, and launch profile."
        />
      </section>
    );
  }

  if (promptQuery.isLoading || promptQuery.data == null) {
    return (
      <section className="detail-pane" aria-label="Prompt detail">
        <div
          style={{
            display: "grid",
            placeItems: "center",
            height: "100%",
          }}
        >
          <LoadingSpinner label="loading prompt…" />
        </div>
      </section>
    );
  }

  const prompt = promptQuery.data;

  return (
    <section className="detail-pane" aria-label="Prompt detail">
      <div className="detail-scroll">
        <header className="detail-header">
          <div className="detail-crumb">
            prompts <span className="sep">›</span> {prompt.tags[0] ?? "untagged"}{" "}
            <span className="sep">›</span> {prompt.slug}
          </div>
          <h1 className="detail-title">{prompt.title}</h1>
          <TelemetryMiniStats prompt={prompt} />
          {prompt.tags.length > 0 && (
            <div className="tag-row">
              {prompt.tags.map((tag) => (
                <span key={tag} className="tag">
                  {tag}
                </span>
              ))}
            </div>
          )}
        </header>

        <section className="prompt-block">
          <div className="prompt-bar">
            <span className="section-label">prompt</span>
          </div>
          <Suspense
            fallback={
              <div
                style={{
                  padding: "16px",
                  fontFamily: "var(--font-mono)",
                  color: "var(--ink-tertiary)",
                  fontSize: "11px",
                }}
              >
                loading editor…
              </div>
            }
          >
            <PromptBodyEditor
              value={prompt.body}
              promptId={prompt.id}
              readOnly
            />
          </Suspense>
        </section>
      </div>

      <Suspense fallback={null}>
        {drawerOpen && <LaunchDrawer prompt={prompt} />}
      </Suspense>
    </section>
  );
}
