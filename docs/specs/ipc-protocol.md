# IPC Protocol Specification

## Overview

TextQuest uses a dual-channel Inter-Process Communication (IPC) system to enable the orchestrator (external manager) to command injected EQ clients and to receive real-time state updates.

- **Command Channel**: Named pipes for request/response communication (bidirectional)
- **State Channel**: Shared memory for read-only state snapshots (write-once per frame)

Both channels are identified using a session ID derived from a cryptographic token, preventing cross-session interference.

## IPC Naming

All IPC endpoints are derived from a random 32-byte session token at injection time.

### Named Pipe Format

```
\\.\pipe\{session_id:x}_cmd_{client_id}
```

**Example**: `\\.\pipe\deadbeef12345678_cmd_1234`

- `session_id:x` — Session ID in lowercase hex (8 hex digits)
- `client_id` — Client PID as a 32-bit unsigned integer

### Shared Memory Format

```
{session_id:x}_state_{client_id}
```

**Example**: `deadbeef12345678_state_1234`

Same format as pipe name, minus the Windows pipe namespace prefix.

### Session Token

- 32 random bytes (256 bits) generated during injection
- First 8 bytes (little-endian) are used to derive `session_id` via `session_id_from_token()`
- Stored in temp file: `%TEMP%\textquest\{session_id:x}.token`
- Validated by the DLL on every pipe connection for anti-tampering

## Message Framing

### Named Pipe Protocol

Messages are framed using a simple length-prefixed binary format:

```
[4 bytes: message length in bytes (u32, little-endian)]
[N bytes: serialized message (bincode)]
```

1. **Send**: Write length + serialized data to pipe as a single atomic write
2. **Receive**: Read 4-byte header, then read exactly that many bytes

### Serialization Format

- **Codec**: bincode (Rust binary serde format)
- **Byte Order**: Little-endian
- **Compression**: None (prioritize latency)

Bincode is self-delimiting for structs/enums. The length prefix enables framing multiplexed messages.

## Correlation IDs

Commands and responses may include an optional correlation ID for request-response matching in high-latency or pipelined scenarios.

### CorrelationIdGenerator

```rust
pub struct CorrelationIdGenerator {
    next: AtomicU64,  // monotonically increasing
}

pub fn new_id(&self) -> u64  // starts at 1; 0 = no correlation
```

### Usage Pattern

1. Orchestrator sends `IpcCommand { command, correlation_id: Some(42) }`
2. DLL processes command, sends `IpcResponse { response, correlation_id: Some(42) }`
3. Orchestrator matches responses by correlation ID in a pending map

Commands without correlation ID are fire-and-forget or matched by convention.

## IpcCommand

Wraps a `Command` with an optional correlation ID.

```rust
pub struct IpcCommand {
    pub command: Command,
    pub correlation_id: Option<u64>,
}

impl IpcCommand {
    pub fn new(command: Command) -> Self  // No correlation
    pub fn with_correlation(command: Command, correlation_id: u64) -> Self
}
```

## IpcResponse

Wraps a `Response` with the correlation ID echoed from the originating command.

```rust
pub struct IpcResponse {
    pub response: Response,
    pub correlation_id: Option<u64>,
}

impl IpcResponse {
    pub fn new(response: Response) -> Self
    pub fn echo(response: Response, correlation_id: Option<u64>) -> Self
}
```

## Shared Memory Layout

**Size**: 64 KB per client (`SHARED_MEMORY_SIZE = 64 * 1024`)

### Structure

The shared memory region holds a serialized `GameState` snapshot. The DLL writes this once per frame tick (~6 second EQ server tick); the orchestrator reads it without locking.

```
[N bytes: bincode-serialized GameState]
```

**Write Pattern** (DLL):

1. Serialize current `GameState` to a temporary buffer
2. Atomic swap the buffer into shared memory (write-once per frame)
3. No explicit locking — single writer, multiple readers

**Read Pattern** (Orchestrator):

1. Open shared memory by name
2. Read the entire region (may see partial/stale data during DLL write)
3. Deserialize to `GameState` if valid, else use previous frame

## Commands

All commands are sent via the named pipe in an `IpcCommand` envelope.

### Movement

