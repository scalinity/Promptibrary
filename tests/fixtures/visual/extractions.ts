// LLM-extraction fixture used by the IPC mock and the L4 import visual
// regression specs. Three candidates of varying confidence so the
// candidate list spec shows the full chip range.

import type {
  CandidatePrompt,
  ExtractionResponse,
} from "@/shared/types/extraction";

const CANDIDATES: CandidatePrompt[] = [
  {
    title: "Refactor streaming response handler",
    summary:
      "Pair with a senior engineer to reproduce a streaming handler bug, apply a tested fix, and verify.",
    body:
      "Role. You are a senior engineer pairing on {{folder:repo}} at branch {{text:branch}}.\n\n" +
      "Goal. Reproduce the symptom from {{file:failing_test}}; apply the smallest possible change; verify.\n\n" +
      "Steps.\n1. Clone {{folder:repo}}; check out {{text:branch}}.\n2. Run the failing test once to capture baseline output.\n3. Make the minimum change.\n4. Run the full suite.\n5. Report the diff and any new failures.\n\nDeliverables.\n- Patch file or branch with the fix.\n- Test output before and after.",
    tags: ["refactor", "streaming"],
    variables: [],
    launchDefaultsPatch: {},
    confidence: "high",
    rationale:
      "Concrete operational instructions for a streaming-handler fix; uses three typed variables.",
    sourceAnchors: [
      {
        chunkOrder: 0,
        quote: "captures the workflow we use",
        reason: "Sets up the role + repository scope.",
      },
    ],
  },
  {
    title: "Bisect failing test to last green commit",
    summary:
      "Use git bisect to find the introducing commit for a known regression.",
    body:
      "Goal. Find the introducing commit for {{text:failing_test}} on {{text:branch}}. Use git bisect, automating the test command with `git bisect run`. Stop only when the offending commit is identified.\n\nDeliverables.\n- The offending commit hash and a one-line diagnosis.",
    tags: ["debugging", "bisect"],
    variables: [],
    launchDefaultsPatch: {},
    confidence: "medium",
    rationale: "Classic bisect workflow; two variables; works on any repo.",
    sourceAnchors: [],
  },
  {
    title: "Generate spec from PR description",
    summary: "Draft a one-page spec from the contents of an open pull request.",
    body:
      "Read the PR at {{text:pr_url}}. Draft a one-page spec with sections: Goals, Non-goals, Open questions. Be brief; aim for under 350 words total. Save the result to docs/specs/{{text:slug}}.md.",
    tags: ["docs", "spec"],
    variables: [],
    launchDefaultsPatch: {},
    confidence: "low",
    rationale: "Lighter-weight prompt; useful but less load-bearing than the first two.",
    sourceAnchors: [],
  },
];

export const CANDIDATES_FIXTURE: ExtractionResponse = {
  schemaVersion: 1,
  candidates: CANDIDATES,
};

export function candidatesFixtureResult(): {
  outcome: "ok";
  response: ExtractionResponse;
} {
  return { outcome: "ok", response: CANDIDATES_FIXTURE };
}
