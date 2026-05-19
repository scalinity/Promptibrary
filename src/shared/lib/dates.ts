// ISO datetime → display formatters.
//
// Pure functions for the chrome (row meta line, status bar, history list).
// All inputs are ISO 8601 strings as written by the Rust side.

const MS_MIN = 60_000;
const MS_HOUR = 3_600_000;
const MS_DAY = 86_400_000;

/**
 * Format an ISO datetime as a relative-time string with the design system's
 * voice: short, lowercase, mono-friendly. `now` is injectable for tests.
 */
export function formatRelative(iso: string, now: Date = new Date()): string {
  const target = new Date(iso);
  const diff = now.getTime() - target.getTime();
  if (Number.isNaN(diff)) return iso;

  const future = diff < 0;
  const abs = Math.abs(diff);

  if (abs < MS_MIN) return future ? "soon" : "just now";
  if (abs < MS_HOUR) {
    const m = Math.round(abs / MS_MIN);
    return future ? `in ${m}m` : `${m}m ago`;
  }
  if (abs < MS_DAY) {
    const h = Math.round(abs / MS_HOUR);
    return future ? `in ${h}h` : `${h}h ago`;
  }
  // Calendar-day stamps when the difference is ≥ 1 day. Use the local
  // tz-aware clock; the status line is dense enough that 24h precision
  // beats "5 days ago" for chrome consistency.
  const sameYear = target.getFullYear() === now.getFullYear();
  return target.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    ...(sameYear ? {} : { year: "numeric" }),
  });
}

/**
 * Format an ISO datetime as a 24h clock (`14:08`). Used in the launch-row
 * crumbs and the past-run header.
 */
export function formatTime24(iso: string): string {
  const target = new Date(iso);
  if (Number.isNaN(target.getTime())) return iso;
  return target.toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

/**
 * Format an absolute duration in seconds as a compact `Nm Ss` string.
 */
export function formatDurationSeconds(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "—";
  const m = Math.floor(seconds / 60);
  const s = Math.round(seconds % 60);
  if (m === 0) return `${s}s`;
  return `${m}m ${s.toString().padStart(2, "0")}s`;
}
