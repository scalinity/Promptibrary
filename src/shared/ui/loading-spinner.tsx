// Loading spinner with an optional label.
//
// Uses the monospace family + accent token so it visually matches the chrome
// rather than introducing a generic spinner aesthetic.

import { cn } from "@/shared/lib/utils";

export interface LoadingSpinnerProps {
  label?: string;
  className?: string;
}

export function LoadingSpinner({
  label,
  className,
}: LoadingSpinnerProps): React.JSX.Element {
  return (
    <div
      role="status"
      aria-live="polite"
      className={cn(
        "inline-flex items-center gap-2 font-mono text-[11px]",
        className,
      )}
      style={{ color: "var(--ink-tertiary)", letterSpacing: "0.04em" }}
    >
      <svg
        width="12"
        height="12"
        viewBox="0 0 12 12"
        aria-hidden="true"
        style={{ animation: "pb-spin 0.9s linear infinite" }}
      >
        <circle
          cx="6"
          cy="6"
          r="4.5"
          fill="none"
          stroke="var(--border-mid)"
          strokeWidth="1.2"
        />
        <path
          d="M 6 1.5 A 4.5 4.5 0 0 1 10.5 6"
          fill="none"
          stroke="var(--accent)"
          strokeWidth="1.4"
          strokeLinecap="round"
        />
      </svg>
      {label != null && <span>{label}</span>}
    </div>
  );
}
