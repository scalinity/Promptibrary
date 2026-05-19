//! Slug derivation from prompt titles.
//!
//! Spec §3 contract for `util::slug::slugify`:
//! - kebab-case
//! - lowercase
//! - ASCII fallback for Unicode (drop emoji, normalize common accents)
//! - collapsed runs of `-`
//! - non-empty (falls back to "untitled")
//!
//! Collision suffixing (`-2`, `-3`, …) lives in `commands::prompts::create_prompt`
//! at create-time only — once a slug is set, renaming the prompt does not
//! change the slug or filename per spec §5.

pub fn slugify(title: &str) -> String {
    let lowered = title.to_lowercase();

    let mut out = String::with_capacity(lowered.len());
    let mut prev_dash = false;
    for ch in lowered.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch)
        } else {
            ascii_fallback(ch)
        };

        match mapped {
            Some(c) if c.is_ascii_alphanumeric() => {
                out.push(c);
                prev_dash = false;
            }
            _ => {
                if !prev_dash && !out.is_empty() {
                    out.push('-');
                    prev_dash = true;
                }
            }
        }
    }

    while out.ends_with('-') {
        out.pop();
    }

    if out.len() > 80 {
        out.truncate(80);
        while out.ends_with('-') {
            out.pop();
        }
    }

    if out.is_empty() {
        "untitled".into()
    } else {
        out
    }
}

fn ascii_fallback(c: char) -> Option<char> {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => Some('a'),
        'è' | 'é' | 'ê' | 'ë' | 'ē' => Some('e'),
        'ì' | 'í' | 'î' | 'ï' | 'ī' => Some('i'),
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' => Some('o'),
        'ù' | 'ú' | 'û' | 'ü' | 'ū' => Some('u'),
        'ç' | 'č' | 'ć' => Some('c'),
        'ñ' | 'ń' => Some('n'),
        'ß' => Some('s'),
        'ž' | 'ź' | 'ż' => Some('z'),
        'ł' => Some('l'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_title_becomes_kebab() {
        assert_eq!(
            slugify("Refactor module with test-first invariants"),
            "refactor-module-with-test-first-invariants"
        );
    }

    #[test]
    fn collapses_repeated_separators() {
        assert_eq!(slugify("Hello   ---   World!!!"), "hello-world");
    }

    #[test]
    fn strips_emoji_and_collapses() {
        assert_eq!(slugify("🚀 Ship 🚀 it"), "ship-it");
    }

    #[test]
    fn handles_accented_letters() {
        assert_eq!(slugify("Café Naïve résumé"), "cafe-naive-resume");
    }

    #[test]
    fn empty_or_only_symbols_falls_back() {
        assert_eq!(slugify(""), "untitled");
        assert_eq!(slugify("???"), "untitled");
        assert_eq!(slugify("🎉"), "untitled");
    }

    #[test]
    fn truncates_long_titles_to_80_chars() {
        let long = "a".repeat(200);
        let s = slugify(&long);
        assert!(s.len() <= 80, "got {} chars: {s}", s.len());
    }

    #[test]
    fn preserves_digits() {
        assert_eq!(slugify("v2 release plan 2026"), "v2-release-plan-2026");
    }
}
