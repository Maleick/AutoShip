# Command Reference

Complete reference for all IPC commands supported by the TextQuest SDK.

## Movement

### `MoveTo`

Navigate to absolute world coordinates.

**Command:**
```rust
Command::MoveTo { x: f32, y: f32, z: f32 }
```

**Python:**
```python
client.move_to(x=100.0, y=200.0, z=50.0)
```

**TypeScript:**
```typescript
await client.moveTo({ x: 100, y: 200, z: 50 });
```

### `StopMovement`

Halt all active movement.

**Command:**
```rust
Command::StopMovement
```

### `NavLoc`

Navigate to specific coordinates (`/nav loc`).

**Command:**
```rust
Command::NavLoc { x: f32, y: f32, z: f32 }
```

### `NavTarget`

Navigate to current target (`/nav target`).

**Command:**
```rust
Command::NavTarget
```

### `NavDoor`

Navigate to nearest door (`/nav door`).

**Command:**
```rust
Command::NavDoor
```

### `NavItem`

Navigate to nearest ground item (`/nav item`).

**Command:**
```rust
Command::NavItem
```

### `NavPause` / `NavResume`

Pause and resume active navigation.

**Command:**
```rust
Command::NavPause
Command::NavResume
```

## Targeting

### `SetTarget`

Set the current target by spawn ID.

**Command:**
```rust
Command::SetTarget { spawn_id: u32 }
```

### `ClearTarget`

Clear the current target.

**Command:**
```rust
Command::ClearTarget
```

### `InteractTarget`

Right-click interact with current target.

**Command:**
```rust
Command::InteractTarget
```

## Combat

### `Attack`

Begin auto-attack on a target.

**Command:**
```rust
Command::Attack { target_id: u32 }
```

### `StopAttack`

Stop auto-attack.

**Command:**
```rust
Command::StopAttack
```

### `CastSpell`

Cast a memorized spell, optionally on a specific target.

**Command:**
```rust
Command::CastSpell {
    spell_slot: u8,        // 1-13 (gem number)
    target_id: Option<u32>, // None = current target
    kill: bool,           // Keep casting until target dies
    recast: u8,          // Number of retries (0 = once)
}
```

**Note:** `kill=true` and `recast>0` are mutually exclusive.

### `CancelCastLoop`

Cancel an active kill-loop or recast-loop.

**Command:**
```rust
Command::CancelCastLoop
```

### `CombatEngage`

Engage a target via the combat FSM.

**Command:**
```rust
Command::CombatEngage { target_id: u32 }
```

### `CombatDisengage`

Disengage from combat.

**Command:**
```rust
Command::CombatDisengage
```

### `CombatEmergencyHeal`

Emergency heal a specific target.

**Command:**
```rust
Command::CombatEmergencyHeal { target_id: u32 }
```

## Navigation Advanced

### `StickTo`

MQ2MoveUtils `/stick` equivalent.

**Command:**
```rust
Command::StickTo { config: StickConfig }
```

### `StickOff`

Stop sticking.

**Command:**
```rust
Command::StickOff
```

### `FollowPlayer`

MQ2MoveUtils `/makecamp player` follow mode.

**Command:**
```rust
Command::FollowPlayer {
    config: FollowConfig,
    anchor_x: f32,
    anchor_y: f32,
    anchor_z: f32,
}
```

### `StopFollow`

Stop player follow mode.

**Command:**
```rust
Command::StopFollow
```

### `CircleKite`

Start circle-kiting mode.

**Command:**
```rust
Command::CircleKite { config: CircleConfig }
```

### `CircleOff`

Stop circle-kiting.

**Command:**
```rust
Command::CircleOff
```

## Utility

### `Sit` / `Stand`

Sit down or stand up.

**Command:**
```rust
Command::Sit
Command::Stand
```

### `LootCorpse` / `LootAll`

Loot the nearest corpse or all items.

**Command:**
```rust
Command::LootCorpse
Command::LootAll
```

### `SlashCommand`

Execute a slash command.

**Command:**
```rust
Command::SlashCommand { command: String }
```

### `Say`

Send a chat message.

**Command:**
```rust
Command::Say {
    channel: SayChannel,
    message: String,
    target: Option<String>, // For tells
}
```

### `Emote`

Perform an emote animation.

**Command:**
```rust
Command::Emote { emote: String }
```

## Login Automation

### `StartLogin`

Start the automated login sequence.

**Command:**
```rust
Command::StartLogin {
    account_name: String,
    password: String,     // Zeroized after use
    server_name: String,
    character_name: String,
}
```

### `LoginPhaseQuery`

Query the current login phase.

**Command:**
```rust
Command::LoginPhaseQuery
```

## Rendering

### `SetRenderMode`

Set the rendering mode.

**Command:**
```rust
Command::SetRenderMode { mode: RenderMode }
```

**RenderMode variants:**
- `Normal` — Full rendering
- `Strobe` — 1 frame per ~5 seconds
- `NullRender` — Zero rendering (GPU idle)

## System

### `Ping`

Heartbeat ping — expects `Pong` response.

**Command:**
```rust
Command::Ping
```

### `Eject`

Terminate the injected DLL session.

**Command:**
```rust
Command::Eject
```

## Response Types

See the [IPC Protocol Specification](../specs/ipc-protocol.md) for complete response documentation.

### Key Responses

- `Pong` — Heartbeat response
- `NavUpdate` — Navigation FSM state transition
- `CombatUpdate` — Combat FSM state transition
- `ChatBatch` — Batched chat messages
- `SpawnEventBatch` — Batched spawn events
- `Error` — Error response with message

## Known Gaps

### Command/Group Operations

The command/group API features are pending the M8 Orchestrator prerequisite. Until that lands:

- Cross-client command coordination is not available
- Group-scoped routing requires manual implementation
- Broadcast commands require individual client calls
