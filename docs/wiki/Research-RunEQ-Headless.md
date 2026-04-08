# RunEQ Headless EverQuest Client -- Binary Analysis

> **Binary:** `runeq.exe` v0.4.6 by xackery (github.com/xackery/runeq -- repo now private/deleted)
> **Format:** PE32+ x86-64, Go 1.21.5, Windows console application
> **Symbol count:** ~1,328 symbols from the `runeq` module alone
> **Analysis date:** 2026-04-04
> **Method:** Static string and symbol extraction from unstripped Go binary

---

## 1. Executive Summary

RunEQ is a fully headless EverQuest client written in Go that implements the EQ network protocol from scratch. It connects directly to login/world/zone servers over UDP, negotiates the EQStream session layer, authenticates, selects a server, enters the world, and zones in -- all without eqgame.exe. Once in-zone, it maintains a spawn manager, processes packets (spawns, HP updates, damage, chat, spell casts), and exposes an interactive prompt with commands like `/attack`, `/cast`, `/warp`, `/target`, `/follow`, `/loot`, and `/memorize`.

The binary targets the **Rain of Fear (RoF)** client protocol version, includes `gopacket` for packet capture/analysis, and uses `bubbletea`/`tview` for its terminal UI. It has a simulation layer (`sim` package) that mirrors the zone client's state, including a `SpawnManager` for tracking all entities.

**Key takeaway for TextQuest:** RunEQ proves the EQ protocol is fully implementable outside eqgame.exe. Its packet definitions, opcode table, protocol flow, and EQStream layer design are directly applicable to TextQuest's future packet sniffer (M6), potential headless-client experimentation, and protocol understanding for the injected DLL's network hooks.

---

## 2. Architecture Overview

### Package Structure

RunEQ is organized into 24 packages:

| Package                  | Purpose                                                                                 |
| ------------------------ | --------------------------------------------------------------------------------------- |
| `client`                 | Top-level client orchestration, interactive login flow                                  |
| `config`                 | TOML configuration loading/saving (`runeq.conf`)                                        |
| `eqstream`               | EQStream protocol layer -- session management, encryption, CRC, sequencing              |
| `filelog`                | File-based logging                                                                      |
| `library`                | Spell database loader, expansion name mapping                                           |
| `linker`                 | Item/spell link resolution                                                              |
| `log`                    | Structured logging (Debug/Info/Warn/Error levels)                                       |
| `login`                  | Login server client -- authentication, server list, play request                        |
| `packet`                 | Core packet utilities -- Encrypt/Decrypt, Inflate, string read/write, Analyze           |
| `packet/common`          | Shared packet types (Ack, Unknown), CRC16/CRC32, EQ float conversions, class/race names |
| `packet/rof`             | Rain of Fear protocol codec -- prefix handlers, packet parsing, stream flush            |
| `packet/rof/loginserver` | Login server packet definitions (8 packet types)                                        |
| `packet/rof/worldserver` | World server packet definitions (22 packet types)                                       |
| `packet/rof/zoneserver`  | Zone server packet definitions (72 packet types)                                        |
| `prompt`                 | Interactive command prompt (bubbletea-based) with 15 user commands                      |
| `rui`                    | Terminal UI layer (tview-based) -- player/target status display                         |
| `sim`                    | Game state simulation -- SpawnManager, player state, polling                            |
| `stopwatch`              | Performance timing utilities                                                            |
| `world`                  | World server client -- login info, world gather, enter world, character create          |
| `zone`                   | Zone server client -- zone info gathering, handler loop, game actions                   |
| `zone/item`              | Inventory/item management                                                               |
| `zone/spawn`             | Zone-level spawn tracking (Add/Remove/Find/FindFirst/Get/SetHP/Flush/Dump)              |
| `zone/spellbar`          | Spell gem management                                                                    |
| `zone/spellbook`         | Spellbook management                                                                    |

### Key Dependencies

| Dependency                           | Purpose                              |
| ------------------------------------ | ------------------------------------ |
| `github.com/google/gopacket`         | Packet capture and protocol analysis |
| `github.com/charmbracelet/bubbletea` | Terminal UI framework (prompt)       |
| `github.com/charmbracelet/lipgloss`  | Terminal styling                     |
| `github.com/charmbracelet/bubbles`   | UI components (key bindings, timer)  |
| `github.com/rivo/tview`              | Terminal UI widgets (status panels)  |
| `github.com/gdamore/tcell/v2`        | Terminal cell rendering              |
| `github.com/jbsmith7741/toml`        | TOML config parsing                  |
| `github.com/xackery/encdec`          | Binary encoding/decoding utilities   |
| `github.com/xackery/hex`             | Hex dump utilities                   |
| `github.com/erikgeiser/promptkit`    | Interactive selection prompts        |
| `github.com/olekukonko/tablewriter`  | ASCII table formatting               |
| `github.com/google/gopacket/pcapgo`  | PCAP file writing                    |
| `golang.org/x/sync`                  | errgroup for concurrent operations   |
| `golang.org/x/net`                   | HTML parsing (likely for item links) |

