# Anti-Detection and Operator Risk

This document summarizes TextQuest's current anti-detection posture and the rules for promoting outside research into roadmap work.

It is intentionally evidence-focused. It does not promise stealth or guarantee safety.

## Canonical Inputs

Use these sources in this order:

1. current code and repo docs
2. `docs/external-research/daybreak-detection-digest.md`
3. official Daybreak policy pages linked from that digest
4. clearly labeled community reporting

## Current Measures in the Repo

Current practical measures include:

- randomized DLL staging names before injection
- session-derived IPC names instead of simple fixed names
- per-session authenticated IPC using the raw token as the first pipe message
- constant-time token comparison inside the DLL
- restrictive current-user DACLs for named pipes and shared memory
- movement humanization and timing variation in higher-level behavior
- render strobing for background clients
- GM flag visibility in operator tooling

## Exposure Review

Use these categories when `M5` work evaluates whether a change expands exposure. The point is to name the surface, point at the repo evidence, and keep the confidence level explicit.

| Category                      | Current repo surface                                                                                                                                                                 | Confidence | Why it matters                                                                                               |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------- | ------------------------------------------------------------------------------------------------------------ |
| Module presence               | `textquest/src/inject/loader.rs` injects `textquest-dll` with `CreateRemoteThread` + `LoadLibraryW`, and `textquest-dll` then remains loaded in `eqgame.exe`                                        | High       | A loaded third-party module is a concrete exposure surface even before any gameplay behavior is considered.  |
| Detour hooks                  | `textquest-dll/src/lib.rs` installs the game-loop and render hooks                                                                                                                        | High       | Hooked code paths create an exposure surface that should be reviewed separately from operator behavior.      |
| In-process function calls     | `textquest-common/src/ipc.rs`, `textquest-dll/src/hooks/game_loop.rs`, and the login/widget helpers execute `InterpretCmd`, UI clicks, and related internal calls inside the client process    | High       | Internal control paths can look different from external input simulation and need their own risk labeling.   |
| IPC naming and authentication | `textquest-common/src/ipc.rs`, `textquest-dll/src/ipc/pipe.rs`, and `textquest-dll/src/ipc/shared.rs` implement session-derived names, per-session raw token authentication, and current-user DACLs | High       | These reduce casual local exposure, but they do not remove host-level forensic or anti-cheat risk.           |
| Timing variation              | `textquest-dll/src/hooks/game_loop.rs`, `textquest-dll/src/nav/humanize.rs`, and `textquest-dll/src/combat/humanize.rs` apply command jitter, movement humanization, and behavior timing variation  | Medium     | These are practical hardening measures, not evidence of safety against any specific Daybreak detection path. |
| Operator environment          | Live machine cleanliness, runner hygiene, artifact handling, and avoiding unrelated cheat tooling                                                                                    | High       | Official Daybreak policy applies account-wide and is not limited to a single game session.                   |
| Community detection claims    | RedGuides, MMOBugs, and similar discussions                                                                                                                                          | Low        | Useful for hypotheses and validation tasks only; not strong enough to prove safety claims.                   |

### Confidence rules

- `High`: directly grounded in current TextQuest code or official Daybreak policy.
- `Medium`: grounded in current TextQuest code, but the actual anti-detection value is inferred rather than proven.
- `Low`: community reporting, speculative interpretation, or exploit-oriented claims without stronger corroboration.

## `M5` / `M7` / `M8` Validation Gates

Use these gates before documenting a risky path as supported or before expanding operator-facing behavior.

| Milestone            | Change type                                                                                              | Gate before keep or promotion                                                                                                                      | Default handling                                                                                   |
| -------------------- | -------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `M5` Anti-Cheat      | new hook, module footprint change, string or artifact exposure change, or anti-detection hardening claim | map the change to exposure categories, cite repo or official evidence, and record a confidence level                                               | official-policy-backed rules may tighten gates immediately; community-only claims stay provisional |
| `M7` Zoning/Movement | zone transition logic, queue flushing, safe-coord recovery, or teleport-style routing                    | name the risky transition, record the failure or recovery checkpoint, and define the live validation path before calling it normal workflow        | keep risky travel claims out of normal operator docs until validated                               |
| `M8` Orchestrator    | broader relay scope, launch/session routing change, or more visible automation behavior                  | prefer the lowest-exposure control path, keep routing scope visible to the operator, and cross-link any unresolved `M5`-`M7` validation dependency | block or defer behavior that silently widens packet, movement, or hook exposure                    |

