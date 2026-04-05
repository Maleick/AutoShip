# EverQuest Network Protocol — Public Research Summary

> Research date: 2026-04-04
> Sources: EQEmu open-source server, EQNet client library, OpenEQC protocol docs, ShowEQ, P99 analysis

---

## 1. Protocol Architecture Overview

EverQuest uses a **custom reliable transport layer over UDP**. The protocol has two distinct layers:

| Layer                           | Purpose                                             | Opcodes                               |
| ------------------------------- | --------------------------------------------------- | ------------------------------------- |
| **Transport (Reliable Stream)** | Session management, reliability, fragmentation, CRC | Single-byte opcodes (0x01–0x15)       |
| **Application**                 | Game logic — spawns, movement, combat, chat         | Two-byte opcodes (change every patch) |

The transport layer adds sequencing, acknowledgement, fragmentation, compression, and CRC
to raw UDP. It is functionally equivalent to a lightweight TCP reimplementation optimized for
game traffic — small packets, low latency, combined message batching.

---

## 2. Transport Layer — "Reliable Stream" Protocol

### 2.1 Protocol Opcodes

All transport packets begin with `0x00` followed by a one-byte opcode:

| Opcode                         | Hex    | Direction | Purpose                                  |
| ------------------------------ | ------ | --------- | ---------------------------------------- |
| Connect (SessionRequest)       | `0x01` | C→S       | Initiate session                         |
| ConnectReply (SessionResponse) | `0x02` | S→C       | Return CRC key + encode params           |
| Combined                       | `0x03` | Both      | Multiple app packets in one UDP datagram |
| Disconnect                     | `0x05` | Both      | Clean session teardown                   |
| KeepAlive                      | `0x06` | Both      | Prevent stale connection timeout         |
| SessionStatRequest             | `0x07` | S→C       | Ping + packet count stats                |
| SessionStatResponse            | `0x08` | C→S       | Reply to stat request                    |
| Packet (Reliable)              | `0x09` | Both      | Single sequenced application packet      |
| Fragment                       | `0x0d` | Both      | Fragment of oversized packet             |
| Ack                            | `0x15` | Both      | Acknowledge received sequence            |

### 2.2 Session Handshake

```
Client                          Server
  |                               |
  |--- Connect (0x01) ----------->|  protocol_version, connect_code, max_packet_size
  |                               |
  |<-- ConnectReply (0x02) -------|  connect_code, encode_key, crc_bytes,
  |                               |  encode_pass1, encode_pass2, max_packet_size
  |                               |
  |--- [Application packets] ---->|  Session established
```

**Connect packet (14 bytes):**

```
Offset  Size  Field
0       1     zero (0x00)
1       1     opcode (0x01)
2       4     protocol_version (uint32)
6       4     connect_code (uint32) — random session ID
10      4     max_packet_size (uint32) — typically 512
```

**ConnectReply packet (17 bytes):**

```
Offset  Size  Field
0       1     zero (0x00)
1       1     opcode (0x02)
2       4     connect_code (uint32) — echoed from request
6       4     encode_key (uint32) — CRC/XOR key for session
10      1     crc_bytes (uint8) — CRC length (typically 2)
11      1     encode_pass1 (uint8) — encoding method
12      1     encode_pass2 (uint8) — encoding method
13      4     max_packet_size (uint32)
```

**Key detail**: The `encode_key` returned in ConnectReply is used for all subsequent CRC
calculations in the session. The client must use this key for outbound CRC and validate
inbound CRC with it.

### 2.3 CRC

- 2-byte CRC appended to every transport packet after session establishment
- Calculated using `CRC32(data, len, encode_key)` then truncated to configured byte count
- Server rejects packets with CRC mismatch

### 2.4 Sequencing and Acknowledgement

Reliable packets (opcode `0x09`) carry a 16-bit sequence number:

```
Offset  Size  Field
0       1     zero (0x00)
1       1     opcode (0x09)
2       2     sequence (uint16, network byte order)
4+      var   compressed application data
```

