# L5 — Polish and Ship observations

Running log of decisions, surprises, and out-of-scope items discovered during the L5 build. Append-only.

---

## 2026-05-19 — Layer kickoff

### Start state

- All L0–L4 layer tags present (`layer-{0,1,2,3,4}-complete`).
- L5 Rust modules exist as 3-line `not_yet_implemented_stub()` placeholders: `index/fts.rs`, `index/embeddings.rs`, `index/telemetry_repo.rs`, `index/runs_repo.rs`, `git/{diff,history,repo,revert}.rs`, `commands/{search,git,system}.rs`.
- `commands/settings.rs` already has the three secret commands (set/clear/get_status) wired from L4; the two settings commands (`get_settings`, `update_settings`) are stubs.
- SQLite migrations through `0007_extraction_candidates_cache.sql` are in place. FTS5 triggers in `0002_fts.sql` + `0006_fts_tags_sync.sql` already keep `prompts_fts.tags` synced from `prompt_tags` — repo layer doesn't need to recompute.
- Cargo deps in place: `tauri-plugin-updater = "2.10"`, `sqlx 0.8.6`, `keyring-core = "1"`, `sqlite-vec = "0.1.9"`.
- **Missing**: `fastembed` is not yet in `Cargo.toml` — will be added with the semantic search ticket.

### Patched-spec posture documented in CLAUDE.md (this layer)

The user pre-edited `CLAUDE.md` to capture the L5 updater clarification:

- "Tauri signer signature" verification on update bundles is mandatory and **distinct from Apple code signing**.
- macOS bundles for V1 are intentionally unsigned by Apple — no Apple Developer account, no notarization, no `APPLE_*` CI secrets.
- First-launch Gatekeeper bypass is documented in `docs/INSTALLING.md` (this layer).
- "Add Apple code signing or notarization to V1" is added to the *Critical don'ts* table as an explicit V2 candidate.

The CLAUDE.md change ships in the same commit as the updater ticket work (not the icon ticket).

### Linear MCP auth intermittency during L5

The Linear MCP server intermittently returns `token expired` mid-session even after a fresh `/login` round-trip. When that happens during L5 sub-ticket filing, the workflow falls back to:

1. Commit references the L5 parent `SCA-729` instead of a sub-ticket ID.
2. The commit subject still encodes the area (`feat(search): ...`), and the body cites the parent.
3. The sub-ticket is filed retroactively once the MCP reconnects, and the back-fill is noted here with `<commit-sha> → SCA-<id>` so the audit trail is recoverable.

Pending back-fills (sub-tickets to file once Linear reconnects):

- `feat(search): FTS5 read surface + search_prompts/suggest_tags IPC` — landed against SCA-729 parent.

### Deferred to follow-up sessions: native-dep tickets

Two L5 tickets require enabling crates that ship native dependencies and are deliberately commented out in `src-tauri/Cargo.toml`:

- **Semantic search (`index/embeddings.rs`)** — requires `fastembed = "5.13"` which transitively pulls `ort` / ONNX Runtime native libs. Real integration also needs:
  1. `build.rs` baking a SHA-256 manifest for the bge-small-en-v1.5 ONNX files (or generated from a vendored manifest committed alongside the model name).
  2. AppData download flow with `index://progress` events, resumability, and fail-closed hash verification.
  3. sqlite-vec `vec0` virtual table created at runtime (the migration intentionally leaves it out because `sqlx::migrate!` does not load extensions).
  4. Reindex queue with concurrency 1 + idle scheduler.

  Hybrid ranking in this layer lands with the **text-only score path**. The semantic toggle in the frontend stays disabled until the model integration lands. The hybrid score formula degrades cleanly to `0.55 * RRF_text + recency_boost + usage_boost` when the semantic component is absent.

- **Git history / diff / revert (`git/*`)** — requires `git2 = "0.20"`. The libgit2 build adds ~30s to clean builds and complicates CI matrix entries. Real integration also needs `git_diff_find_options` with rename detection capped at `versionHistory.renameDetectionWindow = 200` commits per the patched §13.

  Three IPC commands (`get_prompt_history`, `get_prompt_diff`, `revert_prompt_to_commit`) remain stubs after L5 lands. The frontend history panel reads stub results and renders the empty state until the git2 ticket completes.

