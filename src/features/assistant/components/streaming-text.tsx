// SCA-937 — Smooth fade-in streaming text.
//
// As the agent loop appends chunks to a streaming text block, this
// component wraps each *delta* in its own `<span class="pb-as-tok">`
// element with a one-shot fade-in animation. The animation runs once per
// span (CSS `animation: pb-as-tok-fade-in 180ms ease-out 1`) and the spans
// stay in the DOM for normal text selection.
//
// Cost shape: one DOM node per delta arrived (Anthropic typically batches
// multiple words per delta, so a 200-word response produces on the order
// of 20-50 spans — well within React's comfortable range). If a single
// turn produces a pathological number of deltas the caller can fall back
// to a plain `<span>{text}</span>` by passing `enableAnimation={false}`.

import { useRef } from "react";

interface Props {
  /** The full accumulated text. */
  text: string;
  /** Whether the message is still receiving deltas. When `false` we
   * collapse to a single span — the animation has already played for
   * each chunk during streaming, and freezing the chunked DOM after the
   * stream finishes serves no purpose. */
  isStreaming: boolean;
  /** Escape hatch for callers that need plain rendering (e.g. visual
   * regression baselines where animations would cause noise). */
  enableAnimation?: boolean;
}

/**
 * Tracks the chunks rendered so far for a single message instance so
 * each delta keeps a stable identity across re-renders. Without this,
 * React would re-run the keyframe animation on every render even when
 * `text` only grew at the end.
 */
interface ChunkRef {
  /** Accumulated chunks. `text` is the union of these chunks. */
  chunks: string[];
}

export function StreamingText({
  text,
  isStreaming,
  enableAnimation = true,
}: Props): React.JSX.Element {
  const ref = useRef<ChunkRef>({ chunks: [] });

  // Once the stream finishes, render as plain text. This both keeps the
  // DOM lean and matches the "stable post-stream" state that downstream
  // consumers (selection, copy, screenshot diffs) expect.
  if (!isStreaming || !enableAnimation) {
    return <span>{text}</span>;
  }

  // Compute the new chunk (if any) since the last render.
  const prior = ref.current.chunks.join("");
  if (text.startsWith(prior) && text.length > prior.length) {
    ref.current.chunks.push(text.slice(prior.length));
  } else if (text !== prior) {
    // Out-of-band change (rare — e.g. caller replaced the text). Reset.
    ref.current.chunks = text.length > 0 ? [text] : [];
  }

  const chunks = ref.current.chunks;
  // Build keys that are stable across renders: chunk index + content hash
  // by length so React's reconciler reuses the same DOM nodes when the
  // chunk list grows by append.
  return (
    <>
      {chunks.map((c, i) => (
        <span
          key={`${i}:${c.length}`}
          className="pb-as-tok"
          // Honor system reduced-motion preference: the CSS rule
          // `@media (prefers-reduced-motion: reduce)` zeroes the
          // duration. No inline style override needed.
        >
          {c}
        </span>
      ))}
    </>
  );
}

/**
 * The keyframes + per-span styling live in `src/styles/app.css` as a
 * `pb-as-tok-*` block so the feature stays portable (no CSS-in-JS layer
 * just for this) and visual regression fixtures can disable the
 * animation by setting `enableAnimation={false}`. Keep this re-export
 * here so a single import covers both the React component and an
 * `ANIMATION_DURATION_MS` constant downstream code (e.g. Playwright
 * waits) can use.
 */
export const ANIMATION_DURATION_MS = 180;
