// Tag filter chip row — selected tag pins the row list to that tag.
//
// Renders a horizontal `.tag-row` of chips; clicking the active chip clears
// the filter. Chips use the canonical `.tag` style; counts are mono dim.

import { useLibraryStats, useLibraryStore } from "@/features/library/stores/library-store";
import type { TagName } from "@/shared/types/ids";
import { asTagName } from "@/shared/types/ids";
import { cn } from "@/shared/lib/utils";

export function TagFilterBar(): React.JSX.Element {
  const tags = useLibraryStats((s) => s.tags);
  const active = useLibraryStore((s) => s.filter.tag);
  const setTag = useLibraryStore((s) => s.setTag);

  if (tags.length === 0) {
    return <></>;
  }

  return (
    <div className="tag-row" role="toolbar" aria-label="Filter by tag">
      {tags.map((tag) => {
        const isActive = active === asTagName(tag.name);
        return (
          <button
            key={tag.name}
            type="button"
            className={cn("tag")}
            data-state={isActive ? "active" : undefined}
            onClick={() => setTag(isActive ? null : (tag.name as TagName))}
            style={{
              background: isActive ? "var(--accent-tint)" : undefined,
              borderColor: isActive ? "var(--accent-deep)" : undefined,
              color: isActive ? "var(--ink-primary)" : undefined,
              cursor: "pointer",
            }}
          >
            {tag.name}
            <span
              style={{
                marginLeft: 6,
                color: "var(--ink-dim)",
                fontSize: "10.5px",
              }}
            >
              {tag.count}
            </span>
          </button>
        );
      })}
    </div>
  );
}
