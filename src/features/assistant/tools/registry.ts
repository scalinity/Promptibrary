// SCA-938 — three concrete tool handlers for the assistant agent loop.
//
// Tool definitions advertised to Anthropic. Each tool's input_schema is a
// minimal JSON Schema describing the call shape. The handler resolves
// against the live prompt editor store + the loaded Prompt, and (for
// mutations) goes through the existing setBody / updatePrompt paths so
// downstream invariants (autosave, query invalidation, vault file write)
// trigger as they would for a human edit.

import type { Prompt } from "@/shared/types/prompt";
import type {
  ToolDefinition,
  MessageContent,
} from "@/shared/types/assistant";
import type { PromptId } from "@/shared/types/ids";
import { updatePrompt } from "@/shared/api/ipc";
import { usePromptEditorStore } from "@/features/prompt/stores/prompt-editor-store";
import type { ToolDispatcher } from "@/features/assistant/hooks/use-assistant";

export const ASSISTANT_TOOL_DEFINITIONS: ToolDefinition[] = [
  {
    name: "get_current_prompt",
    description:
      "Read the prompt the user is currently editing. Returns title, body, summary, tags, and variables exactly as they appear in the editor (including unsaved edits). Use this BEFORE writing improvements so you understand what's already there.",
    input_schema: { type: "object", properties: {}, additionalProperties: false },
  },
  {
    name: "update_prompt_body",
    description:
      "Replace the entire body of the prompt the user is editing. The body is the markdown text below the YAML frontmatter. Variables use the {{type:key}} syntax — preserve existing references unless the user explicitly asks you to add or remove them.",
    input_schema: {
      type: "object",
      properties: {
        body: {
          type: "string",
          description:
            "The full replacement body text. Do NOT include frontmatter, just the markdown body.",
        },
      },
      required: ["body"],
      additionalProperties: false,
    },
  },
  {
    name: "update_prompt_title",
    description:
      "Replace the title of the prompt the user is editing. Concise, imperative, 5-120 characters.",
    input_schema: {
      type: "object",
      properties: {
        title: {
          type: "string",
          description: "The replacement title (5-120 chars).",
        },
      },
      required: ["title"],
      additionalProperties: false,
    },
  },
];

/**
 * Build a ToolDispatcher closure bound to the open prompt. The returned
 * function is passed to `useAssistant({ dispatchTool: ... })`.
 */
export function buildToolDispatcher(prompt: Prompt | null): ToolDispatcher {
  return async (name, input) => {
    if (prompt == null) {
      return {
        content: "No prompt is currently open — cannot execute tools.",
        isError: true,
      };
    }
    switch (name) {
      case "get_current_prompt": {
        const liveBody = readLiveBody(prompt.id, prompt.body);
        const snapshot = {
          title: prompt.title,
          body: liveBody,
          summary: prompt.summary ?? null,
          tags: prompt.tags,
          variables: prompt.variables,
        };
        return { content: JSON.stringify(snapshot) };
      }
      case "update_prompt_body": {
        const body = readString(input, "body");
        if (body == null) {
          return { content: "Missing required string field: body", isError: true };
        }
        usePromptEditorStore.getState().setBody(prompt.id, body);
        return {
          content: `Body updated (${body.length} chars). The change is visible in the editor and will save shortly.`,
        };
      }
      case "update_prompt_title": {
        const title = readString(input, "title");
        if (title == null) {
          return { content: "Missing required string field: title", isError: true };
        }
        try {
          await updatePrompt({ id: prompt.id, title });
          return { content: `Title updated to "${title}".` };
        } catch (err) {
          return {
            content: err instanceof Error ? err.message : String(err),
            isError: true,
          };
        }
      }
      default:
        return null; // hook synthesizes a generic "unknown tool" result
    }
  };
}

function readString(
  input: Record<string, unknown>,
  key: string,
): string | null {
  const v = input[key];
  return typeof v === "string" ? v : null;
}

function readLiveBody(promptId: PromptId, fallback: string): string {
  const draft = usePromptEditorStore.getState().body[promptId];
  return draft ?? fallback;
}

/**
 * Best-effort coercion of a tool-result content payload (Anthropic only
 * accepts strings or content blocks). Exposed for downstream code that
 * wants to embed structured snapshots without rolling its own JSON.
 */
export function structuredToolResult(
  toolUseId: string,
  value: unknown,
  isError = false,
): MessageContent {
  return {
    type: "tool_result",
    tool_use_id: toolUseId,
    content: typeof value === "string" ? value : JSON.stringify(value),
    is_error: isError || undefined,
  };
}
