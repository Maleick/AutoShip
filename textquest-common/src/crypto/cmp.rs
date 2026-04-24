//! Constant-time comparison helpers for secret material.
//!
//! Use these helpers for tokens, MACs, password-derived values, and other
//! secret byte strings instead of direct `==` comparisons.

use subtle::{Choice, ConstantTimeEq};

/// Compare two byte slices without leaking content through timing.
///
/// This is the canonical TextQuest primitive for comparing secret byte strings.
/// Do not compare tokens, MACs, password-derived bytes, or other secret slices
/// with direct `==` outside this module.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut equal = Choice::from(1);

    for i in 0..a.len().max(b.len()) {
        let a_byte = a.get(i).copied().unwrap_or(0);
        let b_byte = b.get(i).copied().unwrap_or(0);
        equal &= a_byte.ct_eq(&b_byte);
    }

    (equal & a.len().ct_eq(&b.len())).into()
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn equal_slices_match() {
        assert!(constant_time_eq(b"secret-token", b"secret-token"));
    }

    #[test]
    fn unequal_length_slices_do_not_match() {
        assert!(!constant_time_eq(b"secret-token", b"secret"));
    }

    #[test]
    fn unequal_content_slices_do_not_match() {
        assert!(!constant_time_eq(b"secret-token", b"secret-taken"));
    }
}
