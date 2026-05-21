// Single prompt row — matches `.row` in the canonical app.css.
//
// The 3px left spine encodes state via the row's state class
// (`.selected` / `.running` / `.recent`). Title is Outfit 500, tag chips
// inherit the canonical `.row-tags > span` style, meta line is mono dim.
//
// SCA-907: left-swipe reveals a red "delete" panel on the right edge.
// The gesture uses pointer events so trackpad horizontal drags and mouse
// drags both work; touch is automatic on platforms that emit pointer
// events from touch (Tauri on macOS does for trackpad two-finger swipes).
// Click still selects — we distinguish drag vs click by a movement
// threshold inside the pointerup handler.

// eslint-disable-next-line no-restricted-imports -- SCA-907: outside-pointer global subscription needs effect-style cleanup
import { useEffect, useRef, useState } from "react";

import type { PromptListItem } from "@/shared/api/ipc";
import type { PromptId } from "@/shared/types/ids";
import { cn } from "@/shared/lib/utils";
import { formatRelative } from "@/shared/lib/dates";
import { useDeletePrompt } from "@/features/library/hooks/use-delete-prompt";

interface PromptCardProps {
  prompt: PromptListItem;
  selected: boolean;
  running?: boolean;
  recent?: boolean;
  onSelect: (id: PromptId) => void;
}

const DELETE_PANEL_WIDTH = 96;
const SWIPE_REVEAL_THRESHOLD = 30;
const CLICK_VS_DRAG_THRESHOLD = 6;