### Deferred to follow-up sessions: tickets that need user-interactive setup or release infra

- **Tauri signer keypair generation** — `tauri signer generate` is interactive; the private key must be pasted into a GH Actions secret (`TAURI_SIGNING_PRIVATE_KEY`) by the human operator. The L5 ticket lands the manifest URL + `pubkey` placeholder in `tauri.conf.json`, the updater plugin configuration, and the `docs/RELEASING.md` documenting the rotation path; the actual keypair generation is a checklist item, not an automated step.
- **CI release workflow dry-run** — the `release.yml` workflow can be authored without a dry-run, but the spec's stop-condition requires a real `v0.0.1-rc1` tag push and verification that `latest.json` resolves correctly. That happens outside the build agent's reach; the L5 ticket lands the YAML and the verification steps in `docs/RELEASING.md`, but does not gate the layer on a successful release run.
- **Visual regression baselines** — each new spec needs `pnpm test:visual:update` to capture and **human verification against the mockup** before commit. The L5 ticket lands the spec files + fixtures + a capture-script invocation; the manual verification + baseline commit happens in a follow-up.
- **V1 verification report** — depends on all of the above being end-to-end green. Captured at the natural end of the layer, not mid-stream.

### What lands this session

Reachable without native-dep tickets or user-interactive infra:
- ✅ FTS5 text search (committed)
- ⏭ Hybrid ranking — text-only score path (semantic component returns 0)
- ⏭ Cmd-K palette wiring (prompts via FTS, runs via direct query, static actions+routes)
- ⏭ Telemetry events (`record_launch_metric` + L3 lifecycle hooks)
- ⏭ Telemetry aggregates (`prompt_stats`, `tag_stats` rebuild + incremental)
- ⏭ Two destructive actions (clear_telemetry_cache + delete_all_run_history)
- ⏭ Settings get/update (atomic write to local JSON + vault `promptibrary/settings.yml`)
- ⏭ System probes (`probe_dependencies`, `reveal_in_terminal`, `open_path`)
- ⏭ Updater plugin scaffolding (manifest URL, pubkey placeholder, mandatory-verification config)
- ⏭ Docs trio: `docs/RELEASING.md`, `docs/INSTALLING.md`, expanded `docs/V2-CANDIDATES.md`
- ⏭ CLAUDE.md updater-posture commit (already staged in the user's working tree)
- ⏭ `.github/workflows/release.yml` YAML (un-dry-run-verified)

Tagging `layer-5-complete` happens **only after** the deferred set above also lands; this session ends with an explicit handoff list, not a tag.

### Stop-and-surface: L3 launch pipeline never persists `runs` rows

Discovered while wiring L5 telemetry hooks: the launch pipeline (under `src-tauri/src/launch/`) does **not** insert into the `runs` table at any lifecycle moment. `index/runs_repo.rs` is still a 3-line stub from L0. The only `INSERT INTO runs` statements in the codebase live in migration test helpers and in this layer's own test fixtures.

This is a previous-layer gap (L3 tagged complete but missing this surface), so per CLAUDE.md *Layer scope discipline* — *"If a problem in a previous layer surfaces, stop and surface."* — I am not silently patching the launch pipeline.

**Impact on L5:**
- L5 telemetry repo (`record_event` + `prompt_aggregates`) can be implemented and tested correctly against synthetic `runs` rows in unit tests.
- The end-to-end acceptance criterion *"launch the same prompt 3× ; library row shows launch_count: 3"* cannot pass until L3 is reconciled — runs aren't being persisted to count.
- Per-prompt usage stats consumed by the hybrid-ranking commit are likewise inert in production until run rows land.

**Reconciliation path (for the human reviewer):**
- Option A: amend `layer-3-complete` with a follow-up commit that implements `index/runs_repo.rs::{insert_run, update_run_status, complete_run}` and wires the three call sites (start, first-output, finish) into the L3 launch pipeline. File this as a sub-issue of `SCA-729` (or under the L3 parent if it was tracked).
- Option B: carry forward — accept the gap into L5 and treat the runs persistence as part of the telemetry ticket. This blurs the layer boundary but avoids retagging L3.

Recommend Option A — the persistence belongs to L3 conceptually and the runs table is what L5 reads, not what L5 owns. The reviewer makes the call.

### Update 2026-05-19 19:00 — actual L3 scope is bigger than just runs-persistence

While starting the SCA-785 DEF-4 fix I discovered the L3 gap is much broader than the L5-telemetry surfacing implied:

- **Every file under `src-tauri/src/launch/` is a 3-line stub.** `claude_cli.rs`, `pty_session.rs`, `pty_pool.rs`, `prompt_injector.rs`, `transcript_writer.rs`, `signals.rs`, `process_probe.rs` — all `not_yet_implemented_stub` placeholders.
- **`portable-pty = "=0.9.0"`** is commented out in `src-tauri/Cargo.toml` with the note `TODO(L3): re-enable when launch pipeline lands`.
- **`commands::launches::{start_launch, stop_run, send_terminal_input, resize_terminal}`** are all `not_yet_implemented_stub` IPC handlers.

L3 was tagged `layer-3-complete` but the launch pipeline doesn't exist. The L5 telemetry surface was built against synthetic test data; the runs table is never populated in production because nothing inserts into it.

**What SCA-785 lands:** the `runs_repo::{insert_run, update_run_status, complete_run}` storage surface with 5 unit tests. This is the surface the future L3 launch pipeline will call. It's correct and tested in isolation; it just doesn't have any production caller yet.

**What SCA-785 does NOT land:** the actual L3 launch pipeline. That requires:
1. Uncommenting `portable-pty = "=0.9.0"` in Cargo.toml.
2. Implementing `launch::pty_session` (PTY spawn with `CommandBuilder::new(claude_path)`, no shell wrapper per CLAUDE.md).
3. Implementing `launch::prompt_injector` (event-driven bracketed paste after first PTY output).
4. Implementing `launch::signals` (graceful SIGINT → SIGINT → SIGTERM → SIGKILL escalation, force SIGTERM → SIGKILL).
5. Implementing `launch::transcript_writer` (ANSI normalisation, 24-bit color SGR preservation, OSC 8 hyperlinks preserved, OSC clipboard/title stripped).
6. Implementing `launch::pty_pool` (active-run cap per LaunchDefaults).
7. Wiring `commands::launches::{start_launch, stop_run, send_terminal_input, resize_terminal}` against the above.
8. Hooking `runs_repo` writes at the three lifecycle moments (start, status transitions, terminal).
9. Hooking `telemetry_repo::record_event` at the same moments.
10. The §15 fake-claude integration test that exercises the whole pipeline.

This is a genuine L3-from-scratch implementation effort, not the L5-deferral cleanup I expected. The right path for the human reviewer:

- **Option A (recommended):** Amend `layer-3-complete` with a dedicated L3-reconciliation session that follows `Prompts/L3.md` from the top. Tag rotates from `layer-3-complete` → new commit ID.
- **Option B:** Treat the gap as carry-forward, mark V1 as launch-disabled, and ship the rest of V1 with a clear "launching prompts not yet supported" UI state.

The runs_repo surface from SCA-785 is safe under either option — it's correct as a standalone DB surface that the launch pipeline (whenever it lands) will use.

---

## End-of-session summary (2026-05-19)

This session landed the L5 surfaces reachable without enabling commented-out native deps (fastembed, git2) and without user-interactive infrastructure (Tauri signer keypair, real release dry-run, visual baselines). **Layer 5 is not complete.** The `layer-5-complete` tag is deliberately NOT cut.

### What landed (commits on `main`)

| Commit | Subject | Lines |
|---|---|---|
| `0b9bd6a` | `chore(branding): replace app icon assets with new brand image (SCA-728)` | 53 files |
| `7a455ec` | `docs(L5): file parent ticket SCA-729 and open observations log (SCA-729)` | 2 files |
| `74a82c3` | `feat(search): FTS5 read surface + search_prompts/suggest_tags IPC (SCA-729)` | +791/-10 |
| `0d863c6` | `feat(search): hybrid ranking score formula + cmdk_search backend (SCA-729)` | +687/-96 |
| `876f744` | `feat(search): wire Cmd-K palette to cmdk_search IPC + ScoreParts contract (SCA-729)` | +214/-56 |
| `1e28472` | `feat(telemetry): event log + per-prompt aggregates + destructive actions (SCA-729)` | +641/-2 |
| `654d3f7` | `feat(system): probe_dependencies + reveal_in_terminal + open_path (SCA-729)` | +371/-11 |
| `2d1f19c` | `docs(L5): INSTALLING + RELEASING + V2 candidates for ship readiness (SCA-729)` | +161/-0 |
| `678a116` | `ci(release): tag-triggered release workflow with Tauri signer (SCA-729)` | +136/-0 |

Lib test count: 248 → 262 (14 new tests). JS Vitest: 26/26. Typecheck + lint clean.

### Linear back-fill needed

The Linear MCP server's token went stale partway through the session and never recovered cleanly. The commits all reference the L5 parent `SCA-729`; sub-tickets should be filed retroactively from this list. Suggested sub-ticket titles, one per logical-change commit (paired by SHA):

- `74a82c3` → "Wire FTS5 text search read surface + search_prompts/suggest_tags IPC"
- `0d863c6` → "Hybrid ranking score formula + cmdk_search backend"
- `876f744` → "Wire Cmd-K palette to cmdk_search IPC + ScoreParts contract"
- `1e28472` → "Telemetry event log + per-prompt aggregates + destructive actions"
- `654d3f7` → "System commands: probe_dependencies + reveal_in_terminal + open_path"
- `2d1f19c` → "Docs trio: INSTALLING.md + RELEASING.md + V2-CANDIDATES updates"
- `678a116` → "Tag-triggered CI release workflow with Tauri signer"

### Explicit handoff list (what next session needs to do)

1. **Enable `fastembed`** (`src-tauri/Cargo.toml`) and land semantic search per the spec §9 model-download + sqlite-vec wiring described in this file's *Deferred to follow-up sessions* section. Flip the semantic component in `commands/search::hybrid` from the constant 0 to the real cosine value.
2. **Enable `git2`** (`src-tauri/Cargo.toml`) and land Git history / diff / revert per spec §10. Three IPC commands; revert must validate the blob as a Promptibrary prompt before atomic write and trigger a watcher reindex without auto-committing.
3. **Reconcile the L3 launch-pipeline runs-persistence gap** (stop-and-surface item above). Either amend `layer-3-complete` with `index/runs_repo.rs` + hooks, or accept the gap and carry forward.
4. **Generate the Tauri signer keypair** (`tauri signer generate`), paste the public key into `src-tauri/tauri.conf.json::plugins.updater.pubkey`, and store the private key in GitHub Actions secret `TAURI_SIGNING_PRIVATE_KEY`. Until then the release workflow's `pnpm tauri build` will fail at the signing step.
5. **Settings disk persistence** — wire `get_settings`/`update_settings` to actually read/write the local JSON (AppData) + vault YAML (`<vault>/promptibrary/settings.yml`) per spec §13. Currently typed surface only.
6. **Visual regression**: add specs for `05-history-diff`, `06-command-palette`, `07-settings-{vault,secrets,telemetry,telemetry-confirm,diagnostics,diagnostics-failures,updater}`, `08-empty`, `09-disconnected`. Each baseline must be manually verified against the corresponding `Promptibrary Design System/screens/*.html` mockup before commit. The release workflow already gates on `pnpm test:visual` so missing baselines will surface as failures.
7. **Release dry-run** against a real `v0.0.1-rc1` tag push — verify all artifacts, `latest.json` resolution, and the in-app updater path from a previous build. Spec stop-condition requires this before cutting `v0.1.0`.
8. **`docs/V1-VERIFICATION.md`** — write the report after the above lands, with the command + output for each of the 10 spec §1 success criteria.
9. **Tag** `layer-5-complete` AND `v0.1.0-pre` only after the verification report is green.

### Why this session stopped here

The remaining items each need infrastructure outside the build agent's reach:
- Tauri signer keypair generation is interactive.
- The release dry-run requires pushing a real tag and watching GH Actions.
- Visual regression baselines need manual human verification against mockups before commit (the design-system contract is the source of truth, not the captured baseline).
- The V1 verification report needs the above to actually be green.
- Native-dep tickets (fastembed, git2) are each multi-hour focused work with their own test infrastructure cost.

A deeper-but-still-incomplete L5 would be net worse than the explicit handoff this leaves behind. The acceptance-criteria pass at the top of `Prompts/L5.md` is unchecked except for what specifically landed; the reviewer should walk it before unlocking the V1 ship gate.
