//! DJB2 hash function for NT API name resolution.
//!
//! All function lookups use hashes instead of plaintext strings to avoid
//! static analysis identifying which NT APIs we resolve.

/// DJB2 hash of a byte slice (Daniel J. Bernstein's classic string hash).
///
/// Used to hash NT API function names so no plaintext strings appear in the
/// binary. The hash is computed at compile time for known targets and at
/// runtime when walking ntdll exports.
pub const fn djb2(input: &[u8]) -> u32 {
    let mut hash: u32 = 5381;
    let mut i = 0;
    while i < input.len() {
        hash = hash.wrapping_mul(33).wrapping_add(input[i] as u32);
        i += 1;
    }
    hash
}

// Pre-computed hashes for target NT APIs.
// These are the only values stored in the binary — no plaintext function names.
pub const NT_PROTECT_VIRTUAL_MEMORY: u32 = djb2(b"NtProtectVirtualMemory");
pub const NT_SET_CONTEXT_THREAD: u32 = djb2(b"NtSetContextThread");
pub const NT_GET_CONTEXT_THREAD: u32 = djb2(b"NtGetContextThread");
pub const NT_ALLOCATE_VIRTUAL_MEMORY: u32 = djb2(b"NtAllocateVirtualMemory");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn djb2_known_values() {
        assert_eq!(djb2(b"NtProtectVirtualMemory"), NT_PROTECT_VIRTUAL_MEMORY);
        assert_eq!(djb2(b"NtSetContextThread"), NT_SET_CONTEXT_THREAD);
        assert_eq!(djb2(b"NtGetContextThread"), NT_GET_CONTEXT_THREAD);
        assert_eq!(djb2(b"NtAllocateVirtualMemory"), NT_ALLOCATE_VIRTUAL_MEMORY);
    }

    #[test]
    fn djb2_empty_string() {
        assert_eq!(djb2(b""), 5381);
    }

    #[test]
    fn djb2_deterministic() {
        assert_eq!(djb2(b"A"), djb2(b"A"));
        assert_ne!(djb2(b"A"), djb2(b"B"));
    }

    #[test]
    fn djb2_no_collisions_among_targets() {
        let hashes = [
            NT_PROTECT_VIRTUAL_MEMORY,
            NT_SET_CONTEXT_THREAD,
            NT_GET_CONTEXT_THREAD,
            NT_ALLOCATE_VIRTUAL_MEMORY,
        ];
        for i in 0..hashes.len() {
            for j in (i + 1)..hashes.len() {
                assert_ne!(hashes[i], hashes[j], "DJB2 collision between target APIs");
            }
        }
    }

    #[test]
    fn djb2_case_sensitive() {
        assert_ne!(
            djb2(b"NtProtectVirtualMemory"),
            djb2(b"ntprotectvirtualmemory")
        );
    }
}
