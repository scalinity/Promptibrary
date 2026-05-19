// Realistic prompt fixtures matching the design-system mockup content.
// Used by both the IPC mock and the visual regression specs.

import {
  asAbsolutePath,
  asIsoDateTime,
  asPromptId,
  asRelativeVaultPath,
  asTagName,
} from "@/shared/types/ids";
import type { Prompt } from "@/shared/types/prompt";

const baseLaunchDefaults = {
  destination: "claude_code_cli" as const,
  model: "claude-sonnet-4-6" as const,
  verifierMode: "off" as const,
  workingDirectory: null,
  additionalDirectories: [],
  permissionMode: "default" as const,
  allowedTools: [],
  disallowedTools: [],
  mcpConfigPaths: [],
  strictMcpConfig: false,
  appendSystemPrompt: null,
  maxTurns: null,
};

const baseSource = {
  kind: "manual" as const,
  title: null,
  author: null,
  fetchedAt: null,
  contentHash: null,
};

function makePrompt(
  ulid: string,
  title: string,
  summary: string,
  body: string,
  tags: string[],
  updated: string,
  launchCount = 0,
  archived = false,
): Prompt {
  return {
    id: asPromptId(ulid),
    title,
    slug: title
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/(^-|-$)/g, ""),
    summary,
    body,
    vaultPath: asRelativeVaultPath(
      `prompts/${title
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/(^-|-$)/g, "")}.md`,
    ),
    createdAt: asIsoDateTime("2026-04-12T09:14:00Z"),
    updatedAt: asIsoDateTime(updated),
    archivedAt: archived ? asIsoDateTime("2026-04-29T18:00:00Z") : null,
    tags: tags.map((t) => asTagName(t)),
    source: baseSource,
    variables: [],
    launchDefaults: {
      ...baseLaunchDefaults,
      workingDirectory: asAbsolutePath("/Users/operator/code/aurora"),
    },
    telemetry: {
      launchCount,
      lastUsedAt: launchCount > 0 ? asIsoDateTime(updated) : null,
      successRate: launchCount > 0 ? 0.84 : null,
      avgRunSeconds: launchCount > 0 ? 188 : null,
      avgTokenCount: launchCount > 0 ? 18500 : null,
    },
    checksumSha256: `sha256:${ulid.slice(0, 16)}aabbccdd`,
  };
}

const STREAM_HANDLER_BODY = `# Refactor — streaming response handler

Role. You are a senior engineer pairing on {{folder:repo}} at {{text:branch}}.

Goal. Reproduce the failure in {{file:failing_test}}, then bisect to the
introducing commit. The fix should preserve back-pressure on the consumer
side and not break the existing keepalive flag.

Constraints.
  · Touch only files under src/lib/stream/
  · Add or modify tests — never delete green tests
  · No new dependencies

Output. A unified diff plus a one-paragraph rationale.

Context. {{multiline:extra_context}}
`;

const SPEC_GEN_BODY = `# Spec from PR description

Role. You write specifications a careful engineer can implement without
clarifying questions.

Input. The PR description and diff on branch {{text:pr_branch}}.

Output structure.
  1. Problem statement (no jargon)
  2. Acceptance criteria — testable, numbered
  3. Out-of-scope
  4. Open questions (only if any remain)
`;

const BISECT_BODY = `# Bisect failing test → last green commit

Goal. Find the commit that introduced the regression for {{file:failing_test}}.

Method.
  · Confirm failure on HEAD, success on {{text:last_known_good_sha}}.
  · Run \`git bisect\` with the test as the script.
  · Report: introducing commit, author, one-line summary.

Stop. Do not propose a fix; another prompt handles that.
`;

