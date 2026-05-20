# Aesthetic pivot — Dark Carbon × Promptibrary (SCA-866, 2026-05-20)

This file records a coordination-event-level change to the project: the canonical design system was replaced wholesale. Filed outside the L0–L5 layer hierarchy because it touches every layer's surfaces.

## What changed

The "Editorial × Instrumented" aesthetic (Boska serif display + Switzer UI + sodium amber OKLCH accent + mandatory film grain) was replaced with a Dark Carbon × Promptibrary hybrid ported from the sibling app at `~/Documents/Codez/Apps/ClaudeRoom`:

- **Palette** — hex tokens replace OKLCH. Carbon surfaces `#0c0c0c` / `#161616` / `#1c1c1c`; amber accent `#f59e0b` (warm `#fbbf24`, deep `#b45309`); status colours teal-green `#34d399` / red `#ef4444` / orange `#f97316` / info blue `#60a5fa`.
- **Typography** — two faces: Outfit (display + UI, Google Fonts CDN) and JetBrains Mono (mono, bundled locally). Boska + Switzer dropped.
- **Film grain** — removed everywhere. The carbon palette stands on its own.
- **Shadows** — flattened. The launch button now uses a 2px outline glow instead of a radial halo + drop shadow. The only remaining drop shadow is on the `.cmdk-dialog` modal.
- **Radii** — reduced: `--r-xs` 3→2px, `--r-sm` 5→3px, `--r-md` 8→4px, `--r-lg` 12→6px.

## What was preserved

- **§ section markers** — the lowercase `§` glyph treatment carries the editorial signature forward.
- **Typographic glyph icon vocabulary** — `§ ▸ ⌖ ⊟ ⎙ ↗ ▾ ⌗ ∿ ⏵ ⏸ ✓` remains the icon language; no emoji.
- **Variable naming** — every CSS custom property (`--bg-deep`, `--ink-primary`, `--accent`, etc.) kept its name; only values changed. This let CodeMirror, sidebar dots, and launch-drawer scrim re-theme automatically.
- **Spacing scale**, **motion durations**, **chrome dimensions** — unchanged.

## Files touched

12 sub-issues under SCA-866 covered:

1. `tokens.css` x3 (canonical pair + `src/styles/`)
2. `typography.css` x3 + `src/index.css` font @import
3. `base.css` x3 (grain removed, keyframes retuned, OKLCH literal in `pb-spine-pulse` replaced)
4. `app.css` x3 (9 OKLCH literals → hex, `.btn-launch:hover` flattened, Outfit italic dropped on section labels)
5. `src/shared/ui/app-shell.tsx` (grain JSX node deleted)
6. `src/shared/ui/sidebar.tsx` (verified — token cascade covers it; spec-gen swatch shuffle is a follow-up)
7. `src/features/import/import.css` + `src/features/search/cmdk-palette.css`
8. 18 screen mockups (9 × both bundles)
9. `SKILL.md` + `README.md` x both bundles
10. `CLAUDE.md` hard rules
11. `docs/SPEC.md` §8 xterm theme + `docs/V2-CANDIDATES.md`
12. 17 Playwright visual baselines (capture pass)

## Why this lives outside the L0–L5 hierarchy

L0–L5 are the V1 layered build. The retheme is a cross-cutting refactor that touches:

- L0 (tokens, typography, base, app)
- L2 (chrome — topbar, sidebar, statusbar, list rows)
- L3 (xterm theme bridge — still unimplemented per spec §7/§8, but the token names it expects are now hex-backed)
- L4 (no surfaces)
- L5 (settings panels, telemetry confirm dialog)

Filing as an L-numbered child would have falsely scoped it. Instead it's a coordination event with its own parent (SCA-866) and 12 decomposed sub-issues, each landing as a discrete commit per CLAUDE.md push cadence.

## Known follow-ups

- **`spec-gen` tag swatch** — currently `var(--paper-warm)` which maps to `#fbbf24` (amber-on-panel). Visually identical to amber-hover states. A future ticket should re-shuffle the tag swatch palette to give `spec-gen` a distinct hue.
- **Local Outfit bundle** — V2 polish, tracked in `docs/V2-CANDIDATES.md`.
- **xterm `getTerminalTheme()`** — spec §8 documents the bridge; the implementation in `src/features/terminal/` is still a stub (the same stub as before the retheme — not a regression, just unfinished L3/L5 work).

---

## Commit range

- **Parent (retheme):** [SCA-866](https://linear.app/scalinity/issue/SCA-866) — commits `4ae8f9f..597d117` on `main` (13 commits, 12 sub-issues + 1 cleanup pass).
- **Parent (review follow-ups):** [SCA-879](https://linear.app/scalinity/issue/SCA-879) — addresses every finding from `/review-2` against SCA-866. 10 sub-issues (SCA-880..SCA-889).
- To replay: `git log --oneline 4ae8f9f^..main -- Promptibrary\ Design\ System .claude/skills/promptibrary-design src/styles src/index.css src/features/import/import.css src/shared/ui/app-shell.tsx docs/SPEC.md docs/notes/aesthetic-pivot.md docs/V2-CANDIDATES.md CLAUDE.md`
