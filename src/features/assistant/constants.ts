// SCA-936 — frontend-side mirror of `src-tauri/src/assistant/patterns.rs`.
//
// Maps each pattern ID to its display label + its system prompt. The
// system prompts are inlined as TS strings here for two reasons:
//   1. We need them on the frontend to send via the IPC.
//   2. Vite can't `include_str!` like Rust does, so we'd otherwise need a
//      separate IPC just to fetch them — needless round-trips for what
//      are byte-equal constants.
//
// **Sync rule:** If `docs/spec-snippets/fabric-*.md` changes, the
// corresponding constant here must change too. The byte-equality test on
// the Rust side guarantees the spec snippet matches the Rust const; this
// file is the frontend's parallel commitment. To keep that contract
// auditable we fetch the strings via Vite's `?raw` import.

import type { AssistantPattern } from "@/shared/types/settings";

import improvePromptSystem from "../../../docs/spec-snippets/fabric-improve-prompt.md?raw";
import improvePromptXmlSystem from "../../../docs/spec-snippets/fabric-improve-prompt-xml.md?raw";
import improveWritingSystem from "../../../docs/spec-snippets/fabric-improve-writing.md?raw";

export interface PatternDef {
  id: AssistantPattern;
  label: string;
  systemPrompt: string;
}

export const Patterns: Record<AssistantPattern, PatternDef> = {
  improve_prompt: {
    id: "improve_prompt",
    label: "Improve Prompt",
    systemPrompt: improvePromptSystem,
  },
  improve_prompt_xml: {
    id: "improve_prompt_xml",
    label: "Improve Prompt XML",
    systemPrompt: improvePromptXmlSystem,
  },
  improve_writing: {
    id: "improve_writing",
    label: "Improve Writing",
    systemPrompt: improveWritingSystem,
  },
};
