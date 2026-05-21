// React error boundary for route-level render failures. SCA-911.
//
// CLAUDE.md *Error Handling & Recovery*: "Use React Error Boundaries for
// component-level failures — don't let a broken widget crash the page."
// Class component is the only legitimate way to catch synchronous render
// throws in React 19; there is no hook equivalent.
//
// Reset behavior: the wrapper's `resetKey` prop is read on every render —
// when it changes (e.g. on a route navigation), the boundary clears its
// error state and re-renders children. Pair with `useLocation().pathname`
// at the route boundary to recover automatically on next navigation.

import { Component, type ReactNode } from "react";

import { defaultMessage, isAppError } from "@/shared/api/errors";

interface RouteErrorBoundaryProps {
  /**
   * Identity-changing value (e.g. `location.pathname`) that resets the
   * boundary when the user navigates to a different surface.
   */
  resetKey?: string | number;
  children: ReactNode;
}

interface RouteErrorBoundaryState {
  error: unknown;
}

export class RouteErrorBoundary extends Component<
  RouteErrorBoundaryProps,
  RouteErrorBoundaryState
> {
  state: RouteErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: unknown): RouteErrorBoundaryState {
    return { error };
  }

  componentDidUpdate(prev: RouteErrorBoundaryProps) {
    if (this.state.error != null && prev.resetKey !== this.props.resetKey) {
      // Auto-recover on navigation.
      this.setState({ error: null });
    }
  }

  componentDidCatch(error: unknown, info: { componentStack?: string | null }) {
    // SCA-911: never swallow — at minimum log with context per CLAUDE.md.
    console.error("[promptibrary] route render error:", error, info);
  }

  render(): ReactNode {
    const { error } = this.state;
    if (error != null) {
      const message = isAppError(error)
        ? `${defaultMessage(error.kind)} (${error.kind})`
        : error instanceof Error
          ? error.message
          : String(error);
      return (
        <div
          role="alert"
          style={{
            display: "grid",
            placeItems: "center",
            height: "100%",
            padding: "32px",
            color: "var(--ink-secondary)",
            fontFamily: "var(--font-mono)",
            fontSize: "13px",
            lineHeight: 1.55,
            textAlign: "center",
          }}
        >
          <div style={{ maxWidth: "480px" }}>
            <div
              style={{
                fontSize: "11px",
                color: "var(--ink-tertiary)",
                marginBottom: "8px",
                letterSpacing: "0.08em",
              }}
            >
              § render error
            </div>
            <div style={{ color: "var(--ink-primary)", marginBottom: "12px" }}>
              {message}
            </div>
            <div style={{ color: "var(--ink-tertiary)", fontSize: "11px" }}>
              navigate elsewhere to try again, or check the developer console
            </div>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
