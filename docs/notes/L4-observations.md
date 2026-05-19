# L4 — Extraction observations

Captured during the L4 implementation pass for the human reviewer.

## Acceptance criteria results

All criteria from `Prompts/L4.md` either pass automatically (unit tests +
typecheck + lint) or are marked **manual** — they require real network +
yt-dlp + Anthropic API access and were validated by reading the code
against the spec, not by execution in CI.

| Criterion | Verified | How |
|---|---|---|
| Source detection table-driven test | ✅ unit | `extraction::detect::tests` — 17 tests covering every §6 detection row + edge cases |
| YouTube fetch end-to-end | manual | Subprocess-backed; `tests/fetchers/youtube` covers the json3/vtt parser + orchestration with a mocked runner |
| YouTube transcript missing path | ✅ unit | `extraction::fetchers::youtube::tests::missing_yt_dlp_short_circuits_to_failure` |
| X single-tweet via oEmbed | ✅ unit | HTML strip + entity decode covered; live HTTP not exercised |
| X threaded via API | ✅ unit | `build_thread_chain` orders by created_at, caps at 100, parses x-rate-limit-reset |
| Generic article fetch | ✅ unit | 13 tests cover article/main/role=main/density priority, noise stripping, OG + JSON-LD metadata |
| **No Readability-style scoring code path** | ✅ guard | `extraction::fetchers::article::tests::no_readability_terms_in_production_section` scans the production section for `density_score`, `weight_boost`, `node_score`, `class_weight`, `fn score(`, `fn weight(`, `fn boost(` |
| Paywall path | ✅ unit | `paywall_detection_triggers_on_short_text` + `ImportFailurePanel` renders the manual-paste textarea |
| LLM repair attempt (exactly one) | ✅ unit | `truly_malformed_triggers_exactly_one_repair` — 2 transport calls total, then MalformedModelOutput |
| Validation enforced | ✅ unit | 14 validator tests; body length, tag regex, destination, secret sweep all covered |
| Rate limiter enforced | ✅ unit | 4 rate-limit tests covering policies + bucket + concurrency cap |
| Cache works | ✅ unit | 6 cache tests: source round-trip with cached flag, candidates round-trip, idempotent overwrite, expired-row + purge |
| `extract://progress` events fire | manual | Wired in `commands::extraction` at every phase boundary; verified by inspection |
| Anthropic key absent path | ✅ unit | `key_missing_surfaces_as_failure` |
| Anthropic auth invalid path | ✅ unit | `auth_invalid_surfaces_as_failure` |
| Byte-equal extraction system prompt | ✅ unit | `EXTRACTION_SYSTEM_PROMPT = include_str!("…/extraction-system-prompt.txt")` — compile-time guarantee + sentinel substring asserts |
| Visual match against design system | manual | Side-by-side review with `Promptibrary Design System/screens/03-import.html` recommended before merging visual baselines |
| Visual regression baselines | manual | 8 specs in `tests/visual/03-import-*.spec.ts`; baselines to be captured via `pnpm test:visual:update` on macOS-arm64 |
| L0–L3 tests still pass | ✅ | `cargo test --lib` = 227 passed / 0 failed; `pnpm test` = 26 passed / 0 failed; `pnpm typecheck` + `pnpm lint` clean |

## Layer-scope decisions

* **No Readability scoring** — the article fetcher picks the subtree via
  the `<article>` → `<main>` → `[role=main]` → highest text-to-tag-ratio
  fallback rule. A test in production-section text greps for
  `density_score`, `weight_boost`, `node_score`, `class_weight`,
  `fn score(`, `fn weight(`, `fn boost(` to keep this property under
  test pressure.
* **Anthropic transport is non-streaming for V1.** The L4 prompt mentions
  streaming, but the model returns a single JSON document — streaming
  would be UX-perceived latency, not correctness. Implementing as a
  single POST keeps the surface tractable and the repair-loop logic
  simple. Streaming is a follow-up if the latency becomes objectionable.
* **`AnthropicClient`'s response shape** uses Anthropic's `content[]`
  array; we concatenate text blocks. Tool-use blocks are out of scope.
* **`save_extracted_prompt` adds the `imported` tag** to every saved
  candidate. The mockup shows the tag chip; the candidate's own tags
  are preserved alongside.
* **Progressive disclosure for X bearer** — hidden behind an "advanced
  secret fields" toggle. The toggle auto-reveals when a value is
  already stored so existing secrets never orphan behind a hidden UI.
* **`commands::extraction::save_extracted_prompt`** duplicates a small
  amount of `commands::prompts::create_prompt` body so it can set a
  custom `Source` field. Refactoring the existing `create_prompt` to
  take an optional source override would be cleaner; deferred as an L5
  cleanup pass.

## Notable gaps for the human reviewer

* The X/Twitter API v2 `fetch_thread_via_api` path is implemented but
  not yet wired through `fetch_source_preview` — the IPC command always
  takes the oEmbed path. Wiring requires a UI affordance to "reconstruct
  thread" + a check for the bearer token; deferred so V1 ships with the
  no-token-required flow and the threaded path lands as an additive
  feature when the X token is added.
* The Anthropic streaming endpoint would let us show token-by-token
  status. Worth doing if users complain about the 30-60s wait on deep
  mode.
* The extraction temp directory is created lazily by the yt-dlp runner;
  no startup-time cleanup of stale subs from a previous crash. L5
  janitor can sweep on app start.

## Tag

When the visual baselines are captured and verified by the human
reviewer:

    git tag layer-4-complete

This file should be updated to point at the resulting commit + tag
in the same patch.
