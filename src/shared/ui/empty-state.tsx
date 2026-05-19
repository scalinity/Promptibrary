// Empty-state placeholder block.
//
// Used in: prompt list when filters produce no matches, detail pane when
// nothing is selected, history pane before the first commit, etc. The
// `glyph` slot is one of the typographic characters from the design system
// glyph palette — never an emoji.

export interface EmptyStateProps {
  glyph?: string;
  title: string;
  body?: string;
  action?: React.ReactNode;
}

export function EmptyState({
  glyph = "§",
  title,
  body,
  action,
}: EmptyStateProps): React.JSX.Element {
  return (
    <div
      role="status"
      style={{
        display: "grid",
        placeItems: "center",
        gap: "var(--sp-3)",
        padding: "var(--sp-8) var(--sp-4)",
        textAlign: "center",
      }}
    >
      <span
        aria-hidden="true"
        style={{
          fontFamily: "var(--font-mono)",
          fontSize: "32px",
          color: "var(--ink-dim)",
        }}
      >
        {glyph}
      </span>
      <div
        style={{
          fontFamily: "var(--font-display)",
          fontWeight: 500,
          fontStyle: "italic",
          fontSize: "16px",
          color: "var(--ink-secondary)",
          letterSpacing: "-0.01em",
        }}
      >
        {title}
      </div>
      {body != null && (
        <div
          style={{
            fontFamily: "var(--font-ui)",
            fontSize: "12.5px",
            color: "var(--ink-tertiary)",
            maxWidth: "44ch",
            lineHeight: 1.5,
          }}
        >
          {body}
        </div>
      )}
      {action}
    </div>
  );
}
