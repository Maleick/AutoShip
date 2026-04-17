# TypeScript Quick Start

This guide shows how to use the `@textquest/client` package to interact with TextQuest-managed EQ clients.

## Prerequisites

- Node.js 18+
- TypeScript 5.0+ (optional, for type checking)
- Windows (for live EQ integration)

## Installation

```bash
npm install @textquest/client
# or
yarn add @textquest/client
# or
pnpm add @textquest/client
```

## Basic Usage

### Connecting to a Client

```typescript
import { Client, Session } from '@textquest/client';

// Create a session from the token file
const session = Session.fromTokenFile('C:\\Temp\\textquest\\deadbeef.token');
const client = new Client(session);
```

### Sending Commands

```typescript
// Navigate to a location
await client.navLoc({ x: 100, y: 200, z: 50 });

// Cast a spell (gem 1, on current target)
await client.castSpell({ spellSlot: 1 });

// Sit down
await client.sit();
```

### Reading Game State

```typescript
// Get current game state from shared memory
const state = await client.getState();
console.log(`Zone: ${state.zoneName}`);
console.log(`Position: (${state.x}, ${state.y}, ${state.z})`);
```

## Async/Await Pattern

```typescript
import { Client, Session } from '@textquest/client';

async function main() {
  const session = Session.fromTokenFile('C:\\Temp\\textquest\\deadbeef.token');
  const client = new Client(session);
  
  try {
    // Send multiple commands
    await Promise.all([
      client.navLoc({ x: 100, y: 200, z: 50 }),
      client.castSpell({ spellSlot: 1 }),
    ]);
    
    // Read state
    const state = await client.getState();
    console.log(`Zone: ${state.zoneName}`);
  } finally {
    client.close();
  }
}

main();
```

## Command Examples

### Movement

```typescript
// Move to absolute coordinates
await client.moveTo({ x: 100, y: 200, z: 50 });

// Navigate to location
await client.navLoc({ x: 100, y: 200, z: 50 });

// Stop all movement
await client.stopMovement();
```

### Combat

```typescript
// Set target
await client.setTarget({ spawnId: 1234 });

// Begin attack
await client.attack({ targetId: 1234 });

// Stop attack
await client.stopAttack();

// Cast spell on target
await client.castSpell({ spellSlot: 1, targetId: 1234 });
```

### Slash Commands

```typescript
// Execute any slash command
await client.slashCommand({ command: '/target MyTank' });
await client.slashCommand({ command: '/follow' });
await client.slashCommand({ command: '/sit' });
```

### Chat

```typescript
import { SayChannel } from '@textquest/client';

// Send a say message
await client.say({
  channel: SayChannel.Say,
  message: 'Hello world!',
});

// Send a tell
await client.say({
  channel: SayChannel.Tell,
  message: 'Hi!',
  target: 'PlayerName',
});
```

## Event Handling

```typescript
import { Client, Session } from '@textquest/client';

const session = Session.fromTokenFile('C:\\Temp\\textquest\\deadbeef.token');
const client = new Client(session);

// Listen for state changes
client.on('stateUpdate', (state) => {
  console.log(`Zone changed: ${state.zoneName}`);
});

// Listen for chat messages
client.on('chat', (message) => {
  console.log(`[${message.channel}] ${message.sender}: ${message.text}`);
});

// Listen for errors
client.on('error', (error) => {
  console.error('Connection error:', error);
});
```

## Error Handling

```typescript
import { TextQuestError } from '@textquest/client';

try {
  await client.navLoc({ x: 100, y: 200, z: 50 });
} catch (error) {
  if (error instanceof TextQuestError) {
    console.log(`Command failed: ${error.message}`);
  } else {
    throw error;
  }
}
```

## Known Limitations

- Command/group API features require the M8 Orchestrator prerequisite
- Shared memory state may be up to one tick (6 seconds) stale
- Named pipe operations have a 5-second timeout by default

## Next Steps

- [API Reference](api-reference.md)
- [Full Command Reference](commands.md)
