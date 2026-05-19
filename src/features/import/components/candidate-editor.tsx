// The canonical mockup at `screens/03-import.html` has no inline editor —
// users multi-select candidates and bulk-save, then edit (if needed) via
// the existing `/prompt/:id` route. This file is intentionally retained
// as a no-op shim so the import path in src/features/import/components/
// stays declarative; the route no longer imports it.

export function CandidateEditor(): null {
  return null;
}
