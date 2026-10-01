# Promptibrary — Design System

> **Dark Carbon × Promptibrary.** A daily-driver tool for an opinionated power user.
> A clean, flat, dev-tool minimalism — carbon surfaces, amber accents, JetBrains Mono everywhere it matters.
> Harmonized by typography (Outfit display + JetBrains Mono runtime) and pacing (nothing longer than 240ms).

---

## Files

| Path | What it is |
|---|---|
| `tokens.css` | Every CSS custom property (color, geometry, motion, spacing) under `:root` |
| `typography.css` | `@import` Outfit + JetBrains Mono + `.t-*` utility classes |
| `base.css` | Reset, scrollbars, focus rings, motion keyframes, reduced-motion override |
| `app.css` | Every component's styles — buttons, inputs, rows, terminal, chrome |
| `components/index.html` | Single reference page, every component × every state |
| `screens/01..09-*.html` | Nine 1360×880 production-fidelity screens |
| `screens/index.html` | Live thumbnail index of all screens |
| `preview/*.html` | Small cards used by the Design System tab (700px wide) |
| `SKILL.md` | Cross-compatible skill manifest (Claude Code, etc.) |

Load order in any new page: `tokens.css` → `typography.css` → `base.css` → `app.css` → your own styles.

---

## Aesthetic philosophy

Promptibrary is a launch profile catalogue for Claude Code. The interface is **dark carbon minimalism with editorial signatures**:

- **Carbon surfaces** — three flat tones (`#0c0c0c` deep, `#161616` base, `#1c1c1c` raised) with hairline borders. No gradients, no shadows except on the ⌘K palette modal. The eye reads structure through density and contrast, not depth.
- **Amber instrument lights** — `#f59e0b` and `#fbbf24` mark interactive primaries. Status colours (teal `#34d399` running, red `#ef4444` error, orange `#f97316` warn, info blue `#60a5fa` paths) are dialed to read clearly at small sizes without screaming.
- **Mono for everything that matters** — identifiers, paths, runtimes, kbd badges, status pills, telemetry numbers. JetBrains Mono with `+0.01em` tracking; `+0.04em` on the cockpit status line.
- **§ section markers** — carried over from Promptibrary's editorial origin. Outfit 500 lowercase with a `§` glyph in `--ink-dim`. The one persistent typographic signature.

**Dark mode is the only theme.** No light mode, no system preference toggle, no high-contrast variant.

**Restraint is the rule.** No purple gradients, no drop shadows (except `.cmdk-dialog`), no emoji, no celebrations, no animations longer than 240ms except the slow ambient ones (pulse 1.8s, glow 2.4s, blink 1s — all status indicators).

---

## Common patterns

### "Section label above a grid"

12px lowercase Outfit with `§` glyph, then a content grid below. Use everywhere the eye needs a quiet anchor:

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

A 3px left edge whose color encodes state (running = pulsing teal-green, selected = solid amber, recent = amber-on-panel 50%, idle = transparent). The title is Outfit 500; meta is JBM.

### "Pulse on empty"

Required inputs that haven't been filled get `.empty-required`, which breathes accent-glow at 2.4s. Filled inputs are calm. Don't apply this to optional fields — it makes the prompt feel noisy.

### "Glyph then identifier" — terminal lines

Each terminal line starts with a single typographic glyph in accent/accent-dim. The glyph **is** the line's semantics; don't repeat it in prose.

| Glyph | Class | Meaning |
|---|---|---|
| `›` | `.sys` | session / system marker |
| `∿` | `.think` | agent thinking — italic ink-tertiary |
| `⌗` | `.tool` | tool call — amber-on-panel |
| `▸` | `.diag` | diagnosis — ink-secondary |
| `✓` | `.verif` | verifier — status-warn body, status-running glyph |
| `▍` | `.term-cursor` | active cursor — blinking accent |

---

## Pitfalls

- **`--accent-warm` is not a body text colour.** It's hover & highlight. On `--bg-base` it fails AAA contrast for paragraph-length text. Use `--ink-primary` or `--ink-secondary`.
- **Don't pulse multiple things at once.** One running indicator per pane is enough. A status dot _and_ a row spine pulsing simultaneously is jittery.
- **Never use the accent colour as the sole carrier of meaning.** Pair it with a glyph, label, or kbd badge. Red-green colour-blind users (and grayscale exports) still need to read the UI.
- **`§` belongs to lowercase Outfit section labels.** Don't slap it on a UI header or a sentence-case title. Outfit has no true italic — drop italic on display headings even when you're tempted.
- **Mono needs +0.01em tracking.** Without it, JetBrains Mono looks claustrophobic at 11px. On the status line, push it to +0.04em for the instrument-panel feel.
- **No emoji icons, ever.** All glyphs are typographic (`§ ▸ ⌖ ⊟ ⎙ ↗ ▾ ⌗ ∿ ⏵ ⏸ ✓`) or come from a Lucide-style outline set rendered in `--ink-*` / `--accent`.
- **The ⌘K palette is the only thing with a drop shadow.** The launch button uses an outline glow, not a drop shadow. Everything else is flat.
- **No grain.** The dark carbon palette is meant to stand on its own. Don't reintroduce overlay textures.

---

## Type ramp at a glance

```
Display XL  Outfit 600   32 / 1.10   -.02   wordmark on first run
Display L   Outfit 600   26 / 1.15   -.015  detail titles
Display M   Outfit 600   22 / 1.20   -.02   brand mark
Display S   Outfit 600   18 / 1.25   -.01   list headers
Section     Outfit 500   12 / 1.20   +.02   § labels, lowercase
UI L        Outfit 400   15 / 1.5    -.005
UI          Outfit 400   13 / 1.5    -.005  default body
UI S        Outfit 400   12 / 1.45   -.005
UI XS       Outfit 500   11 / 1.30    0     small labels (e.g. "REPO")
Mono        JBM 400      12.5 / 1.65 +.01   prompt body, terminal
Mono S      JBM 400      11   / 1.5  +.01   status line, meta
Mono XS     JBM 400      10.5 / 1.4  +.01   kbd, source tags
```

Apply `.t-mono-instrument` on mono labels that should feel like cockpit readouts (`+.04em` tracking).

---

## Colour ramp at a glance

```
SURFACES (dark carbon)
  bg-deep      #0c0c0c   outer app, behind frame
  bg-base      #161616   primary surface
  bg-raised    #1c1c1c   cards, hovered rows
  bg-sunken    #0c0c0c   inputs, code, status line
  bg-terminal  #0a0a0a   terminal only

INK
  ink-primary    #e0e0e0   body & titles
  ink-secondary  #8a8a8a   supporting
  ink-tertiary   #5a5a5a   labels
  ink-dim        #4a4a4a   separators, disabled

ACCENT (amber)
  accent       #f59e0b   primary
  accent-warm  #fbbf24   hover / on-panel
  accent-dim   #b45309   secondary
  accent-deep  #b45309   outlines
  accent-glow  rgba(245,158,11,0.30)  halo
  accent-tint  rgba(245,158,11,0.10)  selected bg

STATUS
  running    #34d399   teal-green (alive)
  error      #ef4444   red
  warn       #f97316   orange

REPURPOSED SLOTS
  paper-warm  #fbbf24   amber-on-panel (kind dot, syntax kw)
  term-path   #60a5fa   info blue (file paths, links)
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
| `.cmdk-dialog` | The ⌘K command palette. Only modal in the system with a drop shadow |

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
