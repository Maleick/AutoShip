# EverQuest Ability Packet Structures

## Overview

These packet structures were reverse engineered from `eqgame.exe` by analyzing the following functions:
- `Cmd_UseSkill` - Self-buff abilities
- `CharacterZoneClient__UseTargetAbility` - Target abilities (Kick, Bash, etc.)
- `PlayerZoneClient__DoAttack` / `SendAttackPacketToServer` - Combat attacks

---

## Attack Packet (Opcode: 0x612D)

Used for combat skill attacks like Flying Kick, Tail Rake, Eagle Strike, etc.

```
Size: 12 bytes

Offset  Size  Type      Field
------  ----  ----      -----
0x00    4     uint32    Target Spawn ID
0x04    4     uint32    Attack Type
0x08    4     uint32    Skill Type
```

### Attack Types
| Value | Name | Description |
|-------|------|-------------|
| 0x0B | Primary | Primary melee attack |
| 0x0D | Dual Wield | Off-hand attack |
| 0x0E | Ranged | Ranged/bow attack |
| 0x64 | Skill | Combat skill ability |
| 0x65 | Skill Alt | Skill attack variant |

### Skill Types (when Attack Type = 0x64)
| Value | Decimal | Skill |
|-------|---------|-------|
| 0x14 | 20 | Eagle Strike |
| 0x15 | 21 | Tail Rake (Iksar) |
| 0x17 | 23 | Tiger Claw |
| 0x1A | 26 | Flying Kick |
| 0x1C | 28 | Round Kick |

### Example: Flying Kick
```
Opcode: 0x612D
Payload: B6 06 00 00 64 00 00 00 1A 00 00 00
         ^^^^^^^^^^^                         Target ID: 0x6B6 (1718)
                     ^^^^^^^^^^^             Attack Type: 0x64 (skill)
                                 ^^^^^^^^^^^  Skill Type: 0x1A (Flying Kick)
```

---

## Target Ability Packet (Opcode: 0x496B)

Used for basic target abilities like Kick, Bash, Backstab.

```
Size: 4 bytes

Offset  Size  Type      Field
------  ----  ----      -----
0x00    2     uint16    Target Spawn ID
0x02    1     uint8     Skill ID
0x03    1     uint8     Fixed: 0x0B
```

### Skill IDs (Class-Specific)
| Value | Decimal | Skill | Classes |
|-------|---------|-------|---------|
| 0x01 | 1 | Bash | WAR, PAL, SHD, CLR |
| 0x02 | 2 | Kick | MNK, WAR |
| 0x08 | 8 | Backstab | ROG |
| 0x2D | 45 | Round Kick | MNK |
| 0x2E | 46 | Flying Kick | MNK |
| 0x38 | 56 | Frenzy | BER |

---

## Taunt Packet (Opcode: 0x09FD)

Simple taunt ability.

```
Size: 4 bytes

Offset  Size  Type      Field
------  ----  ----      -----
0x00    4     uint32    Target Spawn ID
```

---

## Feign Death Packet (Opcode: 0x7A07)

Self-only ability for Monks/Necros.

```
Size: 12 bytes

Offset  Size  Type      Field
------  ----  ----      -----
0x00    4     uint32    Player Spawn ID
0x04    4     uint32    Reserved (0x00)
0x08    4     uint32    Reserved (0x00)
```

### Example: Feign Death
```
Opcode: 0x7A07
Payload: C3 06 00 00 00 00 00 00 00 00 00 00
         ^^^^^^^^^^^                         Player ID: 0x6C3 (1731)
                     ^^^^^^^^^^^             Reserved: 0
                                 ^^^^^^^^^^^  Reserved: 0
```

---

## Self-Buff Skill Packets

These skills target self and share similar 12-byte structure.

| Opcode | Skill | Description |
|--------|-------|-------------|
| 0x7A07 | Feign Death | Play dead |
| 0x5167 | Hide | Become invisible |
| 0x1DA2 | Mend | Self heal (Monk) |
| 0x15A4 | Sense Traps | Detect traps (Rogue) |
| 0x5C1D | Disarm Traps | Disarm traps (Rogue) |

### Common Structure for Self Skills
```
Size: 12 bytes

Offset  Size  Type      Field
------  ----  ----      -----
0x00    4     uint32    Player Spawn ID
0x04    4     uint32    Reserved (0x00)
0x08    4     uint32    Reserved (0x00)
```

---

## Position Packet (Opcode: 0x5DA2)

Used for position spoofing (works for Taunt, NOT for attack abilities).

```
Size: ~48 bytes (structure varies)

Key Offsets:
0x00    4     uint32    Player Spawn ID
0x04    4     float     X Position
0x08    4     float     Y Position
0x0C    4     float     Z Position
0x1C    4     uint32    Heading (12-bit, scaled by 8.0)
```

---

## Important Notes

### Position Spoofing Limitations
- **Works for**: Taunt (0x09FD), utility abilities
- **Does NOT work for**: Attack packets (0x612D) - server has stricter LOS validation for damage

### Server Validation
- Attack abilities (0x612D) require actual client position within melee range
- The server cross-validates position packets against attack packets
- Utility abilities like Taunt have looser validation

---

## Source Functions (Ghidra)

| Function | Address | Purpose |
|----------|---------|---------|
| Cmd_UseSkill | 0x140238640 | Self-buff skill dispatch (wrapper) |
| CharacterZoneClient__UseTargetAbility | 0x1400ec1b0 | Target ability packet (0x496B) |
| PlayerZoneClient__DoAttack | 0x1406c04b0 | Attack packet construction |
| SendAttackPacketToServer | (inline) | Sends 0x612D packet |

---

## Finding Opcodes in Ghidra

### Taunt Opcode (0x09FD)
Search for byte pattern `FD 09` to find usage locations:
- `0x1402b3274` - Large action handler function
- `0x14051eea3` - Skill processing area

### Target Ability Opcode (0x496B)
Found in `CharacterZoneClient__UseTargetAbility` at:
- `0x1400ec3f6` - MOV EDX, 0x496b

### Attack Opcode (0x612D)
Search for byte pattern `2D 61` to find:
- Used in DoAttack / SendAttackPacketToServer

---

## Utility Abilities (May Work Like Taunt)

These abilities likely use simpler server validation (no strict LOS):

| Ability | Skill ID | Opcode | Packet Type |
|---------|----------|--------|-------------|
| Taunt | 0x25 (37) | 0x09FD | Target ID only |
| Intimidation | 0x4A (74) | ? | Likely target ID |
| Begging | 0x6C (108) | ? | Likely target ID |
| Sense Heading | 0x09 (9) | - | Self only |
| Forage | 0x13 (19) | - | Self only |
| Fishing | 0x37 (55) | - | Self only |

### Skills That Require Melee Range (Strict LOS)
| Ability | Skill ID | Opcode | Notes |
|---------|----------|--------|-------|
| Flying Kick | 0x1A (26) | 0x612D | Attack packet |
| Kick | 0x02 (2) | 0x496B | Target ability |
| Backstab | 0x08 (8) | 0x496B | Target ability |
| Bash | 0x01 (1) | 0x496B | Target ability |

---

## Analysis Date
- Binary: `eqgame.exe` (64-bit)
- Date: 2024
- Tool: Ghidra + packet capture
