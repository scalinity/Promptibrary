// Left navigation sidebar — mirrors `.sidebar` from the design system.
//
// Renders permanent nav items, tag list (synced to L1's `suggest_tags`), and
// vault list. Tag colors resolve via `useTagColors`; counts come from
// `useLibraryStats` and stay zero until L5 wires telemetry.

import { NavLink } from "react-router-dom";

import { useLibraryStats } from "@/features/library/stores/library-store";
import { cn } from "@/shared/lib/utils";

const NAV_ITEMS = [
  { key: "all", glyph: "⌖", label: "All prompts", to: "/" },
  { key: "pinned", glyph: "★", label: "Pinned", to: "/?filter=pinned" },
  { key: "recent", glyph: "↺", label: "Recent", to: "/?filter=recent" },
  { key: "archived", glyph: "⊟", label: "Archived", to: "/?filter=archived" },
] as const;

const TAG_SWATCHES: Record<string, string> = {
  "bug-fix": "var(--accent)",
  refactor: "var(--status-running)",
  "spec-gen": "var(--paper-warm)",
  orchestration: "var(--accent-dim)",
  scratch: "var(--status-warn)",
  tests: "var(--term-path)",
};

function swatchFor(tag: string): string {
  return TAG_SWATCHES[tag] ?? "var(--ink-tertiary)";
}

export function Sidebar(): React.JSX.Element {
  const stats = useLibraryStats();

  return (
    <aside className="sidebar" aria-label="Library navigation">
      {NAV_ITEMS.map((item) => (
        <NavLink
          key={item.key}
          to={item.to}
          end={item.to === "/"}
          className={({ isActive }) => cn("nav-item", isActive && "active")}
        >
          <span className="glyph">{item.glyph}</span>
          {item.label}
          <span className="count">{stats.counts[item.key] ?? 0}</span>
        </NavLink>
      ))}

      {stats.tags.length > 0 && (
        <>
          <div className="side-section">tags</div>
          {stats.tags.map((tag) => (
            <div className="tag-side" key={tag.name}>
              <span
                className="swatch"
                style={{ background: swatchFor(tag.name) }}
                aria-hidden="true"
              />
              {tag.name}
              <span className="count">{tag.count}</span>
            </div>
          ))}
        </>
      )}

      {stats.vaults.length > 0 && (
        <>
          <div className="side-section">vaults</div>
          {stats.vaults.map((vault) => (
            <div className="nav-item" key={vault.path}>
              <span className="glyph">⊟</span>
              {vault.label}
              <span className="count">{vault.count}</span>
            </div>
          ))}
        </>
      )}
    </aside>
  );
}