### Required anti-cheat metadata

When a new `M5` task, issue, or doc slice is created, include:

- touched exposure categories
- confidence per claim
- source basis: official policy, repo-grounded observation, or community reporting
- outcome type: hard gate, operator checklist item, or validation follow-up

## Current Operator Implications

- authenticated IPC is tied to the injected session, not only to a PID
- reconnect-style flows depend on the retained `login_token_<pid>.bin` file
- if clients are relaunched or copied, stale token files should not be trusted
- machine hygiene matters because official Daybreak policy is broader than one single client session

## Operator Hygiene Checklist

Use this checklist before live testing, on the always-on runner, and when writing new `M5` tasks.

### Live machine hygiene

- keep live-play machines free of unrelated cheat tooling, reverse-engineering utilities, or abandoned test binaries that are not required for the current TextQuest workflow
- avoid reusing stale staged DLLs, copied token files, or mixed old/new build artifacts across client launches
- treat `%TEMP%/textquest` logs and token-bearing artifacts as sensitive operational data and clean them up when they are no longer needed
- separate high-risk research machines from normal live-play machines when testing speculative packet, zoning, or exploit-adjacent ideas

### Runner and build hygiene

- keep the self-hosted runner focused on build, test, wiki, and release work rather than live gameplay activity
- do not treat the runner as evidence that a live machine is clean; operator hygiene still has to be assessed per host
- prefer reproducible branch builds over hand-copied binaries so injected artifacts can be traced back to a reviewed commit

### Runtime discipline

- verify the exact branch, commit, and config set before injecting into a client
- prefer the lowest-exposure control path that satisfies the task instead of defaulting to packet or exploit-adjacent routes
- keep operator-visible behavior conservative enough that reports and social exposure do not become the primary risk vector
- record whether a validation result came from official policy, repo-grounded observation, or community reporting

### Escalation triggers

- if a task requires a new hook, a broader module footprint, or riskier movement/control behavior, label the exposure increase explicitly before implementation
- if a claim depends only on community reporting, keep it provisional and open a validation task instead of documenting it as settled guidance
- if a machine cannot be shown to be clean, treat that as an operator blocker rather than assuming current repo hardening is sufficient

## Evidence Rules

Use the following handling model:

- official Daybreak guidance can tighten milestone gates immediately
- primary docs and public code repos can create `Research-backed` slice candidates when repo fit is clear
- community reporting can suggest validation tasks or provisional slices
- exploit-oriented claims stay low-confidence until corroborated
- no community claim alone can satisfy an anti-cheat milestone exit gate

## What Not to Claim

Do not document any of the following as established fact unless they are backed by current code or stronger evidence:

- exact internal Daybreak scan mechanics
- reliable bypass claims
- speculative hook-restoration or scanner-evasion behavior
- exploit-grade zoning or travel shortcuts as normal operator features

## SME-Sourced TLP Anti-Cheat Intel (2026-04-03)

