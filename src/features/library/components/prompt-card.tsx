// Single prompt row — matches `.row` in the canonical app.css.
//
// The 3px left spine encodes state via the row's state class
// (`.selected` / `.running` / `.recent`). Title is Switzer 500, tag chips
// inherit the canonical `.row-tags > span` style, meta line is mono dim.
//
// Data shape is the slim `PromptListItem` returned by `listPrompts` — the
// full prompt body / variables / telemetry isn't available here. Telemetry
// chips light up on the prompt detail route where the full Prompt is fetched.

import type { PromptListItem } from "@/shared/api/ipc";
import type { PromptId } from "@/shared/types/ids";
import { cn } from "@/shared/lib/utils";
import { formatRelative } from "@/shared/lib/dates";

interface PromptCardProps {
  prompt: PromptListItem;
  selected: boolean;
  running?: boolean;
  recent?: boolean;
  onSelect: (id: PromptId) => void;
}

export function PromptCard({
  prompt,
  selected,
  running = false,
  recent = false,
  onSelect,
}: PromptCardProps): React.JSX.Element {
  return (
    <div
      role="button"
      tabIndex={0}
      className={cn(
        "row",
        selected && "selected",
        !selected && running && "running",
        !selected && !running && recent && "recent",
      )}
      onClick={() => onSelect(prompt.id)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect(prompt.id);
        }
      }}
      data-testid="prompt-row"
    >
      <div className="row-title">{prompt.title}</div>
      {prompt.tags.length > 0 && (
        <div className="row-tags">
          {prompt.tags.slice(0, 3).map((tag) => (
            <span key={tag}>{tag}</span>
          ))}
        </div>
      )}
      <div className="row-meta">
        {running ? (
          <span>running · live</span>
        ) : prompt.archivedAt != null ? (
          <span>archived {formatRelative(prompt.archivedAt)}</span>
        ) : (
          <span>{prompt.summary || prompt.slug}</span>
        )}
      </div>
    </div>
  );
}
