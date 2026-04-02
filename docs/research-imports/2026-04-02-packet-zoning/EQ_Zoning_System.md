# EverQuest Zoning System Analysis

## Overview

The EverQuest client uses a multi-stage state machine to handle zone transitions. This document covers the key functions, return codes, zone reasons, and packet flow involved in the zoning process.

---

## Key Functions

| Address | Name | Purpose |
|---------|------|---------|
| `0x140284e30` | `CEverQuest__ZoneLoadingStateMachine` | Master zone controller, manages entire zone load process |
| `0x1402816e0` | `ExecuteZoneTransition` | Handles the actual transition after server approval |
| `0x140294980` | `CEverQuest__SetGameState` | Enables/disables opcode handlers based on game state |
| `0x14029b130` | `CEverQuest__ValidateZoneRequest` | Validates if zone transition is allowed |
| `0x14027b100` | `Zone_Handler` | Anti-cheat fingerprinting during zone |
| `0x14019d180` | `MoveLocalPlayerToSafeCoords` | Positions player after zone completes |
| `0x140302720` | `FlushMovementQueue` | Clears pending movement before zone |
| `0x14021c9a0` | `DisableOpcodeHandler` | Disables a specific opcode handler by ID |
| `0x14021ca10` | `EnableOpcodeHandler` | Enables a specific opcode handler by ID |

---

## Zone Transition Return Codes

Returned by `CEverQuest__ValidateZoneRequest` and checked by `ExecuteZoneTransition`:

| Code | Hex | String ID | Meaning |
|------|-----|-----------|---------|
| **1** | 0x1 | - | Success - zone transition approved |
| **-2** | 0xFFFFFFFE | - | Retry - temporary denial, try alternate path |
| **-3** | 0xFFFFFFFD | 0x13B9 (5049) | Zone locked - expansion/flag required |
| **-4** | 0xFFFFFFFC | 0x13BA (5050) | Zone full - population cap reached |
| **-5** | 0xFFFFFFFB | - | Invalid zone - zone ID doesn't exist |
| **-7** | 0xFFFFFFF9 | 0x13BC (5052) | Access denied - no permission |
| **-8** | 0xFFFFFFF8 | 0xFC (252) | Zone unavailable - zone is down/offline |
| **-9** | 0xFFFFFFF7 | - | Zone string lookup - need to resolve zone name |
| **-10** | 0xFFFFFFF6 | - | Chat message needed - special message display |
| **-12** | 0xFFFFFFF4 | 0x1DB9 | Level too low - restores player to safe coords |

---

## Zone Type (formerly "Zone Reason")

**IMPORTANT:** This field was previously called "Zone Reason" but is actually "Zone Type" - it comes from R12D parameter to `ExecuteZoneTransition`.

The zone type parameter tells the client why/how the zone is happening:

### From Ghidra RE (verified):

| Value | Hex | Source Function | Meaning |
|-------|-----|-----------------|---------|
| 2 | 0x02 | ProcessWorldPacket @ 0x1401f4927 | Server-initiated zone request |
| 3 | 0x03 | ProcessWorldPacket @ 0x1401ef400 | Evacuation (partial) |
| 4 | 0x04 | HandleZoneLineCrossing @ 0x140219020 | Zone line crossing |
| 5 | 0x05 | ProcessWorldPacket @ 0x1401ed10e | Server alternate |
| 8 | 0x08 | HandleSummonTeleport @ 0x140219f40 | Summon/teleport spell |
| 9 | 0x09 | HandleZoneConfirmationPacket @ 0x140216800 | Zone confirmation |
| 10 | 0x0A | ProcessWorldPacket @ 0x1401ee6e6 | Evacuation full |

### From packet captures (additional observed values):

| Value | Hex | Meaning |
|-------|-----|---------|
| 0 | 0x00 | Zone line alternate |
| 1 | 0x01 | Initial request |
| 7 | 0x07 | Portal |
| 11 | 0x0B | Gate/evac |
| -1 | 0xFFFFFFFF | Server approved |

### Zone Type Bit Flags

At `0x140282058`, a bitmask `0x80001C1` is used to test special zone types:
- Bit 0: Normal zone
- Bit 6: Evac
- Bit 7: GM summon
- Bit 8: Unknown
- Bit 31: Instance

---

## Game State Values

Stored at offset `0x5E4` in CEverQuest:

| State | Meaning |
|-------|---------|
| 0x01 | In-game (all handlers enabled) |
| 0x02 | Character select |
| 0x03 | Loading zone data |
| 0x05 | Exiting/cleanup |
| 0xFD | Zone timeout error |
| 0xFF | Idle - not zoning |

---

## Opcode Handler IDs

These internal IDs control which packet types the client processes:

### In-Game Handlers (State 1)
| Handler ID | Hex | Notes |
|------------|-----|-------|
| 546 | 0x222 | Core gameplay |
| 547 | 0x223 | Core gameplay |
| 548 | 0x224 | Core gameplay |
| 549 | 0x225 | Core gameplay |
| 550 | 0x226 | Shared (charselect + ingame) |
| 551 | 0x227 | Shared (charselect + ingame) |
| 361 | 0x169 | Additional ingame |
| 542 | 0x21E | Additional ingame |
| 406 | 0x196 | Additional ingame |
| 4 | 0x04 | Base handler |
| 5 | 0x05 | Base handler |

### Character Select Handlers (State 2)
Only handlers 0x226 and 0x227 plus common set.