- Sequence numbers increment monotonically per direction
- Receiver sends Ack (`0x15`) with the sequence number of the last contiguous packet received
- Auto-ACK fires if no data packet sent within ~4 seconds (keepalive ACK)
- Sender retransmits unacked packets after **150ms minimum** (30ms base \* 1.25 factor, clamped to 150ms–5000ms)
- **Resend timeout**: 30,000ms — connection dropped if no ACK received in 30s
- **Stale connection**: 60,000ms — connection dropped if completely silent for 60s

### 2.5 KeepAlive

- **Interval**: 9,000ms
- Packet is simply `0x00 0x06` (+ CRC)
- If no keepalive or other packet arrives for 60s, connection is declared stale and dropped

### 2.6 Combined Packets

Multiple small application packets batched into one UDP datagram:

```
0x00 0x03 [length1][app_packet1] [length2][app_packet2] ...
```

Each sub-packet is prefixed with its length byte. This reduces UDP overhead for the many
small updates EQ sends (position, HP, buffs).

### 2.7 Fragmentation

Packets exceeding `max_packet_size` (default 512) are fragmented:

**First fragment (opcode 0x0d):**

```
Offset  Size  Field
0       1     zero (0x00)
1       1     opcode (0x0d)
2       2     sequence (uint16)
4       4     total_size (uint32) — total reassembled size
8+      var   payload (up to ~502 bytes)
```

**Subsequent fragments** use the same Reliable header (opcode `0x09`) with incrementing
sequence numbers. The receiver reassembles based on `total_size` from the first fragment.

### 2.8 Compression and Encoding

Three encoding modes observed:

- `EncodeNone (0)` — raw data
- `EncodeCompression (1)` — zlib/deflate compression
- `EncodeXOR (4)` — XOR obfuscation with encode_key

Compression flag byte:

- `0xA5` = uncompressed (data follows as-is)
- `0x5A` (`'Z'`) = compressed (deflate follows)

Packets below a size threshold skip compression (the `0xA5` marker is always present).

### 2.9 Session Statistics

The server periodically sends `SessionStatRequest` (0x07, 40 bytes) containing ping
measurements and packet counters. The client must respond with `SessionStatResponse`
(0x08, 40 bytes) echoing the timestamp and providing its own send/receive counts.

**This is a form of keepalive that also serves as latency measurement.**

### 2.10 Hold Buffer

Outbound packets are batched in a 512-byte hold buffer and flushed every **50ms** or when
full. This means the effective minimum send rate is ~20 flushes/second.

---

## 3. Connection Flow — Login to In-Zone

### 3.1 Three-Server Architecture

```
┌──────────┐     ┌──────────┐     ┌──────────┐
│  Login   │────>│  World   │────>│   Zone   │
│  Server  │     │  Server  │     │  Server  │
└──────────┘     └──────────┘     └──────────┘
   UDP              UDP              UDP
  :5998            :9000           :7000-7999
```

Each transition is a **completely new UDP session** — new Connect/ConnectReply handshake,
new encode_key, new sequence numbers.

### 3.2 Login Server Flow (EQEmu)

```
1. Transport: Connect → ConnectReply (session established)
2. App: OP_SessionReady (0x0001)         C→S
3. App: OP_Login (0x0002)                C→S  [encrypted credentials]
4. App: OP_LoginAccepted (0x0017)        S→C  [session key]
5. App: OP_ServerListRequest (0x0004)    C→S
6. App: OP_ServerListResponse (0x0018)   S→C  [server list]
7. App: OP_PlayEverquestRequest (0x000d) C→S  [server runtime ID]
8. App: OP_PlayEverquestResponse (0x0021) S→C [world IP:port + session key]
```

**Credential encryption**: Login uses a custom encryption function (`EQNet_Encrypt`).
The username and password are combined into a single buffer, encrypted, and sent as the
payload of OP_Login.

**Session key**: The login server issues a session key that the client presents to the
world server for authentication. This key is time-limited.

### 3.3 World Server Flow

```
1. Transport: Connect → ConnectReply (new session)
2. App: OP_SendLoginInfo (0x4dd0)         C→S  [session key from login]
3. App: OP_SendCharInfo (0x4513)          S→C  [character list]
4. App: OP_ExpansionInfo (0x04ec)         S→C  [expansion flags]
5. App: OP_GuildsList (0x6957)            S→C  [guild list]
6. App: OP_EnterWorld (0x7cba)            C→S  [character name, tutorial flag]
7. App: OP_ZoneServerInfo (0x61b6)        S→C  [zone server IP:port]
8. App: OP_World_Client_CRC1 (0x5072)     S→C  [client file CRC check]
9. App: OP_World_Client_CRC2 (0x5b18)     S→C  [client file CRC check]
```

