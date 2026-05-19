//! Tokenize `{{...}}` blocks per spec §5 EBNF.
//!
//! Output is a flat token stream of `Text` and `VariableBlockRaw` tokens.
//! The lexer does not look inside `{{ }}` — that's the parser's job.
//!
//! Co-located with `OffsetMap` so a single forward pass over the template
//! gives both the byte/UTF-16 mapping CodeMirror needs and the token stream.

// ---------- Byte → UTF-16 offset mapping ------------------------------------

pub struct OffsetMap {
    pub byte: Vec<usize>,
    pub utf16: Vec<usize>,
}

impl OffsetMap {
    pub fn build(template: &str) -> Self {
        let mut byte = Vec::with_capacity(template.len() + 1);
        let mut utf16 = Vec::with_capacity(template.len() + 1);
        let mut u = 0usize;
        for (b, ch) in template.char_indices() {
            byte.push(b);
            utf16.push(u);
            u += ch.len_utf16();
        }
        byte.push(template.len());
        utf16.push(u);
        Self { byte, utf16 }
    }

    pub fn to_utf16(&self, byte: usize) -> usize {
        match self.byte.binary_search(&byte) {
            Ok(i) => self.utf16[i],
            Err(i) => {
                if i >= self.utf16.len() {
                    *self.utf16.last().unwrap_or(&0)
                } else {
                    self.utf16[i]
                }
            }
        }
    }
}

// ---------- Token stream ----------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Text,
    VariableBlockRaw,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_utf16: usize,
    pub end_utf16: usize,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub start_byte: usize,
    pub start_utf16: usize,
}

pub fn lex_template(template: &str, offsets: &OffsetMap) -> (Vec<Token>, Vec<LexError>) {
    let mut tokens: Vec<Token> = Vec::new();
    let mut errors: Vec<LexError> = Vec::new();
    let bytes = template.as_bytes();
    let mut i = 0usize;
    let mut text_start = 0usize;

    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if i > text_start {
                push_text(&mut tokens, template, offsets, text_start, i);
            }
            let block_start = i;
            let mut found: Option<usize> = None;
            let mut nested_open = false;
            let mut j = i + 2;
            while j + 1 < bytes.len() {
                if bytes[j] == b'}' && bytes[j + 1] == b'}' {
                    found = Some(j + 2);
                    break;
                }
                if bytes[j] == b'{' && bytes[j + 1] == b'{' {
                    // Another `{{` before any `}}` means the current ref is
                    // unclosed. Treat as unclosed at block_start and re-enter
                    // the outer loop at the nested `{{` position.
                    nested_open = true;
                    break;
                }
                j += 1;
            }
            if nested_open {
                errors.push(LexError {
                    start_byte: block_start,
                    start_utf16: offsets.to_utf16(block_start),
                });
                push_text(&mut tokens, template, offsets, block_start, j);
                text_start = j;
                i = j;
                continue;
            }
            // Edge: closing `}}` at the very end.
            if found.is_none() && bytes.len() >= i + 4 {
                let last = bytes.len() - 2;
                if bytes[last] == b'}' && bytes[last + 1] == b'}' {
                    found = Some(bytes.len());
                }
            }
            match found {
                Some(end) => {
                    tokens.push(Token {
                        kind: TokenKind::VariableBlockRaw,
                        start_byte: block_start,
                        end_byte: end,
                        start_utf16: offsets.to_utf16(block_start),
                        end_utf16: offsets.to_utf16(end),
                        raw: template[block_start..end].to_string(),
                    });
                    i = end;
                    text_start = end;
                }
                None => {
                    errors.push(LexError {
                        start_byte: block_start,
                        start_utf16: offsets.to_utf16(block_start),
                    });
                    push_text(&mut tokens, template, offsets, block_start, template.len());
                    text_start = template.len();
                    i = template.len();
                    break;
                }
            }
        } else {
            i += 1;
        }
    }

    if text_start < template.len() {
        push_text(&mut tokens, template, offsets, text_start, template.len());
    }

    (tokens, errors)
}

fn push_text(out: &mut Vec<Token>, template: &str, offsets: &OffsetMap, start: usize, end: usize) {
    if end <= start {
        return;
    }
    out.push(Token {
        kind: TokenKind::Text,
        start_byte: start,
        end_byte: end,
        start_utf16: offsets.to_utf16(start),
        end_utf16: offsets.to_utf16(end),
        raw: template[start..end].to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(s: &str) -> (Vec<Token>, Vec<LexError>) {
        let off = OffsetMap::build(s);
        lex_template(s, &off)
    }

    #[test]
    fn plain_text_emits_single_text_token() {
        let (toks, errs) = lex("hello world");
        assert!(errs.is_empty());
        assert_eq!(toks.len(), 1);
    }

    #[test]
    fn single_variable_block() {
        let (toks, errs) = lex("{{file:target}}");
        assert!(errs.is_empty());
        assert_eq!(toks.len(), 1);
        assert_eq!(toks[0].raw, "{{file:target}}");
    }

    #[test]
    fn text_then_block_then_text() {
        let (toks, errs) = lex("hi {{text:name}} bye");
        assert!(errs.is_empty());
        assert_eq!(toks.len(), 3);
    }

    #[test]
    fn unclosed_block_reports_error_at_open() {
        let (_t, errs) = lex("prefix {{file:foo");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].start_byte, 7);
    }

    #[test]
    fn utf16_offsets_track_emoji() {
        let (toks, _) = lex("🚀 {{file:a}} done");
        let block = toks
            .iter()
            .find(|t| t.kind == TokenKind::VariableBlockRaw)
            .unwrap();
        assert_eq!(block.start_utf16, 3);
        assert_eq!(block.start_byte, 5);
    }
}