---

## 3. Protocol Flow

### 3.1 Login Phase

```
Client                          Login Server (loginserver.xxx.daybreakgames.com:5999)
  |                                    |
  |--- OP_SessionRequest ------------->|   (UDP session negotiation)
  |<-- OP_SessionResponse -------------|
  |                                    |
  |--- OP_SessionReady --------------->|
  |<-- OP_ACK2 ------------------------|
  |                                    |
  |--- OP_LoginRequest --------------->|   (username + encrypted password)
  |<-- OP_LoginResponse ---------------|   (success/fail + errorMessage)
  |                                    |
  |--- OP_ServerListRequest ---------->|   (expansionbitmask)
  |<-- OP_ServerListResponse ----------|   (server list with Server structs)
  |                                    |
  |--- OP_PlayEverQuestRequest ------->|   (selected server + char)
  |<-- OP_PlayEverQuestResponse -------|   (world server IP/port)
  |                                    |
  |--- OP_SessionGoodbye ------------->|
```

**Key functions:**

- `login.(*Client).Connect` -- initiates connection, starts `readLoop`
- `login.(*Client).requestSession` -- sends OP_SessionRequest
- `login.(*Client).requestReady` -- sends OP_SessionReady
- `login.(*Client).requestLogin` -- sends OP_LoginRequest with credentials
- `login.(*Client).requestServerList` -- sends OP_ServerListRequest
- `login.(*Client).requestPlay` / `login.(*Client).Play` -- sends OP_PlayEverQuestRequest
- `login.(*Client).readLoop` -- background goroutine processing incoming packets
- `login.(*Client).writeDump` -- packet hex dump logging

**Login server packet types (8):**
Ack2, ChatMessage, LoginRequest, LoginResponse, PlayEverQuestRequest, PlayResponse, ServerListRequest, ServerListResponse, SessionGoodbye, SessionReady

### 3.2 World Phase

```
Client                          World Server (ip:port from PlayResponse)
  |                                    |
  |--- OP_SessionRequest ------------->|
  |<-- OP_SessionResponse -------------|   (provides EncodeKey + CRCBytes)
  |                                    |
  |--- OP_SendLoginInfo -------------->|   (login credentials forwarded)
  |<-- OP_GuildsList ------------------|
  |<-- OP_SendMembership --------------|
  |<-- OP_SendMembershipInfo ----------|
  |<-- OP_LogServer -------------------|
  |<-- OP_ApproveWorld ----------------|
  |<-- OP_ExpansionInfo ---------------|
  |<-- OP_SendCharInfo ----------------|   (character list)
  |<-- OP_MaxCharacter ----------------|
  |<-- OP_WorldComplete ---------------|
  |                                    |   <- gatherWorldInfo waits for all of the above
  |                                    |
  |--- OP_WorldClientCRC ------------->|   (3 CRC packets sent)
  |--- OP_WorldClientCRC2 ------------>|
  |--- OP_WorldClientCRC3 ------------>|
  |--- OP_WorldClientReady ----------->|
  |                                    |
  |--- OP_EnterWorld ----------------->|   (requestEnterWorld: character name)
  |<-- OP_ZoneServerInfo --------------|   (zone server IP/port)
  |  OR                                |
  |<-- OP_ZoneUnavailable -------------|
  |                                    |
  |--- OP_SessionDisconnect ---------->|
```

**Key functions:**

- `world.(*Client).Connect` -- connect + readLoop
- `world.(*Client).requestSession` -- UDP session setup
- `world.(*Client).requestSendLoginInfo` -- forward credentials
- `world.(*Client).gatherWorldInfo` -- blocks until all world data received
- `world.(*Client).EnterWorld` / `requestEnterWorld` -- enter world, receive zone info
- `world.(*Client).CrcBytes` / `world.(*Client).EncodeKey` -- expose session crypto params
- `world.(*Client).CreateCharacter` -- character creation support

**World server packet types (22):**
ApproveName, ApproveWorld, CharacterCreate, CharacterCreateRequest, EnterWorld, ExpansionInfo, GuildsList, LogServer, MaxCharacter, PostEnterWorld, SendCharInfo, SendLoginInfo, SendMembership, SendMembershipInfo, WorldClientCRC, WorldClientCRC2, WorldClientCRC3, WorldClientReady, WorldComplete, ZoneChange, ZoneServerInfo, ZoneUnavailable

### 3.3 Zone Phase

