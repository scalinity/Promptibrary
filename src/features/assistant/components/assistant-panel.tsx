// SCA-936 — Assistant drawer.
//
// Right-side overlay (380px) on /prompt/:id. Header: § marker + pattern
// selector + close button. Body: scrolling message list. Footer: composer
// (textarea + send button). ⌘I toggles open.
//
// The agent loop lives in `useAssistant`; this component is a thin shell
// that wires user input → `send` and renders the store's message state.
// Tool dispatch is a stub here (returns null for every tool) and lands
// for real in SCA-XXX (#10).

import { useMemo } from "react";

import { useSettings } from "@/features/settings/hooks/use-settings";
import {
  conversationFor,
  useAssistantStore,
  type UiMessage,
} from "@/features/assistant/store/assistant-store";
import { useAssistant } from "@/features/assistant/hooks/use-assistant";
import { AssistantPatternSelector } from "@/features/assistant/components/pattern-selector";
import { Patterns } from "@/features/assistant/constants";
import type { PromptId } from "@/shared/types/ids";

interface Props {
  promptId: PromptId;
}

export function AssistantPanel({ promptId }: Props): React.JSX.Element | null {
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
  const patternDef = Patterns[pattern];

  const messages = useAssistantStore((s) => conversationFor(s, promptId));

  const { send } = useAssistant({
    promptId,
    systemPrompt: patternDef.systemPrompt,
    pattern,
    model: settingsModel,
    tools: [], // SCA-XXX (#10) wires the three concrete tools.
    dispatchTool: async () => null,
  });

  const isStreaming = status.kind === "streaming" || status.kind === "tool_dispatch";

  // We render the drawer with a transition; keeping the DOM mounted while
  // closed (display: none) avoids losing scroll + composer state across
  // toggles.
  const drawerStyle = useMemo<React.CSSProperties>(
    () => ({
      position: "fixed",
      top: 0,
      right: 0,
      bottom: 0,
      width: 380,
      background: "var(--bg-elev-1, #0f1216)",
      borderLeft: "var(--hairline)",
      display: isOpen ? "flex" : "none",
      flexDirection: "column",
      zIndex: 60,
      boxShadow: "-1px 0 0 rgba(255,255,255,0.04)",
    }),
    [isOpen],
  );

  if (!isOpen) return <div style={drawerStyle} aria-hidden />;

  return (
    <aside
      role="complementary"
      aria-label="Assistant"
      data-testid="assistant-panel"
      style={drawerStyle}
    >
      <header
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "var(--sp-4) var(--sp-5)",
          borderBottom: "var(--hairline)",
        }}
      >
        <div>
          <div
            style={{
              fontFamily: "var(--font-mono)",
              fontSize: 10.5,
              color: "var(--ink-tertiary)",
              letterSpacing: "0.04em",
              textTransform: "uppercase",
            }}
          >
            § assistant
          </div>
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
        role="log"
        aria-live="polite"
        style={{
          flex: 1,
          overflow: "auto",
          padding: "var(--sp-5)",
          display: "flex",
          flexDirection: "column",
          gap: 12,
        }}
      >
        {messages.length === 0 && (
          <EmptyConversationHint patternLabel={patternDef.label} />
        )}
        {messages.map((m) => (
          <MessageBubble key={m.id} message={m} />
        ))}
        {status.kind === "error" && (
          <div
            role="alert"
            style={{
              fontFamily: "var(--font-mono)",
              fontSize: 11,
              color: "var(--ink-danger, #ef4444)",
              padding: "8px 12px",
              border: "var(--hairline)",
              borderRadius: "var(--r-sm)",
            }}
          >
            error · {status.message}
          </div>
        )}
      </div>
      <footer
        style={{
          padding: "var(--sp-4)",
          borderTop: "var(--hairline)",
          display: "flex",
          flexDirection: "column",
          gap: 8,
        }}
      >
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
          className="input"
          style={{
            fontFamily: "var(--font-ui)",
            fontSize: 13,
            resize: "none",
            width: "100%",
            background: "var(--bg-base)",
            color: "var(--ink)",
            border: "var(--hairline)",
            borderRadius: "var(--r-sm)",
            padding: "8px 10px",
          }}
        />
        <div style={{ display: "flex", justifyContent: "flex-end" }}>
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

  async function onSend(): Promise<void> {
    const text = draft;
    setDraft("");
    await send(text);
  }
}

function EmptyConversationHint({
  patternLabel,
}: {
  patternLabel: string;
}): React.JSX.Element {
  return (
    <div
      style={{
        fontFamily: "var(--font-ui)",
        fontSize: 12,
        color: "var(--ink-secondary, var(--ink-dim))",
        padding: "var(--sp-3)",
      }}
    >
      <p style={{ margin: 0 }}>
        Describe what you want, then send. The assistant uses{" "}
        <strong>{patternLabel}</strong> to shape the prompt.
      </p>
      <p style={{ margin: "8px 0 0", color: "var(--ink-tertiary)" }}>
        ⌘I closes the drawer · ⌘↵ sends
      </p>
    </div>
  );
}

function MessageBubble({ message }: { message: UiMessage }): React.JSX.Element {
  const isUser = message.role === "user";
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 4,
        alignItems: isUser ? "flex-end" : "flex-start",
      }}
    >
      <div
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: 10,
          color: "var(--ink-tertiary)",
          letterSpacing: "0.04em",
          textTransform: "uppercase",
        }}
      >
        {isUser ? "you" : "assistant"}
        {message.isStreaming ? " · streaming…" : null}
      </div>
      <div
        style={{
          fontFamily: "var(--font-ui)",
          fontSize: 13,
          lineHeight: 1.5,
          color: "var(--ink)",
          background: isUser ? "var(--bg-elev-2, #161a1f)" : "transparent",
          border: isUser ? "var(--hairline)" : "none",
          borderRadius: "var(--r-sm)",
          padding: isUser ? "8px 12px" : 0,
          maxWidth: "100%",
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
        }}
      >
        {message.content.map((block, i) => {
          if (block.type === "text") return <span key={i}>{block.text}</span>;
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
    <div
      style={{
        display: "inline-flex",
        gap: 6,
        fontFamily: "var(--font-mono)",
        fontSize: 11,
        color: "var(--accent-amber, #f59e0b)",
        marginTop: 4,
      }}
    >
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
      style={{
        fontFamily: "var(--font-mono)",
        fontSize: 11,
        color: isError ? "var(--ink-danger, #ef4444)" : "var(--ink-secondary)",
      }}
    >
      {isError ? "✗" : "✓"} {content.length > 80 ? content.slice(0, 80) + "…" : content}
    </div>
  );
}
