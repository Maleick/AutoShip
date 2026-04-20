# FFI Bridge Architecture Specification

**Version:** 1.0  
**Issue:** #1012  
**Status:** Design Phase

---

## 1. Overview

The FFI bridge enables Rust code (in TextQuest core) to interact with MacroQuest plugins running in the EQ client process. This is a **unidirectional + request-response** pattern:

```
┌─────────────────────────────────────────────────────┐
│  EQ Client Process                                  │
├─────────────────────────────────────────────────────┤
│  MQ2 Core                                           │
│  ├─ MQ2Melee, MQ2Cast, MQ2MoveUtils, etc.          │
│  └─ Shared Memory Region (state + queue)           │
└─────────────────────────────────────────────────────┘
         ↓ (Win32 shared memory / mmap on Linux)
┌─────────────────────────────────────────────────────┐
│  TextQuest Orchestrator Process                     │
├─────────────────────────────────────────────────────┤
│  textquest-ffi (Rust bindings layer)                │
│  ├─ State Reader                                    │
│  ├─ Command Queuer                                  │
│  └─ Message Parser                                  │
└─────────────────────────────────────────────────────┘
```

---

## 2. Shared Memory Layout

### 2.1 Metadata Header (Offset 0x00, 256 bytes)

```
Offset  Size  Name              Description
──────  ────  ────────────────  ────────────────────────────
0x00    4     MAGIC             0x54515645 ("TQVE" = TextQuest Version Epoch)
0x04    4     VERSION           Plugin API version (e.g., 0x00010000 = v1.0)
0x08    4     EQLIB_BUILD_DATE  EQ patch date from eqlib (e.g., 20260420)
0x0C    4     FLAGS             Bit flags: [0]=init, [1]=syncing, [2]=eqbc_ready
0x10    4     RESERVED1
0x14    4     STATE_OFFSET      Byte offset to GameState region (typically 0x100)
0x18    4     COMMAND_OFFSET    Byte offset to command queue (typically 0x500)
0x1C    4     MESSAGE_OFFSET    Byte offset to message buffer (typically 0x800)
0x20    4     STATE_SIZE        Actual size of GameState struct
0x24    4     COMMAND_QUEUE_SZ  Max commands in queue (e.g., 64)
0x28    4     MESSAGE_BUFFER_SZ Message buffer size in bytes (e.g., 4096)
0x2C    4     LAST_PULSE_TIME   Unix timestamp of last MQ2 pulse
0x30    4     PULSE_INTERVAL_MS Expected interval between pulses
0x34    4     SEQUENCE_NUM      Incremented each sync (for version checking)
0x38    196   RESERVED2         (reserved for future flags)
```

### 2.2 Game State Region (Offset 0x100)

```
Offset  Size  Name              Type
──────  ────  ────────────────  ──────────────────
0x00    4     player_id         uint32_t
0x04    4     target_id         uint32_t
0x08    64    player_name       char[64]
0x40    64    target_name       char[64]
0x80    4     level             uint8_t
0x84    3     PADDING
0x87    4     hp_current        int32_t
0x8B    4     hp_max            int32_t
0x8F    4     mana_current      int32_t
0x93    4     mana_max          int32_t
0x97    4     endurance_current int32_t
0x9B    4     endurance_max     int32_t
0x9F    4     x                 float
0xA3    4     y                 float
0xA7    4     z                 float
0xAB    4     heading           float (0-512 scale)
0xAF    1     zone_id           uint8_t
0xB0    1     stand_state       uint8_t (0=stand, 1=sit, 2=feign)
0xB1    2     PADDING
0xB3    4     target_hp         int32_t
0xB7    4     target_hp_max     int32_t
0xBB    4     target_level      uint8_t
0xBC    4     group_count       uint8_t
0xBD    3     PADDING
0xC0    256   group_member[]    GroupMember[12]  (each 20 bytes)
...more fields...
```

### 2.3 Command Queue (Offset 0x500)

Circular queue of up to 64 pending MQ2 commands:

```
Offset  Size  Name              Description
──────  ────  ────────────────  ──────────────────────────
0x00    4     head_idx          Write pointer (next command to queue)
0x04    4     tail_idx          Read pointer (next command to execute)
0x08    4     count             Number of pending commands
0x0C    4     PADDING

Command Entry (32 bytes each, 64 entries = 2048 bytes):
0x00    1     cmd_type          enum { CAST=1, MELEE=2, STICK=3, MOVETO=4, ANIM=5 }
0x01    1     priority          0=low, 1=normal, 2=high (casting interrupts others)
0x02    2     PADDING
0x04    28    args              Raw command args (e.g., spell name, location)
```

### 2.4 Message Buffer (Offset 0x800)