```
Client                          Zone Server (ip:port from ZoneServerInfo)
  |                                    |
  |--- OP_SessionRequest ------------->|
  |<-- OP_SessionResponse -------------|
  |                                    |
  |    [gatherZoneInfo phase]          |
  |<-- OP_PlayerProfile ---------------|   (full character data)
  |<-- OP_NewZone ---------------------|   (zone geometry/weather)
  |<-- OP_ZoneSpawns ------------------|   (bulk spawn data)
  |<-- OP_CharInventory ---------------|   (inventory items)
  |<-- OP_SendAATable -----------------|
  |<-- OP_SendTributes ----------------|
  |<-- OP_TributeInfo -----------------|
  |<-- OP_TributeTimer ----------------|
  |<-- OP_TributeUpdate ---------------|
  |<-- OP_DzExpeditionLockoutTimers ---|
  |<-- OP_Weather ---------------------|
  |<-- OP_TimeOfDay -------------------|
  |<-- OP_WorldObjectsSent ------------|   (signals zone data complete)
  |                                    |
  |--- OP_ClientReady ---------------->|   (signals ready for gameplay)
  |                                    |
  |    [handler loop -- continuous]    |
  |<-->  OP_NewSpawn / OP_DeleteSpawn  |   (spawn tracking)
  |<-->  OP_SpawnAppearance           |   (visual state changes)
  |<-->  OP_ClientUpdateFromServer     |   (position updates)
  |<-->  OP_HPUpdate / OP_MobHealth    |   (health tracking)
  |<-->  OP_Damage / OP_Death          |   (combat events)
  |<-->  OP_BeginCast / OP_CastSpell   |   (spell events)
  |<-->  OP_ChannelMessage / OP_Chat.. |   (communication)
  |<-->  OP_ManaChange / OP_ManaUpdate |   (resource tracking)
  |<-->  OP_ExpUpdate / OP_Stamina     |   (stat updates)
  |<-->  OP_Consider                   |   (con checks)
  |<-->  OP_WhoAllRequest/Response     |   (who queries)
  |<-->  OP_FormattedMessage           |   (server messages)
  |<-->  OP_TargetCommand / OP_Target..|   (targeting)
  |<-->  OP_AutoAttack                 |   (combat toggle)
  |<-->  OP_ItemPacket / OP_LootItem.. |   (items/loot)
  |<-->  OP_WearChange                 |   (equipment visuals)
  |<-->  ...more...                    |
```

**Key functions:**

- `zone.(*Client).Connect` -- connect + readLoop
- `zone.(*Client).requestSession` -- UDP session setup
- `zone.(*Client).gatherZoneInfo` -- blocks for initial zone data (4 sub-functions)
- `zone.(*Client).handler` -- main packet dispatch loop with PlayerUpdate callbacks
- `zone.(*Client).poll` / `pollPacket` -- poll for incoming packets
- `zone.(*Client).syncPlayerProfile` -- update local player state from profile packet

---

## 4. Opcode Reference Table

Opcodes extracted from binary string table. The Go binary concatenates opcode names with adjacent struct field names in the string table, so the raw OP\_ names appear fused with other identifiers. The clean opcode names are derived by cross-referencing with the packet struct types.

### Session Layer (EQStream)

| Opcode                 | Direction | Purpose                                    |
| ---------------------- | --------- | ------------------------------------------ |
| OP_SessionRequest      | C->S      | UDP session initiation                     |
| OP_SessionResponse     | S->C      | Session parameters (encode key, CRC bytes) |
| OP_SessionReady        | C->S      | Client ready for protocol                  |
| OP_SessionDisconnect   | C->S/S->C | Clean disconnect                           |
| OP_SessionGoodbye      | C->S      | Final goodbye                              |
| OP_SessionStatRequest  | C->S/S->C | Statistics request                         |
| OP_SessionStatResponse | S->C/C->S | Statistics response                        |
| OP_AckPacket           | Both      | Reliable packet acknowledgment             |
| OP_Combined            | Both      | Multiple opcodes in single UDP packet      |
| OP_Packet              | Both      | Standard application packet wrapper        |

### Login Server

| Opcode                   | Direction | Purpose                                          |
| ------------------------ | --------- | ------------------------------------------------ |
| OP_LoginRequest          | C->S      | Username + encrypted password (normallogin flag) |
| OP_LoginResponse         | S->C      | Success/fail with errorMessage                   |
| OP_ServerListRequest     | C->S      | Request server list (expansionbitmask)           |
| OP_ServerListResponse    | S->C      | List of Server structs (name, status, players)   |
| OP_PlayEverQuestRequest  | C->S      | Select server to play on (char selection)        |
| OP_PlayEverQuestResponse | S->C      | World server connection info                     |
| OP_ChatMessage           | S->C      | Login server chat/notice                         |
| OP_Motd                  | S->C      | Message of the day                               |
| OP_ACK2                  | S->C      | Login-specific acknowledgment                    |

