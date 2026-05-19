//! Shared SQL utilities used by multiple `index` and `commands`
//! modules. Today this module hosts the LIKE-pattern escape helper;
//! more shared SQL primitives (e.g. `IN` placeholder builders,
//! parameter binders) can land here as they're needed across more
//! than one caller.

/// Escape a user-supplied substring for use in a `LIKE` pattern,
/// pairing with the `ESCAPE '\\'` clause at the SQL site.
///
/// SQLite `LIKE` treats `%` and `_` as wildcards. The escape character
/// itself (here, `\`) must also be doubled. This function escapes all
/// three so a caller building `format!("%{}%", escape_like(query))`
/// matches the literal user input rather than treating their `%` /
/// `_` / `\` characters as pattern metachars.
///
/// SCA-754: this lived inline in `commands/search.rs` until the
/// review surfaced that the next `LIKE`-using caller would be tempted
/// to copy-paste it. Moved to a shared module so future callers reach
/// for the canonical primitive.
pub fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    for ch in input.chars() {
        if matches!(ch, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_percent_underscore_and_backslash() {
        assert_eq!(escape_like("100%"), "100\\%");
        assert_eq!(escape_like("a_b"), "a\\_b");
        assert_eq!(escape_like("a\\b"), "a\\\\b");
    }

    #[test]
    fn passes_through_safe_chars() {
        assert_eq!(escape_like("hello world"), "hello world");
        assert_eq!(escape_like(""), "");
    }
}
