# Cross-Client Communication Transports

TextQuest supports two primary cross-client transports (EQBC and DanNet) plus
a NetBots-style vitals layer. Choose based on your setup and macro suite.

## Transport Selection

| Transport        | Use when                                                                  |
| ---------------- | ------------------------------------------------------------------------- |
| `eqbc` (default) | Legacy `/bc`/`/bca`/`/bct` multibox setups, broadest client compatibility |
| `dannet`         | rgmercs or macros that use `/dnet`, `/dgae`, `${DanNet[...]}`             |
| `both`           | Mixed fleet — some clients use EQBC macros, others use rgmercs            |

Configure via the web dashboard at **Transport → Config** or in `config/transport.toml`.

## EQBC Transport

### Overview

TextQuest ships a native EQBCS-compatible TCP hub server. You do **not** need
to run the legacy MQ2EQBC server binary.

### Configuration

```toml
[transport]
kind = "eqbc"

[transport.eqbc]
serve        = true          # Run as hub server (one instance per LAN)
host         = "127.0.0.1"  # Hub address (clients point here)
port         = 2112
auto_connect = true
```

### Commands

| Command             | Equivalent          | Effect                           |
| ------------------- | ------------------- | -------------------------------- |
| `/bc <cmd>`         | `EqbcClient::bc()`  | Broadcast to all connected peers |
| `/bca <cmd>`        | `EqbcClient::bc()`  | Alias for `/bc`                  |
| `/bct <name> <cmd>` | `EqbcClient::bct()` | Send to one named peer           |

### Web API

```
GET  /api/transport/config    — read config
PUT  /api/transport/config    — update config
GET  /api/transport/peers     — list connected peers
POST /api/transport/bc        — issue /bc command
POST /api/transport/bct       — issue /bct command
```

## DanNet Transport

### Overview

Peer-to-peer UDP transport compatible with rgmercs' DanNet requirements.
Uses JSON-framed UDP datagrams with multicast discovery.

### Configuration

```toml
[transport]
kind = "dannet"

[transport.dannet]
port            = 2114
group           = "all"
multicast_group = "239.255.0.1"
# Static peers for cross-machine setups (multicast doesn't cross subnets):
peers = ["192.168.1.10:2114", "192.168.1.11:2114"]
```

### Commands

| Command                          | Effect                                  |
| -------------------------------- | --------------------------------------- |
| `/dgae <cmd>`                    | Execute on all peers in my server group |
| `/dggaexecute <group> <cmd>`     | Execute on peers in named group         |
| `/dnet observe <peer> <query>`   | Subscribe to a TLO value                |
| `/dnet unobserve <peer> <query>` | Stop observing                          |
| `/dnet peers`                    | List known DanNet peers                 |

### TLO Compatibility

`${DanNet[<peer>].Q[<query>]}` is supported via the DLL's `dannet_tlo` module.

```
; In an MQ2 macro or rgmercs lua:
/echo ${DanNet[Alice].Q[Me.HP]}   ; returns Alice's current HP%
/echo ${DanNet[Bob].Q[Me.Mana]}   ; returns Bob's current mana%
```

Values are populated by `observe` requests; unobserved queries return `NULL`.

### Web API

```
POST /api/transport/dgae      — issue /dgae command
```

## NetBots Vitals Broadcast

Each instance publishes its character vitals within **100 ms** of any change.

### Published fields

| Field           | Description                                    |
| --------------- | ---------------------------------------------- |
| `hp_pct`        | Hit-point percentage                           |
| `mana_pct`      | Mana percentage                                |
| `end_pct`       | Endurance percentage                           |
| `level`         | Current level                                  |
| `class`         | Class abbreviation                             |
| `zone`          | Zone short name                                |
| `target_name`   | Current target name                            |
| `target_hp_pct` | Current target HP%                             |
| `buff_ids`      | Active spell IDs (when `include_buffs = true`) |

### Healer module integration

The healer module uses `HealTargetSelector::lowest_hp_target()` to find the
group member with the lowest HP in `VitalsRegistry`.

### Web API

```
GET /api/transport/vitals     — snapshot of all known peer vitals
```

## NetMQ

**Status: Deferred.** ZeroMQ-based transport — no live demand found. Use
EQBC or DanNet instead. See `textquest-net/src/netmq.rs` for rationale and
upgrade path.

## Cross-Machine Setup

For instances on different machines:

1. Pick one machine as the EQBC hub (`serve = true`).
2. All other machines set `host` to the hub's IP.
3. For DanNet across subnets, list static peer addresses under `peers`.
4. Ensure port 2112 (EQBC) and 2114 (DanNet) are open between machines.