### World Server

| Opcode                    | Direction | Purpose                                    |
| ------------------------- | --------- | ------------------------------------------ |
| OP_SendLoginInfo          | C->S      | Forward login credentials to world         |
| OP_SendCharInfo           | S->C      | Character list (with zoneunavailable flag) |
| OP_SendMembership         | S->C      | Account membership data                    |
| OP_SendMembershipInfo     | S->C      | Detailed membership info                   |
| OP_MaxCharacter           | S->C      | Max character slot count                   |
| OP_GuildsList             | S->C      | Guild list for server                      |
| OP_ExpansionInfo          | S->C      | Available expansion bitmask                |
| OP_LogServer              | S->C      | Logging server address (zone field)        |
| OP_ApproveWorld           | S->C      | World entry approved                       |
| OP_ApproveName            | C->S/S->C | Character name validation (zoneserverinfo) |
| OP_WorldComplete          | S->C      | All world data sent                        |
| OP_WorldClientCRC         | C->S      | Client file CRC verification (1 of 3)      |
| OP_WorldClientCRC2        | C->S      | Client file CRC verification (2 of 3)      |
| OP_WorldClientCRC3        | C->S      | Client file CRC verification (3 of 3)      |
| OP_WorldClientReady       | C->S      | Client ready for world operations          |
| OP_EnterWorld             | C->S      | Request to enter world (character name)    |
| OP_PostEnterWorld         | S->C      | Post-enter data (UnknownSpawn)             |
| OP_ZoneServerInfo         | S->C      | Zone server IP/port for selected zone      |
| OP_ZoneUnavailable        | S->C      | Zone not available                         |
| OP_ZoneChange             | C->S/S->C | Zone transition (charactername)            |
| OP_CharacterCreate        | C->S      | Character creation data (drakkindetails)   |
| OP_CharacterCreateRequest | C->S      | Request to create character                |

### Zone Server