#### MoveTo

Navigate to absolute world coordinates.

```rust
MoveTo { x: f32, y: f32, z: f32 }
```

#### StopMovement

Halt all active movement.

```rust
StopMovement
```

#### NavigateTo

Follow a sequence of waypoints.

```rust
NavigateTo { waypoints: Vec<Waypoint> }
```

#### SetCamp

Move to a camp spot and face a heading.

```rust
SetCamp { spot: CampSpot }
```

#### SetCampConfig

Set up a full camp with scatter positioning.

```rust
SetCampConfig { config: NavCampConfig }
```

#### StopNavigation

Stop navigating immediately.

```rust
StopNavigation
```

#### NavPause / NavResume

Pause and resume active navigation.

```rust
NavPause
NavResume
```

#### NavLoc

Navigate to specific coordinates (`/nav loc`).

```rust
NavLoc { x: f32, y: f32, z: f32 }
```

#### NavTarget / NavDoor / NavItem

Navigate to current target, nearest door, or nearest ground item.

```rust
NavTarget
NavDoor
NavItem
```

#### NavReload

Reload the navmesh for the current zone.

```rust
NavReload
```

#### NavWaypointSave

Save a named waypoint at current position.

```rust
NavWaypointSave { name: String }
```

#### NavWaypointRecall

Navigate to a previously saved waypoint.

```rust
NavWaypointRecall { name: String }
```

#### NavWaypointList

Query all saved waypoints.

```rust
NavWaypointList
```

Response: `Response::NavWaypointList { waypoints: Vec<NamedWaypoint> }`

#### NavWaypointDelete

Delete a saved waypoint.

```rust
NavWaypointDelete { name: String }
```

#### NavSignalsQuery

Query navigation state signals.

```rust
NavSignalsQuery
```

Response: `Response::NavSignals { signals: NavStateSignals }`

#### NavDiagnosticsQuery

Query navigation diagnostics for debug overlay.

```rust
NavDiagnosticsQuery
```

Response: `Response::NavDiagnosticsResult { diagnostics: NavDiagnostics }`

#### FollowPlayer

Start MQ2MoveUtils-style player follow mode.

```rust
FollowPlayer {
    config: FollowConfig,  // leader name, follow distance, leash distance
    anchor_x: f32,
    anchor_y: f32,
    anchor_z: f32,
}
```

#### UpdateFollowAnchor

Update anchor position during active follow.

```rust
UpdateFollowAnchor { x: f32, y: f32, z: f32 }
```

#### UpdateFollowConfig

Update follow mode options without restarting.

```rust
UpdateFollowConfig { config: FollowConfig }
```

#### StopFollow

Stop follow mode.

```rust
StopFollow
```

### Targeting

#### SetTarget

Set the current target by spawn ID.

```rust
SetTarget { spawn_id: u32 }
```

#### ClearTarget

Clear the current target.

```rust
ClearTarget
```

#### InteractTarget

Right-click interact with the current target (NPC, door, or object).

```rust
InteractTarget
```

#### InteractDoor

Target and activate the nearest door or switch.

```rust
InteractDoor
```

#### ClickObject

Click the nearest ground item or world object.

```rust
ClickObject
```

### Combat

#### CastSpell

Cast a memorized spell, optionally on a specific target.

```rust
CastSpell {
    spell_slot: u8,      // 1-13 (gem number)
    target_id: Option<u32>,  // None = current target
    #[serde(default)]
    kill: bool,          // Keep casting until target dies
    #[serde(default)]
    recast: u8,          // 0-255: number of retries (0 = once)
}
```

**Validation**: `kill=true` and `recast>0` are mutually exclusive.

**Recast Behavior**: Exponential backoff between retries (base 8 ticks, cap 30 ticks).

#### CancelCastLoop

Cancel an active kill-loop or recast-loop.

```rust
CancelCastLoop
```

#### Attack

Begin auto-attack on a target.

```rust
Attack { target_id: u32 }
```

#### StopAttack

Stop auto-attack.

```rust
StopAttack
```

#### CombatEngage

Engage a target via the combat FSM.

```rust
CombatEngage { target_id: u32 }
```

#### CombatDisengage

