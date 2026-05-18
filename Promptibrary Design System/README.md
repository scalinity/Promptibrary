# Promptibrary — Design System

> **Editorial × Instrumented.** A daily-driver tool for an opinionated power user.
> Two halves in tension: a librarian's calm archive on the left, an operator's live cockpit on the right.
> Harmonized by typography (serif display / sans UI / variable mono) and pacing (nothing longer than 240ms).

---

## Files

| Path | What it is |
|---|---|
| `tokens.css` | Every CSS custom property (color, geometry, motion, spacing) under `:root` |
| `typography.css` | `@import` Boska / Switzer / JetBrains Mono + `.t-*` utility classes |
| `base.css` | Reset, scrollbars, focus rings, film-grain overlay, motion keyframes, reduced-motion override |
| `app.css` | Every component's styles — buttons, inputs, rows, terminal, chrome |
| `components/index.html` | Single reference page, every component × every state |
| `screens/01..09-*.html` | Nine 1360×880 production-fidelity screens |
| `screens/index.html` | Live thumbnail index of all screens |
| `preview/*.html` | Small cards used by the Design System tab (700px wide) |
| `SKILL.md` | Cross-compatible skill manifest (Claude Code, etc.) |

Load order in any new page: `tokens.css` → `typography.css` → `base.css` → `app.css` → your own styles.

---

## Aesthetic philosophy

The product is a launch profile catalogue. The interface honours both halves:

- **Editorial** = serif headlines (Boska), `§` section markers, italic lowercase labels, hairline rules, paper-warm cream tints. Calm, archival, dense-but-quiet. Think: card catalogue.
- **Instrumented** = monospace for every identifier, runtime, timestamp; sodium-amber accent (the colour of a high-pressure street lamp at 3am); pulsing chartreuse status indicators; an always-on operator status line at the bottom edge. Think: cockpit alive.

**Dark mode is the only theme.** Surfaces have a faint cool-cyan undertone; accents are warm. One paper-warm cream is reserved for editorial moments and a single terminal-line glyph (`⌗ tool call`).

**Restraint is the rule.** No purple gradients. No drop shadows except the launch button hover. No emoji. No celebrations. No animations longer than 240ms except the slow ambient ones (pulse, glow, blink).

---

## Common patterns

### "Section label above a grid"

Boska italic 12px lowercase with `§` glyph, then a content grid below. Use everywhere the eye needs a quiet anchor:

```html
<div class="section-label">variables</div>
<div class="var-grid"> … </div>
```

### "Mono inline run config"

A flat row of `key value` pairs separated by hairlines or whitespace. Used in: launch row, run config strip, statusbar.

```html
<div class="run-config-strip">
  <span class="pair"><span class="k">repo</span><span class="v">~/code/aurora</span></span>
  <span class="pair"><span class="k">model</span><span class="v">sonnet-4.5</span></span>
</div>
```

### "Spine + body" list row

A 3px left edge whose color encodes state (running = pulsing chartreuse, selected = solid accent, recent = paper-warm 50%, idle = transparent). The title is Switzer 500; meta is mono.

### "Pulse on empty"

Required inputs that haven't been filled get `.empty-required`, which breathes accent-glow at 2.4s. Filled inputs are calm. Don't apply this to optional fields — it makes the prompt feel noisy.

### "Glyph then identifier" — terminal lines

Each terminal line starts with a single typographic glyph in accent/accent-dim. The glyph **is** the line's semantics; don't repeat it in prose.

| Glyph | Class | Meaning |
|---|---|---|
| `›` | `.sys` | session / system marker |
| `∿` | `.think` | agent thinking — italic ink-tertiary |
| `⌗` | `.tool` | tool call — paper-warm |
| `▸` | `.diag` | diagnosis — ink-secondary |
| `✓` | `.verif` | verifier — status-warn body, status-running glyph |
| `▍` | `.term-cursor` | active cursor — blinking accent |

---

## Pitfalls

