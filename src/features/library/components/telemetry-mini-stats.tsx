// Mini telemetry strip in the detail header — version count, launch count,
// last-used timestamp.
//
// Values pulled from `Prompt.telemetry` (which is zero/null until L5 wires
// the aggregates). Nulls render as `—` so the strip layout stays stable.

import type { Prompt } from "@/shared/types/prompt";
import { formatTime24 } from "@/shared/lib/dates";

interface Props {
  prompt: Prompt;
}

export function TelemetryMiniStats({ prompt }: Props): React.JSX.Element {
  const t = prompt.telemetry;
  return (
    <div className="detail-strip">
      <span className="src">
        {prompt.source.kind === "manual"
          ? "manual entry"
          : (prompt.source.kind === "youtube" ||
                prompt.source.kind === "x_twitter" ||
                prompt.source.kind === "article") &&
              "originUrl" in prompt.source
            ? prompt.source.originUrl ?? "—"
            : "—"}
      </span>
      <span className="stats">
        <span>
          <span className="num">{t.launchCount}</span> launches
        </span>
        <span>
          last{" "}
          <span className="num">
            {t.lastUsedAt != null ? formatTime24(t.lastUsedAt) : "—"}
          </span>
        </span>
        <span>
          ok{" "}
          <span className="num">
            {t.successRate != null
              ? `${Math.round(t.successRate * 100)}%`
              : "—"}
          </span>
        </span>
      </span>
    </div>
  );
}
