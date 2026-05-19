// Virtualized prompt list — the `.list-pane` body.
//
// Uses `react-virtuoso` so 500+ rows scroll smoothly. Selection state is in
// the library store; the spine state classes come from PromptCard.

import { Virtuoso } from "react-virtuoso";

import { PromptCard } from "./prompt-card";
import { useLibraryStore } from "@/features/library/stores/library-store";
import type { Prompt } from "@/shared/types/prompt";
import { EmptyState } from "@/shared/ui/empty-state";

interface PromptListProps {
  prompts: Prompt[];
  loading: boolean;
}

export function PromptList({ prompts, loading }: PromptListProps): React.JSX.Element {
  const selectedId = useLibraryStore((s) => s.selectedPromptId);
  const select = useLibraryStore((s) => s.select);
  const filter = useLibraryStore((s) => s.filter);
  const sort = useLibraryStore((s) => s.sort);

  if (loading && prompts.length === 0) {
    return (
      <div className="list-rows" data-testid="prompt-list-loading">
        <div
          style={{
            padding: "24px 16px",
            fontFamily: "var(--font-mono)",
            fontSize: "11px",
            color: "var(--ink-tertiary)",
            letterSpacing: "0.04em",
          }}
        >
          loading prompts…
        </div>
      </div>
    );
  }

  const filtered = filterAndSort(prompts, filter, sort);

  if (filtered.length === 0) {
    return (
      <div className="list-rows">
        <EmptyState
          glyph="⌖"
          title="No prompts match"
          body="Try clearing the search query or tag filter."
        />
      </div>
    );
  }

  // Recent set = top 3 most recently updated.
  const recentIds = new Set(
    [...filtered]
      .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
      .slice(0, 3)
      .map((p) => p.id),
  );

  return (
    <div className="list-rows" data-testid="prompt-list">
      <Virtuoso
        data={filtered}
        totalCount={filtered.length}
        overscan={4}
        itemContent={(_index, prompt) => (
          <PromptCard
            key={prompt.id}
            prompt={prompt}
            selected={selectedId === prompt.id}
            recent={recentIds.has(prompt.id)}
            onSelect={select}
          />
        )}
      />
    </div>
  );
}

function filterAndSort(
  prompts: Prompt[],
  filter: { query: string; tag: string | null; archived: boolean },
  sort: string,
): Prompt[] {
  const q = filter.query.trim().toLowerCase();
  const filtered = prompts.filter((p) => {
    if (!filter.archived && p.archivedAt != null) return false;
    if (filter.tag != null && !p.tags.includes(filter.tag as never)) return false;
    if (q.length > 0) {
      const hay = `${p.title} ${p.summary} ${p.tags.join(" ")}`.toLowerCase();
      if (!hay.includes(q)) return false;
    }
    return true;
  });
  const sorted = [...filtered];
  switch (sort) {
    case "title":
      sorted.sort((a, b) => a.title.localeCompare(b.title));
      break;
    case "launch_count":
      sorted.sort(
        (a, b) => b.telemetry.launchCount - a.telemetry.launchCount,
      );
      break;
    case "recently_edited":
      sorted.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
      break;
    case "recently_used":
    default:
      sorted.sort((a, b) => {
        const al = a.telemetry.lastUsedAt ?? a.updatedAt;
        const bl = b.telemetry.lastUsedAt ?? b.updatedAt;
        return bl.localeCompare(al);
      });
  }
  return sorted;
}