Disengage from combat.

```rust
CombatDisengage
```

#### CombatSetAssistTarget

Set the main assist target.

```rust
CombatSetAssistTarget { spawn_id: u32 }
```

#### CombatForceAbility

Force-use a specific combat ability.

```rust
CombatForceAbility { ability_id: u32 }
```

#### CombatEmergencyHeal

Emergency heal a specific target.

```rust
CombatEmergencyHeal { target_id: u32 }
```

#### HealClaimTarget

Claim a heal target (prevents double-healing).

```rust
HealClaimTarget {
    healer_id: u32,
    target_id: u32,
    cast_time_ms: u32,
}
```

#### HealReleaseClaim

Release a heal claim.

```rust
HealReleaseClaim {
    healer_id: u32,
    target_id: u32,
}
```

### Utility

#### Sit / Stand

Sit down or stand up.

```rust
Sit
Stand
```

#### LootCorpse / LootAll

Loot the nearest corpse or all items from an open loot window.

```rust
LootCorpse
LootAll
```

### Login Automation

#### LoginPhaseQuery

Query the current login phase from the DLL.

```rust
LoginPhaseQuery
```

Response: `Response::LoginPhaseUpdate { phase: LoginPhase }`

#### CalibrateLogin

Dump all login-related pointer addresses to the DLL log.

```rust
CalibrateLogin
```

#### StartLogin

Start the automated login sequence.

```rust
StartLogin {
    account_name: String,
    password: String,      // Zeroized after use in DLL
    server_name: String,
    character_name: String,
}
```

Response: Series of `Response::LoginPhaseUpdate` messages.

### Post-Login

#### ReportReady

Report that this client is ready for orchestration.

```rust
ReportReady
```

#### ApplyBuffs

Apply standard buff rotation.

```rust
ApplyBuffs
```

#### JoinGroup

Join a group by group ID.

```rust
JoinGroup { group_id: u32 }
```

### Navigation Advanced

#### MoveToAdvanced

Advanced moveto with full option support.

```rust
MoveToAdvanced { config: MoveToConfig }
```

#### SetAutopause

Enable or disable autopause globally.

```rust
SetAutopause { enabled: bool }
```

#### SetBreakOnGm

Enable or disable break-on-GM safety halt.

```rust
SetBreakOnGm { enabled: bool }
```

#### SetHeadingMode

Set the heading update mode during navigation.

```rust
SetHeadingMode { mode: HeadingMode }
```

#### StickTo

Start sticking to a target (MQ2MoveUtils `/stick`).

```rust
StickTo { config: StickConfig }
```

#### StickOff

Stop sticking.

```rust
StickOff
```

#### StickMod

Adjust stick distance modifier.

```rust
StickMod { delta: f32 }
```

#### CircleKite

Start circle-kiting mode.

```rust
CircleKite { config: CircleConfig }
```

#### CircleOff

Stop circle-kiting.

```rust
CircleOff
```

### Rendering & Capture

#### SetRenderMode

Set the rendering mode (Normal, Strobe, NullRender).

```rust
SetRenderMode { mode: RenderMode }
```

Response: `Response::RenderModeChanged { mode }`

#### CaptureScreenshot

Capture a screenshot in NullRender mode.

```rust
CaptureScreenshot
```

Response: `Response::ScreenshotCaptured { path }` or `Response::ScreenshotFailed { reason }`

### Utilities

#### Ping

Heartbeat ping.

```rust
Ping
```

Response: `Response::Pong { client_id, timestamp_ms }`

#### Eject

Terminate the injected DLL session.

```rust
Eject
```

#### QueryZoneGraph

Request the zone adjacency graph.

```rust
QueryZoneGraph
```

Response: `Response::ZoneGraph { zones }`

#### SlashCommand

Execute a slash command (`/target Mob`, `/follow`, etc.).

```rust
SlashCommand { command: String }
```

#### Say / Emote

Send chat or perform an emote.

```rust
Say {
    channel: SayChannel,
    message: String,
    target: Option<String>,  // For tells
}

Emote { emote: String }
```

#### SoulAction

Execute a soul action (idle behavior, etc.).

```rust
SoulAction { action: SoulAction }
```

