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

