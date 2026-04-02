# EverQuest Network Architecture

## Overview

EverQuest uses a custom UDP-based network protocol with opcode scrambling, encryption, and anti-cheat counters.

---

## SENDING PACKETS

### Layer 1: High-Level Send Functions

These are game-specific functions that construct and send packets:

| Function | Address | Purpose |
|----------|---------|---------|
| `Cmd_UseSkill` | 0x140238640 | Self-buff skills (Mend, Feign, Hide) |
| `SendAttackPacketToServer` | 0x140249530 | Combat attacks (0x612d) |
| `CharacterZoneClient__UseSkill` | 0x1401015d0 | Skill execution |
| `CEverQuest__SendZoneRequestPacket` | 0x1402906e0 | Zone requests |
| `CEverQuest__SendEmoteOrSayPacket` | 0x140292c20 | Chat/emotes |

### Layer 2: NetworkSend (Primary Send Path)

**Function:** `NetworkSend` @ `0x140550030`

```
NetworkSend(gWorld, type, &buffer, size)
  │
  ├─► type 0: NetworkSend_QueuePacket (queued reliable)
  ├─► type 1: NetworkSend_EncryptAndTransmit (immediate)
  ├─► type 2: NetworkSend_QueuePacket (sequenced)
  ├─► type 3: NetworkSend_EncryptAndTransmit (sequenced)
  └─► type 4-7: PacketFragment_AppendData (fragmented)
         │
         └─► NetworkSend_TransmitToSocket @ 0x14054fb80
                │
                └─► Socket sendto()
```

### Layer 3: UdpConnection (Alternate Path)

**Function:** `UdpConnection::SendMessage` (address varies, pattern-found)

- Used by position packets (0x1643)
- May be lower level than NetworkSend
- Currently hooked by MQ2reachit

### Packet Scrambling

**Scrambler Function:** `func_0x00014067b9e0` (hton - host to network)

```c
// How packets are sent:
buffer = PacketScrambler(g_pPacketScrambler, OPCODE, 0);  // Scramble
NetworkSend(gWorld, 4, &buffer, size);                     // Send
```

**Descrambler Function:** Found via pattern, converts network opcode back to internal

---

## RECEIVING PACKETS

### Layer 1: Socket Reception

**Function:** `NetworkSession_HandleIncomingPacket` @ `0x14055a080`

- Receives raw UDP packets
- Handles fragmentation reassembly
- Decrypts if needed

### Layer 2: World Message Handler

**Functions:**
- `CEverQuest__HandleWorldMessage` @ `0x1401ac200` (instance 1)
- `CEverQuest__HandleWorldMessage` @ `0x1402c20a0` (instance 2)

- Dispatches packets based on opcode to specific handlers

### Layer 3: Packet Processors

| Function | Address | Purpose |
|----------|---------|---------|
| `ProcessGroupPacket` | 0x1402de700 | Group updates |
| `ProcessEmotePacket` | 0x14020cea0 | Emotes |
| `ProcessChannelMessagePacket` | 0x14020a1b0 | Chat |
| `ProcessBazaarPacket` | 0x14020eab0 | Bazaar |
| `CEverQuest__ProcessWorldPacket` | 0x1401e6d00 | General world |
| `CEverQuest__ProcessZonePacket` | 0x1402802b0 | Zone-specific |

---

## GLOBAL VARIABLES

### Network Buffers

| Variable | Purpose |
|----------|---------|
| `g_MovementPacketBuffer` | Scrambled packet buffer |
| `g_ZoneRequestPacketBuffer` | Payload data (bytes 0-3) |
| `g_ZoneRequestPacketBuffer_08` | Payload data (bytes 8-11) |
| `___gWorld` | World connection object |
| `_g_pPacketScrambler` | Scrambler instance |

### Anti-Cheat Counters

