//! Byte-pattern signature scanner for resolving EQ function addresses.
//!
//! Scans memory regions for byte patterns with wildcard support, enabling
//! offset resolution that survives EQ client patches. Supports IDA-style
//! patterns (`"48 8B 05 ?? ?? ?? ??"`) and code-style patterns with masks.

/// A byte pattern with optional wildcard positions for memory scanning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    /// Pattern bytes (wildcard positions hold 0x00 but are ignored during
    /// matching).
    bytes: Vec<u8>,
    /// Mask: `true` = must match, `false` = wildcard (skip).
    mask: Vec<bool>,
}

impl Pattern {
    /// Create a pattern from raw bytes and mask vectors.
    ///
    /// # Panics
    ///
    /// Panics if `bytes` and `mask` have different lengths or are empty.
    #[must_use]
    pub fn new(bytes: Vec<u8>, mask: Vec<bool>) -> Self {
        assert!(
            bytes.len() == mask.len(),
            "bytes and mask must have the same length"
        );
        assert!(!bytes.is_empty(), "pattern must not be empty");
        Self { bytes, mask }
    }

    /// Parse an IDA-style pattern string.
    ///
    /// Format: space-separated hex bytes, `??` or `?` for wildcards.
    ///
    /// # Examples
    ///
    /// ```
    /// use textquest_common::scanner::Pattern;
    /// let p = Pattern::from_ida("48 8B 05 ?? ?? ?? ?? 48 85 C0 74");
    /// assert_eq!(p.len(), 11);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the pattern string is empty or contains invalid hex tokens.
    #[must_use]
    pub fn from_ida(s: &str) -> Self {
        let tokens: Vec<&str> = s.split_whitespace().collect();
        assert!(!tokens.is_empty(), "IDA pattern must not be empty");

        let mut bytes = Vec::with_capacity(tokens.len());
        let mut mask = Vec::with_capacity(tokens.len());

        for token in tokens {
            if token == "??" || token == "?" {
                bytes.push(0x00);
                mask.push(false);
            } else {
                let byte = u8::from_str_radix(token, 16)
                    .unwrap_or_else(|_| panic!("invalid hex: {token}"));
                bytes.push(byte);
                mask.push(true);
            }
        }

        Self { bytes, mask }
    }

    /// Parse a code-style pattern with a separate mask string.
    ///
    /// `code` is raw bytes (e.g.
    /// `b"\x48\x8B\x05\x00\x00\x00\x00\x48\x85\xC0\x74"`), `mask_str` uses
    /// `x` for match and `?` for wildcard (e.g. `"xxx????xxxx"`).
    ///
    /// # Panics
    ///
    /// Panics if `code` and `mask_str` have different lengths or are empty.
    #[must_use]
    pub fn from_code(code: &[u8], mask_str: &str) -> Self {
        assert!(
            code.len() == mask_str.len(),
            "code and mask must have the same length"
        );
        assert!(!code.is_empty(), "pattern must not be empty");

        let mask: Vec<bool> = mask_str.bytes().map(|b| b == b'x').collect();
        Self {
            bytes: code.to_vec(),
            mask,
        }
    }

