# Rust Quick Start

This guide shows how to use the `textquest-common` crate to interact with TextQuest-managed EQ clients.

## Prerequisites

- Rust 1.75+
- A running TextQuest orchestrator with at least one injected EQ client

## Basic Setup

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
textquest-common = "0.6"
```

## Connecting to a Client

The IPC connection uses named pipes. First, open the command pipe:

```rust
use textquest_common::ipc::{Command, IpcCommand, CorrelationIdGenerator};
use std::io::Write;

// Session derived from the token file
let session_id: u64 = 0xdeadbeef;
let client_id: u32 = 1234;

let pipe_name = format!(r"\\.\pipe\{:x}_cmd_{}", session_id, client_id);
let mut pipe = NamedPipe::connect(&pipe_name).expect("failed to connect");
```

## Sending Commands

Wrap commands with optional correlation IDs for request-response tracking:

```rust
let mut id_gen = CorrelationIdGenerator::new();

// Send a navigation command
let cmd = IpcCommand::with_correlation(
    Command::NavLoc { x: 100.0, y: 200.0, z: 50.0 },
    id_gen.next_id(),
);

let encoded = bincode::serialize(&cmd).unwrap();
pipe.write_all(&encoded).unwrap();
```

## Receiving Responses

```rust
use textquest_common::ipc::IpcResponse;

// Read response
let mut len_buf = [0u8; 4];
pipe.read_exact(&mut len_buf).unwrap();
let len = u32::from_le_bytes(len_buf) as usize;

let mut data = vec![0u8; len];
pipe.read_exact(&mut data).unwrap();

let response: IpcResponse = bincode::deserialize(&data).unwrap();
println!("Response: {:?}", response);
```

## Common Operations

### Move to Location

```rust
use textquest_common::ipc::Command;

let cmd = Command::MoveTo { x: 100.0, y: 200.0, z: 50.0 };
send_command(pipe, cmd);
```

### Cast a Spell

```rust
use textquest_common::ipc::Command;

let cmd = Command::CastSpell {
    spell_slot: 1,           // Gem 1
    target_id: Some(1234),  // Optional target spawn ID
    kill: false,
    recast: 0,
};
send_command(pipe, cmd);
```

### Sit/Stand

```rust
use textquest_common::ipc::Command;

// Sit down
send_command(pipe, Command::Sit);

// Stand up
send_command(pipe, Command::Stand);
```

### Execute Slash Command

```rust
use textquest_common::ipc::Command;

send_command(pipe, Command::SlashCommand {
    command: "/target MyTank".to_string(),
});
```

## Reading Game State

Game state is published via shared memory. Read it without locking:

```rust
use textquest_common::types::GameState;

let shmem_name = format!("{:x}_state_{}", session_id, client_id);
// Map the shared memory region and deserialize GameState
let state: GameState = read_shared_memory(&shmem_name)?;
```

## Error Handling

Always handle potential failures:

```rust
use anyhow::Result;

fn send_command(pipe: &mut NamedPipe, cmd: Command) -> Result<()> {
    let ipc_cmd = IpcCommand::new(cmd);
    let encoded = bincode::serialize(&ipc_cmd)
        .context("serialization failed")?;
    
    pipe.write_all(&encoded)
        .context("pipe write failed")?;
    
    Ok(())
}
```

## Next Steps

- [API Reference](api-reference.md)
- [Full Command Reference](commands.md)
- [IPC Protocol Specification](https://github.com/Maleick/TextQuest/blob/master/docs/specs/ipc-protocol.md)