| Variable | Purpose |
|----------|---------|
| `_g_CounterA` | Receive counter (decremented on receive) |
| `_g_CounterB` | Send counter (decremented AFTER each send) |
| `_g_CounterC` | Teleport counter |

**Counter Behavior:**
- Decremented after each NetworkSend call
- Refilled when value drops below 2 (ADD 0x37 = 55)
- Server validates counter synchronization

---

## SKILL OPCODES (from Cmd_UseSkill)

| Skill ID | Decimal | Skill Name | Opcode |
|----------|---------|------------|--------|
| 0x19 | 25 | Feign Death | 0x7a07 |
| 0x1b | 27 | Hide | 0x5167 |
| 0x35 | 53 | Mend | 0x1da2 |
| 0x20 | 32 | Safe Fall | 0x75a5 |
| 0x23 | 35 | Disarm | 0x2742 |
| 0x3e | 62 | Sense Traps | 0x2ea9 |
| 0x47 | 71 | Bind Wound | 0x5d7d |
| 0x4b | 75 | Intimidation | 0x3d09 |
| 0x11 | 17 | Begging | 0x61e9 |

**Attack opcode:** 0x612d (used by `SendAttackPacketToServer`)

---

## PACKET FLOW DIAGRAM

```
┌─────────────────────────────────────────────────────────────────┐
│                        GAME LOGIC                                │
│  (Cmd_UseSkill, SendAttackPacketToServer, etc.)                 │
└─────────────────────────┬───────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    PACKET SCRAMBLER                              │
│  func_0x00014067b9e0(scrambler, opcode, 0)                      │
│  Converts internal opcode to network opcode                      │
└─────────────────────────┬───────────────────────────────────────┘
                          │
          ┌───────────────┴───────────────┐
          │                               │
          ▼                               ▼
┌─────────────────────┐       ┌─────────────────────┐
│    NetworkSend      │       │ UdpConnection::     │
│    0x140550030      │       │ SendMessage         │
│                     │       │ (pattern-found)     │
│ Used by:            │       │                     │
│ - Mend (0x1da2)     │       │ Used by:            │
│ - Feign (0x7a07)    │       │ - Position (0x1643) │
│ - Attack (0x612d)   │       │ - Some packets      │
│ - Most abilities    │       │                     │
└─────────┬───────────┘       └─────────┬───────────┘
          │                             │
          ▼                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                    SOCKET LAYER                                  │
│  NetworkSend_TransmitToSocket @ 0x14054fb80                     │
│  sendto() / WSASendTo()                                         │
└─────────────────────────────────────────────────────────────────┘
```

---

## HOOKING STRATEGY

### Current Hook (MQ2reachit)

- **Hooked:** `UdpConnection::SendMessage`
- **Captures:** Position packets (0x1643), some abilities
- **Misses:** NetworkSend path (Mend, most skills)

### Recommended Additional Hooks

1. **NetworkSend** @ `0x140550030`
   - Captures all skill/ability packets
   - Entry point before scrambling

2. **NetworkSend_TransmitToSocket** @ `0x14054fb80`
   - Captures ALL outgoing packets
   - After scrambling/encryption

3. **CEverQuest__HandleWorldMessage**
   - Captures all incoming packets
   - For packet inspection/modification

---

## ANTI-CHEAT CONSIDERATIONS

1. **Counter Synchronization**
   - Must decrement `_g_CounterB` after injected sends
   - Or skip the game's decrement when injecting

2. **Memcheck Functions**
   - `SendMemCheckOnHotButton` @ 0x1403f8d40
   - `SendMemCheckOnSpellCast` @ 0x14039a2d0
   - `Memcheck_SendDetectionPacket_0x1ac8` @ 0x140293400

3. **Hook Detection**
   - Game checks for JMP/indirect JMP at function entry points
   - Use mid-function hooks or IAT hooks to evade

---

## Analysis Date
- Binary: `eqgame.exe` (64-bit)
- Base Address: `0x140000000`
- Date: December 2024
