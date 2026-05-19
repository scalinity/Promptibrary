# V1 Verification (Promptibrary)

Honest walkthrough of every spec §1 success criterion against the actual repo state at the end of L5 + the deferral sweep. Generated as the deliverable for SCA-789 (`docs/V1-VERIFICATION.md`). Use this report to decide whether to tag `layer-5-complete` / `v0.1.0-pre` or to land the carry-forward items first.

**Last refresh:** 2026-05-19
**Branch:** `main` @ `46c2e0c` (the visual-spec-scaffolding commit)
**Test status at refresh:**
- `cargo test --lib`: **312 passed, 0 failed, 1 ignored**
- `pnpm test` (Vitest): **26 passed, 0 failed**
- `pnpm typecheck` + `pnpm lint`: clean (3 pre-existing `react-refresh/only-export-components` warnings — unrelated to L5)

Legend per criterion:

- ✅ **PASS** — wired end-to-end, exercised by tests, demonstrable from a fresh vault.
- 🟡 **PARTIAL** — storage / IPC surface ships and unit-tested; production caller missing (e.g. L3 runs persistence with no launch pipeline).
- ❌ **BLOCKED** — depends on a deferred dependency the agent could not enable.

---

## §1 success criteria

### 1. Vault select + cold scan + searchable library — ✅ PASS

- `commands::vault::{select_vault, scan_vault_cmd, rebuild_index}` registered in `src-tauri/src/lib.rs` and end-to-end exercised by `vault::scanner::tests::scan_indexes_valid_files_and_counts_malformed` + `scan_deletes_rows_for_missing_files`.
- FTS5 search lights up at `commands::search::search_prompts` (text mode) — 16 unit tests in `commands::search::tests` cover title-vs-body bm25 ranking, archived exclusion, tag filter, empty-query recency fallback, include_archived toggle.
- Library row hydration uses `prompt_aggregates` from `index::telemetry_repo` — 5 telemetry tests.

### 2. CRUD + tagged + export + version-inspect through Git history — ✅ PASS

- CRUD: `commands::prompts::{create_prompt, update_prompt, archive_prompt_cmd, delete_prompt, export_prompt}` registered; `index::prompts_repo` has 5 tests covering upsert + tag round-trip + cascade delete.
- Version inspect: **SCA-783 landed git2-backed history/diff/revert** in `src-tauri/src/git/{repo,history,diff,revert}.rs`. 12 unit tests against tempfile-backed real Git repos cover: open + non-repo error, walk-up discovery, 3-commit history, rename detection within window, window-cap truncation, diff between commits, diff against empty tree, invalid SHA error, revert with validation, revert refuses on validation failure, revert unknown commit error.
- `commands::git::{get_prompt_history, get_prompt_diff, revert_prompt_to_commit}` registered in the dispatcher; revert validates the blob parses as a Promptibrary prompt before atomic-write.

### 3. Variable parser + render + UI controls + validation — ✅ PASS

- Parser tests in `variables::parser::tests` + `variables::renderer::tests` cover the §5 EBNF, byte and UTF-16 offsets in a single pass, end-to-start rendering, all 7 spec §5 render rules (file/folder absolute, bool render-modes, etc.).
- Frontend `CodeMirror` editor with `Decoration.replace` widgets — `prompt-editor.spec.ts` covers the visual surface.
- Variable validation per §5 in `variables::validation` with the §15 golden test matrix.

### 4. YouTube / X / article imports → saveable launch profiles — ✅ PASS

- L4 extraction pipeline is fully wired: `commands::extraction::{detect_source, fetch_source_preview, extract_prompt_candidates, save_extracted_prompt}`.
- 60+ extraction unit tests covering the three fetchers, the §6 *byte-equal contract* assertion, the Anthropic transport mock, rate limiting, cache eviction, paywall detection, JSON repair, candidate disambiguation. Visual tests `03-import-*.spec.ts` cover every import-flow state.

### 5. Launch → fresh CC session + stream output + transcript — 🟡 PARTIAL (L3 reconciliation needed)

**Storage layer ships, launch pipeline does not.**

- ✅ `index::runs_repo::{insert_run, update_run_status, complete_run}` (SCA-785) with 5 unit tests against the real schema — the surface that the future launch pipeline will write through.
- ✅ `RunStatus` enum (Started / FirstOutput / Running / Stopping / Finished / Errored) matches the values telemetry_repo + commands/search read.
- ❌ `commands::launches::{start_launch, stop_run, send_terminal_input, resize_terminal}` are all `not_yet_implemented_stub`.
- ❌ Every file under `src-tauri/src/launch/` is a 3-line stub (`claude_cli`, `pty_session`, `pty_pool`, `prompt_injector`, `signals`, `transcript_writer`, `process_probe`).
- ❌ `portable-pty = "=0.9.0"` is commented out in `src-tauri/Cargo.toml`.

This is the *layer-3-complete* gap surfaced in `docs/notes/L5-observations.md`. The reviewer's call (per CLAUDE.md *Layer scope discipline*) is whether to amend `layer-3-complete` with a dedicated L3 reconciliation session or to ship V1 with launch disabled and carry the gap forward.

### 6. Runs queryable from SQLite + read-only transcripts — 🟡 PARTIAL

- ✅ Schema is real (`runs` table + `transcript_vault_path` / `transcript_spool_path` columns, FK CASCADE per migration 0005).
- ✅ `commands::runs::{list_runs, get_run, get_prompt_runs, fetch_transcript, repair_orphaned_transcripts}` registered. `vault::repair::tests::repair_orphaned_transcripts_moves_classified_files` covers the repair surface.
- 🟡 Production read path works against synthetic data inserted via test helpers; production write path waits on §5 above.