#### PollPackets / PollSpawnEvents / PollChat

Poll for batched packet/spawn/chat events.

```rust
PollPackets
PollSpawnEvents
PollChat
```

Responses: `Response::PacketBatch`, `Response::SpawnEventBatch`, `Response::ChatBatch`

#### FlushMovementQueue

Flush pending movements from the queue.

```rust
FlushMovementQueue
```

Response: `Response::MovementQueueFlushed { count_dropped }`

## Responses

All responses are sent via the named pipe in an `IpcResponse` envelope.

### State Notifications

#### Pong

Heartbeat response to Ping.

```rust
Pong {
    client_id: ClientId,
    timestamp_ms: u64,
}
```

#### CommandResult

Generic success/error result.

```rust
CommandResult {
    success: bool,
    message: String,
}
```

#### Error

Error response.

```rust
Error { message: String }
```

#### NavUpdate

Navigation FSM state transition.

```rust
NavUpdate { status: NavStatus }
```

**Note**: `NavStatus` is also authoritative in shared memory `GameState.nav_status`, updated every tick. `NavUpdate` is sent only on state transitions for low-latency notification.

#### LoginPhaseUpdate

Login phase transition.

```rust
LoginPhaseUpdate { phase: LoginPhase }
```

#### PostLoginComplete

Client finished post-login setup.

```rust
PostLoginComplete { client_id: ClientId }
```

#### CombatUpdate

Combat FSM state transition.

```rust
CombatUpdate { status: CombatStatus }
```

#### GameStateChanged

CEverQuest::SetGameState transition.

```rust
GameStateChanged { state: GameState }
```

#### NavWaypointList

All saved named waypoints.

```rust
NavWaypointList { waypoints: Vec<NamedWaypoint> }
```

#### NavSignals

Current navigation state signals.

```rust
NavSignals { signals: NavStateSignals }
```

#### NavDiagnosticsResult

Navigation diagnostics for debug overlay.

```rust
NavDiagnosticsResult { diagnostics: NavDiagnostics }
```

#### RenderModeChanged

Confirmation that render mode was changed.

```rust
RenderModeChanged { mode: RenderMode }
```

#### ScreenshotCaptured / ScreenshotFailed

Screenshot result.

```rust
ScreenshotCaptured { path: String }
ScreenshotFailed { reason: String }
```

### Events

#### PacketBatch

Batched packet events.

```rust
PacketBatch { events: Vec<PacketEventInfo> }
```

Where `PacketEventInfo` includes:

- `client_id: ClientId`
- `opcode: u16`
- `direction: PacketDirection` (Inbound/Outbound)
- `timestamp_ms: u64`
- `payload_size: u32`

#### PacketEvent (Notification)

Single packet event (sent on-demand, not batched).

```rust
PacketEvent {
    client_id: ClientId,
    opcode: u16,
    direction: PacketDirection,
    timestamp_ms: u64,
    payload_size: u32,
}
```

#### ChatBatch

Batched chat messages.

```rust
ChatBatch { messages: Vec<ChatMessageInfo> }
```

Where `ChatMessageInfo` includes:

- `text: String`
- `color: i32`
- `timestamp_ms: u64`

#### ChatMessage (Notification)

Single chat message (sent on-demand).

```rust
ChatMessage {
    text: String,
    color: i32,
    timestamp_ms: u64,
    parsed: Option<ChatEvent>,
}
```

#### SpawnEventBatch

Batched spawn list delta events.

```rust
SpawnEventBatch { events: Vec<SpawnEvent> }
```

Where `SpawnEvent` includes:

- `client_id: ClientId`
- `zone: String`
- `spawn_name: String`
- `kind: SpawnEventKind` (Created/Destroyed/Updated)
- `timestamp_ms: u64`

#### SpawnAlert

Watched/named spawn appeared/disappeared.

```rust
SpawnAlert {
    client_id: ClientId,
    zone: String,
    spawn_name: String,
    is_up: bool,
    timestamp_ms: u64,
}
```

### Diagnostics & Metadata

#### ContextMenuState

All menus visible in CContextMenuManager.

```rust
ContextMenuState { menus: Vec<ContextMenuInfo> }
```

