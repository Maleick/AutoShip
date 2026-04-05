# Cerebrum

> OpenWolf's learning memory. Updated automatically as the AI learns from interactions.
> Do not edit manually unless correcting an error.
> Last updated: 2026-04-05

## User Preferences

- **Scaling beyond 36 clients** — user wants to run MORE than 36 clients. Never assume 36 is the max.
- **Discord-driven discussion** — user prefers to discuss architecture and decisions via Discord before implementation.
- **Multiple choice follow-ups** — when asking follow-up questions, present as multiple choice (A/B/C) with a shareable summary for Matt.
- **Issues before implementation** — user wants GitHub issues created and roadmap updated before writing code ("write it all up in issues").

## Key Learnings

- **Project:** DMFT
- **Description:** [![CI](https://github.com/Maleick/DMFT/actions/workflows/ci.yml/badge.svg)](https://github.com/Maleick/DMFT/actions/workflows/ci.yml)
- **Syscall evasion layering:** RecycledGate (indirect syscalls through existing ntdll stubs) is preferred over direct syscalls for DMFT because the `syscall` instruction executes from within ntdll address range, passing return-address checks. Full .text unhooking is the noisiest option due to VirtualProtect + momentary RWX. Fresh NTDLL mapping from KnownDlls is useful for bootstrapping SSN extraction but must be unmapped immediately to avoid VAD detection.
- **Clean syscall stub pattern:** `4C 8B D1 B8 XX XX 00 00` (mov r10,rcx; mov eax,SSN). Hooked stubs start with `E9` (JMP). Checking first 4 bytes is the standard hook detection method.
- **Bootstrap problem:** When unhooking ntdll, VirtualProtect/NtProtectVirtualMemory may itself be hooked. Solution: resolve NtProtectVirtualMemory from a fresh ntdll mapping and call it directly, or use RecycledGate indirect syscall.
- **DMFT M7 recommendation:** Use RecycledGate for targeted calls (NtSetContextThread, NtGetContextThread, NtProtectVirtualMemory) rather than full .text unhooking. Build syscall table at DLL load from KnownDlls fresh mapping, then unmap immediately.
- **EQ uses Direct3D 11 on live** — verified on Frostreaver (2026-04-05): `d3d11.dll` + `dxgi.dll` loaded, NO `d3d9.dll`. MQ2's `CRender` offset 0x0F00 → `Direct3DDevice9*` is stale. Device creation goes through `D3D11CreateDevice` / DXGI swapchain.
- **Render hook point:** `CDisplay::RealRender_World` at offset `0x1401A4320`. Hooking here skips the 3D scene but leaves game logic + network running.
- **#355 (Launchpad token RE) is dead** — `/patchme` bypasses the launchpad entirely, login FSM handles the rest. Closed as won't-fix.
- **RunEQ (xackery)** — Go headless EQ client, private repo, targets EQEmu RoF protocol. Binary at `.claude/channels/discord/inbox/`. Useful for protocol reference but NOT viable for live TLP servers due to auth/CRC/truebox.
- **EQ headless approaches ranked:** Null renderer (LOW risk) > Login proxy (LOW-MED) > Socket steal (HIGH) > Full headless client (VERY HIGH). For live TLP, null renderer is the only practical option.
- **EQEmu has existing headless clients:** `Server/hc/` (C++), EQNet by Zaela (C), AkkStack Headless (Docker). EQEmu protocol is well-documented and stable.
- **IPC command dispatch in DLL:** Commands flow through `listener_loop` → `handle_immediate_command` (for login/calibrate) or `PENDING_COMMANDS` queue → `dispatch_command` in game loop. Simple atomic-setting commands (SetAutoAccept, SetRenderMode) go through `dispatch_command`.

## Do-Not-Repeat

<!-- Mistakes made and corrected. Each entry prevents the same mistake recurring. -->
<!-- Format: [YYYY-MM-DD] Description of what went wrong and what to do instead. -->

## Decision Log

<!-- Significant technical decisions with rationale. Why X was chosen over Y. -->
