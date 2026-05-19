# L1 Observations

## Architectural deviations

- **VariableType not Hash**: I needed a per-type shorthand counter in the parser. Rather than add `#[derive(Hash)]` to the domain `VariableType` (and risk touching L0 territory while a parallel agent was actively iterating on domain types), I used `Vec<(VariableType, usize)>` for O(N) lookup. Negligible cost given N <= 7. If the codebase ever wants Hash on VariableType, the parser will switch back to a HashMap.

- **OffsetMap co-located in lexer**: Originally planned as `variables::offsets` but parallel agent kept reverting `variables/mod.rs` to not include `pub mod offsets`. Inlined into `lexer.rs` instead.

- **regex crate added**: Variable `pattern` validation uses the full `regex` crate (default-features=false, std + unicode-perl) rather than a hand-rolled shim. This was the right call — the shim was incomplete and the spec mandates real regex behavior.

## Open items for L2 reviewer

- Watcher events plumb to `vault://changed` in L2 once Tauri events module is wired.
- Scanner progress callback drops emissions in commands::vault::scan_vault_cmd; L2 hooks it to `index://progress` when the AppHandle is available.
- SqlitePool path is a single per-install file at `~/Library/Application Support/promptibrary/index.sqlite`. Multi-vault routing arrives in L5.

## L0 hygiene that the parallel /address agent already addressed

- SCA-564: db.rs PRAGMA wiring
- SCA-565: FK cascade
- SCA-566: FTS tags sync
- SCA-570/571: domain field cleanups (origin_url on ManualSource, allow_multiple/allow_custom)
- SCA-572: sqlx error mapping with extended codes
- SCA-573: i64 byte counts
- SCA-574: ULID Default impls removed

L1 work absorbed these changes without conflict.
