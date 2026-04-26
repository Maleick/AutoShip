//! Network packet handler dispatch — server-initiated integrity/memcheck opcodes.
//!
//! This module contains the dispatch table for inbound opcodes that the EQ server
//! sends to probe client memory. Handlers return pre-spoofed bytes (sourced from
//! the PRNG-aware clean-hash cache in `hooks::memcheck`) instead of live `.text`
//! reads, satisfying the server without exposing hook trampolines.
//!
//! # Opcode coverage
//!
//! | Opcode   | Handler                             | Parent issue |
//! |----------|-------------------------------------|--------------|
//! | `0x4f27` | `handle_server_memcheck`            | #3397 (A1)   |
//!
//! # Integration
//!
//! Handlers are called from the WSASend/WSARecv hook dispatch path
//! (`hooks::packet_hook`). They receive the raw inbound payload slice and
//! return a spoofed response byte vector that is forwarded to the caller.
//!
//! # Evidence basis
//!
//! `docs/wiki/Research-Anti-Detection.md` §Server-initiated memcheck.
//! Parent issue #2173. Ghidra analysis of `FUN_1400B5720`.

pub mod handlers;
