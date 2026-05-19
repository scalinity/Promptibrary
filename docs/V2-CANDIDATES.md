# V2 candidates

Items surfaced during V1 implementation that are out of V1 scope but worth tracking for a V2 pass.

- Bundle Boska Variable and Switzer Variable locally instead of loading from Fontshare CDN.
- Variable-ref constraint quoting (SCA-610): the spec §5 grammar splits constraint values on bare `,` and `=` with no escape, so a `pattern` constraint can't contain those chars directly. Workaround today is to author patterns without them or move the pattern to the YAML `variables[].pattern` field. V2 should introduce a quoting mechanism (backslash-escape or single-quoted values) and update the EBNF to match.
- Regex cache LRU bound (SCA-625): `variables::regex_cache` is unbounded process-wide. Bound via an LRU cache (cap ~256) once we have data on real-world pattern counts. Today's working-set is small enough that this is V2-grade work.
- UNIQUE constraint on `prompts.slug` (SCA-626): W-1 / SCA-589 closed the slug-collision race with an AppServices mutex. V2 should land a schema migration adding `UNIQUE(slug)` plus an INSERT … ON CONFLICT(slug) retry-with-bumped-suffix loop, then drop the mutex. The mutex is correct for V1 single-user; the schema-level fix is the long-term answer.
