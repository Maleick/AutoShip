# Packet Engine: Send and Receive Pipeline Inventory

Curated `M5` send/receive pipeline inventory for the DMFT packet engine capability boundary.

This document turns the April 2026 network architecture import into an explicit send/receive surface map for DMFT. It does not treat imported reverse-engineering notes as proof that DMFT already supports packet-first execution. The current evidence state for all receive-side and packet-sender rows remains `Needs Live Proof` unless explicitly marked otherwise.

## Scope

- map the EQ outbound (send) and inbound (receive) packet layers to DMFT's current control posture
- label each layer as `In-process`, `Packet candidate`, or `Blocked` relative to existing DMFT code
- identify which layers require anti-cheat counter handling before any packet-first work can be safe
- provide a bounded inventory that feeds the M5 exit gate

## Source Stack

Primary repo inputs:

- `docs/implementation-roadmap.md`
- `dmft-dll/src/eq/mod.rs`
- `dmft-dll/src/hooks/game_loop.rs`
- `dmft-common/src/ipc.rs`
- `dmft-common/src/offsets.rs`

Curated import summaries:

- `docs/research-imports/2026-04-02-packet-zoning/EQ_Network_Architecture.md`
- `docs/research-imports/2026-04-02-packet-zoning/EQ_Ability_Packet_Structures.md`

Related M5 ledgers:

- `docs/external-research/packet-zoning-send-path-and-state-ledger.md`
- `docs/external-research/ability-packet-coverage-and-targetability-validation.md`

## Send Pipeline Inventory

### Layer 1: High-level game helpers

These are EQ-internal functions that DMFT already calls in-process via the DLL.

| Function | Preferred-base address | Purpose | DMFT status |
| --- | --- | --- | --- |
| `Cmd_UseSkill` | `0x140238640` | Self-only skills (Mend, Feign, Hide) | `In-process` — reached via `UseSkill` wrapper in `dmft-dll/src/eq/mod.rs` |
| `CharacterZoneClient__UseSkill` | `0x1401015d0` | Skill execution with class checks | `In-process` — same path as above |
| `DoCombatAbility` | `0x1401015d0` (mapped via `PcZoneClient`) | Combat abilities via `do_combat_ability` | `In-process` — reached via `do_combat_ability` in `dmft-dll/src/eq/mod.rs` |
| `SendAttackPacketToServer` | `0x140249530` | Combat attack packets (opcode `0x612D`) | `Packet candidate` — opcode documented in import, but DMFT does not call this address directly; combat loop uses in-process path instead |
| `CEverQuest__SendZoneRequestPacket` | `0x1402906e0` | Zone transition requests | `Blocked` — no DMFT dispatch; zoning is tracked in `M6/#50` |
| `CEverQuest__SendEmoteOrSayPacket` | `0x140292c20` | Chat messages and emotes | `Blocked` — `Say`/`Emote` IPC commands exist in `dmft-common/src/ipc.rs` but the DLL dispatcher does not implement them |

Current evidence state: `Research-backed` for address and function identity; `Needs Live Proof` for current-build correctness and anti-cheat side effects.

### Layer 2: NetworkSend (primary send path)

The import identifies `NetworkSend` at `0x140550030` as the primary send path for skills, abilities, and most packet types. It dispatches based on a `type` argument:

| Type | Path | Used for |
| --- | --- | --- |
| 0 | `NetworkSend_QueuePacket` (queued reliable) | standard skill and ability packets |
| 1 | `NetworkSend_EncryptAndTransmit` (immediate) | time-critical transmissions |
| 2 | `NetworkSend_QueuePacket` (sequenced) | ordered reliable delivery |
| 3 | `NetworkSend_EncryptAndTransmit` (sequenced) | ordered immediate |
| 4–7 | `PacketFragment_AppendData` → `NetworkSend_TransmitToSocket` | fragmented large payloads |

DMFT status: `Blocked` — DMFT does not call `NetworkSend` directly. All current sends go through the high-level in-process helpers in Layer 1.

Current evidence state: `Research-backed` for function address and dispatch table; `Needs Live Proof` for type routing and counter behavior on the current build.

### Layer 3: UdpConnection::SendMessage (alternate path)

The import identifies `UdpConnection::SendMessage` as a lower-level path used primarily by position packets (opcode `0x1643`) and currently hooked by MQ2reachit for movement observation.

DMFT status: `Blocked` — DMFT does not use this path. The navigation system writes heading and speed fields directly via in-process memory writes rather than injecting movement packets.

Current evidence state: `Research-backed` for the existence and use by MQ2reachit; `Needs Live Proof` for current-build address and packet format.

### Packet scrambling and anti-cheat counters

The import identifies two layers that any future packet-first implementation would need to handle before sends are safe:

1. **Opcode scrambling**: `func_0x00014067b9e0` converts internal opcodes to network opcodes before `NetworkSend`. Any direct call to `NetworkSend` that bypasses Layer 1 helpers must scramble the opcode first.

2. **Anti-cheat counters**:

| Variable | Role |
| --- | --- |
| `_g_CounterA` | Receive counter, decremented on receive |
| `_g_CounterB` | Send counter, decremented after each `NetworkSend` call |
| `_g_CounterC` | Teleport counter |

Counter behavior: each counter is decremented after the relevant event and refilled by `+0x37` (55) when the value drops below 2. The server validates synchronization. Any injected send that bypasses the game's `NetworkSend` path must decrement `_g_CounterB` or skip the game's decrement to stay in sync.

DMFT status: `Blocked` — DMFT currently has no counter tracking, opcode scrambler call, or packet injector. These are hard requirements for any packet-first capability work.

