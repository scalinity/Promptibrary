// Tiny toast hook — pairs a string-or-null state with a self-clearing
// timer that gets cleaned up on unmount or on the next showToast call.
//
// Replaces the bare `window.setTimeout(() => setToast(null), N)` pattern
// that previously leaked across 5 surfaces (SCA-634). The hook owns the
// timer id and clears it before scheduling a new one OR when the host
// component unmounts, so a navigation mid-toast doesn't fire setState on
// an unmounted component.

// eslint-disable-next-line no-restricted-imports -- SCA-723: pre-CLAUDE.md useEffect, refactor in follow-up cleanup pass
import { useCallback, useEffect, useRef, useState } from "react";

export interface UseToastResult {
  toast: string | null;
  showToast: (message: string) => void;
  clearToast: () => void;
}

export function useToast(duration = 2000): UseToastResult {
  const [toast, setToast] = useState<string | null>(null);
  const timerRef = useRef<number | null>(null);

  const clearToast = useCallback(() => {
    if (timerRef.current != null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    setToast(null);
  }, []);

  const showToast = useCallback(
    (message: string) => {
      if (timerRef.current != null) {
        window.clearTimeout(timerRef.current);
      }
      setToast(message);
      timerRef.current = window.setTimeout(() => {
        timerRef.current = null;
        setToast(null);
      }, duration);
    },
    [duration],
  );

  // Cleanup-on-unmount — needed because StrictMode double-mounts in dev,
  // and a stranded timer would otherwise call setState on the dropped
  // instance. Pure timer cleanup; no external sync.
  useEffect(() => {
    return () => {
      if (timerRef.current != null) {
        window.clearTimeout(timerRef.current);
        timerRef.current = null;
      }
    };
  }, []);

  return { toast, showToast, clearToast };
}
