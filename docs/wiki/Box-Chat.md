# Box-Chat

TextQuest box-chat is the EQBC-style relay surface for cross-character
coordination. It currently has two wire modes:

- TextQuest JSON lines for native peers.
- Legacy EQBC command lines for simple external-tool compatibility.

## Configuration

`[box_chat]` in `config/textquest.toml` controls the runtime:

```toml
[box_chat]
enabled = true
host = "127.0.0.1"
port = 2112
auto_connect = false
```

The integrated relay binds loopback only. Use `auto_connect = true` when this
process should connect to an upstream relay instead of acting only as the local
hub.

## Native JSON Lines

Native peers send one JSON object per line. The supported message types are:

| Type | Purpose |
| --- | --- |
| `hello` | Register a node and its character names |
| `update_characters` | Refresh owned character names |
| `broadcast` | Submit a broadcast slash command |
| `target` | Submit a targeted slash command |
| `tell_forward` | Forward a tell event for display or logging |
| `channel_broadcast` | Forward a named channel event for display or logging |
| `box_controller_command` | Relay unified box-controller state commands |
| `box_controller_state` | Publish current box-controller state |
| `execute_broadcast` | Execute a broadcast command on receivers |
| `execute_target` | Execute a targeted command on receivers |

Examples:

```json
{"type":"broadcast","command":"/assist MainTank"}
{"type":"target","character":"Cleric01","command":"/cast 1"}
{"type":"tell_forward","from":"MainTank","to":"Cleric01","message":"CH now"}
```

## Legacy EQBC Lines

When a peer sends a legacy line first, the relay treats that peer as an
EQBC-line peer and returns command traffic as lines instead of JSON. Supported
incoming aliases are:

| Line | Native route |
| --- | --- |
| `bc //sit` | Broadcast `/sit` |
| `/bc /assist MainTank` | Broadcast `/assist MainTank` |
| `bca //stand` | Broadcast `/stand` |
| `bcaa //sit` | Broadcast `/sit` |
| `bct Cleric01 //cast 1` | Target `Cleric01` with `/cast 1` |

This is a compatibility scaffold. It accepts and emits the common EQBC command
family, but it does not yet implement the full OpenVanilla/MQ2EQBC handshake,
names list protocol, or external EQBCS authentication behavior.