Primary research from Matt (Blownt) via Ghidra decompilation of eqgame.exe. No public documentation exists for any of this. Confidence: Medium (single SME source, awaiting independent Ghidra verification via #343).

### Confirmed Detection Systems

| System                     | Location                     | Frequency                          | What it checks                                                                        |
| -------------------------- | ---------------------------- | ---------------------------------- | ------------------------------------------------------------------------------------- |
| Inline byte count checks   | Main game loop body          | Continuous                         | Code section sizes — detects JMP patches that change byte counts                      |
| Memshift detection         | Main game loop body (inline) | Every ~3 minutes                   | Whether main loop memory has been modified since baseline                             |
| Movement agreement packets | Client network layer         | Every ~15ms (path summaries 1/sec) | Movement pattern matching — position, velocity, path data                             |
| A/B packet counter         | Send/receive pipeline        | Per packet                         | Send vs receive counter balance — injected or dropped packets cause drift             |
| Process enumeration        | Client startup / periodic    | Unknown                            | Running processes cross-correlated across all accounts on the machine                 |
| MAC address tracking       | Client startup               | Per session                        | Hardware fingerprinting across clients                                                |
| Computer name tracking     | Client startup               | Per session                        | Machine identity correlation                                                          |
| Memcheck 1-4               | Unknown (awaiting decompile) | Unknown                            | Server-initiated: server sends address range, client hashes region and returns result |

### Critical Constraints

Both inline systems (byte count + memshift) are **not separate functions** — they are woven into the main game loop code. They provide circular protection: patching one out triggers the other. The main loop is a no-touch zone.

Server-initiated memcheck can request a hash of any address range in .text — code section patches are directly detectable server-side.

### Recommended M7 Evasion Architecture

Based on research across C2/Red Team techniques and Matt's constraints:

| Layer       | Technique                                | Purpose                                                                                              | Risk                                                |
| ----------- | ---------------------------------------- | ---------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| Injection   | Reflective DLL / Manual mapping          | Avoid PEB module list, no disk artifact                                                              | VAD enumeration from kernel                         |
| Hooking     | Hardware breakpoints (DR0-DR3)           | Zero code bytes modified — immune to byte count, memshift, JMP checks                                | **VERIFIED SAFE** — GetThreadContext NOT in IAT     |
| Timing      | Early Bird APC injection                 | Hooks in place before AC baselines — modifications ARE the clean state                               | AC may init via kernel callback first               |
| Persistence | Gargoyle timer-based execution           | Code is RW (non-executable) between frames, briefly RX during execution — evades 3-min memshift scan | VirtualProtect call monitoring, ROP chain detection |
| Stealth     | PEB module unlinking + PE header erasure | DLL invisible to module enumeration                                                                  | VirtualQuery address space walk                     |
| Identity    | Per-client MAC + hostname spoofing       | Break hardware cross-correlation across 36 clients                                                   | Deeper HWID fingerprinting                          |
| Threads     | Thread pool callbacks (PoolParty)        | Execute on legitimate OS worker threads, no suspicious CreateRemoteThread                            | TP structure enumeration                            |
| Syscalls    | Direct syscalls (SysWhispers/HellsGate)  | Bypass ntdll hooks the AC may have installed                                                         | 0F 05 scanning outside ntdll                        |
| Data hooks  | VTable / IAT patching                    | Safer than .text patches — memcheck primarily scans code sections                                    | Pointer integrity validation                        |

### Techniques Assessed and Rejected

| Technique                | Why rejected                                                                    |
| ------------------------ | ------------------------------------------------------------------------------- |
| VEH + PAGE_GUARD hooking | Performance catastrophe on hot code paths — fires on every page access          |
| Process hollowing        | Requires new process creation, breaks authentication flow                       |
| Instrumentation callback | Trivially detected via NtQueryInformationProcess, only one callback per process |
| Standard detour hooking  | Directly caught by inline byte count and JMP checks                             |

### Open Questions (Partially Resolved via Ghidra MCP — 2026-04-03)

1. ~~Do memcheck 1-4 also cover functions called BY the main loop, or only the loop body itself?~~ — **RESOLVED:** Server-initiated memcheck uses opcode `0x4f27` → handler `FUN_1400b5720`. Server sends memory region specs, client copies 0x100-byte blocks and returns hashes. Server can request any address range.
2. ~~Does the AC read debug registers (DR0-DR3) via GetThreadContext?~~ — **RESOLVED: NO.** `GetThreadContext` and `SetThreadContext` are NOT in the import table. HWBP hooks are safe from client-side detection.
3. ~~What specific opcodes do the 6 memshift checks use?~~ — **RESOLVED:** The "inline byte count checks" are actually a **message counter heartbeat system**. Two counters (`DAT_140f60ed8` outbound, `DAT_140f60ed4` inbound) are decremented by every opcode handler in the main loop (40+ locations). Every 500ms, `FUN_1401a4320` sends negated counter values to the server via opcode `0xbb29`. Server compares against its own counts.
4. What is the movement agreement packet opcode and full field layout? — **PARTIALLY RESOLVED:** `CMovementHistoryClientException` RTTI class confirms server-side validation exists. Awaiting Matt's input on opcode.
5. Can Early Bird injection win the race against EQ's AC initialization? — **OPEN** (requires live testing)

### Ghidra-Verified Findings (2026-04-03)

Independent verification via GhidraMCP analysis of eqgame.exe on Frostreaver. Evidence state: `Live-validated` (binary analysis).

**Anti-debug surface:**

- `IsDebuggerPresent` is the only anti-debug import — trivially bypassed via PEB flag
- `GetThreadContext` / `SetThreadContext` — NOT imported. HWBP hooks invisible to EQ client
- `NtQueryInformationProcess` / `CheckRemoteDebuggerPresent` — NOT imported
- `VirtualProtect` — NOT imported (EQ doesn't change page protections)
- No ETW APIs (`EtwEventWrite`, `NtTraceEvent`) — no telemetry

**SystemFingerprint system (`FUN_140594840`):**

- Serializes: VideoCardId, NetworkCardId, HardriveId, ComputerName, NetworkAdapters list, HardDrives list
- Uses two hash seed GUIDs for composite fingerprint
- `/afp` command: `-get`, `-send`, `-getworld`, `-testvm`
- **CRITICAL:** 36 clients with identical fingerprints = instant flag. Each client needs spoofed unique hardware IDs

**Process scanning (full chain):**

- `CreateToolhelp32Snapshot` → `Process32First` / `Process32Next`
- `K32EnumProcesses` + `K32EnumProcessModules` + `OpenProcess` + `QueryFullProcessImageNameA`
- PEB unlinking and module name spoofing are essential

**VM detection:**

- `GetSystemFirmwareTable` + `EnumSystemFirmwareTables` (SMBIOS-based)
- `/afp -testvm` with Type + Flags classification

**CheaterLdFlag:**

- Server-side flag at player struct offset 0x2C4, adjacent to KillMe (0x2C0), NoRent (0x2C5), Corpse (0x2C6), ClientGmFlagSet (0x2C7)

**Network send function:**

- `FUN_140563130` at 0x140563130 (247 bytes) — main packet send. Called from 29+ functions including main loop. Contains critical section lock. Not yet in offsets.rs.

**Message counter heartbeat system (`FUN_1401a4320`):**

- Two global counters: `DAT_140f60ed8` (outbound, decremented on every packet send via `FUN_140563130`) and `DAT_140f60ed4` (inbound, decremented on every WorldAuth message)
- Main loop touches outbound counter at 40+ locations — every opcode handler decrements after sending
- Every 500ms: checks if counters < 2, refills (+0x37 or +0x55), negates both, sends via opcode `0xbb29`
- Server compares against its own message counts — mismatch = detected
- **Any hook intercepting packet sends MUST preserve counter decrements**

**File integrity checks (dispatcher `FUN_14021d730`):**

- Check 1x: `eqgame.exe` self-hash via `GetModuleFileNameA` → opcode `0x8bdc`
- Check 1sa: `Resources/BaseData.txt` → opcode `0xe91d`
- Check 1sa: `Resources/SkillCaps.txt` → opcode `0x9562`
- Each hashes file + samples 256 random DWORDs using Lagged Fibonacci PRNG (`FUN_14025abd0`, state at `DAT_140e8d148`)
- PRNG is deterministic — server knows which positions were sampled
- Triggered on world connect via `FUN_1402c9c80` (WorldAuthenticate)

**Server-initiated memcheck (opcode `0x4f27` → `FUN_1400b5720`):**

- Server sends counted list of memory region specs
- Client copies 0x100-byte blocks, hashes them, returns results
- Server can request hash of any address range — code patches in .text are directly detectable

**Checksum mismatch disconnect (opcode `0xd799`):**

- Server sends `"World disconnecting because the checksums didn't match."` and kills connection

**Zone entry integrity (`FUN_1402824c0`):**

- On zone connect: hashes player name (32 bytes), spell data, UI string data
- Sends via opcode `0xe4b3`

**CheaterLdFlag persistence:**

- Server-set flag at player struct offset 0x2C4 — persists in save data across sessions. Once flagged, character stays flagged forever.

### Evidence Status

SME-reported detection systems (byte count, memshift, memcheck 1-4) remain `SME-reported, partially verified`. Ghidra-confirmed findings above are `Live-validated` via binary analysis. Main loop internals remain unverified due to decompiler timeout on the ~20KB function body.

## Near-Term `M5` Hardening Focus

Current roadmap work should focus on:

- official-policy-driven anti-cheat gates
- hook, module, string, and environment exposure review
- timing, naming, and operator-hygiene hardening
- validation tasks for packet, zoning, and orchestrator changes
- Ghidra MCP setup for independent binary verification (#343)
- hardware breakpoint hooking prototype to replace current detour approach
- reflective injection to replace current LoadLibrary injection

## Related Docs

- `docs/external-research/daybreak-detection-digest.md`
- `docs/implementation-roadmap.md`
- `docs/wiki/Security-and-Anti-Detection-Notes.md`