Current evidence state: `Research-backed` for the existence and general behavior; `Needs Live Proof` for current-build counter addresses and refill thresholds.

## Receive Pipeline Inventory

The receive pipeline is currently observational only in DMFT. The DLL uses `ReadProcessMemory` and shared-memory IPC to observe game state rather than tapping the inbound packet stream.

### Layer 1: Socket reception

The import identifies `NetworkSession_HandleIncomingPacket` at `0x14055a080` as the raw UDP receive entry point. It handles fragmentation reassembly and decryption.

DMFT status: `Blocked` — DMFT does not hook this function. Game state is observed via shared memory and EQ struct reads, not packet interception.

Current evidence state: `Research-backed` for function address; `Needs Live Proof` for current-build correctness.

### Layer 2: World message handler

The import identifies two instances of `CEverQuest__HandleWorldMessage` (`0x1401ac200` and `0x1402c20a0`) as the opcode dispatch layer for inbound packets.

DMFT status: `Blocked` — DMFT does not hook these handlers. Hooking them would be the natural entry point for inbound packet observation, but no M5 task currently requires it.

Current evidence state: `Research-backed` for existence and dual-instance pattern; `Needs Live Proof` for current-build addresses and dispatch behavior.

### Layer 3: Packet processors

| Function | Address | Purpose | DMFT status |
| --- | --- | --- | --- |
| `ProcessGroupPacket` | `0x1402de700` | Group membership and updates | `Blocked` — group state read from EQ structs, not from packet hook |
| `ProcessEmotePacket` | `0x14020cea0` | Inbound emote handling | `Blocked` — no emote receive path in DMFT |
| `ProcessChannelMessagePacket` | `0x14020a1b0` | Chat channel messages | `Blocked` — no in-process chat receive path; candidates for `M10` Soul Engine chat |
| `ProcessBazaarPacket` | `0x14020eab0` | Bazaar transaction data | `Blocked` — economy work is `M11` |
| `CEverQuest__ProcessWorldPacket` | `0x1401e6d00` | General world state updates | `Blocked` — world state read from EQ memory, not from packet hook |
| `CEverQuest__ProcessZonePacket` | `0x1402802b0` | Zone-specific inbound packets | `Blocked` — zoning work is `M6` |

Current evidence state: `Research-backed` for address set; `Needs Live Proof` for current-build addresses and dispatch coverage.

## Capability Boundary Summary

The table below formalizes the M5 packet engine capability boundary for DMFT. It answers the question: for each major packet surface, what does DMFT support today and what is explicitly out of scope until further validation?

| Surface | Current DMFT capability | Packet-first status | Requirement to unblock |
| --- | --- | --- | --- |
| Self-only skills (Mend, Feign, Hide) | `In-process` via `Cmd_UseSkill` / `UseSkill` wrapper | `Packet candidate` | Live-proof opcode, payload shape, and `_g_CounterB` handling |
| Combat abilities (kick, bash, spells) | `In-process` via `DoCombatAbility` / `CastSpell` | `Packet candidate` | Live-proof opcode, target rules, counter handling, melee-range constraints |
| Attack packets (Flying Kick, combat attacks) | `In-process` via combat FSM | `Packet candidate` (high-risk) | Live-proof stricter server validation noted in import; treat as last to promote |
| Chat and emotes (Say, Emote) | `Blocked` — IPC enum exists, no DLL dispatch | `Blocked` | Map to `SlashCommand`/`InterpretCmd` first; packet path only after message encoding and target rules are explicit |
| Zone requests | `Blocked` — no DMFT zone send surface | `Blocked` — `M6` dependency | Complete M5 ability and control-matrix validation first |
| Inbound packet observation | `Blocked` — game state read from EQ structs | `Blocked` | No M5 task currently requires hooking receive path |
| Direct `NetworkSend` injection | `Blocked` | `Blocked` | Requires opcode scrambler call and counter handling before any use |
| Anti-cheat counter tracking | `Blocked` | `Blocked` | Hard prerequisite for any packet-first send path |

## M5 Exit Gate Contribution

This inventory contributes to the M5 exit gate as follows:

### Satisfied

- packet control paths are labeled with explicit acceptance criteria (`In-process`, `Packet candidate`, or `Blocked`)
- receive pipeline is inventoried and current DMFT posture is documented

### Open (needs live validation to close)

| Unknown | Required evidence | Current state |
| --- | --- | --- |
| Self-only skill opcode correctness on current build | Live client capture confirming opcode and payload shape | `Needs Live Proof` |
| `_g_CounterB` current-build address and refill threshold | Live client inspection or pattern scan | `Needs Live Proof` |
| `CEverQuest__HandleWorldMessage` current-build address | Pattern scan or live inspection | `Needs Live Proof` |
| Combat ability packet acceptance without melee-state dependency | Live client test with a target-only skill | `Needs Live Proof` |
| Chat packet encoding and tell-target rules | Live client test with `SendEmoteOrSayPacket` | `Needs Live Proof` |

### Explicit provisional items (not proof of milestone completion)

- `UdpConnection::SendMessage` current-build address and movement packet format
- `Intimidation` and `Begging` packet structure (see `ability-packet-coverage-and-targetability-validation.md`)
- Hook detection evasion requirements (tracked in `M7`)

## Follow-On Slice Candidates

- `#60` (`ability-packet-coverage-and-targetability-validation.md`): per-ability targetability, range, and live-proof rules
- `#50` (zoning): zone request state machine, failure codes, and receive-side zone acknowledgment (feed from `ProcessZonePacket`)
- `M7` anti-cheat: counter tracking, opcode scrambler integration, and hook exposure review
- Future receive-side work: `ProcessChannelMessagePacket` hook candidate for Soul Engine chat ingest (`M10`)
