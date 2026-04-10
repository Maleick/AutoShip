# Research: Active Hacks & Opcode Discovery

## Overview
This document tracks research, opcodes, and implementations for "active hacks"—capabilities that bypass normal game mechanics by injecting constructed network packets or manipulating memory beyond intended design. Because these actions carry a high risk of detection, suspension, or banning, they must be strictly gated behind opt-in safety configurations in the TextQuest UI.

## Known Hacks

### 1. Living Shield (Cross-Class Invulnerability)
*   **Concept:** The "Shield" ability is intended for Warriors. However, sending the specific opcode allows *any* class to cast it.
*   **Duration:** 12 seconds.
*   **Cooldown:** 3 minutes.
*   **Strategy:** By coordinating 16+ characters (Bards, Clerics, Paladins, Shadow Knights, Necromancers) with invulnerability (e.g., Divine Aura), we can permanently shield the Main Tank.

**Historical Offsets (January 2025 - EQGame):**
*   `OPCODE_LIVING_SHIELD`: `0x3A44`
*   `OFFSET_PACKET_SCRAMBLER`: `0xdc6340`
*   `OFFSET_HTON`: `0x682e50`
*   `OFFSET_NETWORK_SEND`: `0x557090`

**Implementation Requirements:**
1.  Locate updated `NetworkSend`, `PacketScrambler`, and `hton` offsets in the current Ghidra export.
2.  Implement a raw packet injector in `textquest-dll`.
3.  Add an opt-in toggle in the TextQuest Web UI and `textquest.toml` config (e.g., `enable_unsafe_hacks = true`).
4.  Implement the `/livingshield` command to trigger the packet send.

### 2. Warp
*   **Concept:** Manipulating coordinates or sending specific movement packets to instantly translate a character across the zone.
*   **Status:** Pending deep research into `NetworkSend` coordinate packets or local memory manipulation. Requires the same opt-in safety gate as Living Shield.

## Opcode Discovery (Autoresearch Target)
We are actively researching the local `data/ghidra-export/` cache to identify:
1.  Updated `NetworkSend` dispatch functions.
2.  The `hton` (Host TO Network) packet scrambler logic.
3.  Undocumented or exploitable opcodes that can be fed to Kara for further capability expansion.

That cache is local runtime/debug tooling state only. Canonical immutable exports and manifests live in the sibling `Maleick/TextQuest-Ghidra` repo under `snapshots/`.

*Note: The test server patch drops tomorrow; this research will be repeated to establish the delta for the following week.*
