# Python Quick Start

This guide shows how to use the Python SDK to interact with TextQuest-managed EQ clients.

## Prerequisites

- Python 3.10+
- Windows (for live EQ integration)
- A running TextQuest orchestrator

## Installation

```bash
pip install textquest
```

## Basic Usage

### Connecting to a Client

```python
from textquest import Client, Session

# Create a session from the token file
session = Session.from_token_file("C:\\Temp\\textquest\\deadbeef.token")
client = Client(session)
```

### Sending Commands

```python
# Navigate to a location
client.nav_loc(100.0, 200.0, 50.0)

# Cast a spell (gem 1, on current target)
client.cast_spell(slot=1)

# Sit down
client.sit()
```

### Reading Game State

```python
# Get current game state from shared memory
state = client.get_state()
print(f"Zone: {state.zone_name}")
print(f"Position: ({state.x}, {state.y}, {state.z})")
```

## Async Usage

For high-throughput applications, use the async client:

```python
import asyncio
from textquest import AsyncClient, Session

async def main():
    session = Session.from_token_file("C:\\Temp\\textquest\\deadbeef.token")
    async with AsyncClient(session) as client:
        # Send multiple commands concurrently
        await asyncio.gather(
            client.nav_loc(100.0, 200.0, 50.0),
            client.cast_spell(slot=1),
        )
        
        # Read state
        state = await client.get_state()
        print(f"Zone: {state.zone_name}")

asyncio.run(main())
```

## Command Examples

### Movement

```python
# Move to absolute coordinates
client.move_to(x=100.0, y=200.0, z=50.0)

# Navigate to location
client.nav_loc(100.0, 200.0, 50.0)

# Stop all movement
client.stop_movement()
```

### Combat

```python
# Set target
client.set_target(spawn_id=1234)

# Begin attack
client.attack(target_id=1234)

# Stop attack
client.stop_attack()

# Cast spell on target
client.cast_spell(slot=1, target_id=1234)

# Emergency heal
client.combat_emergency_heal(target_id=5678)
```

### Slash Commands

```python
# Execute any slash command
client.slash_command("/target MyTank")
client.slash_command("/follow")
client.slash_command("/sit")
```

### Chat

```python
from textquest import SayChannel

# Send a say message
client.say(channel=SayChannel.SAY, message="Hello world!")

# Send a tell
client.say(channel=SayChannel.TELL, message="Hi!", target="PlayerName")
```

## Error Handling

```python
from textquest import TextQuestError

try:
    client.nav_loc(100.0, 200.0, 50.0)
except TextQuestError as e:
    print(f"Command failed: {e}")
except ConnectionError:
    print("Lost connection to client")
```

## Known Limitations

- Command/group API features require the M8 Orchestrator prerequisite
- Shared memory state may be up to one tick (6 seconds) stale
- Named pipe operations have a 5-second timeout by default

## Next Steps

- [API Reference](api-reference.md)
- [Full Command Reference](commands.md)
