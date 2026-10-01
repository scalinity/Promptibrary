---
name: promptibrary-design
description: Use this skill to generate well-branded interfaces and assets for Promptibrary — the agentic-coding operator console with a launch-profile catalogue. The aesthetic is Dark Carbon × Promptibrary (Outfit display + JetBrains Mono + amber accent, dark mode only, § section markers, typographic glyph icons). Use either for production code or for throwaway prototypes, mocks, presentations, marketing pieces, etc.
user-invocable: true
---

# Promptibrary design

Read `README.md` for the full aesthetic philosophy, type ramp, colour system, common patterns, and pitfalls. Then explore:

- `tokens.css` — every CSS custom property (`--bg-*`, `--ink-*`, `--accent-*`, `--status-*`, motion, spacing, radii)
- `typography.css` — the two faces (Outfit + JetBrains Mono) and `.t-*` utility classes
- `base.css` — reset, focus ring, motion keyframes, reduced-motion override
- `app.css` — full component set: buttons, inputs, rows, terminal, chrome
- `components/index.html` — every component × every state on one page
- `screens/01..09-*.html` — nine production-fidelity 1360×880 references (compose, run, import, past-run, history-diff, ⌘K palette, settings, empty, disconnected)

## When invoked

If the user gives no other guidance, ask what they want to build (interface mock, presentation, marketing piece, production code) and ask 3–5 questions about audience, scope, and which screen archetype they're starting from. Then act as the expert designer.

## Visual rules — the short list

1. **Dark mode only.** Surfaces are pure carbon (`#0c0c0c` / `#161616` / `#1c1c1c`); accents are warm amber. No light mode, ever.
2. **Two faces.** Outfit (display + UI), JetBrains Mono (everything runtime/identifier/numeric). Don't introduce a third.
3. **Amber is the only accent.** `#f59e0b` for primary, `#fbbf24` for on-panel/hover, `#b45309` for dim/deep. Status colours: teal-green `#34d399` running, red `#ef4444` error, orange `#f97316` warn, info blue `#60a5fa` for paths/links. That's the whole palette.
4. **§ labels.** Section markers are 12px lowercase Outfit with a `§` prefix in `--ink-dim`. Carried over from Promptibrary's editorial roots — still the signature.
5. **Motion ≤ 240ms.** Hovers snap (120ms), panes glide (200ms). The slow ambient animations (pulse 1.8s, glow 2.4s, blink 1s) are the only exceptions — they're statuses, not transitions.
6. **No emoji.** All glyphs are typographic (`§ ▸ ⌖ ⊟ ⎙ ↗ ▾ ⌗ ∿ ⏵ ⏸ ✓`) or Lucide outlines in `--ink-*` / `--accent`.
7. **No grain.** The dark carbon palette stands on its own — no overlay textures, gradients, or atmospheric effects.
8. **Flat, with one shadow exception.** No drop shadows anywhere except a single subtle shadow on `.cmdk-dialog` (the ⌘K palette modal). The launch button uses an outline glow (`box-shadow: 0 0 0 2px var(--accent-glow)`), not a drop shadow.
9. **One launch button per screen.** Solid amber, uppercase mono label, outline glow on hover.
10. **Realistic content always.** No Lorem. Use prompts a Claude Code power user would actually write (refactor, bisect failing test, spec generation, multi-agent orchestration). The mockup screens are the canon.

## Producing artifacts

- For static HTML mocks: copy `tokens.css`, `typography.css`, `base.css`, `app.css` into your output folder and `<link>` them in that order.
- For presentations/marketing: the amber accent + dark carbon reads as a dev-tool aesthetic. Generous hairlines, instrument-panel mono labels, and `§` section markers carry the brand identity.
- For production: every token in `tokens.css` ports cleanly to Tailwind v4 via `bg-[var(--bg-base)]`-style arbitrary classes.

## Hard constraints — do not violate

- Do not invent new accent colours.
- Do not use any other font family.
- Do not produce a light mode.
- Do not use emoji icons.
- Do not add drop shadows except on `.cmdk-dialog`.
- Do not add a film-grain overlay.
- Do not show `Lorem ipsum` in any user-facing artifact.
