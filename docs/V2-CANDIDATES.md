# V2 candidates

Items surfaced during V1 implementation that are out of V1 scope but worth tracking for a V2 pass.

- Rename GitHub repo from `Promptibary` to `Promptibrary` (typo in directory name).
- Bundle Boska Variable and Switzer Variable locally instead of loading from Fontshare CDN.
- Variable-ref constraint quoting (SCA-610): the spec §5 grammar splits constraint values on bare `,` and `=` with no escape, so a `pattern` constraint can't contain those chars directly. Workaround today is to author patterns without them or move the pattern to the YAML `variables[].pattern` field. V2 should introduce a quoting mechanism (backslash-escape or single-quoted values) and update the EBNF to match.