### Logout (State 5)
Disables handlers 0x222-0x227.

---

## Zone Packets

### Client to Server
| Opcode | Purpose |
|--------|---------|
| 0x3937 | ZoneRequest (102 bytes) - main zone request packet |
| 0x22B5 | Zone string info |
| 0x2A54 | Zone request/confirmation |
| 0x4BEE | Zone confirm / keepalive |
| 0x4C4 | Zone request ack |

### Server to Client
| Opcode | Purpose |
|--------|---------|
| 0x60F7 | Zone ready notification |
| 0x6A30 | Client capabilities sync |
| 0x5F58 | Graphics/settings sync |
| 0x66F8 | Zone complete |

---

## ZoneRequest Packet (0x3937) Structure

**Total Size:** 102 bytes (2 byte opcode + 100 byte payload)
**Function:** `CEverQuest::SendZoneRequest` @ `0x14027da50`

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0-1 | 2 | Opcode | Scrambled 0x3937 |
| 2-65 | 64 | CharName | Character name (null-terminated) |
| 66-69 | 4 | ZoneID | Target zone ID (uint32) |
| 70-73 | 4 | InstanceID | Instance ID (-1 if none) |
| 74-77 | 4 | ExtInstanceID | Extended instance ID (-1 if none) |
| 78-81 | 4 | Y | Y coordinate (float) - **Y before X** |
| 82-85 | 4 | X | X coordinate (float) |
| 86-89 | 4 | Z | Z coordinate (float) |
| 90-93 | 4 | ZoneType | Zone transition type (see Zone Type section) |
| 94-97 | 4 | Reserved | Always 0 |
| 98-101 | 4 | ZoneFlags | **LOW BYTE ONLY** - see warning below |

### ZoneFlags Bug (CRITICAL)

**Problem:** The ZoneFlags field has a bug where only 1 byte is intentionally written, but 4 bytes are sent.

**Technical Details:**
- In `SendZoneRequest`, `MOV [RBP-0x40], R15B` writes only 1 byte (0 or 1)
- `MOVUPS` later copies 16 bytes at a time, including the 3 uninitialized bytes
- Result: Upper 3 bytes contain stack garbage

**Example:**
- Packet shows `ZoneFlags: 0x71EEC700` → Only `0x00` is meaningful
- Mask with `& 0xFF` to get actual flag value

---

## Call Flow Diagrams

### Zone Loading State Machine
```
Game_MainLoopInit
    |
    v
CEverQuest__ZoneLoadingStateMachine (0x140284e30)
    |
    +--[1] Check current game state (offset 0x5E4)
    |      - 0xFF = Not zoning
    |      - 0x03 = Loading zone
    |      - 0x05 = Cleanup/exit
    |
    +--[2] Create/destroy zone connection
    |      - Sends packet 0x2A54 (zone request)
    |
    +--[3] Wait for zone data
    |      - Loop with 500ms poll
    |      - Timeout after ~120 seconds
    |
    +--[4] Call Zone_Handler (anti-cheat)
    |      - Fingerprint hardware
    |      - Hash memory regions
    |      - Schedule next memcheck
    |
    +--[5] Initialize zone
    |      - SetGameState(1) - enable handlers
    |      - Call MainGameLoop
    |
    +--[6] Send completion packets
           - 0x60F7, 0x6A30, 0x5F58, 0x66F8
```

### Execute Zone Transition
```
CEverQuest__ProcessWorldPacket
    |
    v
ExecuteZoneTransition (0x1402816e0)
    |
    +-- FlushMovementQueue
    |
    +-- Validate zone (check coords, level, flags)
    |
    +-- CEverQuest__ValidateZoneRequest
    |       |
    |       +-- Returns: 1 (success) or negative (error code)
    |
    +-- On Success:
    |       +-- Store destination coords (X, Y, Z, Heading)
    |       +-- Send zone packets
    |       +-- Enter loading loop:
    |       |       +-- ProcessGameEvents
    |       |       +-- RealRender_World
    |       |       +-- DrawWindows (loading UI)
    |       +-- MoveLocalPlayerToSafeCoords
    |
    +-- On Failure:
            +-- Display error message (string table lookup)
            +-- Return to safe location
```

### Anti-Cheat During Zone
```
Zone_Handler (0x14027b100)
    |
    +-- Fingerprint_AggregateAllHardware
    |       - Collects CPU, GPU, MAC, disk info
    |
    +-- __MemChecker0
    |       - Hashes memory regions for fingerprint
    |
    +-- NetworkSend
    |       - Sends fingerprint to server
    |
    +-- ScheduleNextMemcheck
            - Sets timer for next integrity scan
```

---

## Global Variables

| Address | Purpose |
|---------|---------|
| `0x140E772F0` | Local player spawn pointer |
| `0x140E76E58` | Zone client data pointer |
| `0x140E76E70` | Network connection pointer |
| `0x140E773D0` | Zone name string buffer |
| `0x140E7C9C4` | Zone error code |
| `0x140E7CED0` | Zone reason storage |
| `0x140EF6108` | CEverQuest instance pointer |

---

## Notes

- Every zone transition triggers anti-cheat fingerprinting via `Zone_Handler`
- The client disables gameplay packet handlers during zone to prevent exploits
- Zone reason 0x1B (guild hall) has special instance handling
- Timeout is approximately 120 seconds before zone fails
- Level-too-low zones restore player to previous safe coordinates
