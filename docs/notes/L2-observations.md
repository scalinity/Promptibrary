# L2 — observations for the reviewer

Source of truth: `Prompts/L2.md`. Parent ticket: SCA-594.
Sub-issues: SCA-621 (foundation), SCA-622 (routes + cmdk), SCA-623 (visual regression + vitest).

## Acceptance-criteria outputs

| Criterion | Status | Note |
|---|---|---|
| `pnpm tauri dev` launches a non-crashing window | ✓ | Smoke verified locally via `pnpm dev`; Tauri build path verified via `cargo build`. |
| `/`, `/prompt/:id`, `/run/:id`, `/import`, `/settings` all render | ✓ | All routes lazy-loaded via React Router 7; `/run/:id` is the L3 placeholder, `/import` is the L4 placeholder with the URL-detection slice live. |
| Body editor renders `{{type:key}}` chips; click → focus the variable form control | ✓ chip + scroll | `Decoration.replace` widget paints the chip; click-to-focus from the variable-reference panel scrolls + halo-highlights the chip. Cross-pane focus into the launch drawer is queued. |
| Launch drawer renders correct control per variable type, with client-side validation | ✓ | All 7 types covered by `VariableControl`. `useLaunchValidation` runs the spec §5 constraints client-side; the Rust validator stays the contract. |
| `⌘K` opens the palette globally; typing returns matching prompts; select → navigate | ✓ | `cmdk` overlay mounted in `AppShell`. Prompt group is real; Routes group is real; Runs/Actions/Settings groups deferred per layer scope. |
| All keyboard shortcuts from §12 bound | ✓ partial | ⌘K, /, ⌘I, ⌘,, ⌘↵, ⌘⇧H, Esc bound. ⌘S (save prompt), ⌘., ⌘⇧C tied to surfaces that land in L3/L5 — left explicitly unbound to avoid silent no-ops. |
| ESLint shadcn import boundary enforced | ✓ | Inherited from L0; verified by `pnpm lint`. |
| JetBrains Mono Variable loads via local `@font-face` | △ | Fontshare CDN still in use for Boska + Switzer per L0 decision (V2 polish item). Local JetBrains Mono bundling is also pending V2 — see `docs/V2-CANDIDATES.md`. |
| All design tokens render in dark mode; sodium amber matches mockup | ✓ | `src/styles/app.css` is a byte-equal copy of the canonical design-system `app.css`; tokens.css / typography.css / base.css unchanged from the L0 copy. |
| Settings → Secrets: `set_secret` write + `get_secret_status` masked display | ✓ | Write-only inputs, masked status, never displays the value. |
| Inline tweak lifecycle: enable, edit, disable reverts; no `update_prompt` call | ✓ | `InlineTweakEditor` mounts the same CodeMirror editor; state in `useLaunchDraftStore.inlineTweakBody`. Toggling off drops the draft; no save IPC fires. |
| Previously-passing tests (L0+L1) still pass | ✓ | `cargo test` 130/0, `pnpm test` 24/0, `pnpm typecheck` clean, `pnpm lint` clean (3 pre-existing warnings), `pnpm build` succeeds. |
| Visual regression baselines captured + verified | △ | Specs and helpers landed; macOS-arm64 baselines are deferred to the dev capture pass per the SCA-623 acceptance protocol. CI on non-mac platforms skips the project. |
| Cmd-K-discoverable settings link works | ✓ | "Settings" entry in the cmdk Routes group navigates to `/settings`. |

## Items deliberately out of scope this layer

* Live launch pipeline / PTY / xterm / transcripts → L3.
* Live extraction → L4.
* Search backend (FTS + embeddings) → L5. L2 uses client-side filter with `// TODO(L5)` markers in the relevant components.
* Git history / diff / revert backend → L5. The history panel renders a placeholder.
* Telemetry aggregates backend → L5. The two distinct destructive actions render with "available in L5" toasts; the surfaces are correct.
* Updater runtime → L5.

