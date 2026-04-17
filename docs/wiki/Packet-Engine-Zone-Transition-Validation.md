# Packet Engine Zone-Transition Validation

This page is the canonical validation ledger for issue `#1270` and the source
of truth for any claim that TextQuest's packet engine can validate zone
transitions on the current `master` checkout.

Use this page to separate what the repository can prove from what still needs a
live EverQuest client session. Do not promote packet-based zoning support or
packet-backed zone detection beyond the evidence state recorded here.

## Current Evidence State

| Claim | Evidence state | Repo basis | Notes |
| --- | --- | --- | --- |
| The DLL contains concrete `WSASend` / `WSARecv` detour code and can resolve the Winsock exports it wants to hook. | Research-backed | `textquest-dll/src/hooks/packet_hook.rs` | `packet_hook::install()` resolves `ws2_32.dll`, looks up both exports, and initializes retour detours. This is code proof only, not live proof. |
| The manager and TUI already have an end-to-end packet polling surface. | Research-backed | `textquest/src/orchestrator/mod.rs`, `textquest/src/tui/run.rs`, `textquest-dll/src/ipc/mod.rs` | `PollPackets` drains `Response::PacketEvent` entries into the packet monitor UI, so the operator surface exists if the DLL emits events. |
| Normal DLL startup installs the packet hooks automatically. | Invalidated | `textquest-dll/src/lib.rs` | `install_remaining_hooks()` installs render, chat, `set_game_state`, DX11, and detour manager hooks, but it never calls `packet_hook::install()`. The current checkout therefore has no normal packet-hook activation path. |
| The packet parser is robust against partial packets, multi-buffer sends, overlapped receives, and out-of-order delivery. | Blocked | `textquest-dll/src/hooks/packet_hook.rs` | The detours read only the first `WSABUF`, drop inbound packets shorter than four bytes, and explicitly skip overlapped `WSARecv` completion paths. The code records an opcode and payload length only; it does not reassemble or reorder traffic. |
| Packet buffering is lossless enough to prove zone transitions under heavy load. | Invalidated | `textquest-dll/src/ipc/mod.rs` | The IPC queue is capped at `4096` responses and intentionally drops `Response::PacketEvent` entries when full so other responses survive packet floods. |
| The current issue verify steps match the current repo layout. | Invalidated | issue `#1270`, repo search | The issue still points at `textquest/src/process/packet_capture.rs`, expects packet HWBP hooks, and tells operators to use a `--packet-debug` flag that does not exist in the current CLI or TUI code. |
| Packet-based zone detection agrees with memory-based zone detection within 100 ms. | Blocked | repo search, `textquest/src/orchestrator/mod.rs`, `textquest-common/src/ipc.rs` | The repo captures packet timestamps, but no checked-in comparison harness correlates them with the zoning state machine or memory-based zone transitions. |
| TextQuest has live proof for the zoning packet sequence on a current TLP server. | Needs Live Proof | no in-repo capture artifact | This workspace contains no packet capture artifact, no checked-in session log, and no validated opcode sequence for live zone transitions. |

## Repo-Grounded Findings

- `packet_hook::install()` is real code, but it is dormant on the current
  checkout because the DLL startup path never calls it.
- The packet monitor UI is real, but it depends on packet events that the
  default DLL init path does not currently produce.
- The current implementation captures only enough information for a lightweight
  opcode monitor:
  - opcode from bytes `[2..4]`
  - direction
  - timestamp
  - payload size
- The current implementation does not provide proof for the edge cases that
  matter most for zoning validation:
  - multiple `WSABUF` entries in one send/recv call
  - asynchronous or overlapped receive completion
  - cross-thread ordering guarantees
  - lossless capture under packet flood
- The issue body is partly historical. The current packet hook path uses retour
  detours, not HWBP, and the orchestrator path now lives under
  `textquest/src/orchestrator/mod.rs`.

## What This Workspace Verified

- The packet hook code resolves `WSASend` and `WSARecv` by name and enables both
  detours in one install call.
- The orchestrator packet polling path is wired through `Command::PollPackets`
  and the TUI packet monitor.
- The current `master` checkout does not install the packet hooks during normal
  DLL initialization.
- The packet queue is intentionally lossy under pressure because packet events
  are dropped once the bounded IPC response queue fills up.
- The issue's repo-local verification recipe is stale and cannot be followed
  verbatim from this checkout.

## Needs Live Proof Before This Issue Can Close

- Run a packet-hook-enabled Windows build against a real EQ client and capture a
  complete zone transition from request to zone-entry steady state.
- Record the observed opcode order and confirm whether the expected zoning
  packets appear at all in the current client build.
- Correlate packet timestamps with memory-based zoning state and prove the
  packet path stays within the required `100 ms` agreement window.
- Measure packet loss and ordering under a representative multibox load rather
  than assuming the bounded IPC queue is sufficient.
- Decide whether dormant packet hooks should stay opt-in for validation only or
  become part of normal DLL startup after live proof exists.

## Current Conclusion

As of the current `master` checkout, TextQuest does **not** have live-proof
packet validation for zone transitions.

The repository has packet-capture primitives and a TUI/operator polling path,
but the hooks are not installed during normal DLL startup, the parser is not a
general reassembly or ordering engine, and the IPC queue can intentionally drop
packet events under load. Treat packet-based zone detection as blocked for `M7`
until a live Windows capture session produces current-build evidence.