| Opcode                       | Direction | Purpose                                             |
| ---------------------------- | --------- | --------------------------------------------------- |
| **Spawn Management**         |           |                                                     |
| OP_NewSpawn                  | S->C      | New entity spawned (buttons field)                  |
| OP_Spawn                     | S->C      | Spawn data (helm field)                             |
| OP_DeleteSpawn               | S->C      | Entity despawned                                    |
| OP_ZoneSpawns                | S->C      | Bulk spawn list                                     |
| OP_SpawnAppearance           | S->C      | Appearance change                                   |
| OP_ZoneEntryClient           | C->S      | Client zone entry                                   |
| OP_ZoneEntryServer           | S->C      | Server zone entry confirmation                      |
| OP_GroundSpawn               | S->C      | Ground item spawn                                   |
| OP_SendFindableNPCs          | S->C      | Findable NPC list (propertiesCount)                 |
| **Movement/Position**        |           |                                                     |
| OP_ClientUpdateFromClient    | C->S      | Client position update (equipment2 field)           |
| OP_ClientUpdateFromServer    | S->C      | Server position update (equipment2 field)           |
| OP_UpdateMovementEntry       | S->C      | Movement update                                     |
| OP_PlayerStateAdd            | S->C      | Player state flag added (destructable1)             |
| OP_PlayerStateRemove         | S->C      | Player state flag removed (isGuildNameShown)        |
| **Combat**                   |           |                                                     |
| OP_AutoAttack                | C->S      | Toggle auto-attack (isenabled flag)                 |
| OP_Damage                    | S->C      | Damage event (force field)                          |
| OP_Death                     | S->C      | Death event (slot field)                            |
| OP_Consider                  | C->S/S->C | Consider target (faction field)                     |
| OP_TargetCommand             | C->S      | Target by command                                   |
| OP_TargetMouse               | C->S      | Target by mouse click (rankstring)                  |
| OP_TargetHoTT                | S->C      | Target of target                                    |
| OP_XTargetResponse           | S->C      | Extended target response                            |
| **Spells/Casting**           |           |                                                     |
| OP_BeginCast                 | S->C      | Spell cast started (decoding flag)                  |
| OP_CastSpell                 | C->S      | Cast spell request (targetid field)                 |
| OP_MemorizeSpell             | C->S      | Memorize spell to gem                               |
| **Health/Mana/Stats**        |           |                                                     |
| OP_HPUpdate                  | S->C      | HP update                                           |
| OP_MobHealth                 | S->C      | NPC health (response field)                         |
| OP_ManaChange                | S->C      | Mana change event                                   |
| OP_ManaUpdate                | S->C      | Mana update (reduction field)                       |
| OP_Stamina                   | S->C      | Stamina/endurance                                   |
| OP_EnduranceUpdate           | S->C      | Endurance update (itemtint field)                   |
| OP_ExpUpdate                 | S->C      | Experience update (stringid field)                  |
| **Communication**            |           |                                                     |
| OP_ChannelMessage            | C->S/S->C | Channel message (channelnumber field)               |
| OP_ChatMessage               | S->C      | Chat message (isApproved flag)                      |
| OP_FormattedMessage          | S->C      | Formatted server message (equipment field)          |
| OP_SpecialMessage            | S->C      | Special message (targetspawnid field)               |
| OP_SimpleMessage             | S->C      | Simple text message (npcTintIndex field)            |
| OP_OnLevelMessage            | S->C      | Level-up message (soundcontrols field)              |
| OP_SetChatServer             | S->C      | Chat server assignment                              |
| **Items/Inventory**          |           |                                                     |
| OP_CharInventory             | S->C      | Full inventory                                      |
| OP_ItemPacket                | S->C      | Item data packet                                    |
| OP_ItemLinkClick             | C->S      | Item link clicked (spawnlooting field)              |
| OP_MoveItem                  | C->S      | Move item in inventory                              |
| OP_LootRequest               | C->S      | Request to loot (negativeid field)                  |
| OP_LootItem                  | S->C      | Loot item result                                    |
| OP_MoneyOnCorpse             | S->C      | Money on corpse (zoneLongName field)                |
| OP_AdvLoot                   | S->C      | Advanced loot window (fromid field)                 |
| OP_CancelTrade               | C->S      | Cancel trade (hitheading field)                     |
| **Zone Management**          |           |                                                     |
| OP_PlayerProfile             | S->C      | Full player profile (zonechange field)              |
| OP_NewZone                   | S->C      | Zone data                                           |
| OP_ClientReady               | C->S      | Client ready for gameplay                           |
| OP_ReqNewZone                | C->S      | Request new zone data (spelltype field)             |
| OP_RequestClientZoneChange   | S->C      | Zone change request                                 |
| OP_Weather                   | S->C      | Weather update                                      |
| OP_TimeOfDay                 | S->C      | Time of day                                         |
| OP_WorldObjectsSent          | S->C      | All world objects sent (signals zone data complete) |
| **Misc**                     |           |                                                     |
| OP_ChangeSize                | C->S      | Change entity size (vehicleid field)                |
| OP_Camp                      | C->S      | Camp/logout (exp field)                             |
| OP_WhoAllRequest             | C->S      | /who query (formatstring field)                     |
| OP_WhoAllResponse            | S->C      | /who results                                        |
| OP_WearChange                | S->C      | Equipment visual change                             |
| OP_MoveDoor                  | C->S      | Door interaction                                    |
| OP_EnvDamage                 | S->C      | Environmental damage                                |
| OP_DZCompass                 | S->C      | Dynamic zone compass                                |
| OP_GroupTarget               | C->S      | Group target                                        |
| OP_RaidUpdate                | S->C      | Raid update                                         |
| OP_RespondAA                 | S->C      | AA response                                         |
| OP_SendAATable               | S->C      | AA table data (bitfields1 field)                    |
| OP_SendTributes              | S->C      | Tribute data (equipchest2 field)                    |
| OP_TributeInfo               | S->C      | Tribute info                                        |
| OP_TributeTimer              | S->C      | Tribute timer                                       |
| OP_TributeUpdate             | S->C      | Tribute update                                      |
| OP_DzExpeditionLockoutTimers | S->C      | DZ lockout timer data (itemtint2 field)             |
| OP_Unknown                   | Both      | Unrecognized opcode (fallback)                      |

---

## 5. EQStream Protocol Layer

The `eqstream` package implements the EQ UDP transport protocol. This is the layer that sits between raw UDP and the application-level opcodes.

### EQStream Type

```
eqstream.EQStream
  Methods:
    Marshal(data) -> encrypted+CRC'd packet bytes
    Unmarshal(bytes) -> decrypted application data
    NextSequence() -> uint16 (incrementing sequence number)
    PacketCount() -> int
    Read(conn) -> parsed packet
    Resend() -> retransmit unacked
    SetCRCBytes(n) -> set CRC byte count (from SessionResponse)
    SetEncodeKey(key) -> set encryption key (from SessionResponse)
    SetFilterMode(mode) -> set packet filtering
    StreamFlush() -> flush pending writes
    WritePacket(packet) -> send application packet
```

### Packet Prefix System (RoF codec)

The RoF protocol codec (`packet/rof.RoF`) uses a prefix byte to identify packet framing types:

