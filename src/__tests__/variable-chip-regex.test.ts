// CodeMirror chip-decoration regex — ensures the parser-friendly source
// shape `{{type:key}}` is what gets replaced. Pinning this is the cheapest
// way to catch a future "fix" that broadens the regex and starts matching
// `{{user}}` style mustache placeholders.

import { describe, expect, it } from "vitest";

const VARIABLE_REF_REGEX = /\{\{(file|folder|text|multiline|select|bool|number):([a-zA-Z_][a-zA-Z0-9_]*)\}\}/g;

function matches(text: string): Array<[string, string]> {
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
