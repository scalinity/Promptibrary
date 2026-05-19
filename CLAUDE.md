# Promptibrary — CLAUDE.md

This file augments the global `~/.claude/CLAUDE.md` with project-specific rules for Promptibrary. The global file's engagement posture, directness, response calibration, mode sensing, scope/scale, debugging discipline, review/commit discipline, session hygiene, learning style, deliverable shape, technical defaults, and conflict resolution rules apply unchanged. Everything below is **additive**.

---

## Project at a glance

**Promptibrary** is a Tauri 2.x desktop app — an agentic-coding operator console for Claude Code. The unit is a *launch profile*: a saved prompt with typed variables that resolves into a fresh Claude Code session running in an embedded terminal. Library + cockpit. Editorial × instrumented.

- **Spec (source of truth):** `docs/SPEC.md`. Treat as authoritative. If your priors conflict with the spec, follow the spec.
- **Build plan:** `~/Documents/Obsidian Vault/Promptibrary/Build Plan.md` and the six `Prompts/L*.md` files. Sectioned build: each layer is one `/goal` run with a hard human review checkpoint between layers.
- **Design system:** `Promptibrary Design System/` at the project root — canonical `tokens.css`, `typography.css`, `base.css`, plus all nine screen mockups in `screens/`. **Do not regenerate.** Copy or reference.

---

## Current build status

Source of truth: latest Git tag matching `layer-*-complete`. Check the box and tag when you finish a layer.

- [x] L0 — Foundations
- [x] L1 — Vault and Variables
- [x] L2 — IPC and Frontend Shell
- [x] L3 — Launch Pipeline
- [x] L4 — Extraction
- [ ] L5 — Polish and Ship

---

## Layer scope discipline

Each `Prompts/L*.md` declares an **In scope** list and an **Out of scope** list. Hold the line:

- If a problem surfaces outside the current layer's scope, log it in `docs/notes/L<n>-observations.md` and continue. Do not fix it.
- Exception: if the observation *blocks* current work, stop and surface. Don't route around it silently.
- If a problem in a *previous* layer surfaces (missing domain field, wrong contract, etc.), stop and surface. Don't patch the previous layer silently. The reviewer decides whether to amend the previous tag or carry forward.

---

## Implementation notes log

Maintain a running `docs/implementation-notes.html` documenting anything the human reviewer needs to know that **isn't already captured by the spec, a Linear ticket, or a commit message**. This is the "things you should know" channel — decisions, deviations, and tradeoffs that don't have an obvious home elsewhere.

### What goes in it

- **Decisions made under-spec**: the spec was silent or ambiguous, you chose a path, here's what and why.
- **Deviations from the spec**: you had to do X instead of Y; reference the spec section, explain why, link the reconciling commit/ticket if any.
- **Tradeoffs**: chose simplicity over completeness, perf over readability, etc. — note what was given up.
- **Surprises**: an API behaved differently than expected, a crate version didn't have a documented feature, a CLI flag was renamed.
- **Workarounds**: temporary hacks with a "remove when X" condition. Each gets a follow-up ticket ID inline.

### What does NOT go in it

- Spec-conformant work (the spec already says it).
- Bug fixes (the commit + ticket say it).
- Layer-scoped observations that go in `docs/notes/L<n>-observations.md` (those stay there).
- Anything sensitive (keys, paths with PII, internal URLs).

### Format

Single HTML file, append-only, newest entries at the top. Each entry:

```html
<article data-date="YYYY-MM-DD" data-layer="L<n>" data-ticket="<TICKET-ID>">
  <h3>Short title</h3>
  <p><strong>Context:</strong> what you were doing.</p>
  <p><strong>Decision / change / tradeoff:</strong> what you did and why.</p>
  <p><strong>Impact:</strong> what this means going forward, what to revisit, when.</p>
</article>
```

