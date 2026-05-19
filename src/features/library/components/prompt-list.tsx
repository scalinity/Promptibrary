// Virtualized prompt list — the `.list-pane` body.
//
// Uses `react-virtuoso` so 500+ rows scroll smoothly. Selection state is in
// the library store; the spine state classes come from PromptCard.
//
// Operates on `PromptListItem[]` (slim rows from `listPrompts`). Telemetry +
// timestamps are absent from this surface — sort by launch count requires
// the full Prompt and is deferred to L5 when the FTS-backed search lands.

import { Virtuoso } from "react-virtuoso";

import { PromptCard } from "./prompt-card";
import { useLibraryStore } from "@/features/library/stores/library-store";
import type { PromptListItem } from "@/shared/api/ipc";
import { EmptyState } from "@/shared/ui/empty-state";

interface PromptListProps {
  prompts: PromptListItem[];
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

  // Recent set — first 3 in the (sorted) filtered list. Without per-row
  // timestamps on the slim shape, "recent" tracks list position rather than
  // wall-clock recency. L5 swaps in real timestamps when the detail/list
  // queries are unified.
  const recentIds = new Set(filtered.slice(0, 3).map((p) => p.id));

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
  prompts: PromptListItem[],
  filter: { query: string; tag: string | null; archived: boolean },
  sort: string,
): PromptListItem[] {
  const q = filter.query.trim().toLowerCase();
  const filtered = prompts.filter((p) => {
    if (!filter.archived && p.archivedAt != null) return false;
    if (filter.tag != null && !p.tags.includes(filter.tag)) return false;
    if (q.length > 0) {
      const hay = `${p.title} ${p.summary ?? ""} ${p.tags.join(" ")}`.toLowerCase();
      if (!hay.includes(q)) return false;
    }
    return true;
  });
  const sorted = [...filtered];
  switch (sort) {
    case "title":
      sorted.sort((a, b) => a.title.localeCompare(b.title));
      break;
    // `launch_count` and `recently_edited` both require telemetry +
    // updatedAt on the row, which the slim shape doesn't carry. Fall
    // through to slug-stable order; real ordering arrives with L5's
    // hybrid-rank search.
    case "launch_count":
    case "recently_edited":
    case "recently_used":
    default:
      sorted.sort((a, b) => a.slug.localeCompare(b.slug));
  }
  return sorted;
}
