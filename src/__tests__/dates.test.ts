// Date formatter unit tests.

import { describe, expect, it } from "vitest";

import { formatRelative, formatTime24, formatDurationSeconds } from "@/shared/lib/dates";

const NOW = new Date("2026-05-18T18:08:00Z");

describe("formatRelative", () => {
  it("'just now' for <60s diff", () => {
    expect(formatRelative("2026-05-18T18:07:30Z", NOW)).toBe("just now");
  });
  it("'Nm ago' under an hour", () => {
    expect(formatRelative("2026-05-18T17:45:00Z", NOW)).toBe("23m ago");
  });
  it("'Nh ago' under a day", () => {
    expect(formatRelative("2026-05-18T08:08:00Z", NOW)).toBe("10h ago");
  });
  it("calendar day past the 24h horizon", () => {
    const out = formatRelative("2026-04-12T10:00:00Z", NOW);
    expect(out).toMatch(/Apr 12/);
  });
});

describe("formatTime24", () => {
  it("formats as HH:mm", () => {
    expect(formatTime24("2026-05-18T18:08:00Z")).toMatch(/^\d{2}:\d{2}$/);
  });
});

describe("formatDurationSeconds", () => {
  it("seconds only when under a minute", () => {
    expect(formatDurationSeconds(42)).toBe("42s");
  });
  it("minutes + zero-padded seconds otherwise", () => {
    expect(formatDurationSeconds(125)).toBe("2m 05s");
  });
  it("returns '—' for negative / NaN", () => {
    expect(formatDurationSeconds(-1)).toBe("—");
    expect(formatDurationSeconds(NaN)).toBe("—");
  });
});
