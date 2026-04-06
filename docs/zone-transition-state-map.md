# Zone Transition State Map

Operator-facing reference for EQ zone transitions: states, failure codes, recovery actions, and validation tasks.

Synthesized from Ghidra RE (`docs/wiki/Research-EQ-Zoning-System.md`), the packet/zoning ledger (`docs/wiki/Research-Packet-Zoning.md`), the zoning validation doc (`docs/wiki/Research-Zoning-Validation.md`), and DLL codebase inspection. Evidence state labels follow the implementation roadmap convention: **Validated** (observed on live client), **Research-backed** (from Ghidra RE, high confidence), **Needs Live Proof** (plausible but unconfirmed on current build).

---

## 1. State Machine

### Game States (CEverQuest offset `0x5E4`)

| State | Hex    | Name            | Description                                  |
| ----- | ------ | --------------- | -------------------------------------------- |
| 0xFF  | `0xFF` | Idle            | Not zoning, normal in-game or pre-game       |
| 0x01  | `0x01` | InGame          | All opcode handlers enabled, gameplay active |
| 0x02  | `0x02` | CharacterSelect | Character select screen                      |
| 0x03  | `0x03` | LoadingZone     | Zone data loading in progress                |
| 0x05  | `0x05` | Exiting         | Cleanup/logout in progress                   |
| 0xFD  | `0xFD` | ZoneTimeout     | Zone load timed out (~120s)                  |

### Transition Flow

```
                        ┌─────────────────────────────────────────────┐
                        │                                             │
   InGame (0x01)        │   Zone Trigger                              │
       │                │   (zone line, /zone, evac, port, server)    │
       ▼                │                                             │
  FlushMovementQueue    │                                             │
       │                │                                             │
       ▼                │                                             │
  ValidateZoneRequest ──┼── returns 1 (success)                       │
       │                │       │                                     │
       │ (failure)      │       ▼                                     │
       │                │  ExecuteZoneTransition                      │
       ▼                │       │                                     │
  Display Error ────────┘       ├── Store dest coords (X, Y, Z, H)   │
  Return to Safe Coords        ├── Send ZoneRequest (0x3937)         │
                                ├── Disable gameplay handlers         │
                                ▼                                     │
                         LoadingZone (0x03)                           │
                                │                                     │
                      ┌─────────┼─────────┐                          │
                      │         │         │                           │
                      ▼         ▼         ▼                           │
                   Zone data  Timeout   Zone_Handler                  │
                   arrives    (~120s)   (anti-cheat)                  │
                      │         │         │                           │
                      │         ▼         │                           │
                      │    ZoneTimeout    │                           │
                      │    (0xFD)         │                           │
                      │         │         │                           │
                      │         ▼         │                           │
                      │    Error/DC       │                           │
                      │                   │                           │
                      ▼                   │                           │
                   Initialize Zone        │                           │
                      │                   │                           │
                      ├── SetGameState(1) │                           │
                      ├── Enable handlers │                           │
                      ▼                   │                           │
              MoveLocalPlayerToSafeCoords │                           │
                      │                   │                           │
                      ▼                   │                           │
                   InGame (0x01) ─────────┘                           │
                      │                                               │
                      ▼                                               │
                Send completion packets                               │
                (0x60F7, 0x6A30, 0x5F58, 0x66F8)                     │
                                                                      │
```

### Zone Trigger Types (R12D parameter to ExecuteZoneTransition)

| Value | Source                       | Meaning               | Evidence State   |
| ----- | ---------------------------- | --------------------- | ---------------- |
| 0     | Packet capture               | Zone line alternate   | Needs Live Proof |
| 1     | Packet capture               | Initial request       | Needs Live Proof |
| 2     | ProcessWorldPacket           | Server-initiated zone | Research-backed  |
| 3     | ProcessWorldPacket           | Evacuation (partial)  | Research-backed  |
| 4     | HandleZoneLineCrossing       | Zone line crossing    | Research-backed  |
| 5     | ProcessWorldPacket           | Server alternate      | Research-backed  |
| 7     | Packet capture               | Portal                | Needs Live Proof |
| 8     | HandleSummonTeleport         | Summon/teleport spell | Research-backed  |
| 9     | HandleZoneConfirmationPacket | Zone confirmation     | Research-backed  |
| 10    | ProcessWorldPacket           | Evacuation full       | Research-backed  |
| 11    | Packet capture               | Gate/evac spell       | Needs Live Proof |
| -1    | Packet capture               | Server approved       | Needs Live Proof |

### Zone Type Bit Flags (bitmask `0x80001C1` at `0x140282058`)

| Bit | Meaning       |
| --- | ------------- |
| 0   | Normal zone   |
| 6   | Evacuation    |
| 7   | GM summon     |
| 8   | Unknown       |
| 31  | Instance zone |

---

