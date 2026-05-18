// App shell.
//
// L0 scaffold — renders a minimal empty surface so `pnpm tauri dev` boots a
// non-crashing window. Real routing and chrome land in L2.

export function App(): React.JSX.Element {
  return (
    <main
      style={{
        fontFamily: "var(--font-display)",
        background: "var(--bg-deep)",
        color: "var(--ink-primary)",
        minHeight: "100vh",
        display: "grid",
        placeItems: "center",
      }}
    >
      <div style={{ textAlign: "center" }}>
        <h1
          style={{
            fontFamily: "var(--font-display)",
            fontSize: "32px",
            margin: 0,
            letterSpacing: "-0.02em",
          }}
        >
          Promptibrary
        </h1>
        <p
          style={{
            fontFamily: "var(--font-ui)",
            color: "var(--ink-tertiary)",
            marginTop: "0.5rem",
          }}
        >
          L0 — foundations scaffold
        </p>
      </div>
    </main>
  );
}