**CRC checks**: The world server sends two CRC validation packets. On EQEmu these are
typically ignored/stubbed. On live Daybreak servers, these validate specific client files
to detect modifications.

### 3.4 Zone Server Flow — Entry Sequence

```
1.  Transport: Connect → ConnectReply (new session)
2.  App: OP_ZoneEntry (0x7213)            C→S  [character name]
3.  App: OP_PlayerProfile (0x75df)        S→C  [~19,500 byte character data]
4.  App: OP_NewZone (0x0920)              S→C  [zone metadata, fog, gravity]
5.  App: OP_CharInventory (0x5394)        S→C  [full inventory]
6.  App: OP_TimeOfDay (0x1580)            S→C  [game time]
7.  App: OP_SendZonepoints (0x3eba)       S→C  [zone connection points]
8.  App: OP_SpawnDoor (0x4c24)            S→C  [door objects]
9.  App: OP_ReqClientSpawn (0x0322)       C→S  [ready for spawns]
10. App: OP_ZoneSpawns (0x2e78)           S→C  [all spawns in zone — large]
11. App: OP_SetServerFilter (0x6563)      C→S  [message filter preferences]
12. App: OP_ClientReady (0x5e20)          C→S  [client fully loaded]
```

After step 12, the client is "in zone" and begins the normal game loop.

---

## 4. Position Updates

### 4.1 Client → Server (OP_ClientUpdate)

Opcode: `0x14cb` (changes per patch)

**PlayerPositionUpdateClient_Struct:**

```
Offset  Size  Field
0       2     sequence (uint16) — incrementing counter
2       2     spawn_id (uint16)
4       2     vehicle_id (uint16)
6       4     delta_x (float)
10      4     delta_z (float)
14      4     delta_y (float)
18      12b   heading:12 (packed bitfield)
        10b   animation:10 (packed bitfield)
        ...   padding:10
```

### 4.2 Server → Client (OP_MobUpdate / SpawnPositionUpdate)

**PlayerPositionUpdateServer_Struct** uses bitfield packing:

- `x_pos:19, y_pos:19, z_pos:19` — signed 19-bit fixed-point
- `heading:12` — 0–4095 mapped to 0–360 degrees
- `animation:10` — current animation state
- Delta fields for velocity

### 4.3 Update Frequency

- **Client sends position ~4 times/second** when moving
- **Stationary clients**: approximately every **10 seconds**
- Server validates against velocity and previous position (warp detection)
- `Range::ClientPositionUpdates` rule defaults to **300 units** — beyond this range,
  other clients only receive position updates every ~10s

### 4.4 Anti-Idle / Linkdead Detection

The server tracks client states:

- `CLIENT_CONNECTING` → `CLIENT_CONNECTED` → `CLIENT_LINKDEAD` → `DISCONNECTED`

**Timeout triggers**:

- Transport-level stale connection: **60 seconds** of no packets → linkdead
- The KeepAlive opcode (0x06) every 9s prevents this
- No separate "anti-AFK" packet exists — the transport keepalive suffices
- EQEmu does not enforce a mandatory position update rate — a client can sit idle
  sending only keepalives indefinitely

---

## 5. Daybreak Live Server Differences

### 5.1 Login Flow

Live servers use the **Daybreak LaunchPad** which adds an HTTP/HTTPS authentication layer
before the UDP game protocol:

1. LaunchPad authenticates via HTTPS to Daybreak's web auth service
2. Receives a **session ticket** (opaque token)
3. LaunchPad launches `eqgame.exe` with the session ticket as a command-line parameter
4. `eqgame.exe` presents the ticket to the login server instead of raw credentials
5. The `eqhost.txt` file contains the login server address

This means a headless client targeting live servers would need to:

- Replicate the HTTPS auth flow to obtain a session ticket
- Present the ticket via the same OP_Login mechanism

### 5.2 Opcode Rotation

Live servers **change application-layer opcodes with every patch**. The transport layer
opcodes (0x01–0x15) remain stable. Application opcodes must be re-discovered after each
patch via packet capture and pattern matching.

