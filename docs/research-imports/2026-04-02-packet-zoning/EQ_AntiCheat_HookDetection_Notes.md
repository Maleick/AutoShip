# EverQuest Anti-Cheat Hook Detection Notes

## Overview

EverQuest's `eqgame.exe` contains hook detection code in the main game loop (`MainGameLoop_AntiCheatValidation` at `0x140270d00`). The game checks if certain Windows API functions have been hooked by examining the first bytes of each function.

## Detection Method

The game checks for two common hooking patterns:

```c
// Pattern 1: Direct JMP (0xE9)
if ((char)FunctionAddress == 0xE9) {
    hooked = true;
}

// Pattern 2: Indirect JMP (0xFF 0x25 or 0xFF 0x24)
else if ((char)FunctionAddress == 0xFF &&
         ((byte)(FunctionAddress[1] - 0x24) < 2)) {
    hooked = true;
}
```

### Byte Patterns Detected
| Pattern | Bytes | Meaning |
|---------|-------|---------|
| Direct JMP | `E9 xx xx xx xx` | 5-byte relative jump |
| Indirect JMP | `FF 25 xx xx xx xx` | 6-byte absolute indirect jump |
| Indirect JMP | `FF 24 ...` | SIB-based indirect jump |

## Monitored Functions

### Kernel32.dll Functions
| Function | Report Code | Purpose |
|----------|-------------|---------|
| `GetComputerNameW` | 0xb24 | System identification |
| `OpenProcess` | 0x7253 | Process access (injection detection) |
| `K32EnumProcesses` | 0x20d8 | Process enumeration |
| `K32EnumProcessModules` | 0x45d5 | Module enumeration |
| `GetModuleFileNameA` | 0x227c | Module path retrieval |
| `GetCurrentProcess` | 0x390 | Current process handle |
| `QueryFullProcessImageNameA` | 0xce0 | Process image name |

### Iphlpapi.dll Functions
| Function | Report Code | Purpose |
|----------|-------------|---------|
| `GetAdaptersAddresses` | 0x2b65 | Network adapter info |

### Internal EQ Functions (by address)
| Address | Report Code | Likely Purpose |
|---------|-------------|----------------|
| 0x140549a00 | 0x7e60 | Unknown |
| 0x1405843c0 | 0x1e0d | Unknown |
| 0x140584610 | 0x16d7 | Unknown |
| 0x140582690 | 0x1939 | Unknown |
| 0x140584450 | 0x2f3d | Unknown |
| 0x140290f60 | 0x7b44 | Unknown |
| 0x1402910d0 | 0x1d3 | Unknown |
| 0x140292c50 | 0x214 | Unknown |
| 0x140269290 | 0x2233 | Unknown |
| 0x1406b3460 | 0x921 | Unknown |
| 0x1406b3330 | 0x40a6 | Unknown |
| 0x1406b35c0 | 0x4e98 | Unknown |
| 0x1406b28c0 | 0x5aee | Unknown |
| 0x1406b3040 | 0x5858 | Unknown |
| 0x1406b2d70 | 0x6a0b | Unknown |
| 0x1406b2e70 | 0x2cf5 | Unknown |

## Reporting Mechanism

When a hook is detected, the game sends opcode `0xfbb` with:
- Player spawn ID
- Detection type (0x2e)
- Report code (identifies which function was hooked)

```c
// Example from decompilation:
if (cVar8 != DAT_140ef0c00) {  // State changed
    uStack_a60 = (undefined4)_DAT_140e762c0[0x2d];  // Spawn ID
    uStack_a5c = CONCAT22(uStack_a5c._2_2_, 0x2e);  // Type
    uStack_a58 = 0xb24;  // Report code for GetComputerNameW
    uStack_a50 = 0;
    DAT_140ef0c00 = cVar8;  // Update state
    // ... send packet 0xfbb ...
}
```

## Evasion Considerations

### For legitimate tools (MacroQuest, etc.):
1. **Trampoline Hooks**: Use trampolines that preserve original bytes
2. **Mid-function Hooks**: Hook after the first few bytes
3. **IAT Hooks**: Hook Import Address Table instead of function prologue
4. **VEH/VCH**: Use Vectored Exception Handlers instead of inline hooks
5. **Hardware Breakpoints**: Use debug registers (DR0-DR3)

### Detection Timing
- Checks run periodically in main game loop
- State is cached - only reports on state CHANGE
- Random interval: `(iVar29 % 0x51 + 0x53)` milliseconds between checks

## Related Anti-Cheat Components

### Counter System (separate from hook detection)
| Address | Name | Purpose |
|---------|------|---------|
| 0x140f43e68 | g_AntiCheatCounter_Teleport | Position validation |
| 0x140f43e6c | g_AntiCheatCounter_Packet | Packet validation |

Server validation packet `0x350f` sent every 500ms with negated counter values.

## Source

- Binary: `eqgame.exe` (64-bit)
- Base Address: `0x140000000`
- Analysis Date: 2025-11-30
- Function: `MainGameLoop_AntiCheatValidation` @ `0x140270d00`