    /// Number of bytes in this pattern.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether this pattern is empty (always false after construction).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// Scan a byte slice for the first occurrence of a pattern.
///
/// Returns the offset from the start of `data` where the pattern matches,
/// or `None` if no match is found.
#[must_use]
pub fn scan_region(data: &[u8], pattern: &Pattern) -> Option<usize> {
    let pat_len = pattern.len();
    if pat_len == 0 || data.len() < pat_len {
        return None;
    }

    let end = data.len() - pat_len + 1;

    // Find first non-wildcard byte index for a fast lead-byte check
    let lead_info: Option<(usize, u8)> = pattern
        .mask
        .iter()
        .enumerate()
        .find(|(_, m)| **m)
        .map(|(idx, _)| (idx, pattern.bytes[idx]));

    for i in 0..end {
        // Fast reject on lead byte
        #[allow(clippy::collapsible_if)]
        if let Some((lead_idx, lead_byte)) = lead_info {
            if data[i + lead_idx] != lead_byte {
                continue;
            }
        }

        if matches_at(data, i, pattern) {
            return Some(i);
        }
    }

    None
}

/// Scan a byte slice for all occurrences of a pattern.
///
/// Returns a vector of offsets from the start of `data`.
#[must_use]
pub fn scan_all(data: &[u8], pattern: &Pattern) -> Vec<usize> {
    let pat_len = pattern.len();
    if pat_len == 0 || data.len() < pat_len {
        return Vec::new();
    }

    let end = data.len() - pat_len + 1;
    let mut results = Vec::new();

    // Fast lead-byte check (same optimization as scan_region)
    let lead_info: Option<(usize, u8)> = pattern
        .mask
        .iter()
        .enumerate()
        .find(|(_, m)| **m)
        .map(|(idx, _)| (idx, pattern.bytes[idx]));

    for i in 0..end {
        #[allow(clippy::collapsible_if)]
        if let Some((lead_idx, lead_byte)) = lead_info {
            if data[i + lead_idx] != lead_byte {
                continue;
            }
        }
        if matches_at(data, i, pattern) {
            results.push(i);
        }
    }

    results
}

/// Check if the pattern matches at a specific offset in the data.
#[inline]
fn matches_at(data: &[u8], offset: usize, pattern: &Pattern) -> bool {
    for (j, (pat_byte, must_match)) in pattern.bytes.iter().zip(&pattern.mask).enumerate() {
        if *must_match && data[offset + j] != *pat_byte {
            return false;
        }
    }
    true
}

// ─── Platform-specific memory scanning ───

/// Scan a live process memory region by pointer and size.
///
/// # Safety
///
/// The caller must ensure `base` points to readable memory of at least `size`
/// bytes.
#[cfg(windows)]
pub unsafe fn scan_process_memory(
    base: *const u8,
    size: usize,
    pattern: &Pattern,
) -> Option<usize> {
    let data = unsafe { std::slice::from_raw_parts(base, size) };
    scan_region(data, pattern)
}

/// Scan a live process memory region (macOS stub — always returns `None`).
///
/// # Safety
///
/// Stub implementation; pointer is never dereferenced.
#[cfg(not(windows))]
pub unsafe fn scan_process_memory(
    _base: *const u8,
    _size: usize,
    _pattern: &Pattern,
) -> Option<usize> {
    None
}

/// Scan live process memory for all matches.
///
/// # Safety
///
/// The caller must ensure `base` points to readable memory of at least `size`
/// bytes.
#[cfg(windows)]
pub unsafe fn scan_process_memory_all(
    base: *const u8,
    size: usize,
    pattern: &Pattern,
) -> Vec<usize> {
    let data = unsafe { std::slice::from_raw_parts(base, size) };
    scan_all(data, pattern)
}

/// Scan live process memory for all matches (macOS stub — always returns
/// empty).
///
/// # Safety
///
/// Stub implementation; pointer is never dereferenced.
#[cfg(not(windows))]
pub unsafe fn scan_process_memory_all(
    _base: *const u8,
    _size: usize,
    _pattern: &Pattern,
) -> Vec<usize> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Pattern parsing ───

    #[test]
    fn ida_pattern_basic() {
        let p = Pattern::from_ida("48 8B 05 ?? ?? ?? ?? 48 85 C0 74");
        assert_eq!(p.len(), 11);
        assert_eq!(p.bytes[0], 0x48);
        assert_eq!(p.bytes[2], 0x05);
        assert!(p.mask[0]); // 48 = must match
        assert!(!p.mask[3]); // ?? = wildcard
        assert!(!p.mask[6]); // ?? = wildcard
        assert!(p.mask[7]); // 48 = must match
    }

