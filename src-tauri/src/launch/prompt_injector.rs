//! Bracketed-paste prompt injection per spec §7.
//!
//! After the PTY's *first* output chunk fires (event-driven, not a
//! fixed sleep — see `launch::pty_session`), the injector writes the
//! user's resolved prompt into the PTY's stdin wrapped in the xterm
//! bracketed-paste sequence:
//!
//! ```text
//! ESC [ 200 ~
//! <resolved prompt>
//! ESC [ 201 ~
//! \r
//! ```
//!
//! The trailing `\r` submits the prompt (claude treats it as <Enter>).
//! Bracketed-paste mode is enabled by xterm-256color by default and
//! tells claude this is pasted (multi-line) content rather than typed
//! input — keeps embedded newlines from triggering line submissions
//! one-by-one.

const BRACKETED_PASTE_START: &[u8] = b"\x1b[200~";
const BRACKETED_PASTE_END: &[u8] = b"\x1b[201~";

/// Build the byte sequence the PTY stdin should receive to inject
/// `resolved_prompt` as bracketed paste followed by Enter.
pub fn build_inject_bytes(resolved_prompt: &str) -> Vec<u8> {
    let body = resolved_prompt.as_bytes();
    let mut out = Vec::with_capacity(BRACKETED_PASTE_START.len() + body.len()
        + BRACKETED_PASTE_END.len() + 1);
    out.extend_from_slice(BRACKETED_PASTE_START);
    out.extend_from_slice(body);
    out.extend_from_slice(BRACKETED_PASTE_END);
    out.push(b'\r');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_start_and_end_with_paste_sequences() {
        let bytes = build_inject_bytes("hello");
        assert!(bytes.starts_with(b"\x1b[200~"));
        assert!(bytes.ends_with(b"\x1b[201~\r"));
    }

    #[test]
    fn body_appears_between_markers() {
        let bytes = build_inject_bytes("explain this code");
        let s = String::from_utf8_lossy(&bytes);
        assert!(
            s.contains("explain this code"),
            "body should appear verbatim: {s:?}"
        );
    }

    #[test]
    fn multiline_prompt_passes_through_verbatim() {
        let prompt = "line one\nline two\nline three";
        let bytes = build_inject_bytes(prompt);
        let inner: Vec<u8> = bytes
            .iter()
            .skip(BRACKETED_PASTE_START.len())
            .take(prompt.len())
            .copied()
            .collect();
        assert_eq!(inner, prompt.as_bytes());
    }

    #[test]
    fn empty_prompt_still_produces_valid_envelope() {
        let bytes = build_inject_bytes("");
        assert_eq!(
            bytes,
            [BRACKETED_PASTE_START, BRACKETED_PASTE_END, b"\r"].concat()
        );
    }
}
