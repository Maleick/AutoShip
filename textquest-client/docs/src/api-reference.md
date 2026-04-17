# API Reference

## Rust SDK (`textquest-common`)

### Core Types

#### `ipc::IpcCommand`

Wraps a command with an optional correlation ID for request-response tracking.

```rust
pub struct IpcCommand {
    pub command: Command,
    pub correlation_id: Option<u64>,
}

impl IpcCommand {
    pub fn new(command: Command) -> Self
    pub fn with_correlation(command: Command, correlation_id: u64) -> Self
}
```

#### `ipc::IpcResponse`

Response wrapper that echoes the correlation ID from the originating command.

```rust
pub struct IpcResponse {
    pub response: Response,
    pub correlation_id: Option<u64>,
}
```

#### `ipc::CorrelationIdGenerator`

Generates monotonically increasing correlation IDs.

```rust
pub struct CorrelationIdGenerator {
    next: AtomicU64,
}

impl CorrelationIdGenerator {
    pub fn new() -> Self
    pub fn next_id(&self) -> u64
}
```

### IPC Endpoints

#### Named Pipe Format

```
\\.\pipe\{session_id:x}_cmd_{client_id}
```

#### Shared Memory Format

```
{session_id:x}_state_{client_id}
```

### Serialization

- **Format**: bincode (binary serde)
- **Byte Order**: Little-endian
- **Wire Framing**: 4-byte length prefix

## Python SDK (`textquest`)

### Classes

#### `Client`

Synchronous client for TextQuest IPC.

```python
class Client:
    def __init__(self, session: Session)
    def nav_loc(self, x: float, y: float, z: float) -> None
    def cast_spell(self, slot: int, target_id: int | None = None) -> None
    def sit(self) -> None
    def get_state(self) -> GameState
    def close(self) -> None
```

#### `AsyncClient`

Asynchronous client for high-throughput applications.

```python
class AsyncClient:
    async def __aenter__(self) -> AsyncClient
    async def __aexit__(self, *args) -> None
    async def nav_loc(self, x: float, y: float, z: float) -> None
    async def cast_spell(self, slot: int, target_id: int | None = None) -> None
    async def get_state(self) -> GameState
```

#### `Session`

IPC session configuration derived from token files.

```python
class Session:
    @classmethod
    def from_token_file(cls, path: str) -> Session
    @property
    def session_id(self) -> int
    @property
    def client_id(self) -> int
```

#### `GameState`

Current game state snapshot from shared memory.

```python
class GameState:
    zone_name: str
    x: float
    y: float
    z: float
    heading: float
    zone_id: int
    spawn_id: int
```

### Exceptions

```python
class TextQuestError(Exception):
    """Base exception for TextQuest SDK errors."""
    pass

class ConnectionError(TextQuestError):
    """Failed to connect to IPC endpoint."""
    pass

class TimeoutError(TextQuestError):
    """Command timed out."""
    pass
```

## TypeScript SDK (`@textquest/client`)

### Classes

#### `Client`

Main client for TextQuest IPC communication.

```typescript
class Client {
  constructor(session: Session);
  navLoc(params: { x: number; y: number; z: number }): Promise<void>;
  castSpell(params: { spellSlot: number; targetId?: number }): Promise<void>;
  sit(): Promise<void>;
  getState(): Promise<GameState>;
  close(): void;
}
```

#### `Session`

IPC session configuration.

```typescript
class Session {
  static fromTokenFile(path: string): Session;
  get sessionId(): bigint;
  get clientId(): number;
}
```

#### `GameState`

Current game state snapshot.

```typescript
interface GameState {
  zoneName: string;
  x: number;
  y: number;
  z: number;
  heading: number;
  zoneId: number;
  spawnId: number;
}
```

### Enums

```typescript
enum SayChannel {
  Say = 0,
  Tell = 1,
  Group = 2,
  Raid = 3,
  Shout = 4,
}
```

### Events

```typescript
client.on('stateUpdate', (state: GameState) => void);
client.on('chat', (message: ChatMessage) => void);
client.on('error', (error: Error) => void);
```