| Prefix Handler              | Purpose                                             |
| --------------------------- | --------------------------------------------------- |
| `prefixSessionRequest`      | Session initiation (0x01)                           |
| `prefixSessionResponse`     | Session parameters (0x02)                           |
| `prefixCombined`            | Multiple app packets in one UDP datagram            |
| `prefixPacket`              | Standard reliable application packet                |
| `prefixFragment`            | Fragmented packet (large payloads split across UDP) |
| `prefixAck`                 | Reliable delivery acknowledgment                    |
| `prefixOutOfOrder`          | Out-of-order packet notification                    |
| `prefixSessionDisconnect`   | Session teardown                                    |
| `prefixSessionStatRequest`  | Session statistics query                            |
| `prefixSessionStatResponse` | Session statistics data                             |
| `prefixUnreliableOpCode`    | Fire-and-forget application packet                  |

### Encryption / Integrity

- `packet.Encrypt(data, key)` -- XOR-based stream cipher using encode key
- `packet.Decrypt(data, key)` -- reverse of Encrypt
- `packet.Inflate(data)` -- zlib decompression for compressed packets
- `common.GenerateCRC16(data)` -- 16-bit CRC for login packets
- `common.GenerateCRC32(data)` -- 32-bit CRC for world/zone packets
- `common.AppendCRC(data, crcBytes)` -- append CRC to outgoing packet
- `SetEncodeKey` / `SetCRCBytes` -- configured per-session from SessionResponse

### Sequence Tracking

- Each packet has a `SequenceID` (get/set via interface)
- `NextSequence()` provides monotonically increasing sequence numbers
- `lastSequence` / `LastSequence` / `NextSequence` fields track stream position
- `Resend()` handles retransmission of unacknowledged packets

### EQ Float Conversions

The `packet/common` package provides fixed-point float converters used in position/heading packets:

| Function                      | Bits   | Purpose                 |
| ----------------------------- | ------ | ----------------------- |
| `EQ10ToFloat` / `FloatToEQ10` | 10-bit | Heading values          |
| `EQ12ToFloat` / `FloatToEQ12` | 12-bit | Pitch/roll              |
| `EQ13ToFloat` / `FloatToEQ13` | 13-bit | Position coordinates    |
| `EQ19ToFloat` / `FloatToEQ19` | 19-bit | High-precision position |

### Common Packet Interface

Every packet type implements this interface (visible from method signatures):

```go
type OpCodeImplementer interface {
    OpCode() uint16
    Name() string
    Data() []byte
    Read(reader) error
    Write(writer) error
    Size() int
    SequenceID() uint16
    SetSequenceID(uint16)
}
```

---

## 6. Zone Client API

The `zone.Client` exposes these game actions:

### Actions

| Method                              | Purpose                        |
| ----------------------------------- | ------------------------------ |
| `CastSpell(spellID, targetID, gem)` | Cast a spell on target         |
| `SetTarget(spawnID)`                | Set current target             |
| `SetAutoAttack(bool)`               | Enable/disable auto-attack     |
| `ToggleAutoAttack()`                | Toggle auto-attack state       |
| `Warp(x, y, z)`                     | Teleport to coordinates        |
| `Say(message)`                      | Say in local chat              |
| `ChannelMessage(channel, message)`  | Send to specific channel       |
| `Consider(spawnID)`                 | Consider a target              |
| `Memorize(spellID, gem)`            | Memorize spell to gem slot     |
| `Loot(corpseID)`                    | Loot a corpse                  |
| `ItemLinkClick(itemID)`             | Click an item link             |
| `SelfDamage(amount)`                | Apply damage to self (testing) |
| `SetFollow(spawnID)`                | Follow a target                |
| `IsFollowEnabled()`                 | Check follow state             |

### State Management

| Method                    | Purpose                               |
| ------------------------- | ------------------------------------- |
| `PlayerProfile()`         | Get full player profile               |
| `syncPlayerProfile()`     | Sync from server PlayerProfile packet |
| `ShortName()`             | Current zone short name               |
| `LongName()`              | Current zone long name                |
| `NextSequence()`          | Get next packet sequence number       |
| `Subscribe(callback)`     | Subscribe to packet events            |
| `updateMovementHistory()` | Track position history                |

### Lifecycle

| Method                    | Purpose                                    |
| ------------------------- | ------------------------------------------ |
| `Connect(host, port)`     | Connect to zone server                     |
| `Close()`                 | Disconnect                                 |
| `requestSession()`        | Initiate UDP session                       |
| `gatherZoneInfo()`        | Block until all initial zone data received |
| `handler()`               | Main packet dispatch loop                  |
| `readLoop()`              | Background packet reader goroutine         |
| `poll()` / `pollPacket()` | Poll for packets                           |
| `Write(data)`             | Send raw data                              |
| `WritePacket(packet)`     | Send application packet                    |
| `writeDump(packet)`       | Hex dump packet for debugging              |

### Interactive Prompt Commands

The `prompt` package exposes 15 user-facing commands:

| Command                     | Function                                        |
| --------------------------- | ----------------------------------------------- |
| `attack <target>`           | `commandAttack` -- toggle auto-attack on target |
| `cast <spell> [target]`     | `commandCast` -- cast spell                     |
| `target <name>`             | `commandTarget` -- target by name               |
| `follow <name>`             | `commandFollow` -- follow a spawn               |
| `warp <x> <y> <z>`          | `commandWarp` -- teleport                       |
| `loc`                       | `commandLoc` -- print current location          |
| `spawn`                     | `commandSpawn` -- dump spawn list               |
| `loot`                      | `commandLoot` -- loot nearby corpse             |
| `memorize <spell> <gem>`    | `commandMemorize` -- memorize spell             |
| `findspell <name\|list>`    | `commandFindSpell` -- search spell database     |
| `itemlink <id>`             | `commandItemLink` -- click item link            |
| `channelmessage <ch> <msg>` | `commandChannelMessage` -- chat                 |
| `echo <message>`            | `commandEcho` -- echo text                      |
| `selfdamage <amount>`       | `commandSelfDamage` -- test self-damage         |
| `help`                      | `commandHelp` -- list commands                  |
| `quit`                      | `commandQuit` -- exit                           |

---

## 7. Data Structures

### Known Struct Types (from Go type info)

| Package       | Struct                     | Purpose                                               |
| ------------- | -------------------------- | ----------------------------------------------------- |
| `config`      | `Config`                   | Application configuration                             |
| `login`       | `WorldCredential`          | Server credentials for world login                    |
| `loginserver` | `Server`                   | Server list entry (has `CleanName`, `String` methods) |
| `zoneserver`  | `PlayerProfileBind`        | Bind point data                                       |
| `zoneserver`  | `SpellBuff`                | Active buff data                                      |
| `zoneserver`  | `Item`                     | Item reference                                        |
| `zoneserver`  | `ItemData`                 | Full item data                                        |
| `zoneserver`  | `BandolierItem`            | Bandolier weapon set item                             |
| `zoneserver`  | `DZExpeditionLockoutTimer` | DZ lockout data                                       |
| `library`     | `spell`                    | Spell database entry                                  |
| `sim`         | `Sim`                      | Game state simulation                                 |

### Spawn Fields (from field name extraction)

Fields identified in spawn/player update packets:

- Identity: `spawn`, `spawnid`, `targetspawnid`, `lastname`, `title`, `guild`
- Position: `position`, `heading`, `deltaheading`, `velocity`
- Stats: `hp`, `mana`, `endurance`, `level`
- Class/Race: `class`, `race`, `deity`, `gender`, `face`
- Appearance: `equipment`, `tint`, `helm`, `hair`, `beard`, `eye`
- State: `sitting`, `standing`, `ducking`, `afk`, `anon`, `lfg`, `gm`
- Relations: `pet`, `owner`
- Drakkin-specific: `drakkindetails` (heritage, tattoo, etc.)

### Sim/SpawnManager

```
sim.Sim
  Methods: HPString, ManaString, PlayerLevel, PlayerName, poll, pollHandle,
           ServerShortName, SetServerShortName, SetTarget, ZoneShortName

sim.SpawnManager (also zone/spawn.Spawn)
  Methods: Add, Count, Dump, Find, FindFirst, Flush, Get, Remove, SetHP
```

### Zone Sub-managers

```
zone/item.Item: InventoryItemByID, ItemByID, SetItem
zone/spellbar.SpellBar: SetGem, SpellID
zone/spellbook.SpellBook: SetSpell, SpellID
```

---

## 8. Configuration

Config file: `runeq.conf` (TOML format, via `jbsmith7741/toml`)

Known config fields (from string extraction):

- `EQHost` -- server address (format: `ip:port`, e.g., `192.168.1.100:5999`)
- Username / password credentials
- Server selection
- Logging level

The `config.NewConfig` function loads config, `config.Save` persists it, and `config.(*Config).Verify` validates settings.

The `client.NewInteractiveLogin` function provides a guided setup when config is missing, using `promptkit/selection` for interactive server selection.

Data files referenced:

- `spells_us` -- spell database (loaded by `library.loadSpell`)
- EQHost.txt format noted in error strings: `EQHost.txt (ex: 192.168.1.100:5999)`

---

## 9. Implications for TextQuest

### 9.1 Headless Client Potential

RunEQ demonstrates that a fully headless EQ client is achievable. For TextQuest's 36-box setup, this has massive implications:

**Resource savings:** A headless client would eliminate the ~2-4GB RAM and GPU overhead per eqgame.exe instance. For 36 boxes, that is 72-144GB of RAM savings and eliminates the GPU bottleneck entirely. RunEQ as a Go binary is likely under 100MB RAM per instance.

**Simplified architecture:** No need for DLL injection, memory reading, process hooks, or anti-cheat evasion. The headless client IS the controller -- it sends packets directly.