EQEmu maintains patch-specific opcode maps in files like `patch_RoF2.conf`, `patch_SoD.conf`
that translate between internal canonical opcodes and client-version-specific opcodes.

### 5.3 Client Validation

Live Daybreak servers perform:

- **File CRC checks** via `OP_World_Client_CRC1` / `OP_World_Client_CRC2` — validates
  specific client files haven't been modified
- **Class variable integrity checks** — server-side validation (since ~2006)
- **Warp detection** — position update velocity validation
- **No Warden-style kernel anti-cheat** — EQ does not use a kernel driver like Warden/Vanguard

### 5.4 Truebox Enforcement

TLP (Time-Locked Progression) servers enforce "truebox" rules:

- Detects multiple EQ instances on the same machine
- Detects some virtual machines
- Can detect MacroQuest injection (DLL in process memory)
- **Protocol-level**: No known packet-level truebox validation — detection is client-side
  and reports back to the server

---

## 6. Anti-Cheat Analysis (Protocol Level)

### 6.1 What the Server Validates

| Check                 | Mechanism                      | Severity           |
| --------------------- | ------------------------------ | ------------------ |
| Position warp         | Velocity vs. previous position | Kick/flag          |
| CRC mismatch          | Transport CRC with session key | Packet rejected    |
| Session key expired   | Login → World handoff timeout  | Connection refused |
| File CRC (live only)  | OP_World_Client_CRC1/CRC2      | Kick               |
| Stat request response | SessionStatRequest/Response    | Timeout disconnect |

### 6.2 What the Server Does NOT Validate (EQEmu)

- No mandatory position update cadence (keepalives suffice)
- No client binary hash verification
- No memory scanning or module enumeration
- No encrypted/signed packets beyond transport CRC
- No behavioral heuristics (cast timing, reaction time, etc.)

### 6.3 P99 Additional Protections

Project1999 adds a custom anti-cheat thread in a modified client:

- **ModuleChecker**: Enumerates loaded DLLs via `CreateToolhelp32Snapshot` + `Module32First`,
  checks names against a blacklist (MacroQuest, ShowEQ, etc.)
- **ProcessChecker**: Walks the Windows Z-order via `GetTopWindow`, checks window titles
  against a list of known cheat tools
- **System info collection**: Computer name, username, IP — used for multi-box detection
- **Themida/WinLicense obfuscation** on the anti-cheat DLL (`dsetup.dll`)

These are all **client-side** checks that report flags back to the server — they can be
bypassed by a headless client that simply never runs the checker code.

---

## 7. Headless Client — Existing Implementations

### 7.1 EQEmu Official Headless Client (`hc/`)

The EQEmu Server repository contains an official headless client in the `hc/` directory:

- `hc/main.cpp` — Entry point, loads `hc.json` config
- `hc/login.h` — Login protocol handling
- `hc/world.cpp` — World server connection + character select
- `hc/eq.cpp` — Zone client, packet handling

Uses `DaybreakConnection` class from `common/net/` for transport.
Requires OpenSSL or mbedTLS for login encryption.

### 7.2 EQNet Library (Zaela)

**Repository**: https://github.com/Zaela/EQNet

Clean C library implementing the full client protocol stack:

- `EQNet_LoginToServerSelect()` — Login with credentials
- `EQNet_LoginToWorld()` — Server selection
- `EQNet_WorldToZone()` — Character select + zone entry
- `EQNet_KeepAlive()` — Manual keepalive sending
- `EQNet_SendRawPacket()` — Arbitrary opcode injection
- Full ACK manager with retransmission
- Fragment reassembly
- Event-driven polling (`EQNet_Poll`)

Supports multiple client versions via `EQNet_SetClientVersion()`.

### 7.3 AkkStack Headless

Docker Compose environment that packages the EQEmu headless client for automated testing
and server management. Demonstrates that headless operation is fully supported on EQEmu
infrastructure.

---

## 8. Key Packet Structures Reference

### PlayerProfile_Struct (~19,500 bytes)

Core character data sent on zone entry:

