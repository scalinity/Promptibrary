# L0 — Observations and assumptions

This file records non-blocking observations made during the L0 scaffold so the L0 reviewer can audit drift against the spec.

Per CLAUDE.md, problems outside the L0 scope are **logged here, not fixed**. Problems inside L0 are fixed and committed.

## Conventions

- Each entry is dated and references the spec section it relates to.
- Inline `// Assuming X because Y.` comments in code use this file as their public counterpart.
- Cross-layer concerns get an entry plus a `TODO(Ln):` comment at the touch point.

## Observations

### 2026-05-18 — Rust toolchain installed via rustup

Rust was not present on the dev machine. Installed `stable-aarch64-apple-darwin` (cargo 1.95.0 / rustc 1.95.0) via `rustup-init -y --profile minimal`. No project-side change required; recorded here so future agents know the toolchain entry point.

### 2026-05-18 — Boska / Switzer remain on Fontshare CDN for L0

Per the L0 prompt: JetBrains Mono Variable is bundled locally; Boska Variable and Switzer Variable load from the Fontshare CDN for now. Local bundling of those two families is a V2 polish item.

### 2026-05-18 — Repository name is `Promptibary` (no `r`) but product is `Promptibrary`

The working directory and GitHub repo name are `Promptibary` (a typo that predated L0). Product/binary/spec spelling is `Promptibrary` (with the `r`). No code change in L0; flag as a V2 candidate if the user wants to rename the repo.

### 2026-05-18 — Four files added beyond literal spec §3

The following files exist in the L0 scaffold but are not literally enumerated in spec §3. Each is justified and documented in-file:

1. `src/shared/ui/button.tsx` — wrapper re-export per spec §3's wrapper-layer pattern. Required by the L0 prompt's ESLint boundary smoke test ("fix it by re-exporting through `src/shared/ui/button.tsx`").
2. `src/shared/lib/utils.ts` — shadcn convention. `components.json` aliases `utils` to `@/shared/lib/utils` per the L0 prompt requirement; the existing `src/shared/lib/cn.ts` re-exports from it.
3. `src/shared/types/enums.ts` — shared enum types (`LaunchDestination`, `ClaudeModelId`, etc.) that the other typed files import. Spec §4 declares these enums but doesn't name a host file; consolidating them avoids duplication.
4. `src/__tests__/smoke.test.ts` — Vitest smoke test mandated by L0 prompt ("Vitest: src/__tests__/smoke.test.ts").

### 2026-05-18 — `src/shared/ui/button.tsx` added as wrapper (not literally in spec §3)

Spec §3 lists the `shared/ui/shadcn/` directory for shadcn primitives and a row of wrapper components (app-shell, sidebar, topbar, etc.) but does not literally enumerate `button.tsx` as a wrapper file. The L0 prompt explicitly requires it ("fix it by re-exporting through `src/shared/ui/button.tsx`"). I treated this as a justified extension of §3's wrapper-layer pattern. The ESLint `no-restricted-imports` boundary is verified to catch a direct `@/shared/ui/shadcn/button` import in a feature component, and the wrapper at `@/shared/ui/button` passes.

### 2026-05-18 — `tauri build` in CI requires Linux system dependencies

The spec's CI matrix builds on both `macos-14` and `ubuntu-24.04`. Ubuntu Tauri 2.x builds need apt packages (`libwebkit2gtk-4.1-dev`, `build-essential`, `curl`, `wget`, `file`, `libxdo-dev`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`). The L0 CI workflow installs these on the Ubuntu runner.

## Cross-layer carry-overs

- `// TODO(L1):` markers appear on every IPC command stub whose real implementation lands in L1+.
- `// TODO(L1):` markers appear on every domain `mod.rs` whose types are minimal in L0 and grow in L1+.
- The variable parser/renderer/validator skeletons exist in `src-tauri/src/variables/` but are stubs; L1 will implement them.

## V2 punch list (also in `docs/V2-CANDIDATES.md`)

- Rename GitHub repo `Promptibary` → `Promptibrary`.
- Bundle Boska + Switzer locally instead of CDN.