Keep it readable when opened in a browser — minimal styling is fine, but it should be scannable, not a wall of text. Update it the same commit as the change it documents; never batch retroactively.

---

## Linear ticket discipline

**Every piece of work gets a Linear ticket before you start it.** Features, bug fixes, refactors, doc updates, dependency bumps, CI changes, spec corrections — all of it. No silent commits.

### Linear coordinates (don't go searching — they're here)

- **Team:** `Scalinity` (key `SCA`) — pass `team: "Scalinity"` to `mcp__linear-server__save_issue`.
- **Project:** `Promptibrary` (slug `promptibrary-a74809f89842`) — pass `project: "Promptibrary"`.
- **Layer parent issues** (use as `parentId` when filing sub-issues):

  | Layer | Parent ID | Title | Status |
  |---|---|---|---|
  | L0 | `SCA-543` | L0 — Foundations | filed |
  | L1 | `SCA-585` | L1 — Vault and Variables | filed |
  | L2 | `SCA-594` | L2 — IPC and Frontend Shell | filed |
  | L3 | — | L3 — Launch Pipeline | filed |
  | L4 | `SCA-680` | L4 — Extraction | filed |
  | L5 | — | L5 — Polish and Ship | **not yet filed** |

  When you start a new layer, file the parent ticket first, then update this table in the same commit as the first sub-issue's work.

- **Labels — reality check.** The taxonomy this file prescribes (`feature` / `bug` / `refactor` / `docs` / `infra` / `chore`, plus `L0`–`L5`) is **not yet provisioned** in the SCA team. Existing SCA tickets ship with `labels: []`. Until the labels are created, match existing convention and file without labels. If you provision them via `mcp__linear-server__create_issue_label`, update this note in the same commit.

### Workflow

1. **Before starting work**, create a Linear ticket via the Linear MCP. Required fields:
   - **Title** — concise, imperative. Examples: *"Implement variable parser per §5 EBNF"*, *"Fix PTY signal escalation timing"*, *"Bundle JetBrains Mono Variable locally"*.
   - **Description** — what you're going to do, in 2–5 bullets. Reference the spec section if relevant: *"per `docs/SPEC.md` §5 *Parser contract*"*.
   - **Acceptance criteria** — concrete and testable. Borrow from the layer prompt's acceptance criteria where applicable. If the layer doesn't pin them, write your own.
   - **Parent issue** — link to the layer-level ticket (e.g., *"L1 — Vault and Variables"*). Each layer has one parent ticket; modules within a layer are sub-issues.
   - **Labels** — `feature` / `bug` / `refactor` / `docs` / `infra` / `chore`, plus a layer label (`L0`, `L1`, …).

2. **Do the work.** Stay within the ticket's scope. If you discover an adjacent problem mid-work, file a separate ticket — don't expand the current one. "While we're in there" expansions are how commits become unreviewable.

3. **Reference the ticket in every commit message** for that work. Format:
   ```
   <type>(<area>): <subject> (<TICKET-ID>)
   ```
   Example: `feat(variables): implement parser with byte/UTF-16 offsets (PROM-42)`.

4. **Mark the ticket Done** only after the work is **committed AND pushed**. Not before. "Done" means:
   - Code is on the remote.
   - CI is green on that commit.
   - The acceptance criteria literally pass when re-read against the diff.

### Ticket granularity

One ticket per logical change worth a separate commit. The variable parser is **not** one ticket — it decomposes into:

| Ticket | Commit |
|---|---|
| Implement variable lexer per §5 EBNF | `feat(variables): lexer` |
| Implement variable parser with byte/UTF-16 offsets | `feat(variables): parser` |
| Implement variable renderer (end-to-start replacement) | `feat(variables): renderer` |
| Implement variable validation per §5 rules | `feat(variables): validation` |
| Variable parser golden test fixtures (§15 matrix) | `test(variables): §15 matrix` |

Five tickets, five commits, five pushes. Each independently reviewable.

