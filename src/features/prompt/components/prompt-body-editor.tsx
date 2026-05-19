// CodeMirror 6 body editor with `{{type:key}}` variable-ref decorations.
//
// The centerpiece of L2's prompt route. Implementation notes:
//
//   • Decoration.replace: each `{{file:target}}` token visually becomes a chip
//     while the source text remains intact for serialization.
//   • Byte/UTF-16 mapping: `parse_variables` returns both; CodeMirror operates
//     in UTF-16, so we use the UTF-16 offsets directly (do not derive from byte
//     offsets — multi-byte glyphs would break it).
//   • Re-parse is debounced 200ms on doc change, called via parseVariables IPC.
//   • Autocomplete on `{{` suggests variable types + already-declared keys.
//
// Styling: derive a theme from the design tokens so the editor matches the
// chrome. No hard-coded ANSI / CodeMirror default colors anywhere.

import CodeMirror, { type Extension } from "@uiw/react-codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { EditorView, Decoration, WidgetType } from "@codemirror/view";
import type { DecorationSet } from "@codemirror/view";
import { StateField, EditorState, Range } from "@codemirror/state";
import { autocompletion, type CompletionContext } from "@codemirror/autocomplete";
import { useEffect, useMemo, useState } from "react";

import { parseVariables, type VariableRef } from "@/shared/api/ipc";
import type { PromptId } from "@/shared/types/ids";

interface PromptBodyEditorProps {
  promptId: PromptId;
  value: string;
  onChange?: (value: string) => void;
  readOnly?: boolean;
}

const PROMPTIBRARY_THEME = EditorView.theme(
  {
    "&": {
      backgroundColor: "var(--bg-sunken)",
      color: "var(--ink-primary)",
      fontFamily: "var(--font-mono)",
      fontSize: "12.5px",
      lineHeight: "1.65",
      letterSpacing: "0.01em",
    },
    ".cm-content": {
      caretColor: "var(--accent)",
      padding: "var(--sp-4)",
    },
    "&.cm-focused .cm-cursor": { borderLeftColor: "var(--accent)" },
    "&.cm-focused": { outline: "none" },
    ".cm-gutters": {
      backgroundColor: "var(--bg-sunken)",
      color: "var(--ink-dim)",
      border: "none",
    },
    ".cm-activeLineGutter, .cm-activeLine": {
      backgroundColor: "transparent",
    },
    ".cm-selectionBackground, ::selection": {
      backgroundColor: "var(--accent-tint) !important",
    },
    ".cm-tooltip": {
      backgroundColor: "var(--bg-raised)",
      border: "1px solid var(--border-mid)",
      borderRadius: "var(--r-md)",
      color: "var(--ink-primary)",
      fontFamily: "var(--font-mono)",
      fontSize: "11.5px",
    },
    ".cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]": {
      backgroundColor: "var(--accent-tint)",
      color: "var(--ink-primary)",
    },
    ".pb-var-chip": {
      display: "inline-flex",
      alignItems: "center",
      gap: "4px",
      padding: "0 8px",
      height: "20px",
      lineHeight: "20px",
      borderRadius: "var(--r-xs)",
      background: "var(--accent-tint)",
      color: "var(--ink-primary)",
      border: "1px solid var(--accent-deep)",
      fontFamily: "var(--font-mono)",
      fontSize: "11.5px",
      letterSpacing: "0.01em",
      cursor: "pointer",
      verticalAlign: "baseline",
    },
    ".pb-var-chip .pb-var-type": {
      color: "var(--accent)",
      fontSize: "9px",
      letterSpacing: "0.08em",
      textTransform: "uppercase",
    },
  },
  { dark: true },
);

const VARIABLE_REF_REGEX = /\{\{(file|folder|text|multiline|select|bool|number):([a-zA-Z_][a-zA-Z0-9_]*)\}\}/g;