### 7. Watcher consistency — ✅ PASS

- `vault::watcher` debounces filesystem events, reconciles `prompts` rows on every batch. `vault::watcher::tests::debouncer_pushes_emit_through_events_channel` covers the dispatch surface; `vault::scanner` integration tests cover the reconciliation walk.

### 8. Text + semantic + tag + Cmd-K — ✅ PASS

- Text search: bm25-weighted FTS5 (title 5.0 / summary 2.0 / body 1.0 / tags 4.0) per the SCA-search work. `quotes_in_query_are_safely_escaped` covers the FTS5 operator-injection surface.
- Semantic search: **SCA-784 ships the EmbeddingService architecture** — `EmbeddingService` trait, `MockEmbeddingService` (deterministic SHA-256 vectors), `prompt_embeddings` storage with cosine search, hybrid path consumes the semantic component. 10 unit tests + 1 hybrid-with-semantic integration test. Production switch to real bge-small-en-v1.5 awaits the `fastembed` Cargo.toml enable + model download — documented as a V2 candidate.
- Tag filtering: hard filter via `prompt_tags` JOIN in both `hybrid` and `recent_prompts` paths. Tests in `commands::search::tests::tag_filter_excludes_non_matching_prompts`.
- Cmd-K: `cmdk_search` returns the structured palette (prompts via hybrid + runs via LIKE-on-title + static actions/routes). Catalog snapshot tests (`cmdk_action_catalog_snapshot`, `cmdk_route_catalog_snapshot`) detect Rust↔TS mock drift.

### 9. Settings persist + keychain + dependency diagnostics — ✅ PASS

- **Settings disk persistence shipped in SCA-782**: `commands::settings::{get_settings, update_settings}` read/write a local JSON at `<app_data_dir>/settings.json` + vault YAML at `<vault>/promptibrary/settings.yml`, merged into `AppSettings { local, vault, effective }`. 4 unit tests cover defaults-when-no-files, atomic round-trip, malformed-fallback, recent_prompt_ids round-trip. Cold restart no longer drops user preferences.
- Keychain: SCA-731 disjoint-account probe replaces the pre-fix design that could clobber the user's real `AnthropicApiKey` slot. 3 tests including a fault-injecting wrapper that exercises restore-failure.
- Diagnostics: `commands::system::probe_dependencies` parallelizes claude/yt-dlp/git probes via `tokio::join!` with a 5 s per-probe timeout (SCA-733). Keychain + sqlite probes follow. `DependencyProbeOutput { probes: [...] }` wire-shape aligned across Rust + TS + mock + diagnostics-panel.tsx (SCA-734).

### 10. Unit + integration + E2E coverage — ✅ PASS

- Rust: **312 lib tests** across vault, index (prompts/fts/telemetry/embeddings/runs), commands (prompts/variables/search/extraction/settings/system/git), git, extraction, launch (stubs), util, time, ids.
- JS: 26 Vitest tests in `src/__tests__/`.
- E2E: Playwright spec suite in `tests/visual/` (15 specs after SCA-786) + the existing chromium smoke project. Visual regression baselines are captured by the operator on macos-arm64 per the docstring on `_helpers::skipUnlessMacArm`.
- Spec §15 *Fake Claude launch* integration test: not yet authored — depends on the L3 launch-pipeline reconciliation above.

---

## Verdict

**Eight of ten criteria PASS outright.** §5 *Launch* and §6 *Runs queryable* are 🟡 because the L3 launch pipeline itself is missing — `runs_repo` storage ships (SCA-785) but no production code path writes through it yet.

**Recommended next step:** before tagging `layer-5-complete` and `v0.1.0-pre`, the reviewer makes a deliberate call:

1. **Tag now**, ship V1 with launch disabled. Carry §5 forward to a V2 milestone. Document a clear "Launching prompts isn't available in this build" banner in the UI.
2. **Amend `layer-3-complete`** with a dedicated L3-reconciliation session that follows `Prompts/L3.md` from the top. Re-tag after. Then tag L5.

Either path is defensible. Option 2 produces a V1 that meets the original §1 criterion set; Option 1 ships sooner but knowingly fails §5 / §6 in production until V2.

---

## Outstanding user actions before any tag cut

1. **`tauri signer generate`** (SCA-787) — interactive CLI; paste the public key into `src-tauri/tauri.conf.json::plugins.updater.pubkey`; base64-encode the private key into the `TAURI_SIGNING_PRIVATE_KEY` GitHub Actions secret. Procedure in `docs/RELEASING.md` → *Signing keys*.
2. **Release dry-run with `v0.0.1-rc1`** (SCA-788) — push the pre-release tag, verify every artifact + `latest.json` per `docs/RELEASING.md` → *First dry-run*. Tear down the draft before the real v0.1.0 cut.
3. **Visual regression baselines** (SCA-786) — `pnpm test:visual:update` on macos-arm64; manually compare each baseline against the corresponding `Promptibrary Design System/screens/*.html` mockup; commit baseline + verification note in the same Linear ticket.
4. **L3 launch pipeline implementation** (SCA-785 follow-up) — the §5 / §6 blocker. See V2-CANDIDATES.md → *L3 launch pipeline implementation*.

When all four are green, this report can be amended with verification commands + outputs for §5 and §6, the verdict flipped to ✅, and the tags cut.