    #[test]
    fn ida_pattern_single_question_mark() {
        let p = Pattern::from_ida("FF ? 90");
        assert_eq!(p.len(), 3);
        assert!(p.mask[0]);
        assert!(!p.mask[1]);
        assert!(p.mask[2]);
    }

    #[test]
    fn ida_pattern_all_wildcards() {
        let p = Pattern::from_ida("?? ?? ??");
        assert_eq!(p.len(), 3);
        assert!(p.mask.iter().all(|&m| !m));
    }

    #[test]
    fn ida_pattern_no_wildcards() {
        let p = Pattern::from_ida("48 89 5C 24 08");
        assert_eq!(p.len(), 5);
        assert!(p.mask.iter().all(|&m| m));
    }

    #[test]
    fn ida_pattern_lowercase() {
        let p = Pattern::from_ida("4c 8b dc");
        assert_eq!(p.bytes, vec![0x4C, 0x8B, 0xDC]);
    }

    #[test]
    #[should_panic(expected = "invalid hex")]
    fn ida_pattern_invalid_hex() {
        let _ = Pattern::from_ida("48 ZZ 05");
    }

    #[test]
    #[should_panic(expected = "IDA pattern must not be empty")]
    fn ida_pattern_empty() {
        let _ = Pattern::from_ida("");
    }

    #[test]
    fn code_pattern_basic() {
        let code = b"\x48\x8B\x05\x00\x00\x00\x00\x48\x85\xC0\x74";
        let p = Pattern::from_code(code, "xxx????xxxx");
        assert_eq!(p.len(), 11);
        assert!(p.mask[0]);
        assert!(!p.mask[3]);
        assert!(p.mask[7]);
    }

    #[test]
    #[should_panic(expected = "code and mask must have the same length")]
    fn code_pattern_length_mismatch() {
        let _ = Pattern::from_code(b"\x48\x8B", "x");
    }

    // ─── Scanning ───

    #[test]
    fn scan_finds_pattern_in_middle() {
        let data = [0x00, 0x00, 0x48, 0x8B, 0x05, 0xFF, 0x00, 0x00];
        let p = Pattern::from_ida("48 8B 05");
        assert_eq!(scan_region(&data, &p), Some(2));
    }

    #[test]
    fn scan_finds_pattern_at_start() {
        let data = [0x48, 0x8B, 0x05, 0xFF, 0xFF];
        let p = Pattern::from_ida("48 8B 05");
        assert_eq!(scan_region(&data, &p), Some(0));
    }

    #[test]
    fn scan_finds_pattern_at_end() {
        let data = [0xFF, 0xFF, 0x48, 0x8B, 0x05];
        let p = Pattern::from_ida("48 8B 05");
        assert_eq!(scan_region(&data, &p), Some(2));
    }

    #[test]
    fn scan_no_match() {
        let data = [0x00, 0x00, 0x00, 0x00];
        let p = Pattern::from_ida("48 8B 05");
        assert_eq!(scan_region(&data, &p), None);
    }

    #[test]
    fn scan_wildcards() {
        let data = [0x48, 0x8B, 0x05, 0xAA, 0xBB, 0xCC, 0xDD, 0x48, 0x85];
        let p = Pattern::from_ida("48 8B 05 ?? ?? ?? ?? 48 85");
        assert_eq!(scan_region(&data, &p), Some(0));
    }

    #[test]
    fn scan_wildcard_matches_any_byte() {
        let data = [0x48, 0x8B, 0x05, 0x00, 0x00, 0x00, 0x00, 0x48, 0x85];
        let p = Pattern::from_ida("48 8B 05 ?? ?? ?? ?? 48 85");
        assert_eq!(scan_region(&data, &p), Some(0));

        let data2 = [0x48, 0x8B, 0x05, 0xFF, 0xFF, 0xFF, 0xFF, 0x48, 0x85];
        assert_eq!(scan_region(&data2, &p), Some(0));
    }

    #[test]
    fn scan_data_shorter_than_pattern() {
        let data = [0x48, 0x8B];
        let p = Pattern::from_ida("48 8B 05");
        assert_eq!(scan_region(&data, &p), None);
    }

