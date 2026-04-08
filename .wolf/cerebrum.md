# Cerebrum

> OpenWolf's learning memory. Updated automatically as the AI learns from interactions.
> Do not edit manually unless correcting an error.
> Last updated: 2026-04-03

## User Preferences

<!-- How the user likes things done. Code style, tools, patterns, communication. -->

## Key Learnings

- **Project:** DMFT
- **Description:** [![CI](https://github.com/Maleick/DMFT/actions/workflows/ci.yml/badge.svg)](https://github.com/Maleick/DMFT/actions/workflows/ci.yml)
- **Syscall evasion layering:** RecycledGate (indirect syscalls through existing ntdll stubs) is preferred over direct syscalls for DMFT because the `syscall` instruction executes from within ntdll address range, passing return-address checks. Full .text unhooking is the noisiest option due to VirtualProtect + momentary RWX. Fresh NTDLL mapping from KnownDlls is useful for bootstrapping SSN extraction but must be unmapped immediately to avoid VAD detection.
- **Clean syscall stub pattern:** `4C 8B D1 B8 XX XX 00 00` (mov r10,rcx; mov eax,SSN). Hooked stubs start with `E9` (JMP). Checking first 4 bytes is the standard hook detection method.
- **Bootstrap problem:** When unhooking ntdll, VirtualProtect/NtProtectVirtualMemory may itself be hooked. Solution: resolve NtProtectVirtualMemory from a fresh ntdll mapping and call it directly, or use RecycledGate indirect syscall.
- **DMFT M7 recommendation:** Use RecycledGate for targeted calls (NtSetContextThread, NtGetContextThread, NtProtectVirtualMemory) rather than full .text unhooking. Build syscall table at DLL load from KnownDlls fresh mapping, then unmap immediately.

## Do-Not-Repeat

<!-- Mistakes made and corrected. Each entry prevents the same mistake recurring. -->
<!-- Format: [YYYY-MM-DD] Description of what went wrong and what to do instead. -->

## Decision Log

<!-- Significant technical decisions with rationale. Why X was chosen over Y. -->
