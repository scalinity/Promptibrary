---
name: promptibrary-design
description: Use this skill to generate well-branded interfaces and assets for Promptibrary — the agentic-coding operator console with a launch-profile catalogue. The aesthetic is Editorial × Instrumented (serif display + monospace + sodium-amber accent, dark mode only). Use either for production code or for throwaway prototypes, mocks, presentations, marketing pieces, etc.
user-invocable: true
---

# Promptibrary design

Read `README.md` for the full aesthetic philosophy, type ramp, colour system, common patterns, and pitfalls. Then explore:

- `tokens.css` — every CSS custom property (`--bg-*`, `--ink-*`, `--accent-*`, `--status-*`, motion, spacing, radii)
- `typography.css` — the three faces (Boska / Switzer / JetBrains Mono) and `.t-*` utility classes
- `base.css` — reset, film-grain overlay, focus ring, motion keyframes, reduced-motion override
- `app.css` — full component set: buttons, inputs, rows, terminal, chrome
- `components/index.html` — every component × every state on one page
- `screens/01..09-*.html` — nine production-fidelity 1360×880 references (compose, run, import, past-run, history-diff, ⌘K palette, settings, empty, disconnected)

## When invoked

If the user gives no other guidance, ask what they want to build (interface mock, presentation, marketing piece, production code) and ask 3–5 questions about audience, scope, and which screen archetype they're starting from. Then act as the expert designer.

## Visual rules — the short list

1. **Dark mode only.** Surfaces are dark with a faint cool-cyan undertone; accents are warm. No light mode, ever.
2. **Three faces.** Boska (display), Switzer (UI), JetBrains Mono (everything runtime/identifier/numeric). Don't introduce a fourth.
3. **Sodium amber is the only accent.** Chartreuse for running status, warm red for danger, paper-warm cream for editorial moments — that's the whole palette.
4. **§ labels.** Section markers are Boska 500 italic 12px lowercase with a `§` prefix in `--ink-dim`.
5. **Motion ≤ 240ms.** Hovers snap (120ms), panes glide (200ms). The slow ambient animations (pulse 1.8s, glow 2.4s, blink 1s) are the only exceptions — they're statuses, not transitions.
6. **No emoji.** All glyphs are typographic (`§ ▸ ⌖ ⊟ ⎙ ↗ ▾ ⌗ ∿ ⏵ ⏸ ✓`) or Lucide outlines in `--ink-*` / `--accent`.
7. **Film grain everywhere.** Include `<div class="grain"></div>` near the start of `<body>`. It's not optional.
8. **One launch button per screen.** Solid accent, uppercase mono label, glow on hover. The only drop-shadowed element in the entire system.
9. **Realistic content always.** No Lorem. Use prompts a Claude Code power user would actually write (refactor, bisect failing test, spec generation, multi-agent orchestration). The mockup screens are the canon.

## Producing artifacts

- For static HTML mocks: copy `tokens.css`, `typography.css`, `base.css`, `app.css` into your output folder and `<link>` them in that order.
- For presentations/marketing: the editorial half is your friend. Lean on Boska, paper-warm tints, generous hairlines, and `§` labels.
- For production: every token in `tokens.css` ports cleanly to Tailwind v4 via `bg-[var(--bg-base)]`-style arbitrary classes.

## Hard constraints — do not violate

- Do not invent new accent colours.
- Do not use any other font family.
- Do not produce a light mode.
- Do not use emoji icons.
- Do not add drop shadows except on a hovered `.btn-launch`.
- Do not show `Lorem ipsum` in any user-facing artifact.
