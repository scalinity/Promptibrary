// Type-specific control for a single Variable.
//
// Switches on `Variable.type` and renders the correct primitive per spec §5
// "UI control mapping". Each control runs client-side validation and surfaces
// inline error text. Working-dir + permission editors are separate components.

import { open as openDialog } from "@tauri-apps/plugin-dialog";

import type { Variable, SelectVariable } from "@/shared/types/variable";
import type { ResolvedVariableValue } from "@/shared/types/launch";
import {
  asAbsolutePath,
  type AbsolutePath,
} from "@/shared/types/ids";

interface VariableControlProps {
  variable: Variable;
  value: ResolvedVariableValue | undefined;
  onChange: (value: ResolvedVariableValue) => void;
  onClear?: (key: string) => void;
  error?: string | null;
}

export function VariableControl({
  variable,
  value,
  onChange,
  onClear,
  error,
}: VariableControlProps): React.JSX.Element {
  return (
    <div className="var-field" data-required={variable.required ? "true" : undefined}>
      <div className="var-label">
        <span className="name">{variable.key}</span>
        <span className="type">{variable.type.toUpperCase()}</span>
        {variable.required ? (
          <span className="req" aria-label="required">
            *
          </span>
        ) : (
          <span className="opt">— optional</span>
        )}
      </div>
      <Control variable={variable} value={value} onChange={onChange} onClear={onClear} />
      {error != null && (
        <div
          role="alert"
          style={{
            fontFamily: "var(--font-mono)",
            fontSize: "10.5px",
            color: "var(--status-error)",
            marginTop: 4,
            letterSpacing: "0.01em",
          }}
        >
          {error}
        </div>
      )}
    </div>
  );
}

function Control({
  variable,
  value,
  onChange,
  onClear,
}: {
  variable: Variable;
  value: ResolvedVariableValue | undefined;
  onChange: (value: ResolvedVariableValue) => void;
  onClear?: (key: string) => void;
}): React.JSX.Element {
  switch (variable.type) {
    case "file":
      return (
        <FilePicker
          mode="file"
          variableKey={variable.key}
          value={value?.type === "file" ? value.value : null}
          onChange={(v) => onChange({ key: variable.key, type: "file", value: v })}
        />
      );
    case "folder":
      return (
        <FilePicker
          mode="folder"
          variableKey={variable.key}
          value={value?.type === "folder" ? value.value : null}
          onChange={(v) =>
            onChange({ key: variable.key, type: "folder", value: v })
          }
        />
      );
    case "text":
      return (
        <input
          className="input"
          value={value?.type === "text" ? value.value : ""}
          onChange={(e) =>
            onChange({ key: variable.key, type: "text", value: e.target.value })
          }
          placeholder={variable.label}
        />
      );
    case "multiline":
      return (
        <textarea
          className="textarea"
          value={value?.type === "multiline" ? value.value : ""}
          onChange={(e) =>
            onChange({
              key: variable.key,
              type: "multiline",
              value: e.target.value,
            })
          }
          placeholder={variable.label}
        />
      );
    case "select": {
      const sel = variable as SelectVariable;
      return (
        <div className="select-wrap">
          <select
            className="select"
            value={value?.type === "select" ? value.value : ""}
            onChange={(e) =>
              onChange({
                key: variable.key,
                type: "select",
                value: e.target.value,
              })
            }
          >
            <option value="" disabled>
              choose…
            </option>
            {sel.options.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>
      );
    }
    case "bool":
      return (
        <label
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 10,
            cursor: "pointer",
            fontFamily: "var(--font-mono)",
            fontSize: "12px",
            color: "var(--ink-secondary)",
          }}
        >
          <input
            type="checkbox"
            checked={value?.type === "bool" ? value.value : false}
            onChange={(e) =>
              onChange({
                key: variable.key,
                type: "bool",
                value: e.target.checked,
              })
            }
            style={{ accentColor: "var(--accent)" }}
          />
          <span>
            renders as “
            {(value?.type === "bool" && value.value) ||
            (value?.type !== "bool" && variable.defaultValue === true)
              ? variable.renderTrue
              : variable.renderFalse}
            ”
          </span>
        </label>
      );
    case "number":
      return (
        <input
          className="input"
          type="number"
          min={variable.min ?? undefined}
          max={variable.max ?? undefined}
          step={variable.step ?? undefined}
          value={value?.type === "number" ? value.value : ""}
          onChange={(e) => {
            const raw = e.target.value;
            // SCA-640 — empty input means "no value"; drop the key from
            // the draft so validation sees missing rather than the old
            // number and submission doesn't carry stale state.
            if (raw === "") {
              onClear?.(variable.key);
              return;
            }
            const n = Number(raw);
            if (Number.isFinite(n))
              onChange({ key: variable.key, type: "number", value: n });
          }}
          placeholder={variable.label}
        />
      );
  }
}

function FilePicker({
  mode,
  variableKey: _variableKey,
  value,
  onChange,
}: {
  mode: "file" | "folder";
  variableKey: string;
  value: AbsolutePath | null;
  onChange: (value: AbsolutePath) => void;
}): React.JSX.Element {
  return (
    <button
      type="button"
      className={value == null ? "picker empty" : "picker"}
      onClick={async () => {
        const result = await openDialog({
          directory: mode === "folder",
          multiple: false,
        });
        if (typeof result === "string") {
          onChange(asAbsolutePath(result));
        }
      }}
    >
      <span className="glyph">{mode === "folder" ? "⊟" : "⎙"}</span>
      <span className="path">{value ?? `choose ${mode}…`}</span>
    </button>
  );
}
