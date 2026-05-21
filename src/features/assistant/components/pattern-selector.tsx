// SCA-936 — Pattern dropdown for the assistant drawer header.
//
// Reads the per-session override from the assistant store (if any) and
// falls back to the global `assistantDefaultPattern` setting. Selecting
// a value updates only the session-local override — to change the global
// default the user goes to Settings → Assistant.

import { useSettings } from "@/features/settings/hooks/use-settings";
import { useAssistantStore } from "@/features/assistant/store/assistant-store";
import { Dropdown, type DropdownOption } from "@/shared/ui/dropdown";
import type { AssistantPattern } from "@/shared/types/settings";

const OPTIONS: DropdownOption<AssistantPattern>[] = [
  { value: "improve_prompt", label: "Improve Prompt" },
  { value: "improve_prompt_xml", label: "Improve Prompt XML" },
  { value: "improve_writing", label: "Improve Writing" },
];

export function AssistantPatternSelector(): React.JSX.Element {
  const selectedPattern = useAssistantStore((s) => s.selectedPattern);
  const setSelectedPattern = useAssistantStore((s) => s.setSelectedPattern);
  const settings = useSettings();
  const fallback = settings.data?.local.assistantDefaultPattern ?? "improve_prompt";
  const current = selectedPattern ?? fallback;

  return (
    <Dropdown
      ariaLabel="assistant pattern"
      value={current}
      options={OPTIONS}
      onChange={(v) => setSelectedPattern(v)}
    />
  );
}