### Small work still gets a ticket

Yes, for all of:
- One-line spec typo fix.
- Cargo / pnpm dependency bump.
- Adding a missing TODO comment that references a future layer.
- Renaming a Rust function for clarity.

The overhead is ~30 seconds. The benefit is that nothing in the repo history is unexplained.

### When NOT to create a ticket

Only when the work produces no committed artifact — e.g., running `cargo doc --open` to read API docs, or experimenting in a scratch buffer that won't be saved. The moment a tracked file changes on disk and will be committed, a ticket exists.

---

## Commit and push discipline

Inherits the global *Review and commit discipline*. Project-specific cadence:

### Commit cadence

- **One logical change per commit.** Granularity matches Linear ticket granularity (above).
- **Each commit compiles and passes its own verification.** `cargo check` for touched crates, `cargo test` for touched modules, `pnpm typecheck`, `pnpm lint` all green. Bisect-hostile commits erase their own diagnostic value.
- **Diff-first.** Before every `git commit`, run `git diff --staged` and confirm only intended changes are staged. Summaries lie; diffs don't.
- **Commit message format:**
  ```
  <type>(<area>): <subject> (<TICKET-ID>)
  ```
  - Types: `feat`, `fix`, `refactor`, `chore`, `docs`, `test`, `perf`, `style`, `build`, `ci`.
  - Areas: `vault`, `variables`, `launch`, `extraction`, `search`, `git`, `telemetry`, `ui`, `tokens`, `cli`, `tauri`, `ipc`, `settings`, `terminal`, `updater`.
  - Good: `feat(launch): direct PTY spawn without shell wrap (PROM-87)`
  - Good: `fix(variables): UTF-16 offset off-by-one in emoji prefix (PROM-104)`
  - Bad: `fixed stuff` / `wip` / `updates`

### Push cadence

- **Push after every ticket.** Not at end of layer. Not "at the end of the session." Per-ticket.
- Immediately after `git push`, transition the Linear ticket to Done. The two actions are paired; don't drift.
- If CI fails on the push, the **next ticket** is "Fix CI on `<TICKET-ID>`". File it, fix it, push it. Don't proceed to new work on red.
- **Don't rewrite pushed history.** Unpushed history is free to amend, rebase, or squash. Once pushed, it's coordination overhead. Push only after reviewing the chain as a whole — usually 1–2 commits per ticket.

### Branching

For the V1 layered build: work directly on `main`. Each layer's tag (`layer-0-complete`, etc.) is the integration point. Once V1 ships and we're into feature work, switch to feature branches with PR review per ticket.

---

## Architecture invariants

Pulled from the spec and from the verified `claude --help` reconciliation. Violating any of these is a build break, not a style preference.

### Frontend

- React 19, TypeScript, Vite 8, Tailwind v4 (CSS-first via `@theme`, no `tailwind.config.js`).
- shadcn/ui generated to `src/shared/ui/shadcn/`. Wrappers at `src/shared/ui/`. **ESLint enforces:** only `@/shared/ui/*` may import from `@/shared/ui/shadcn/*`. Other code imports the wrappers.
- All colors via CSS custom properties from `src/styles/tokens.css` (OKLCH). **No hex anywhere.** If you find yourself writing `#ff5c5c`, stop and use the right token.
- JetBrains Mono Variable bundled locally (no CDN). Boska + Switzer from Fontshare CDN OK in V1.
- CodeMirror 6 for the prompt body editor with `Decoration.replace` WidgetTypes for variable refs. The decoration replaces display; source text remains `{{type:key}}` for serialization.

### Backend (Rust)

