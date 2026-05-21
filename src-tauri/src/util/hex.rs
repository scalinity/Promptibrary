//! SCA-928 (B16, DRY): single hex-encoding helper.
//!
//! Replaces four near-identical implementations previously scattered
//! across `util::hash`, `vault::writer::hex_lower`,
//! `variables::renderer::hex::encode_lower`, and the inline byte
//! iterator in `variables::parser::compute_ref_id`. All four used
//! `format!("{:02x}", b)`; only the surrounding code differed.

/// Lowercase hex-encode `bytes`. Allocates exactly `bytes.len() * 2`
/// chars.
pub fn encode_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_yields_empty() {
        assert_eq!(encode_lower(&[]), "");
    }

    #[test]
    fn boundary_values() {
        assert_eq!(encode_lower(&[0x00, 0x0f, 0x10, 0xff]), "000f10ff");
    }

    #[test]
    fn known_sha256_excerpt() {
        // Roundtrip against the well-known sha256("abc") prefix.
        let prefix = [0xba, 0x78, 0x16, 0xbf];
        assert_eq!(encode_lower(&prefix), "ba7816bf");
    }
}
