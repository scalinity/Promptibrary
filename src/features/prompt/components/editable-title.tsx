// Inline-editable prompt title.
//
// Controlled input styled to match the static h1 it replaces in the editor
// header. Saves on blur and Enter; reverts on Escape. Empty input reverts
// to the previous title rather than persisting a blank value.
//
// State resets between prompts via `key={prompt.id}` at the call site, so
// no effect is needed to sync the draft when navigating.

import { useState } from "react";

import { useUpdatePrompt } from "@/features/prompt/hooks/use-update-prompt";
import type { Prompt } from "@/shared/types/prompt";

// Mirrors MAX_PROMPT_TITLE_CHARS in src-tauri/src/commands/prompts.rs (SCA-966).
const MAX_TITLE_LEN = 200;

interface Props {
  prompt: Prompt;
}

export function EditableTitle({ prompt }: Props): React.JSX.Element {
  const [draft, setDraft] = useState(prompt.title);
  const updateMutation = useUpdatePrompt();

  const commit = (): void => {
    const next = draft.trim();
    if (next.length === 0) {
      setDraft(prompt.title);
      return;
    }
    if (next === prompt.title) return;
    updateMutation.mutate({ id: prompt.id, title: next });
  };

  return (
    <input
      type="text"
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter" && !e.metaKey && !e.ctrlKey) {
          e.preventDefault();
          e.currentTarget.blur();
        } else if (e.key === "Escape") {
          e.preventDefault();
          setDraft(prompt.title);
          e.currentTarget.blur();
        }
      }}
      maxLength={MAX_TITLE_LEN}
      spellCheck={false}
      aria-label="Prompt title"
      style={{
        display: "block",
        width: "100%",
        boxSizing: "border-box",
        fontFamily: "var(--font-display)",
        fontWeight: 500,
        fontSize: 22,
        lineHeight: 1.2,
        letterSpacing: "-0.02em",
        color: "var(--ink-primary)",
        background: "transparent",
        border: "none",
        outline: "none",
        padding: 0,
        margin: "4px 0 0",
      }}
    />
  );
}