#### ContextMenuActivated

Context menu item dispatch confirmation.

```rust
ContextMenuActivated {
    success: bool,
    message: String,
}
```

#### ContainerSlots

Open inventory container slots.

```rust
ContainerSlots { slots: Vec<ContainerSlotInfo> }
```

#### MemoryData

Raw bytes read from process address space.

```rust
MemoryData {
    address: usize,
    bytes: Vec<u8>,
}
```

#### ZoneGraph

Zone adjacency graph.

```rust
ZoneGraph { zones: Vec<ZoneGraphEntry> }
```

Where `ZoneGraphEntry = (u16, String, i32, i32, Vec<(u16, u8, bool)>)`:

- `zone_id`, `name`, `min_level`, `max_level`, `[(dest_zone_id, transfer_type, disabled)]`

#### ZoneStateChanged

Zone state transition completion.

```rust
ZoneStateChanged {
    zone_id: u32,
    old_state: String,
    new_state: String,
    timestamp: u64,
}
```

#### ZoneValidationFailed

Zone validation failure.

```rust
ZoneValidationFailed {
    zone_id: u32,
    error_code: i32,
    reason: String,
}
```

#### SafeCoordsRetrieved

Safe spawn point coordinates for a zone.

```rust
SafeCoordsRetrieved {
    x: f32,
    y: f32,
    z: f32,
    zone_id: u32,
}
```

#### RelogProgress

Relog phase transition.

```rust
RelogProgress { phase: RelogPhase }
```

#### SwitchResult

Result of SwitchServer or SwitchCharacter.

```rust
SwitchResult {
    success: bool,
    message: String,
}
```

#### MovementQueueFlushed

Movement queue flush confirmation.

```rust
MovementQueueFlushed { count_dropped: u32 }
```

## Timeouts & Reliability

### Pipe Write Timeout

- **Default**: 5000 ms (Windows named pipe default)
- Configurable via pipe handle options

### Pipe Read Timeout

- **Default**: Blocking (no timeout)
- Caller-configurable via pipe handle options

### Shared Memory Read

- **No timeout**: Always completes immediately
- **Staleness**: May read data up to one tick (6 seconds) old

### Retry Strategy

Commands without guaranteed delivery (fire-and-forget) should be resent if:

- No `CommandResult` response received within N seconds
- Connection is lost and re-established
- Client crashes and re-injects

## Error Codes

Errors are communicated via:

1. `Response::Error { message }` — Generic error with description
2. `Response::CommandResult { success: false, message }` — Command-specific failure
3. Return values in DLL logs and orchestrator logs

Common error scenarios:

- **Invalid spawn ID**: Target not found in spawn list
- **Memory access failed**: Address not readable
- **Zone validation failed**: Zone inaccessible or invalid
- **Login failed**: Bad credentials, server unavailable
- **Navigation failed**: Navmesh not loaded, path not found
- **Combat failed**: Invalid ability ID, insufficient resources

## Versioning Strategy

### Major Version Changes

Break IPC compatibility when:

- Command enum variants are removed or renamed
- Response enum variants are removed or renamed
- Serialization format changes fundamentally

**Migration**: Orchestrator and DLL must be updated in lockstep.

### Minor Version Changes

Backward-compatible additions:

- New `Command` variants (DLL ignores unknown commands)
- New `Response` variants (Orchestrator ignores unknown responses)
- New optional fields in structs (with `#[serde(default)]`)

**Deployment**: DLL can be updated independently; orchestrator can be older.

### Versioning Header (Recommended Future)

Add a version field to the IpcCommand/IpcResponse envelope:

```rust
pub struct IpcCommand {
    pub version: u16,  // 1 = current
    pub command: Command,
    pub correlation_id: Option<u64>,
}
```

Allows graceful degradation when orchestrator and DLL versions drift.

## References

- Command/Response definitions: `textquest-common/src/ipc.rs`
- Protocol encode/decode: `textquest-common/src/protocol.rs`
- Shared memory state: `textquest-common/src/types.rs` (GameState)
- Navigation types: `textquest-common/src/nav.rs`
- Combat types: `textquest-common/src/combat.rs`
- Login types: `textquest-common/src/login.rs`