- Tauri 2.x. Commands in `src-tauri/src/commands/`, registered in `src-tauri/src/lib.rs`.
- Domain types in `src-tauri/src/domain/` mirror TS types in `src/shared/types/` exactly. Mismatched serde casing silently breaks IPC — test at least one round-trip.
- Errors via `AppError` / `AppErrorKind` / `IpcError`. Every IPC handler returns `Result<T, AppError>`. Stubbed commands return `Err(AppError::internal("not_yet_implemented"))`.
- SQLite via `sqlx`. PRAGMA: `journal_mode=WAL`, `busy_timeout=5000`, `foreign_keys=ON`.
- Atomic writes for vault files: `<path>.tmp.<pid>` → fsync → rename. Use `util::atomic_write`.

### Variable system (L1)

- Parser is **pure**: no filesystem access, no symlink resolution. Validation lives in `validation.rs`.
- Compute **both** byte and UTF-16 offsets in the parser. CodeMirror operates in UTF-16; the conversion has to be exact, single-pass, inline.
- Renderer replaces from **end of template to start** to preserve offsets.
- `bool` rendering uses `renderTrue`/`renderFalse` strings (`frontmatter_strings` mode by default), not literal `true`/`false`.
- ULID is set at create-time and **never** changes. Renaming a prompt changes neither slug nor filename. Filename collisions append `-2`, `-3` at create-time.

### Launch pipeline (L3)

- **Direct PTY spawn, no shell wrapper.** `portable_pty::CommandBuilder::new(claude_path)`, never `CommandBuilder::new("/bin/zsh").arg("-lc")`.
- **Event-driven prompt injection.** Bracketed paste (`ESC[200~ ... ESC[201~ \r`) writes *after* the first PTY output signal fires, not after a 1200ms `sleep`.
- **DO NOT pass `--max-turns`** to `claude`. Not in the current CLI surface. The field exists in `LaunchDefaults` for forward-compat but is never passed to the invocation builder. Re-verify against `claude --help` at L3 pre-flight.
- Stop escalation: graceful = SIGINT → 5s → SIGINT → 5s → SIGTERM → 3s → SIGKILL. Force = SIGTERM → 1s → SIGKILL. Signals to the claude child PID directly.
- xterm theme via `getTerminalTheme()` reading CSS custom properties. Never hard-code ANSI colors.
- 24-bit color SGR preserved through ANSI normalization. OSC 8 hyperlinks preserved. OSC clipboard/title sequences stripped.

### Extraction (L4)

- **The article extractor is the LLM**, not Rust. Article fetcher uses deterministic subtree selection only: `<article>` → `<main>` → `[role=main]` → text-to-tag-ratio fallback. **No Readability-style scoring, no node weighting, no class-name heuristics.** If you find yourself writing `density_score` or `weight_boost`, stop.
- Extraction system prompt is a **byte-equal contract**. Canonical text in `docs/spec-snippets/extraction-system-prompt.txt`, mirrored as `EXTRACTION_SYSTEM_PROMPT` in `src-tauri/src/extraction/prompts.rs`. A test asserts byte-equality. Do not paraphrase, summarize, or "improve."
- Default model: `claude-sonnet-4-6`. Deep mode: `claude-opus-4-7`. Never silently upgrade.
- API keys never leave Rust. Frontend gets `SecretStatus`, never the value.
- Source text caps: standard = 60,000 chars, deep = 160,000 chars. Summarize structurally before sending if over cap.
- JSON repair: one attempt only. Do not loop.

### Search & embeddings (L5)

- bge-small-en-v1.5 via fastembed, dim 384, downloaded once to AppData with SHA-256 verified against a build-time manifest. **Fail closed on hash mismatch.**
- Embeddings local-only. No network calls after the one-time model download.
- Hybrid ranking: `0.55 * reciprocal_rank_text + 0.35 * normalized_semantic + recency_boost + usage_boost`. Exact title match pins to top.
- FTS5 weights: title=5.0, summary=2.0, body=1.0, tags=4.0.

### Settings & destructive actions