Ring buffer for chat/event messages:

```
Offset  Size  Name              Description
──────  ────  ────────────────  ──────────────────────────
0x00    4     write_pos         Next byte to write
0x04    4     read_pos          Next byte to read
0x08    2     RESERVED

Message Format (variable length, delimited by 0xFF):
[color:2][sender:32][text:???][0xFF]
```

---

## 3. Synchronization Protocol

### 3.1 MQ2 → Rust (Pull-Based)

On each Rust pulse:
1. Read `SEQUENCE_NUM` from metadata
2. If changed since last read, re-read `GameState` region
3. Validate `MAGIC` and `VERSION` match
4. Check `LAST_PULSE_TIME` — abort if >1000ms old (client hung)

```rust
pub fn sync_game_state() -> Result<GameState> {
    let header = read_shared_memory::<Header>(0x00)?;
    
    // Validate
    if header.magic != 0x54515645 {
        return Err("Invalid magic");
    }
    if header.version != 0x00010000 {
        return Err("API version mismatch");
    }
    
    let age_ms = now() - Duration::from_secs(header.last_pulse_time as u64);
    if age_ms > Duration::from_millis(1000) {
        return Err("Stale state (client hung)");
    }
    
    let state = read_shared_memory::<GameState>(header.state_offset)?;
    Ok(state)
}
```

### 3.2 Rust → MQ2 (Push-Based Queue)

On command dispatch:
1. Calculate next free slot in command queue
2. Write command entry (8 byte header + 24 byte args)
3. Increment `head_idx` atomically
4. MQ2 picks up on next pulse

```rust
pub fn queue_command(cmd_type: CommandType, priority: u8, args: &str) -> Result<()> {
    let mut header = read_shared_memory::<Header>(0x00)?;
    
    let queue_offset = header.command_offset as usize;
    let entry_size = 32usize;
    let max_entries = header.command_queue_sz as usize;
    
    let next_head = (header.head_idx + 1) % max_entries;
    if next_head == header.tail_idx {
        return Err("Command queue full");
    }
    
    let cmd_entry = CommandEntry {
        cmd_type: cmd_type as u8,
        priority,
        args: args.as_bytes().to_vec(),
    };
    
    write_shared_memory(
        queue_offset + (header.head_idx as usize) * entry_size,
        &cmd_entry
    )?;
    
    header.head_idx = next_head;
    write_shared_memory(0x00, &header)?;
    Ok(())
}
```

---

## 4. Plugin Command Reference

### 4.1 MQ2Melee Commands

```
/melee on                              Enable melee
/melee off                             Disable melee
/melee reload                          Reload INI config
/melee aggro on|off                    Toggle aggro mode (tank vs dps)
/melee petassist on|off                Pet assist enable
```

**Rust binding:**
```rust
pub enum MeleeCommand {
    On,
    Off,
    Reload,
    SetAggro(bool),
    SetPetAssist(bool),
}

impl MeleeCommand {
    pub fn execute(&self) -> Result<()> {
        let args = match self {
            MeleeCommand::On => "on".to_string(),
            MeleeCommand::Off => "off".to_string(),
            // ...
        };
        queue_command(CommandType::MELEE, PRIORITY_NORMAL, &args)
    }
}
```

### 4.2 MQ2Cast Commands

```
/casting <spell_name> [options]
  -maxtries|N                         Retry N times on failure
  -kill                               Retry until target dies
  -targetid <id>                      Cast on specific spawn ID

/memorize <spell_name> gem1|gem2|...  Memorize spell to gem slots
/interrupt                            Interrupt current cast
```

**Rust binding:**
```rust
pub struct CastRequest {
    pub spell_name: String,
    pub target_id: Option<u32>,
    pub max_retries: Option<u8>,
    pub retry_until_dead: bool,
}

impl CastRequest {
    pub fn execute(&self) -> Result<()> {
        let args = format!(
            "{} {}",
            self.spell_name,
            if let Some(id) = self.target_id {
                format!("-targetid|{}", id)
            } else {
                String::new()
            }
        );
        queue_command(CommandType::CAST, PRIORITY_HIGH, &args)
    }
}
```

### 4.3 MQ2MoveUtils Commands

```
/stick [distance] [position]           Stick to target at position
  behind                               Position behind target
  front                                Position in front
  pin                                  Position on side
  
/moveto loc <y> <x> [z]               Move to coordinates
/circle on [radius]                    Circle around location
/makecamp on|off [radius]              Setup camp with return radius
```

