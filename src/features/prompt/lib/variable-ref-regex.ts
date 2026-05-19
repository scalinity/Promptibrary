// Single source of truth for the `{{type:key}}` token shape that the
// CodeMirror chip decoration recognises. Mirrors the spec §5 EBNF and
// the Rust lexer's accepted shape — keep the three in sync.
//
// Exported (not inlined into the editor) so the vitest pin in
// `variable-chip-regex.test.ts` references the same literal. If you
// widen the regex here, both the editor and the test pick it up
// automatically (SCA-646).

export const VARIABLE_REF_REGEX =
  /\{\{(file|folder|text|multiline|select|bool|number):([a-zA-Z_][a-zA-Z0-9_]*)\}\}/g;
