// SCA-936 — Assistant drawer.
//
// Right-side overlay (380px) on /prompt/:id. Header: § marker + pattern
// selector + close button. Body: scrolling message list. Footer: composer
// (textarea + send button). ⌘I toggles open.
//
// The agent loop lives in `useAssistant`; this component is a thin shell
// that wires user input → `send` and renders the store's message state.
//
// SCA-951 — styling lives in the feature-local CSS module (./assistant-panel.css)
// per the tokens-only policy. No hex literals in this file.

import { useMemo } from "react";

import { useSettings } from "@/features/settings/hooks/use-settings";
import {
  conversationFor,
  useAssistantStore,
  type UiMessage,
} from "@/features/assistant/store/assistant-store";
import { useAssistant } from "@/features/assistant/hooks/use-assistant";
import { AssistantPatternSelector } from "@/features/assistant/components/pattern-selector";
import { StreamingText } from "@/features/assistant/components/streaming-text";
import { Patterns } from "@/features/assistant/constants";
import {
  ASSISTANT_TOOL_DEFINITIONS,
  buildToolDispatcher,
} from "@/features/assistant/tools/registry";
import { usePrompt } from "@/features/prompt/hooks/use-prompt";
import type { PromptId } from "@/shared/types/ids";

import "@/features/assistant/assistant-panel.css";

interface Props {
  promptId: PromptId;
}

export function AssistantPanel({ promptId }: Props): React.JSX.Element {
  const isOpen = useAssistantStore((s) => s.isOpen);
  const setOpen = useAssistantStore((s) => s.setOpen);
  const draft = useAssistantStore((s) => s.draft);
  const setDraft = useAssistantStore((s) => s.setDraft);
  const status = useAssistantStore((s) => s.status);
  const selectedPattern = useAssistantStore((s) => s.selectedPattern);

  const settings = useSettings();
  const settingsPattern =
    settings.data?.local.assistantDefaultPattern ?? "improve_prompt";
  const settingsModel =
    settings.data?.local.assistantModel ?? "claude-sonnet-4-6";
  const pattern = selectedPattern ?? settingsPattern;
  const patternDef = Patterns[pattern] ?? Patterns.improve_prompt;

  const messages = useAssistantStore((s) => conversationFor(s, promptId));

  const promptQuery = usePrompt(promptId);
  const prompt = promptQuery.data ?? null;
  const dispatchTool = useMemo(() => buildToolDispatcher(prompt), [prompt]);

  const { send } = useAssistant({
    promptId,
    systemPrompt: patternDef.systemPrompt,
    pattern,
    model: settingsModel,
    tools: ASSISTANT_TOOL_DEFINITIONS,
    dispatchTool,
  });

  const isStreaming =
    status.kind === "streaming" || status.kind === "tool_dispatch";

  async function onSend(): Promise<void> {
    const text = draft;
    setDraft("");
    await send(text);
  }

  // SCA-953 — keep the entire DOM tree mounted even when closed so
  // scroll position and any other internal child state survives a
  // ⌘I close/reopen cycle. `data-open` drives the display: none rule.
  return (
    <aside
      role="complementary"
      aria-label="Assistant"
      aria-hidden={!isOpen}
      data-testid="assistant-panel"
      data-open={isOpen ? "true" : "false"}
      className="assistant-drawer"
    >
      <header className="assistant-drawer-header">
        <div>
          <div className="assistant-drawer-label">§ assistant</div>
          <AssistantPatternSelector />
        </div>
        <button
          type="button"
          className="icon-btn"
          aria-label="Close assistant"
          onClick={() => setOpen(false)}
          title="Close (⌘I)"
        >
          ✕
        </button>
      </header>
      <div
        className="assistant-drawer-body"
        role="log"
        aria-live="polite"
      >
        {messages.length === 0 && (
          <EmptyConversationHint patternLabel={patternDef.label} />
        )}
        {messages.map((m) => (
          <MessageBubble key={m.id} message={m} />
        ))}
        {status.kind === "error" && (
          <div role="alert" className="assistant-error-banner">
            error · {status.message}
          </div>
        )}
      </div>
      <footer className="assistant-drawer-footer">
        <textarea
          aria-label="Message to assistant"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
              e.preventDefault();
              void onSend();
            }
          }}
          placeholder={
            isStreaming
              ? "Streaming…"
              : `Ask the assistant — pattern: ${patternDef.label}`
          }
          rows={3}
          disabled={isStreaming}
          className="assistant-composer"
        />
        <div className="assistant-composer-actions">
          <button
            type="button"
            className="btn-launch"
            disabled={isStreaming || draft.trim().length === 0}
            onClick={onSend}
          >
            send <span className="kbd-inline">⌘↵</span>
          </button>
        </div>
      </footer>
    </aside>
  );
}

function EmptyConversationHint({
  patternLabel,
}: {
  patternLabel: string;
}): React.JSX.Element {
  return (
    <div className="assistant-empty-hint">
      <p>
        Describe what you want, then send. The assistant uses{" "}
        <strong>{patternLabel}</strong> to shape the prompt.
      </p>
      <p className="hint-meta">⌘I closes the drawer · ⌘↵ sends</p>
    </div>
  );
}

function MessageBubble({ message }: { message: UiMessage }): React.JSX.Element {
  return (
    <div className="assistant-message" data-role={message.role}>
      <div className="assistant-message-role">
        {message.role === "user" ? "you" : "assistant"}
        {message.isStreaming ? " · streaming…" : null}
      </div>
      <div className="assistant-message-bubble">
        {message.content.map((block, i) => {
          if (block.type === "text") {
            // Assistant text uses the fade-in stream component; user text
            // doesn't stream and stays plain.
            if (message.role === "assistant") {
              return (
                <StreamingText
                  key={i}
                  text={block.text}
                  isStreaming={message.isStreaming}
                />
              );
            }
            return <span key={i}>{block.text}</span>;
          }
          if (block.type === "tool_use") {
            return (
              <ToolCallChip
                key={i}
                name={block.name}
                input={block.input}
              />
            );
          }
          if (block.type === "tool_result") {
            return (
              <ToolResultChip
                key={i}
                content={block.content}
                isError={block.is_error ?? false}
              />
            );
          }
          return null;
        })}
      </div>
    </div>
  );
}

function ToolCallChip({
  name,
  input,
}: {
  name: string;
  input: Record<string, unknown>;
}): React.JSX.Element {
  const summary = useMemo(() => {
    const keys = Object.keys(input);
    if (keys.length === 0) return "no args";
    return keys.join(", ");
  }, [input]);
  return (
    <div className="assistant-tool-chip">
      <span>⚙</span>
      <span>
        {name}({summary})
      </span>
    </div>
  );
}

function ToolResultChip({
  content,
  isError,
}: {
  content: string;
  isError: boolean;
}): React.JSX.Element {
  return (
    <div
      className="assistant-tool-result"
      data-error={isError ? "true" : "false"}
    >
      {isError ? "✗" : "✓"}{" "}
      {content.length > 80 ? content.slice(0, 80) + "…" : content}
    </div>
  );
}