- **`--accent-warm` is not a body text colour.** It's hover & highlight. On `--bg-base` it fails AAA contrast for paragraph-length text. Use `--ink-primary` or `--ink-secondary`.
- **Don't pulse multiple things at once.** One running indicator per pane is enough. A status dot _and_ a row spine pulsing simultaneously is jittery.
- **Never use the accent colour as the sole carrier of meaning.** Pair it with a glyph, label, or kbd badge. Red-green colour-blind users (and grayscale exports) still need to read the UI.
- **`§` belongs to italic lowercase Boska labels.** Don't slap it on a Switzer header or a sentence-case title.
- **Mono needs +0.01em tracking.** Without it, JetBrains Mono looks claustrophobic at 11px. On the status line, push it to +0.04em for the instrument-panel feel.
- **No emoji icons, ever.** All glyphs are typographic (`§ ▸ ⌖ ⊟ ⎙ ↗ ▾ ⌗ ∿ ⏵ ⏸ ✓`) or come from a Lucide-style outline set rendered in `--ink-*` / `--accent`.
- **The launch button is the only thing with a drop shadow.** Treat it as a single physical interaction, not a visual style.
- **Don't crop the film grain.** It's a full-viewport overlay with `mix-blend-mode: overlay`. Removing it from a card or modal breaks the visual continuity with the rest of the app.

---

## Type ramp at a glance

```
Display XL  Boska 500   32 / 1.10   -.02   wordmark on first run
Display L   Boska 500   26 / 1.15   -.015  detail titles
Display M   Boska 500   22 / 1.20   -.02   brand mark
Display S   Boska 500   18 / 1.25   -.01   list headers
Section     Boska 500i  12 / 1.20   +.02   § labels, lowercase
UI L        Switzer     15 / 1.5    -.005
UI          Switzer     13 / 1.5    -.005  default body
UI S        Switzer     12 / 1.45   -.005
UI XS       Switzer 500 11 / 1.30    0     small labels (e.g. "REPO")
Mono        JBM         12.5 / 1.65 +.01   prompt body, terminal
Mono S      JBM         11   / 1.5  +.01   status line, meta
Mono XS     JBM         10.5 / 1.4  +.01   kbd, source tags
```

Apply `.t-mono-instrument` on mono labels that should feel like cockpit readouts (`+.04em` tracking).

---

## Colour ramp at a glance

```
SURFACES (cool-cyan undertone)
  bg-deep    L .135   outer app, behind frame
  bg-base    L .170   primary surface
  bg-raised  L .205   cards, hovered rows
  bg-sunken  L .110   inputs, code, status line
  bg-terminal L .095  terminal only

INK (warm cast)
  ink-primary    L .96   body & titles
  ink-secondary  L .74   supporting
  ink-tertiary   L .54   labels
  ink-dim        L .38   separators, disabled

ACCENT (sodium amber — chroma 0.16, hue 75)
  accent / -warm / -dim / -deep / -glow (α 0.30) / -tint (α 0.08)

STATUS
  running    chartreuse  oklch(.78 .18 145)
  error      warm red    oklch(.68 .22 25)
  warn       amber-yel   oklch(.82 .15 90)

EDITORIAL
  paper-warm  oklch(.92 .045 80)  — sparing
```

---

## When to use each component

| Component | Use it when… |
|---|---|
| `.btn` | Any neutral action — Edit, History, Copy log, Cancel |
| `.btn-launch` | **One per screen.** The primary "do the thing" action: Launch run, Save prompts |
| `.btn-stop` | Stopping an in-progress run. Never the only red button on a screen — pair with diagnostic context |
| `.icon-btn` | 30×30 toolbar item, always with `aria-label` |
| `.input` / `.textarea` | Free-form value entry. Add `.empty-required` only on required + unfilled |
| `.picker` | File/folder selection. Leading `⎙` (file) or `⊟` (folder) glyph |
| `.tag` | Inline metadata that the user can filter by. Always `#`-prefixed |
| `.kbd` | Surface shortcut hints whenever they apply. Don't bury them in tooltips alone |
| `.tooltip` | Optional supplement; never the only path to a piece of information |
| `.row` (list) | Promptibrary's primary "thing-in-a-list" pattern. Pick exactly one state class: `.selected`, `.running`, `.recent` |
| `.terminal` | Only inside a run/past-run pane. Don't recreate elsewhere |
| `.statusbar` | Pinned to the bottom of every full-app screen |

---

## Accessibility checklist

- [x] All body text ≥ 7:1 contrast on its background (AAA)
- [x] Accent never carries meaning alone — paired with a glyph or label
- [x] Focus rings are visible on keyboard nav (`:focus-visible` rule in `base.css`)
- [x] All icon-only buttons declare `aria-label`
- [x] `prefers-reduced-motion` disables pulse, blink, glow

---

## Index

- **Foundation** — `tokens.css`, `typography.css`, `base.css`
- **Components** — `app.css` (styles), `components/index.html` (reference)
- **Screens** — `screens/index.html` (thumbnail grid) → `screens/01..09-*.html`
- **Preview cards** — `preview/*.html` (rendered in the Design System tab)
- **Skill manifest** — `SKILL.md`