- `name[64]`, `last_name[32]`, `level`, `class_`, `race`, `gender`
- Stats: `STR`, `STA`, `AGI`, `DEX`, `WIS`, `INT`, `CHA`
- Currency: `platinum`, `gold`, `silver`, `copper` (inventory + bank)
- Spells: `spell_book[SPELLBOOK_SIZE]`, `mem_spells[SPELL_GEM_COUNT]`
- Buffs: `SpellBuff_Struct buffs[BUFF_COUNT]`
- Position: `x`, `y`, `z`, `heading`
- Bind points: `BindStruct binds[5]`

### Spawn_Struct (~383+ bytes)

Entity definition for each mob/player in zone:

- `name[64]`, `race`, `class_`, `level`, `gender`
- Position: `x`, `y`, `z`, `heading:12`
- `spawnId` (uint32), `runspeed`, `walkspeed`
- `equipment` (TextureProfile)

### SpawnAppearance_Struct (8 bytes)

State change broadcast:

- `spawn_id` (uint16), `type` (uint16), `parameter` (uint32)
- Types include: animation state, AFK, linkdead, invisible, etc.

### BeginCast_Struct / CastSpell_Struct

- `caster_id`, `spell_id`, `cast_time` (ms)
- `slot`, `target_id`, position coordinates

---

## 9. Implications for DMFT Headless Client

### What's Needed

1. **Transport layer**: Implement the Reliable Stream protocol (Connect, Ack, Fragment,
   Combined, KeepAlive, CRC). This is stable across patches — ~500 lines of Rust.

2. **Session management**: Handle three sequential UDP sessions (Login → World → Zone)
   with session key passthrough.

3. **Application codec**: Serialize/deserialize the key structs (position updates, spawn
   data, spell casts). Opcodes change per patch — need a configurable opcode table.

4. **Keepalive loop**: Send KeepAlive (0x06) every 9 seconds + respond to SessionStatRequest.

5. **Position update loop**: Send OP_ClientUpdate ~4x/sec when "moving", ~0.1x/sec when idle.
   Must be physically plausible (velocity-consistent) to avoid warp detection.

### What's NOT Needed (for EQEmu targets)

- LaunchPad HTTPS auth (EQEmu uses direct credential login)
- Client file CRC responses (EQEmu stubs these)
- Anti-cheat module (no client-side scanner on EQEmu)
- Kernel-level evasion (no anti-cheat driver)

### Risks for Live Daybreak Servers

- Opcode rotation every patch requires a discovery/update pipeline
- File CRC checks require knowing which files and their expected hashes
- LaunchPad session ticket flow requires HTTPS reverse engineering
- Truebox detection is client-side — a headless client wouldn't trigger it, but also
  wouldn't send the expected "I'm clean" reports, which could itself be suspicious
- Behavioral analysis (if any) would need humanized timing patterns

---

## 10. Source References

- [EQEmu Server Repository](https://github.com/EQEmu/Server) — full server source
- [EQEmu Packet & Opcode Analysis Docs](https://docs.eqemu.io/developer/packet-and-opcode-analysis/)
- [EQEmu opcodes.conf](https://github.com/EQEmu/Server/blob/master/utils/patches/opcodes.conf)
- [EQEmu eq_packet_structs.h](https://github.com/EQEmu/Server/blob/master/common/eq_packet_structs.h)
- [EQEmu Reliable Stream Structs](https://github.com/EQEmu/Server/blob/master/common/net/reliable_stream_structs.h)
- [EQEmu Reliable Stream Connection](https://github.com/EQEmu/Server/blob/master/common/net/reliable_stream_connection.h)
- [EQNet Client Library (Zaela)](https://github.com/Zaela/EQNet)
- [OpenEQC Login Protocol Spec](https://github.com/aceoyame/OpenEQC/blob/master/LS/Login/Protocol.txt)
- [P99 Spawn Decryption Analysis](https://medium.com/@Packet99/decrypting-project1999-spawns-7248acb1797b)
- [P99 Anti-Cheat Analysis](https://medium.com/@Packet99/understanding-project1999-protections-b62b686f7c7e)
- [EQEmu Server Network Architecture Forum Thread](https://www.eqemulator.org/forums/showthread.php?t=39843)
- [AkkStack Headless](https://github.com/wcassis/akk-stack-headless)
- [ShowEQ Project](https://github.com/ShowEQ/ShowEQ)
