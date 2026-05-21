// Route: `/` — prompt library home (compose).
//
// Two-pane within AppShell's workspace: prompt list + detail. Sidebar lives
// in the chrome. Selection state is in `useLibraryStore`; the URL doesn't
// carry it for V1 (matches the design system mockup which has no per-prompt
// URL on the home route).

// eslint-disable-next-line no-restricted-imports -- SCA-723: pre-CLAUDE.md useEffect, refactor in follow-up cleanup pass
import { useEffect } from "react";

import { LibraryDetailPane } from "@/features/library/components/library-detail-pane";
import { LibraryToolbar } from "@/features/library/components/library-toolbar";
import { PromptList } from "@/features/library/components/prompt-list";
import { usePrompts } from "@/features/library/hooks/use-prompts";
import {
  useLibraryStats,
  useLibraryStore,
} from "@/features/library/stores/library-store";
import { useStatusLine } from "@/shared/ui/use-status-line";

export function LibraryRoute(): React.JSX.Element {
  const prompts = usePrompts();
  const setCounts = useLibraryStats((s) => s.setCounts);
  const setTags = useLibraryStats((s) => s.setTags);
  const setVaults = useLibraryStats((s) => s.setVaults);
  const setPromptCount = useStatusLine((s) => s.setPromptCount);
  const selectedId = useLibraryStore((s) => s.selectedPromptId);

  // Sidebar + status line counts stay in sync with the prompt query.
  // SCA-925 (W11): the proper fix is to refactor every consumer of
  // useLibraryStats + useStatusLine to read directly from `usePrompts`
  // with a select callback — that work is V2 (see docs/V2-CANDIDATES.md
  // "Library-route TanStack→Zustand effect bridge"). The effect stays
  // here as the documented exception until that lands.
  useEffect(() => {
    if (prompts.data == null) return;
    const all = prompts.data;
    const archived = all.filter((p) => p.archivedAt != null);
    const active = all.filter((p) => p.archivedAt == null);

    setCounts({
      all: active.length,
      pinned: 0,
      recent: Math.min(active.length, 25),
      archived: archived.length,
    });

    const tagCounts = new Map<string, number>();
    active.forEach((p) =>
      p.tags.forEach((t) => tagCounts.set(t, (tagCounts.get(t) ?? 0) + 1)),
    );
    setTags(
      [...tagCounts.entries()]
        .map(([name, count]) => ({ name, count }))
        .sort((a, b) => b.count - a.count)
        .slice(0, 8),
    );

    setVaults([]);
    setPromptCount(active.length);
  }, [prompts.data, setCounts, setTags, setVaults, setPromptCount]);

  return (
    <>
      <section className="list-pane" aria-label="Prompt list">
        <LibraryToolbar />
        <PromptList prompts={prompts.data ?? []} loading={prompts.isLoading} />
      </section>
      <LibraryDetailPane selectedId={selectedId} />
    </>
  );
}