**Rust binding:**
```rust
pub enum StickPosition {
    Melee,      // Default: target's melee range
    Behind,
    Front,
    Pin,
    Custom(f32),
}

pub struct StickRequest {
    pub position: StickPosition,
    pub target_id: Option<u32>,
}

impl StickRequest {
    pub fn execute(&self) -> Result<()> {
        let args = format!("{}", match self.position {
            StickPosition::Melee => String::new(),
            StickPosition::Behind => "behind".to_string(),
            StickPosition::Front => "front".to_string(),
            StickPosition::Pin => "pin".to_string(),
            StickPosition::Custom(d) => format!("{}", d),
        });
        queue_command(CommandType::STICK, PRIORITY_NORMAL, &args)
    }
}
```

---

## 5. TLO (Top-Level Object) Access

### 5.1 Design

TLOs are read-only snapshots updated on each MQ2 pulse. We expose them via Rust trait:

```rust
pub trait TLOProvider {
    fn get_melee(&self) -> MeleeTLO;
    fn get_cast(&self) -> CastTLO;
    fn get_stick(&self) -> StickTLO;
    fn get_auto_loot(&self) -> AutoLootTLO;
}

pub struct LiveTLOProvider {
    last_sync: Instant,
}

impl TLOProvider for LiveTLOProvider {
    fn get_melee(&self) -> MeleeTLO {
        // Read from shared memory offset 0xA00 (post-GameState)
        read_shared_memory::<MeleeTLO>(MELEE_TLO_OFFSET).unwrap_or_default()
    }
}
```

### 5.2 TLO Definitions

#### MeleeTLO

```rust
#[repr(C)]
pub struct MeleeTLO {
    pub active: bool,
    pub engaged: bool,
    pub swing_hits: u32,
    pub taken_hits: u32,
    pub melee_aggro: bool,
    pub pet_assist: bool,
}
```

#### CastTLO

```rust
#[repr(C)]
pub struct CastTLO {
    pub active: bool,
    pub last_result: CastResultCode,  // u8: SUCCESS=0, FIZZLE=5, INTERRUPT=1, etc.
    pub timing_ms: u32,
    pub spell_id: u32,
    pub target_id: u32,
}

#[repr(u8)]
pub enum CastResultCode {
    Success = 0,
    Interrupted = 1,
    Resist = 2,
    Fizzle = 5,
    Standing = 6,
    Stunned = 7,
    // ... 21 total codes
}
```

#### StickTLO

```rust
#[repr(C)]
pub struct StickTLO {
    pub active: bool,
    pub target_id: u32,
    pub distance: f32,
    pub in_range: bool,
    pub broken: bool,
}
```

---

## 6. Version & Compatibility Handling

### 6.1 Versioning Strategy

Each plugin API uses semantic versioning:

```
VERSION = 0x00MMmmss
  MM = major (breaking changes)
  mm = minor (backward-compatible additions)
  ss = patch (bug fixes)
```

At plugin load time:
1. MQ2 writes its API version to shared memory
2. Rust reads and validates compatibility
3. If versions don't match, both sides disable that plugin's commands

```rust
pub fn validate_version(client_version: u32) -> Result<()> {
    const SUPPORTED_MAJOR: u32 = 1;
    const SUPPORTED_MINOR: u32 = 0;
    
    let major = (client_version >> 16) & 0xFF;
    let minor = (client_version >> 8) & 0xFF;
    
    if major != SUPPORTED_MAJOR {
        return Err(format!(
            "Major version mismatch: expected {}, got {}",
            SUPPORTED_MAJOR, major
        ));
    }
    
    // Minor version updates are backward-compatible
    if minor > SUPPORTED_MINOR {
        eprintln!("Warning: client has newer minor version ({})", minor);
    }
    
    Ok(())
}
```

### 6.2 EQ Patch Handling

When EQ patches occur (typically Wednesdays), offsets change. MQ2 will fail to read/write memory until `eqlib` is updated. We detect this:

```rust
pub fn detect_offset_mismatch() -> bool {
    let header = read_shared_memory::<Header>(0x00).unwrap_or_default();
    
    // If EQ patch date != our compiled eqlib date, offsets may be wrong
    if header.eqlib_build_date != COMPILED_EQLIB_DATE {
        eprintln!(
            "EQ patch detected: eqlib date {} vs client {}",
            COMPILED_EQLIB_DATE, header.eqlib_build_date
        );
        return true;
    }
    
    false
}
```

---

## 7. Error Handling & Recovery

### 7.1 Error Categories

```rust
pub enum FFIError {
    // Shared memory errors
    SharedMemoryNotFound,
    MagicMismatch,
    VersionMismatch,
    StateStale,          // MQ2 pulse >1000ms old
    
    // Command queue errors
    CommandQueueFull,
    InvalidCommand,
    
    // Synchronization errors
    SyncTimeout,
    OffsetMismatch,
    
    // Resource errors
    PermissionDenied,
    InvalidAddress,
}
```