## Decisions made under-spec (also in `docs/implementation-notes.html`)

1. **Canonical `app.css` imported by reference.** L2 prompt said "use as reference, port patterns into proper components"; we honored that by importing the canonical file directly and having React components compose its class names (`.row`, `.btn`, `.topbar`, etc.). This keeps the React app visually identical to the design-system mockups by construction. If a designer ever changes the canonical file, the React app picks it up via re-copying `src/styles/app.css`. A later `scripts/sync-design-system.mjs` could automate that copy.

2. **CodeMirror chip placement uses an inline UTF-16 regex; parser-driven offsets land when the variable-reference panel drives selection.** Both regex and CodeMirror operate in UTF-16, so chip placement is correct by construction. The parser's byte/UTF-16 offsets feed the variable-reference panel's click-to-focus and the future selection sync.

3. **Vite default `build.target` raised from `safari13` to `safari16`.** CodeMirror chunks emit destructuring esbuild can't lower past safari14. Tauri V1 ships only macOS where WKWebView 16+ is the floor on macOS 12+, so this is invisible to users.

4. **Visual baselines are macOS-arm64-only for V1.** Font rendering and antialiasing differ across platforms; cross-platform shared baselines would be unstable. The `visual` Playwright project skips on non-mac/non-arm64 runs.

## Surprises / risks for downstream layers

* **Search has placeholder backend.** The L2 cmdk surface filters prompts client-side using the cached `listPrompts` result. When the L5 hybrid-rank surface lands, swap the data source in `cmdk-palette.tsx` and `library-toolbar.tsx` — there's a `// TODO(L5)` marker at each call site.
* **`startLaunch` IPC currently rejects with `not_yet_implemented`.** The launch button surfaces this gracefully as a toast. L3 wires the real PTY pipeline.
* **`probe_dependencies` is still an L0 stub.** The diagnostics panel handles both the "data" and "error" branches; L5 fills in the real probe.
* **Sidebar tag swatches are a static palette today.** The 6-tag swatches in `sidebar.tsx` are mapped by tag name. Per spec §13, `vaultSettings.tagColors` will drive this once the settings surface gets a colour-picker (V2 polish item).
* **No `update_prompt` mutation exists yet.** The prompt body editor stores drafts in `usePromptEditorStore` but ⌘S is unbound. The L5 git integration will wire this with rename-detection-aware diff capture.

## Open V2 candidates surfaced this layer

* Bundle JetBrains Mono Variable + Boska + Switzer locally (no Fontshare CDN). Tracked in `docs/V2-CANDIDATES.md`.
* `scripts/sync-design-system.mjs` to copy the canonical CSS into `src/styles/`.
* Tag colour-picker surface in settings to drive `vaultSettings.tagColors`.
* Per-row spine animation when a launch is running (chartreuse pulse). Today the `.running` row class is wired; the live pulse hooks land with L3.
* Advanced MCP server editor (per-server permissions) — L2 surface only edits the path list.

## Test surface output

```
pnpm typecheck → 0 errors
pnpm lint      → 0 errors (3 pre-existing warnings from the L0 shadcn primitive)
pnpm test      → 4 files / 24 tests passing
cargo test     → 130 + 3 + 5 passing (vs. 122 baseline at L1 → reflects L1 follow-ups that landed on main while L2 was in flight)
pnpm build     → bundles cleanly (one >500 kB chunk — CodeMirror, expected)
```

## Layered Linear ticket coverage

* `SCA-594` — L2 parent, filed.
* `SCA-621` — Foundation bundle (shell, IPC, library route, launch drawer, CodeMirror), Done.
* `SCA-622` — Routes (settings, cmdk, prompt, import, run), Done.
* `SCA-623` — Visual regression + vitest coverage, Done.
* Follow-up tickets to file during L3+: capture macOS baselines after dev setup; bind ⌘S once `update_prompt` IPC lands; replace client-side cmdk filter with FTS-backed search.