const MIGRATE_TOML_BODY = `# Migrate config from JSON to TOML

Working directory. {{folder:repo}}

Step 1. Inventory every \`*.json\` config file the app reads. Skip
\`package.json\`, \`tsconfig.json\`, and lockfiles.

Step 2. For each, produce an equivalent \`.toml\` file matching the schema
the app expects. Comments preserved as TOML comments.

Step 3. Update the loader to read TOML, with a one-release window where
JSON is still accepted as a fallback. Behind a {{bool:enable_compat_loader}}.
`;

const PBV_LOOP_BODY = `# Plan · build · verify (three-agent loop)

Phase 1 — plan. Read {{file:spec}} and produce a step-by-step implementation
plan. No code yet.

Phase 2 — build. Implement the plan against {{folder:repo}}. Touch as few
files as possible.

Phase 3 — verify. Run the full test suite. If any test fails, report what
failed and stop — do not loop on your own.

Model. {{select:model}}
`;

export const PROMPT_FIXTURES: Prompt[] = [
  makePrompt(
    "01HX1ABCDEFGHJKMNPQRSTV01",
    "Refactor streaming response handler",
    "Bisect, reproduce, and fix without breaking back-pressure or keepalive.",
    STREAM_HANDLER_BODY,
    ["bug-fix", "backend", "stream", "verifier-loop"],
    "2026-05-18T18:08:00Z",
    37,
  ),
  makePrompt(
    "01HX2ABCDEFGHJKMNPQRSTV02",
    "Generate spec from PR description",
    "Turn a PR description + diff into a numbered, testable spec.",
    SPEC_GEN_BODY,
    ["spec-gen", "orchestration"],
    "2026-05-18T17:54:00Z",
    21,
  ),
  makePrompt(
    "01HX3ABCDEFGHJKMNPQRSTV03",
    "Bisect failing test to last green commit",
    "Locate the introducing commit; do not propose a fix.",
    BISECT_BODY,
    ["bug-fix", "tests"],
    "2026-05-18T17:42:00Z",
    9,
  ),
  makePrompt(
    "01HX4ABCDEFGHJKMNPQRSTV04",
    "Migrate config from JSON to TOML",
    "Inventory → translate → swap loader with a compat window.",
    MIGRATE_TOML_BODY,
    ["refactor"],
    "2026-05-17T22:00:00Z",
    6,
  ),
  makePrompt(
    "01HX5ABCDEFGHJKMNPQRSTV05",
    "Plan · build · verify (three-agent loop)",
    "Decompose a spec into plan + build + verify phases with hard stops.",
    PBV_LOOP_BODY,
    ["orchestration", "multi-agent"],
    "2026-04-04T11:30:00Z",
    21,
  ),
  makePrompt(
    "01HX6ABCDEFGHJKMNPQRSTV06",
    "Write failing test from a bug report",
    "Convert a reproduction into a deterministic failing test.",
    "# Failing test from bug report\n\nReport: {{multiline:bug_report}}\n",
    ["bug-fix", "tests"],
    "2026-04-02T16:20:00Z",
    4,
  ),
  makePrompt(
    "01HX7ABCDEFGHJKMNPQRSTV07",
    "Inline a single-use util & delete the file",
    "Find single-call utilities and inline them with their caller.",
    "# Inline single-use util\n\nTarget. {{file:util_file}}\n",
    ["refactor"],
    "2026-03-28T14:00:00Z",
    2,
  ),
  makePrompt(
    "01HX8ABCDEFGHJKMNPQRSTV08",
    "Draft a CHANGELOG entry from git log",
    "Turn a range of commits into a user-facing changelog block.",
    "# Changelog draft\n\nRange. {{text:commit_range}}\n",
    ["scratch"],
    "2026-03-24T10:00:00Z",
    1,
  ),
];

export const TAG_FIXTURES = [
  { name: "bug-fix", count: 38 },
  { name: "refactor", count: 22 },
  { name: "spec-gen", count: 14 },
  { name: "orchestration", count: 9 },
  { name: "scratch", count: 17 },
  { name: "tests", count: 19 },
];