- Two distinct destructive actions for telemetry, **never bundled**:
  - *Clear telemetry cache* — drops aggregates, preserves runs/transcripts. Single click confirmation.
  - *Delete all run history* — drops `runs` table AND deletes transcript files. **Requires typed `"delete"` confirmation.**
- Secrets via keychain only (`keyring` 4.x). No secrets in vault YAML, ever.

### Updater (L5)

- tauri-plugin-updater, GitHub Releases, manifest URL embedded at build time.
- Signature verification **mandatory**. No trust-on-first-use escape hatch. Refuse install on verification failure.
- Private signing key in GH Actions secret `TAURI_SIGNING_PRIVATE_KEY`. Public key embedded in `tauri.conf.json`.

---

## Design system discipline

- `Promptibrary Design System/` at the project root is the **canonical source**. Do not regenerate `tokens.css`, `typography.css`, or `base.css`. L0 copies them into `src/styles/`.
- `Promptibrary Design System/screens/index.html` renders all nine mockup screens side-by-side. This is the visual contract for L2 — every layout, every interaction state, every component is encoded there.
- If you need an additional token for a Tauri-only concern (e.g., terminal-specific), append it to `src/styles/tokens.css` under a `/* === Additions beyond canonical design system === */` comment block and note the divergence in `docs/notes/L<n>-observations.md`.
- "No drift from the design system" is a hard rule. If a layout decision in your React port conflicts with `Promptibrary Design System/screens/`, flag it — don't choose the divergent path silently.

---

## Visual regression discipline

The design system is enforced via Playwright visual regression tests against committed baselines. Each screen in `Promptibrary Design System/screens/` has one or more specs under `tests/visual/`. Pixel diffs catch drift the eyeball check misses.

### How it works

1. **E2E mode IPC mock.** With `VITE_E2E_MODE=true`, `vite.config.ts` aliases `@/shared/api/ipc` to `@/shared/api/ipc.mock`. The mock exports the same typed function signatures as the real module but returns deterministic fixture data designed to mirror the corresponding design system mockup's content.
2. **One spec per screen-state combination.** Each Playwright test loads the app at the relevant route with mocked data and asserts `await expect(page).toHaveScreenshot({ maxDiffPixelRatio: 0.02 })`.
3. **Baselines live in `tests/visual/__screenshots__/`** and are committed to the repo. Platform-specific: macOS-aarch64 only for V1.
4. **CI runs visual tests on macOS only.** Font rendering and antialiasing differ across platforms; cross-platform visual baselines are not stable. Linux CI runs everything *except* `pnpm test:visual`.

### Baseline capture rules

- First-time baseline: `pnpm test:visual:update`.
- **Before committing the baseline, manually verify it matches the corresponding `Promptibrary Design System/screens/*.html` mockup side-by-side.** The baseline becomes the canonical "what the app should look like" — if wrong at capture time, the regression test is meaningless.
- Capture, verification, and commit happen in the same ticket as the screen's React implementation. Don't split across tickets.
- The Linear ticket description must note which baseline was verified against which mockup (e.g., "verified `01-library-compose.png` against `screens/01-library-compose.html`").

### Failure handling

If a visual test fails:

1. Open `pnpm test:visual:report` to see the pixel diff side-by-side.
2. **Intentional change** (design system updated): update **both** the baseline AND the corresponding `Promptibrary Design System/screens/*.html` mockup in the same commit. They don't drift independently — they're both the design source of truth.
3. **Unintentional change**: fix the React code. Do not update the baseline to silence the test.

### Threshold

`maxDiffPixelRatio: 0.02` (2%) project-wide default. Lower catches structural drift; higher tolerates subpixel font rendering and animation easing. If a specific test needs a different threshold, configure it inline with a comment justifying it (e.g., terminal scroll animation; semantic search loading shimmer).

### Adding a new screen

When a layer introduces a new UI surface:

1. Implement the React component(s).
2. Add a spec under `tests/visual/<screen-name>.spec.ts`.
3. Add fixtures to `tests/fixtures/visual/` matching what the mockup shows.
4. Run `pnpm test:visual:update` to capture the baseline.
5. Manually verify the baseline against `Promptibrary Design System/screens/<name>.html`.
6. Commit implementation + spec + fixtures + baseline in the same ticket.

---

## Critical don'ts

Each has a spec section justifying the rule. These are the highest-value foot-guns to avoid.

| Don't | Why | Spec |
|---|---|---|
| Pass `--max-turns` to `claude` | Not in current CLI surface; invocation will fail | §7 risk note |
| Shell-wrap the claude invocation | Signals stop reaching the child PID | §7 *Stop action* |
| `sleep(1200ms)` before bracketed paste | Event-driven only via first-output signal | §7 *Prompt injection* |
| Reimplement Readability-style scoring | The LLM is the extractor | §6 *Article fetcher* (patched) |
| Paraphrase the extraction system prompt | Byte-equal contract | §6 *Extraction prompts* |
| Bundle the two destructive actions | Two distinct UIs, two distinct confirmations | §13 (patched) |
| Trust-on-first-use the updater signature | Mandatory verification | §16 (patched) |
| Store secrets in the vault | Keychain only | §13 *Secrets* |
| Pass `ANTHROPIC_API_KEY` to claude child env | CC handles its own auth | §7 *Environment* |
| Compute UTF-16 offsets in a second pass | Inline in the parser, single pass | §5 *Parser contract* |
| Auto-commit after revert | User commits when they want | §10 *Revert* |
| Use hex colors anywhere | OKLCH tokens via CSS variables | §8 *Design tokens* |
| Render `bool` as literal `true`/`false` | Use `renderTrue`/`renderFalse` strings | §5 *Renderer* |
| Skip the §15 fake-claude integration test | The PTY pipeline must be exercised, not simulated | §15 *Fake Claude launch* |

---

## Tooling priority (project-specific layering on top of global)

1. **Sequential Thinking** for planning before non-trivial work. Especially L1 parser, L3 PTY pipeline, L4 extraction state machine, L5 hybrid ranking.
2. **Linear MCP** — ticket creation *before* every work unit, transition to Done *after* push.
3. **Morph (`filesystem-with-morph:edit_file`)** for multi-edit files (Cargo.toml, package.json, settings panels, ESLint config).
4. **Documentation lookup (`context7`, `Ref`)** for unfamiliar APIs. High-likelihood targets: shadcn/ui + Tailwind v4, CodeMirror 6 `Decoration.replace` + `StateField`, `portable-pty` 0.9 API, `fastembed` 5.x, `git2` rename detection options, `tauri-plugin-updater` signing flow, Apple `notarytool` async submission.
5. **Playwright** for E2E coverage at the end of each layer.
6. **Filesystem MCP** for file operations not covered above.

---

## Spec reconciliation rule

If your priors conflict with the spec, follow the spec.

If the spec conflicts with reality (e.g., `claude --help` shows a flag the spec says doesn't exist, or a Rust crate's API has changed), **stop**, update the spec in a dedicated commit titled `chore: reconcile <section> against <reality>` with its own ticket, then continue. Do not silently work around spec/reality mismatches.

If the spec is genuinely wrong (not just stale), surface in `docs/notes/spec-disputes.md` before changing it. The spec is the contract; changing it is a coordination event, not a minor edit.

---

## Session checkpoints

Per the global rule on long sessions: at every layer boundary (i.e., before tagging `layer-N-complete`), produce a status sync:

- Acceptance criteria outputs from each test.
- Tickets closed in this layer, with their commit hashes.
- Anything in `docs/notes/L<n>-observations.md` the human reviewer needs to see.
- Open V2 candidates surfaced during the layer (go to `docs/V2-CANDIDATES.md`).

This is the artifact the human reviews before unlocking the next layer.
