// List-pane header — title + sort picker.
//
// Mirrors `.list-header` from the canonical app.css. The sort dropdown is a
// plain `<select>` styled as the canonical `.sort` glyph; the cmdk palette
// owns the search input itself, so this surface stays compact.

import { useLibraryStore, type LibrarySort } from "@/features/library/stores/library-store";

const SORT_LABELS: Record<LibrarySort, string> = {
  recently_used: "recently used",
  recently_edited: "recently edited",
  title: "title",
  launch_count: "most launched",
};

export function LibraryToolbar(): React.JSX.Element {
  const sort = useLibraryStore((s) => s.sort);
  const setSort = useLibraryStore((s) => s.setSort);

  return (
    <div className="list-header">
      <span className="title">All prompts</span>
      <label className="sort" style={{ cursor: "pointer" }}>
        <select
          value={sort}
          onChange={(e) => setSort(e.target.value as LibrarySort)}
          style={{
            appearance: "none",
            background: "transparent",
            border: "none",
            color: "inherit",
            font: "inherit",
            letterSpacing: "inherit",
            cursor: "pointer",
            padding: "0 14px 0 0",
          }}
          aria-label="Sort prompts"
        >
          {Object.entries(SORT_LABELS).map(([value, label]) => (
            <option key={value} value={value}>
              {label}
            </option>
          ))}
        </select>
        <span className="glyph" aria-hidden="true">
          ▾
        </span>
      </label>
    </div>
  );
}
