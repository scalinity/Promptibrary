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