    #[test]
    fn scan_exact_length_match() {
        let data = [0x48, 0x8B, 0x05];
        let p = Pattern::from_ida("48 8B 05");
        assert_eq!(scan_region(&data, &p), Some(0));
    }

    #[test]
    fn scan_exact_length_no_match() {
        let data = [0x48, 0x8B, 0x06];
        let p = Pattern::from_ida("48 8B 05");
        assert_eq!(scan_region(&data, &p), None);
    }

    #[test]
    fn scan_empty_data() {
        let data: [u8; 0] = [];
        let p = Pattern::from_ida("48");
        assert_eq!(scan_region(&data, &p), None);
    }

    // ─── scan_all ───

    #[test]
    fn scan_all_multiple_matches() {
        let data = [0x90, 0xFF, 0x90, 0xFF, 0x90, 0x00];
        let p = Pattern::from_ida("90 FF");
        let results = scan_all(&data, &p);
        assert_eq!(results, vec![0, 2]);
    }

    #[test]
    fn scan_all_no_matches() {
        let data = [0x00, 0x00, 0x00];
        let p = Pattern::from_ida("FF");
        assert!(scan_all(&data, &p).is_empty());
    }

    #[test]
    fn scan_all_overlapping() {
        // Pattern: AA AA — appears overlapping at 0 and 1 in [AA, AA, AA]
        let data = [0xAA, 0xAA, 0xAA];
        let p = Pattern::from_ida("AA AA");
        let results = scan_all(&data, &p);
        assert_eq!(results, vec![0, 1]);
    }

    #[test]
    fn scan_all_single_match() {
        let data = [0x00, 0x48, 0x8B, 0x00];
        let p = Pattern::from_ida("48 8B");
        let results = scan_all(&data, &p);
        assert_eq!(results, vec![1]);
    }

    // ─── Realistic EQ-like patterns ───

    #[test]
    fn scan_realistic_eq_pattern() {
        // Simulate a function prologue pattern with a relative offset wildcard
        let mut data = vec![0xCC; 256];
        // Place a "function" at offset 0x40
        data[0x40] = 0x48;
        data[0x41] = 0x89;
        data[0x42] = 0x5C;
        data[0x43] = 0x24;
        data[0x44] = 0x08;

        let p = Pattern::from_ida("48 89 5C 24 08");
        assert_eq!(scan_region(&data, &p), Some(0x40));
    }

    #[test]
    fn scan_with_code_style_pattern() {
        let data = [0x48, 0x8B, 0x05, 0x12, 0x34, 0x56, 0x78, 0x48, 0x85, 0xC0];
        let code = b"\x48\x8B\x05\x00\x00\x00\x00\x48\x85\xC0";
        let p = Pattern::from_code(code, "xxx????xxx");
        assert_eq!(scan_region(&data, &p), Some(0));
    }

    #[test]
    fn scan_large_region() {
        // 1MB region with pattern near the end
        let mut data = vec![0x00u8; 1024 * 1024];
        let offset = 1024 * 1024 - 10;
        data[offset] = 0x48;
        data[offset + 1] = 0x8B;
        data[offset + 2] = 0x05;

        let p = Pattern::from_ida("48 8B 05");
        assert_eq!(scan_region(&data, &p), Some(offset));
    }

    // ─── Pattern construction ───

    #[test]
    fn pattern_new_direct() {
        let p = Pattern::new(vec![0x90, 0x00, 0xCC], vec![true, false, true]);
        assert_eq!(p.len(), 3);
        assert!(!p.is_empty());
    }

    #[test]
    #[should_panic(expected = "bytes and mask must have the same length")]
    fn pattern_new_mismatch() {
        let _ = Pattern::new(vec![0x90], vec![true, false]);
    }

    #[test]
    #[should_panic(expected = "pattern must not be empty")]
    fn pattern_new_empty() {
        let _ = Pattern::new(vec![], vec![]);
    }
}
