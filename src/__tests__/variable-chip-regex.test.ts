// CodeMirror chip-decoration regex — pinned via the SHARED export. If a
// future broadening makes the editor accept `{{user}}`-style placeholders,
// these tests automatically catch it (no second copy of the regex to
// drift away from). SCA-646.

import { describe, expect, it } from "vitest";

import { VARIABLE_REF_REGEX } from "@/features/prompt/lib/variable-ref-regex";

function matches(text: string): Array<[string, string]> {
  // Reset lastIndex because the regex is `/g` and is shared across tests.
  VARIABLE_REF_REGEX.lastIndex = 0;
  return [...text.matchAll(VARIABLE_REF_REGEX)].map((m) => [m[1], m[2]]);
}

describe("variable chip regex", () => {
  it("matches every supported type", () => {
    const text =
      "{{file:a}} {{folder:b}} {{text:c}} {{multiline:d}} {{select:e}} {{bool:f}} {{number:g}}";
    expect(matches(text)).toEqual([
      ["file", "a"],
      ["folder", "b"],
      ["text", "c"],
      ["multiline", "d"],
      ["select", "e"],
      ["bool", "f"],
      ["number", "g"],
    ]);
  });

  it("rejects unknown types", () => {
    expect(matches("{{url:link}}")).toEqual([]);
    expect(matches("{{:no_type}}")).toEqual([]);
  });

  it("rejects malformed keys", () => {
    expect(matches("{{file:9bad}}")).toEqual([]);
    expect(matches("{{file:bad-name}}")).toEqual([]);
  });

  it("doesn't swallow surrounding text", () => {
    const t = "Run on {{folder:repo}} with branch {{text:branch}}";
    expect(matches(t)).toEqual([
      ["folder", "repo"],
      ["text", "branch"],
    ]);
  });
});