class VariableChipWidget extends WidgetType {
  readonly varType: string;
  readonly key: string;
  constructor(varType: string, key: string) {
    super();
    this.varType = varType;
    this.key = key;
  }
  toDOM(): HTMLElement {
    const span = document.createElement("span");
    span.className = "pb-var-chip";
    span.setAttribute("data-var-key", this.key);
    span.setAttribute("data-var-type", this.varType);
    const typeBadge = document.createElement("span");
    typeBadge.className = "pb-var-type";
    typeBadge.textContent = this.varType;
    const name = document.createElement("span");
    name.textContent = this.key;
    span.append(typeBadge, name);
    return span;
  }
  eq(other: WidgetType): boolean {
    return (
      other instanceof VariableChipWidget &&
      other.varType === this.varType &&
      other.key === this.key
    );
  }
  ignoreEvent(): boolean {
    return false;
  }
}

function decorateVariables(state: EditorState): DecorationSet {
  const decorations: Range<Decoration>[] = [];
  const text = state.doc.toString();
  for (const match of text.matchAll(VARIABLE_REF_REGEX)) {
    const start = match.index ?? 0;
    const end = start + match[0].length;
    decorations.push(
      Decoration.replace({
        widget: new VariableChipWidget(match[1], match[2]),
      }).range(start, end),
    );
  }
  return Decoration.set(decorations, true);
}

const variableDecorations = StateField.define<DecorationSet>({
  create: (state) => decorateVariables(state),
  update: (deco, tx) => (tx.docChanged ? decorateVariables(tx.state) : deco),
  provide: (f) => EditorView.decorations.from(f),
});

const VARIABLE_TYPES = [
  "file",
  "folder",
  "text",
  "multiline",
  "select",
  "bool",
  "number",
];

function variableAutocomplete(context: CompletionContext) {
  const before = context.matchBefore(/\{\{[a-z]*:?[a-zA-Z0-9_]*$/);
  if (!before) return null;
  const text = before.text;
  const colonIdx = text.indexOf(":");
  if (colonIdx === -1) {
    const typed = text.slice(2);
    return {
      from: before.from + 2,
      options: VARIABLE_TYPES.filter((t) => t.startsWith(typed)).map(
        (type) => ({
          label: type,
          apply: `${type}:`,
          type: "type",
        }),
      ),
    };
  }
  return null;
}

export function PromptBodyEditor({
  value,
  onChange,
  readOnly = false,
}: PromptBodyEditorProps): React.JSX.Element {
  const [parseError, setParseError] = useState<string | null>(null);
  const [, setRefs] = useState<VariableRef[]>([]);

  // Debounced parse: 200ms after the user stops typing, ping the Rust
  // parser. Direct `useEffect` here is the rare legitimate case — we're
  // bridging a typed React value into an async backend probe.
  useEffect(() => {
    const handle = window.setTimeout(() => {
      parseVariables(value)
        .then((result) => {
          setRefs(result.refs);
          setParseError(
            result.errors.length === 0
              ? null
              : result.errors[0]?.message ?? "parse error",
          );
        })
        .catch(() => setParseError(null));
    }, 200);
    return () => window.clearTimeout(handle);
  }, [value]);

  const extensions = useMemo<Extension[]>(
    () => [
      markdown(),
      variableDecorations,
      autocompletion({ override: [variableAutocomplete] }),
      EditorView.lineWrapping,
      PROMPTIBRARY_THEME,
    ],
    [],
  );

  return (
    <div style={{ position: "relative" }}>
      <CodeMirror
        value={value}
        height="100%"
        minHeight="280px"
        extensions={extensions}
        editable={!readOnly}
        readOnly={readOnly}
        onChange={(v) => onChange?.(v)}
        basicSetup={{
          lineNumbers: false,
          foldGutter: false,
          highlightActiveLine: false,
          highlightActiveLineGutter: false,
          drawSelection: true,
          dropCursor: true,
          allowMultipleSelections: false,
          autocompletion: false,
        }}
      />
      {parseError != null && (
        <div
          role="alert"
          style={{
            position: "absolute",
            bottom: 6,
            right: 8,
            fontFamily: "var(--font-mono)",
            fontSize: "10.5px",
            color: "var(--status-error)",
            background: "var(--bg-sunken)",
            border: "1px solid var(--status-error)",
            padding: "2px 6px",
            borderRadius: "var(--r-xs)",
          }}
        >
          {parseError}
        </div>
      )}
    </div>
  );
}