### 7.2 Timeout & Retry

```rust
pub struct CommandRequest {
    id: u64,
    created_at: Instant,
    timeout_ms: u64,
}

impl CommandRequest {
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed().as_millis() as u64 > self.timeout_ms
    }
    
    pub fn execute_with_retry(&self, max_retries: u8) -> Result<()> {
        for attempt in 0..max_retries {
            match queue_command(...) {
                Ok(()) => return Ok(()),
                Err(e) if e == FFIError::CommandQueueFull => {
                    if attempt < max_retries - 1 {
                        std::thread::sleep(Duration::from_millis(50));
                        continue;
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Err(FFIError::CommandQueueFull)
    }
}
```

---

## 8. Windows vs Linux Considerations

### 8.1 Platform Differences

| Aspect              | Windows        | Linux                 |
|---------------------|----------------|-----------------------|
| Shared Memory       | Named Pipes    | mmap (tmpfs)          |
| Mutex               | CRITICAL_SECTION | pthread_mutex_t     |
| Memory Protection   | PAGE_READWRITE | 0666 perms            |
| Socket IPC          | Named Pipes    | Unix domain sockets   |

### 8.2 Fallback Strategy

On **macOS** (no EQ client):
- Compile TextQuest with conditional code paths
- All FFI calls return `Err(FFIError::SharedMemoryNotFound)`
- Orchestrator logs and continues in sim mode

```rust
#[cfg(windows)]
mod ffi {
    pub fn sync_game_state() -> Result<GameState> {
        // Real implementation
    }
}

#[cfg(not(windows))]
mod ffi {
    pub fn sync_game_state() -> Result<GameState> {
        Err(FFIError::SharedMemoryNotFound)  // EQ not running
    }
}
```

---

## 9. Testing & Validation

### 9.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_header_layout() {
        // Verify struct sizes match memory layout
        assert_eq!(size_of::<Header>(), 256);
        assert_eq!(size_of::<GameState>(), 1024);
        assert_eq!(size_of::<CommandEntry>(), 32);
    }
    
    #[test]
    fn test_magic_validation() {
        // Create mock header with wrong magic
        let header = Header { magic: 0xDEADBEEF, ..Default::default() };
        assert!(validate_version(header.version).is_err());
    }
}
```

### 9.2 Integration Tests

1. **Live client test**: Spawn EQ + MQ2, verify shared memory created
2. **Command round-trip**: Queue command, verify MQ2 executes
3. **State sync**: Verify GameState updates match character state
4. **Multibox sync**: Verify EQBC coordination across multiple clients

---

## 10. Security Considerations

### 10.1 Attack Surface

- **Shared memory poisoning:** Attacker writes garbage to command queue
- **Buffer overflows:** Arg string >24 bytes
- **Integer overflow:** head/tail indices wraparound
- **Privilege escalation:** Non-admin user modifying admin client's memory

### 10.2 Mitigations

```rust
// Validate all string inputs
pub fn validate_command_args(args: &str) -> Result<()> {
    if args.len() > 24 {
        return Err(FFIError::InvalidCommand);
    }
    
    // No null bytes
    if args.contains('\0') {
        return Err(FFIError::InvalidCommand);
    }
    
    // Only alphanumeric + spaces + pipes (for arg separators)
    if !args.chars().all(|c| c.is_alphanumeric() || c.is_whitespace() || c == '|') {
        return Err(FFIError::InvalidCommand);
    }
    
    Ok(())
}
```

---

## 11. Future Extensions

### 11.1 Webhook Integration

Add optional async HTTP notifications:
```
MQ2 Pulse → Rust FFI → HTTP POST to TextQuest API
(for remote clients monitoring this instance)
```

### 11.2 Lua Scripting Layer

Expose FFI as Lua bindings for macro-level control:
```lua
local player = ffi.get_player()
if player.hp < 50 then
    ffi.queue_command("healing_spell")
end
```

### 11.3 Automatic Offset Detection

Scan EQ memory for known signatures instead of hardcoded offsets:
```rust
fn auto_detect_offsets() -> Result<OffsetMap> {
    // Scan for "pLocalPlayer" signature pattern
    // Calculate offset relative to known module base
}
```

---

## Approval Checklist

- [ ] Type definitions match MQ2 memory layout (struct sizes validated)
- [ ] Command queue design accommodates burst traffic (64 entries)
- [ ] TLO access provides real-time data (<100ms latency)
- [ ] Error handling covers all failure modes
- [ ] Platform-specific code paths tested (Windows/macOS)
- [ ] Security validations prevent injection attacks
- [ ] Versioning strategy handles EQ patches gracefully
