// Fetched-source fixtures used by the IPC mock and the L4 import visual
// regression specs. Each fixture corresponds to one of the three source
// kinds Promptibrary can extract from.

import { asIsoDateTime } from "@/shared/types/ids";
import type { FetchedSourceContent } from "@/shared/types/extraction";

const FETCHED_AT = asIsoDateTime("2026-05-19T12:00:00Z");

export const ARTICLE_FIXTURE: FetchedSourceContent = {
  source: {
    kind: "article",
    originUrl: "https://blog.scalinity.com/refactor-streaming",
    title: "Refactor a Streaming Response Handler",
    author: "Scalinity Team",
    fetchedAt: FETCHED_AT,
    contentHash: "deadbeef0001",
    siteName: "Scalinity Blog",
    byline: "Scalinity Team",
    publishedAt: asIsoDateTime("2026-05-12T09:30:00Z"),
  },
  canonicalUrl: "https://blog.scalinity.com/refactor-streaming",
  fetchedAt: FETCHED_AT,
  title: "Refactor a Streaming Response Handler",
  author: "Scalinity Team",
  text:
    "# Refactor a Streaming Response Handler\n\n" +
    "This walkthrough captures the workflow we use when one of our agents " +
    "needs to pair with a senior engineer on a streaming handler — clone, " +
    "branch, reproduce, change, verify, ship.\n\n" +
    "1. Pull `{{folder:repo}}` and check out `{{text:branch}}`.\n" +
    "2. Reproduce the symptom against the failing test in `{{file:failing_test}}`.\n" +
    "3. Apply the change.\n" +
    "4. Run the full suite and capture the diff.\n",
  chunks: [
    {
      kind: "article",
      order: 0,
      text: "Refactor a Streaming Response Handler\nThis walkthrough captures the workflow…",
      url: "https://blog.scalinity.com/refactor-streaming",
      timestampSeconds: null,
    },
  ],
  rawMetadata: {},
  contentHash: "deadbeef0001",
  cached: false,
};

export const YOUTUBE_FIXTURE: FetchedSourceContent = {
  source: {
    kind: "youtube",
    originUrl: "https://www.youtube.com/watch?v=fixture",
    title: "Plan · Build · Verify Three-Agent Loop",
    author: "Promptibrary Channel",
    fetchedAt: FETCHED_AT,
    contentHash: "deadbeef0002",
    videoId: "fixture",
    channelName: "Promptibrary Channel",
    transcriptLanguage: "en",
    durationSeconds: 612,
  },
  canonicalUrl: "https://www.youtube.com/watch?v=fixture",
  fetchedAt: FETCHED_AT,
  title: "Plan · Build · Verify Three-Agent Loop",
  author: "Promptibrary Channel",
  text:
    "Welcome to the Plan-Build-Verify pattern.\nThe planner reads the spec at the start.\nThe builder writes code.\nThe verifier runs tests and reports.",
  chunks: [
    {
      kind: "transcript",
      order: 0,
      text: "Welcome to the Plan-Build-Verify pattern.",
      url: "https://www.youtube.com/watch?v=fixture&t=0",
      timestampSeconds: 0,
    },
    {
      kind: "transcript",
      order: 1,
      text: "The planner reads the spec at the start.",
      url: "https://www.youtube.com/watch?v=fixture&t=12",
      timestampSeconds: 12,
    },
    {
      kind: "transcript",
      order: 2,
      text: "The builder writes code.",
      url: "https://www.youtube.com/watch?v=fixture&t=28",
      timestampSeconds: 28,
    },
    {
      kind: "transcript",
      order: 3,
      text: "The verifier runs tests and reports.",
      url: "https://www.youtube.com/watch?v=fixture&t=42",
      timestampSeconds: 42,
    },
  ],
  rawMetadata: {},
  contentHash: "deadbeef0002",
  cached: false,
};

export const X_FIXTURE: FetchedSourceContent = {
  source: {
    kind: "x_twitter",
    originUrl: "https://x.com/dario/status/1",
    title: null,
    author: "Dario",
    fetchedAt: FETCHED_AT,
    contentHash: "deadbeef0003",
    postId: "1",
    username: "dario",
    threadPostIds: ["1"],
  },
  canonicalUrl: "https://x.com/dario/status/1",
  fetchedAt: FETCHED_AT,
  title: null,
  author: "Dario",
  text: "If your test passes against a mocked DB but fails in CI against a real one, the test is lying.",
  chunks: [
    {
      kind: "post",
      order: 0,
      text: "If your test passes against a mocked DB but fails in CI against a real one, the test is lying.",
      url: "https://x.com/dario/status/1",
      timestampSeconds: null,
    },
  ],
  rawMetadata: {},
  contentHash: "deadbeef0003",
  cached: false,
};

/// Pick the right fixture by URL shape — used by the IPC mock so visual
/// specs don't need separate per-route handlers.
export function fetchedSourceFixtureFor(url: string): {
  outcome: "ok";
  content: FetchedSourceContent;
} {
  if (url.includes("youtube") || url.includes("youtu.be")) {
    return { outcome: "ok", content: YOUTUBE_FIXTURE };
  }
  if (url.includes("twitter") || url.includes("x.com")) {
    return { outcome: "ok", content: X_FIXTURE };
  }
  return { outcome: "ok", content: ARTICLE_FIXTURE };
}