**Risk:** This is the most detectable approach possible. A headless client has no DirectX rendering, no eqgame.exe process, completely different network fingerprint (Go's UDP stack vs Windows Winsock), and would fail any client-side integrity check. On a TLP server with active anti-cheat, this would be an instant ban.

### 9.2 Protocol Reference for TextQuest

Even without building a headless client, RunEQ's extracted protocol data is directly useful:

**Packet sniffer (M6 TUI):** The opcode table and packet structures map directly to what TextQuest's packet sniffer hook needs to decode. The `OP_*` names, field names, and RoF packet definitions serve as a decode reference.

**Protocol validation:** TextQuest's DLL hooks send/recv at the network layer. RunEQ's protocol flow confirms the exact sequence of packets during login, zoning, and gameplay -- useful for validating that our hooks see the right traffic.

**EQStream layer:** The session negotiation, encryption (XOR with encode key), CRC verification, sequencing, fragmentation, and combined packet handling documented here matches what TextQuest needs to decode in its packet sniffer.

**Float encoding:** The EQ10/12/13/19 fixed-point float conversions are critical for decoding position packets. These confirm the encoding format we need for movement data.

### 9.3 Specific Technical Insights

1. **CRC scheme:** Login uses CRC16, world/zone use CRC32. CRC byte count is negotiated per-session.
2. **Encryption:** Simple XOR stream cipher with key from SessionResponse. Not cryptographically strong.
3. **Three CRC packets:** World server requires three separate client CRC checks (WorldClientCRC/CRC2/CRC3) before allowing world entry. These verify client file integrity.
4. **Combined packets:** Multiple application opcodes can be packed into a single UDP datagram (OP_Combined), which the sniffer must handle.
5. **Fragment reassembly:** Large packets (like CharInventory, PlayerProfile) are fragmented across multiple UDP datagrams and must be reassembled by sequence ID.
6. **RoF protocol version:** RunEQ targets Rain of Fear protocol. TLP servers may use different opcode values depending on the expansion era, but the framing layer (EQStream) is consistent.
7. **gopacket integration:** RunEQ includes full gopacket + pcapgo, suggesting it can also operate in packet-capture mode (sniffing real EQ traffic), not just as a direct client.

### 9.4 Detection Considerations

If TextQuest ever explored a hybrid approach (headless clients for non-visible boxes, real clients for visible ones):

- **Network fingerprint:** Go's UDP stack produces different packet timing, MTU behavior, and socket options than eqgame.exe's Winsock calls. Detectable by server-side analysis.
- **Missing client data:** Headless clients cannot provide valid WorldClientCRC values without having the actual game files and computing real CRCs. Faking these is possible but another detection vector.
- **No rendering callbacks:** Server-side checks that expect client-side rendering state (screenshot requests, UI element queries) would fail.
- **Session characteristics:** Packet timing, update frequency, and response latency patterns would differ from a real client.

**Verdict for TextQuest:** Use RunEQ as a protocol reference, not as an operational approach. The injected DLL strategy provides full client fidelity while RunEQ's protocol documentation accelerates packet sniffer development and protocol understanding.

---

## 10. Key Unknowns / Gaps

1. **Opcode numeric values:** The binary contains opcode names but the actual hex values are likely in the `init()` functions as map entries. Without disassembly or the source, we have names but not values. EQEmu/MQ2 opcode files can provide the RoF-era numeric mappings.

2. **Packet field layouts:** We know field names exist within packets (e.g., `targetid` in CastSpell, `channelnumber` in ChannelMessage) but not the exact byte offsets within each packet struct. The `xackery/encdec` library handles binary encoding, and the field order in Go structs determines wire order.

3. **Session negotiation details:** The exact bytes of SessionRequest/SessionResponse (protocol version, session ID format, max packet size negotiation) are not visible from strings alone.

4. **RunEQ version compatibility:** This is v0.4.6 targeting RoF protocol. It is unknown whether it works against live/TLP servers, or only against EQEmu servers that support the RoF protocol version.

5. **Character creation details:** The `CharacterCreate` packet references `drakkindetails`, suggesting RoF+ character creation support, but the full field layout is unknown.

6. **Source availability:** The GitHub repo (xackery/runeq) appears to be private or deleted. The author (xackery) is active in the EQEmu community and may have other related projects. The `xackery/encdec` encoding library is likely still public.

7. **Compression handling:** `packet.Inflate` exists but the conditions for when packets are compressed vs. uncompressed are not clear from strings alone. The `IsCompressed` and `uncompressed` strings suggest a flag in the packet header.

8. **Server-specific opcodes:** TLP servers on different expansion locks may use different opcode values. The RoF opcodes extracted here may need remapping for other eras (Classic, Kunark, Velious, etc.).