## 2. Failure Codes

Returned by `CEverQuest::ValidateZoneRequest` (`0x14029b130`), checked by `ExecuteZoneTransition`.

| Code | Hex          | String ID | Failure                      | Observable Symptom                                         | Evidence State  |
| ---- | ------------ | --------- | ---------------------------- | ---------------------------------------------------------- | --------------- |
| 1    | `0x01`       | -         | **Success**                  | Zone loading screen appears                                | Research-backed |
| -2   | `0xFFFFFFFE` | -         | Retry / temporary denial     | No visible error; client may re-attempt via alternate path | Research-backed |
| -3   | `0xFFFFFFFD` | 5049      | Zone locked (flag/expansion) | "You cannot enter this zone" message                       | Research-backed |
| -4   | `0xFFFFFFFC` | 5050      | Zone full (pop cap)          | "This zone is at capacity" message                         | Research-backed |
| -5   | `0xFFFFFFFB` | -         | Invalid zone ID              | No message (zone ID doesn't resolve)                       | Research-backed |
| -7   | `0xFFFFFFF9` | 5052      | Access denied                | "You do not have permission" message                       | Research-backed |
| -8   | `0xFFFFFFF8` | 252       | Zone unavailable (down)      | "Zone is unavailable" message                              | Research-backed |
| -9   | `0xFFFFFFF7` | -         | Zone string lookup needed    | No visible error; internal name resolution step            | Research-backed |
| -10  | `0xFFFFFFF6` | -         | Chat message needed          | Special message display (informational)                    | Research-backed |
| -12  | `0xFFFFFFF4` | 0x1DB9    | Level too low                | Error message; player restored to safe coords              | Research-backed |

### Timeout Failure

| Condition             | Observable Symptom                                    | Evidence State   |
| --------------------- | ----------------------------------------------------- | ---------------- |
| Zone load > ~120s     | Game state transitions to `0xFD` (ZoneTimeout)        | Research-backed  |
| Anti-cheat rejection  | DC during zone (Zone_Handler fingerprint mismatch)    | Needs Live Proof |
| Network loss mid-zone | Loading screen hangs, eventually timeout or client DC | Needs Live Proof |

---

## 3. Recovery Actions

### Per-Failure Recovery Matrix

| Failure                   | Operator/Automation Action                                                                                          | Priority |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------- | -------- |
| **Retry (-2)**            | Wait 2-5s, retry the zone request. If 3 consecutive retries fail, escalate to Blocked.                              | Auto     |
| **Zone locked (-3)**      | Alert operator: character lacks flag or expansion. Do NOT retry. Mark slot as Blocked with "flag required" reason.  | Alert    |
| **Zone full (-4)**        | Queue retry with exponential backoff (5s, 10s, 20s, max 60s). Alert after 3 minutes. Common on TLP launch days.     | Auto     |
| **Invalid zone (-5)**     | Log error with zone ID. This is a bug in the zone routing table. Mark slot Blocked, alert operator.                 | Alert    |
| **Access denied (-7)**    | Alert operator: permission or GM restriction. Do NOT retry.                                                         | Alert    |
| **Zone unavailable (-8)** | Server-side zone is down. Retry every 30s up to 5 minutes, then alert. Zone may be rebooting after crash.           | Auto     |
| **String lookup (-9)**    | Internal resolution step, not a true failure. Wait for client to resolve and re-check game state.                   | Ignore   |
| **Chat message (-10)**    | Informational, not a failure. Log and continue.                                                                     | Ignore   |
| **Level too low (-12)**   | Character will be restored to safe coords. Alert operator: wrong zone for this character's level.                   | Alert    |
| **Timeout (0xFD)**        | Zone load exceeded ~120s. Kill process, increment restart count, re-launch. Investigate if recurring.               | Auto     |
| **Anti-cheat DC**         | Disconnected during Zone_Handler fingerprint. This may indicate detection. Pause fleet, alert operator immediately. | Urgent   |
| **Network loss**          | Client hangs on loading screen. Wait for timeout (120s), then treat as timeout failure.                             | Auto     |

### Recovery State Machine Integration

These failures map into the orchestration loop's `SlotLifecycle` (from `docs/design/orchestration-loop.md`):

```
Zone failure detected
    │
    ├── Retryable (-2, -4, -8, timeout)
    │       → Stay in Live state
    │       → Retry zone with backoff
    │       → After max retries → Recovering → Relaunching
    │
    ├── Non-retryable (-3, -5, -7, -12)
    │       → Transition to Blocked
    │       → Alert operator via TUI toast + Discord
    │       → Requires manual intervention or config change
    │
    └── Detection signal (anti-cheat DC)
            → Transition ALL slots to Blocked (fleet pause)
            → Urgent operator alert
            → Do NOT auto-restart
```

---

## 4. Validation Tasks

Live tests to run on Frostreaver to confirm each state and failure path. Each task should record: trigger, observed game state value, visible client behavior, and recovery outcome.

### 4.1 Normal Zone Line Crossing

**Target:** Walk a character across a zone line (e.g., PoK → Nexus).

**Record:**

- Game state sequence: InGame → LoadingZone → InGame
- Whether `FlushMovementQueue` effect is observable (nav/movement stops before zone handoff)
- Time from trigger to zone complete
- Completion packets sent (if packet logging is active)

**Pass:** Game state transitions match `0x01 → 0x03 → 0x01` sequence. Movement stops cleanly before loading.

**Evidence state:** Needs Live Proof

### 4.2 Server-Initiated Zone (Evac/Port)

**Target:** Have a druid or wizard cast Gate, Evacuate, or a port spell.

**Record:**

- Zone type value (expect type 3, 8, 10, or 11)
- Whether same game state sequence applies
- Any difference in timing vs zone line crossing

**Pass:** Game state transitions are identical to zone line; zone type differs.

**Evidence state:** Needs Live Proof

### 4.3 Queue Flush Checkpoint

**Target:** Zone while the Navigator FSM is actively issuing movement commands.

**Record:**

- Whether nav movement stops before zone handoff or bleeds into loading
- Any leftover movement burst after the zone request
- Whether DLL `dispatch_command` queue is drained

**Pass:** Movement ceases before `LoadingZone` state is entered. No stale movement commands execute after zone.

**Evidence state:** Needs Live Proof

### 4.4 Zone Timeout

**Target:** Trigger a zone load timeout (controlled test: network interruption during zone, or zone to an intentionally slow-loading zone if available).

**Record:**

- Elapsed time from zone handoff to timeout
- Game state value during wait and at timeout (expect `0x03 → 0xFD`)
- Client behavior after timeout: returns to previous zone? Shows error? DCs?
- Whether the client is recoverable without restart

**Pass:** Timeout occurs near ~120s mark. Game state reaches `0xFD`. Recovery behavior is documented.

**Evidence state:** Needs Live Proof

### 4.5 Zone Full (Pop Cap)

**Target:** Attempt to zone into a full zone (may be difficult to reproduce on Frostreaver; note if untestable).

**Record:**

- Return code (expect -4)
- Client-visible message
- Whether client stays in origin zone cleanly

**Pass:** Client displays capacity message and remains in origin zone without state corruption.

**Evidence state:** Needs Live Proof (may require TLP launch-day conditions)

### 4.6 Level-Gated Zone Denial

**Target:** Attempt to zone into a level-restricted zone with an under-leveled character.

**Record:**

- Return code (expect -12)
- Whether `MoveLocalPlayerToSafeCoords` fires
- Where the character ends up (safe coords in origin zone? bind point?)

**Pass:** Character is restored to safe coords. Error message is displayed.

**Evidence state:** Needs Live Proof

### 4.7 Anti-Cheat Fingerprint During Zone

**Target:** Observe `Zone_Handler` behavior during a normal zone transition (passive observation, not avoidance).

**Record:**

- Whether `Zone_Handler` fires during every zone
- Whether the hardware fingerprint + memcheck call sequence is visible in logs
- Any observable latency from the anti-cheat pass

**Pass:** `Zone_Handler` fires consistently during zone. No DC on clean client.

**Evidence state:** Needs Live Proof

### 4.8 Safe-Coordinate Recovery

**Target:** After a denied zone or timeout, observe where the client ends up.

**Record:**

- Trigger category (denial vs timeout vs same-zone recovery)
- Resulting player position and whether it matches a known safe point
- Whether recovery is to the origin zone safe point or the destination zone safe point

**Pass:** At least one recovery case is confirmed with a clear trigger-to-position mapping.

**Evidence state:** Needs Live Proof

---

## 5. Implementation Notes

### Current TextQuest Codebase Mapping

| Research Concept                 | Codebase Location                                        | Status        |
| -------------------------------- | -------------------------------------------------------- | ------------- |
| Zone graph reading               | `textquest-dll/src/nav/zone_graph.rs`                    | Implemented   |
| Zone graph IPC                   | `textquest-common/src/ipc.rs` (`QueryZoneGraph` command) | Implemented   |
| Zone name reading                | `textquest-dll/src/hooks/game_loop.rs:read_zone_names()` | Implemented   |
| Game state reading               | `textquest-dll/src/hooks/game_loop.rs` (offset `0x5E4`)  | Observational |
| Zone request packet send         | Not implemented                                          | Blocked       |
| ValidateZoneRequest hooking      | Not implemented                                          | Blocked       |
| Zone_Handler (anti-cheat) bypass | Not implemented (M5 scope)                               | Blocked       |
| FlushMovementQueue integration   | Not implemented                                          | Blocked       |
| MoveLocalPlayerToSafeCoords hook | Not implemented                                          | Blocked       |

### Key EQ Function Addresses (preferred-base, must `rebase()`)

| Address       | Function                              | Relevance                           |
| ------------- | ------------------------------------- | ----------------------------------- |
| `0x140284e30` | `CEverQuest::ZoneLoadingStateMachine` | Master zone controller              |
| `0x1402816e0` | `ExecuteZoneTransition`               | Transition execution after approval |
| `0x14029b130` | `CEverQuest::ValidateZoneRequest`     | Zone request validation + codes     |
| `0x14027b100` | `Zone_Handler`                        | Anti-cheat during zone              |
| `0x14019d180` | `MoveLocalPlayerToSafeCoords`         | Post-zone positioning               |
| `0x140302720` | `FlushMovementQueue`                  | Pre-zone movement drain             |
| `0x14027da50` | `CEverQuest::SendZoneRequest`         | Sends 0x3937 packet                 |

### Recommended Hook Points for M7

1. **`ValidateZoneRequest` hook** — Intercept return codes to detect failures before the client displays error messages. Enables proactive recovery routing.

2. **Game state polling** — Read `CEverQuest + 0x5E4` every DLL tick to track `InGame → LoadingZone → InGame` transitions. This is the cheapest way to observe zone state without hooking.

3. **`ExecuteZoneTransition` hook** — Capture zone type and destination coords. Enables the orchestrator to know _where_ each client is going and _why_ (zone line vs evac vs port).

4. **`MoveLocalPlayerToSafeCoords` hook** — Detect when a client has been restored to safe coords (failure recovery or normal post-zone placement). Enables position validation after zone complete.

### IPC Extensions Needed

| New Command/Response                                       | Purpose                                          |
| ---------------------------------------------------------- | ------------------------------------------------ |
| `Response::ZoneStateChanged { state, zone_id, zone_type }` | DLL notifies orchestrator of zone transitions    |
| `Response::ZoneValidationFailed { code, zone_id }`         | DLL notifies orchestrator of zone denial         |
| `Command::RequestZone { zone_id }`                         | Orchestrator requests DLL to initiate zone (M7+) |

### Integration with Orchestration Loop

The `LifecycleManager` (from `docs/design/orchestration-loop.md`) should handle zone transitions as part of its health-check phase:

- **During `Live` state:** Poll game state via IPC. If `LoadingZone` (0x03) is detected, start a 120s watchdog timer.
- **On `InGame` return:** Reset watchdog, update zone name, verify client is in expected zone.
- **On `ZoneTimeout`:** Treat as crash, enter `Recovering` state.
- **On zone denial:** Check if retryable (see recovery matrix above). Either retry or transition to `Blocked`.
- **Zone mismatch after recovery:** If client zones into wrong zone (bind point after crash), mark as `Blocked` with "wrong zone" until M7 cross-zone nav is complete (per orchestration loop design doc, open question #4).

---

## Appendix: ZoneRequest Packet Structure (0x3937)

For reference only. TextQuest does NOT currently send this packet.

| Offset | Size | Field         | Notes                                                                                 |
| ------ | ---- | ------------- | ------------------------------------------------------------------------------------- |
| 0-1    | 2    | Opcode        | Scrambled `0x3937`                                                                    |
| 2-65   | 64   | CharName      | Null-terminated                                                                       |
| 66-69  | 4    | ZoneID        | Target zone (`uint32`)                                                                |
| 70-73  | 4    | InstanceID    | `-1` if none                                                                          |
| 74-77  | 4    | ExtInstanceID | `-1` if none                                                                          |
| 78-81  | 4    | Y             | Float (Y before X in wire order)                                                      |
| 82-85  | 4    | X             | Float                                                                                 |
| 86-89  | 4    | Z             | Float                                                                                 |
| 90-93  | 4    | ZoneType      | See zone trigger types table                                                          |
| 94-97  | 4    | Reserved      | Always 0                                                                              |
| 98-101 | 4    | ZoneFlags     | **Bug:** only low byte is meaningful (mask `& 0xFF`), upper 3 bytes are stack garbage |

Total: 102 bytes (2 opcode + 100 payload).

---

## Appendix: Opcode Handler Groups

The client enables/disables handler groups based on game state. This affects what packets the client processes during zone transitions.

| Handler IDs         | Active During            | Notes                  |
| ------------------- | ------------------------ | ---------------------- |
| 0x222-0x225         | InGame only              | Core gameplay handlers |
| 0x226-0x227         | InGame + CharacterSelect | Shared handlers        |
| 0x169, 0x21E, 0x196 | InGame only              | Additional gameplay    |
| 0x04, 0x05          | InGame only              | Base handlers          |

During `LoadingZone`, gameplay handlers are disabled. This prevents exploits but also means the DLL cannot send gameplay commands while zoning.