export function PromptCard({
  prompt,
  selected,
  running = false,
  recent = false,
  onSelect,
}: PromptCardProps): React.JSX.Element {
  const deleteMutation = useDeletePrompt();
  const [offset, setOffset] = useState(0);
  const startX = useRef<number | null>(null);
  const draggedDistance = useRef(0);
  const rowRef = useRef<HTMLDivElement | null>(null);
  const innerRowRef = useRef<HTMLDivElement | null>(null);
  const wheelSnapTimer = useRef<number | null>(null);
  const isOpen = offset <= -SWIPE_REVEAL_THRESHOLD;

  // Outside-pointerdown closes the revealed panel. Only subscribed while
  // open so most rows have zero listeners attached.
  useEffect(() => {
    if (!isOpen) return;
    const onPointer = (e: PointerEvent) => {
      const target = e.target as Node | null;
      if (target == null || !rowRef.current?.contains(target)) {
        setOffset(0);
      }
    };
    document.addEventListener("pointerdown", onPointer);
    return () => document.removeEventListener("pointerdown", onPointer);
  }, [isOpen]);

  // SCA-914 — trackpad two-finger horizontal swipe emits wheel events
  // (with deltaX), not pointer events. React's onWheel prop is passive
  // since React 17, so preventDefault is a no-op there. Attach a
  // non-passive native listener to the inner row so we can intercept
  // horizontal swipes and update the offset.
  useEffect(() => {
    const el = innerRowRef.current;
    if (el == null) return;

    const onWheel = (e: WheelEvent) => {
      // Horizontal-dominant only — don't interfere with vertical scroll
      // of the list itself.
      if (Math.abs(e.deltaX) <= Math.abs(e.deltaY)) return;
      e.preventDefault();
      setOffset((cur) => {
        // Natural-scroll deltaX: swipe-left → positive deltaX → we want
        // offset to go MORE negative (slide left). cur - deltaX does it.
        const next = cur - e.deltaX;
        return Math.min(0, Math.max(-DELETE_PANEL_WIDTH, next));
      });

      // Debounced snap: 120ms after the last wheel event, snap to fully
      // open or closed based on whether the offset crossed the reveal
      // threshold. Clear and reset on every event so a continuous
      // gesture doesn't snap mid-swipe.
      if (wheelSnapTimer.current != null) {
        window.clearTimeout(wheelSnapTimer.current);
      }
      wheelSnapTimer.current = window.setTimeout(() => {
        wheelSnapTimer.current = null;
        setOffset((cur) =>
          cur <= -SWIPE_REVEAL_THRESHOLD ? -DELETE_PANEL_WIDTH : 0,
        );
      }, 120);
    };

    el.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      el.removeEventListener("wheel", onWheel);
      if (wheelSnapTimer.current != null) {
        window.clearTimeout(wheelSnapTimer.current);
        wheelSnapTimer.current = null;
      }
    };
  }, []);

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>): void => {
    if (e.button !== 0) return;
    startX.current = e.clientX;
    draggedDistance.current = 0;
    // SCA-908 — without setPointerCapture, fast or off-axis pointer
    // moves drop out of the row's hit area and pointermove/up stop
    // firing mid-drag. Capturing keeps the event stream on this row.
    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {
      // Safari/older browsers may throw; non-fatal — drag still works
      // for short in-bounds moves.
    }
  };

  const onPointerMove = (e: React.PointerEvent<HTMLDivElement>): void => {
    if (startX.current == null) return;
    const dx = e.clientX - startX.current;
    draggedDistance.current = Math.max(draggedDistance.current, Math.abs(dx));
    // Negative (leftward) drag only; clamp at the panel width.
    const next = Math.min(0, Math.max(-DELETE_PANEL_WIDTH, dx));
    // Engage swipe only after a small horizontal threshold so vertical
    // scrolling and accidental drift don't trigger reveal.
    if (Math.abs(dx) > CLICK_VS_DRAG_THRESHOLD) {
      setOffset(next);
    }
  };

  const onPointerUp = (e: React.PointerEvent<HTMLDivElement>): void => {
    const moved = draggedDistance.current;
    startX.current = null;
    // Snap: if past the reveal threshold, hold open; else close.
    setOffset((cur) => (cur <= -SWIPE_REVEAL_THRESHOLD ? -DELETE_PANEL_WIDTH : 0));
    // Distinguish click vs drag — only fire selection on small movement.
    if (moved < CLICK_VS_DRAG_THRESHOLD) {
      onSelect(prompt.id);
      e.preventDefault();
    }
  };

  const confirmDelete = (): void => {
    const yes = window.confirm(
      `Delete "${prompt.title}"? This removes the prompt file from the vault.`,
    );
    if (!yes) return;
    deleteMutation.mutate(prompt.id);
  };

  return (
    <div
      ref={rowRef}
      className="prompt-row-wrapper"
      style={{ position: "relative", overflow: "hidden" }}
      data-testid="prompt-row-wrapper"
    >
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          confirmDelete();
        }}
        disabled={deleteMutation.isPending}
        aria-label={`Delete ${prompt.title}`}
        style={{
          position: "absolute",
          top: 0,
          right: 0,
          bottom: 0,
          width: `${DELETE_PANEL_WIDTH}px`,
          background: "var(--status-error)",
          color: "var(--bg-deep)",
          border: "none",
          fontFamily: "var(--font-mono)",
          fontSize: "12px",
          fontWeight: 500,
          letterSpacing: "0.04em",
          textTransform: "lowercase",
          cursor: deleteMutation.isPending ? "wait" : "pointer",
        }}
      >
        {deleteMutation.isPending ? "…" : "delete"}
      </button>
      <div
        ref={innerRowRef}
        role="button"
        tabIndex={0}
        className={cn(
          "row",
          selected && "selected",
          !selected && running && "running",
          !selected && !running && recent && "recent",
        )}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onSelect(prompt.id);
          } else if (e.key === "Escape" && isOpen) {
            e.preventDefault();
            setOffset(0);
          }
        }}
        data-testid="prompt-row"
        style={{
          position: "relative",
          background: "var(--bg-base)",
          transform: `translateX(${offset}px)`,
          transition: startX.current == null ? "transform var(--motion-snap)" : "none",
          touchAction: "pan-y",
        }}
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
        {/* SCA-908 — hover-visible ✕ for desktop users who don't think
            to swipe. Stops propagation so it doesn't fire the row's
            pointer-handlers. */}
        <button
          type="button"
          className="prompt-row-delete-glyph"
          onPointerDown={(e) => e.stopPropagation()}
          onClick={(e) => {
            e.stopPropagation();
            confirmDelete();
          }}
          disabled={deleteMutation.isPending}
          aria-label={`Delete ${prompt.title}`}
          title="Delete prompt"
        >
          ✕
        </button>
      </div>
    </div>
  );
}
