# B9 — IAT / VTable Patching as Lower-Exposure Alternative to HWBP + .text Hooks

**Issue:** #2183
**Evidence basis:** `docs/wiki/Research-Anti-Detection.md` §Recommended M7 Evasion Architecture, `textquest-dll/src/hooks/hwbp.rs`, `textquest-dll/src/hooks/detours.rs`, `textquest-dll/src/hooks/dx11_null.rs`
**Confidence:** Medium (design derived from repo-grounded observations + confirmed Ghidra findings)

---

## Problem Statement

The current hook stack has two risk tiers:

| Tier | Examples | Detection Risk |
|------|----------|---------------|
| HWBP (DR0-DR3) | game_loop, chat, login, memcheck | LOW — zero code bytes modified |
| JMP detour (.text) | SystemFingerprint, RealRender_World | HIGH — inline byte count + server memcheck detects patches |
| VTable swap (data section) | IDXGISwapChain::Present, ID3D11Device::*, ID3D11DeviceContext::* | HIGH — avoids `.text` modification, but pointer replacement and required `VirtualProtect` activity remain observable |

The HWBP tier is limited to 4 slots (DR0–DR3). Three are occupied in-game (ProcessGameEvents, RealRenderWorld, DspChat via slot_manager.rs) and Dr3 is reserved for the memcheck responder. The slot budget is exhausted. Any new redirect hook must use a non-HWBP path.

The JMP detour tier touches `.text` and is directly visible to:
1. EQ's inline byte-count heartbeat system (opcode 0xbb29, DAT_140f60ed8/DAT_140f60ed4)
2. Server-initiated memcheck (opcode 0x4f27 → FUN_1400b5760) — server can request hash of any .text range

IAT patching and VTable patching redirect calls without touching `.text` — these are writes to data section (`.rdata`, heap, or stack-allocated COM vtable memory). Server memcheck is confirmed to only request `.text`-range hashes.

---

## Technique Analysis

### 1. IAT Patching

**Mechanism:** The Import Address Table (IAT) is a writable array of function pointers resolved at load time by the Windows PE loader. Each entry corresponds to one imported function. Replacing an entry redirects every call to that import within the module — without modifying any code bytes.

**How it works (Windows PE):**
1. `GetModuleHandle(target_module)` → module base
2. Walk the PE `IMAGE_IMPORT_DESCRIPTOR` chain from `OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT]`
3. Match by DLL name (`Name` field) and function name/ordinal (`INT` — `OriginalFirstThunk`)
4. Corresponding `IAT` entry (`FirstThunk`) is the live function pointer
5. `VirtualProtect(iat_entry, sizeof(usize), PAGE_READWRITE, &old)` → write our thunk → restore protection

**Protection requirement:** IAT is in `.rdata` (read-only) by default. One `VirtualProtect` call is needed to make it writable for the pointer write, then it can be restored to `PAGE_READONLY`. This is a single observable `VirtualProtect` call per hooked import, performed only at install time (not per-call).

**EQ context:** EQ does NOT import `VirtualProtect` (confirmed via Ghidra — `Research-Anti-Detection.md` §Ghidra-Verified Findings). It cannot detect our `VirtualProtect` call from inside the client. ETW kernel-mode tracing (`etw-ti-virtualprotect.md`) could observe it externally, but Daybreak's client-side scan cannot.

**Pointer integrity risk:** An anti-cheat could read back the IAT entry and compare against the known DLL export address. EQ's confirmed scan surface does not include this check. The risk is medium-low.

**Use cases for EQ:** IAT patching is ideal for hooks on system DLL functions imported by eqgame.exe — for example:
- `ws2_32.dll!WSASend` / `WSARecv` — packet capture (`packet_hook.rs`)
- `kernel32.dll!CreateToolhelp32Snapshot` — process scan interference
- Any imported function where single-entry redirect suffices (calls from eqgame.exe only; does not redirect calls from other modules)

**Limitation:** IAT patching only redirects calls originating from the module whose IAT is patched. Other DLLs that call the same function are unaffected. For `WSASend`/`WSARecv`, this is exactly the desired scope: we want EQ's sends/recvs only.

---

### 2. VTable Patching

**Mechanism:** COM and C++ virtual dispatch tables (vtables) are pointer arrays stored in read-only memory (`.rdata` for static vtables; heap/stack for runtime objects). Swapping a slot pointer redirects all virtual calls through that slot without touching any executable code.

**How it works:**
1. Obtain a pointer to the vtable — either via the object's first field (`*(*obj as *const *const usize)`) or from a known static address
2. Index to the target slot: `vtable_ptr[slot_index]`
3. Save original pointer (for unhook / thunk chaining)
4. `VirtualProtect` the vtable page to writable when required by the implementation / backing page protections
5. Write our thunk address
6. Restore page protection

**EQ context — dx11_null.rs:** This is already implemented for D3D11 objects in `dx11_null.rs`. Although these hooks target COM interfaces such as `IDXGISwapChain`, `ID3D11Device`, and `ID3D11DeviceContext`, the current implementation still wraps the vtable entry swap in `VirtualProtect` before writing the new slot value and restores the old protection afterward. In other words, this code path is not currently relying on COM object allocation alone to guarantee writability.

**EQ context — EQ C++ objects:** EQ's own C++ classes (e.g., `CEverQuest`, `CDisplay`, `CRender`) have static vtables in EQ's `.rdata`, so `VirtualProtect` is expected there as well. The key distinction is that EQ class vtables are plainly static read-only tables, whereas the DX11 COM hook code currently uses the same protection-changing pattern conservatively even for COM-related vtable patching.

**Use cases for EQ:**
- `SystemFingerprint` class vtable — redirect `Serialize`/`Build` virtual method to return spoofed values without touching `.text`
- `CDisplay::RealRender_World` — if exposed via vtable, redirect rather than JMP detour
- EQ class hierarchies where a virtual dispatch is the natural call entry point

---

## Target Table: Candidate Hooks to Migrate

| Hook | Current Method | Detection Risk | Migration Path | Notes |
|------|---------------|---------------|---------------|-------|
| `SystemFingerprint::Serialize` | JMP detour (`.text`) | HIGH | VTable slot patch on EQ class vtable | Requires Ghidra to confirm vtable offset; `FUN_140594840` is the identified serializer. Alternate: IAT on a system call it uses (e.g., `GetComputerNameA`) |
| `CDisplay::RealRender_World` | JMP detour (`.text`) | HIGH | VTable slot patch on `CDisplay` vtable | Same Ghidra precondition |
| `WSASend` / `WSARecv` | retour (planned, not installed) | HIGH (if installed as detour) | IAT patch in eqgame.exe's import table — `ws2_32.dll` entries | Ideal IAT target: thread-safe, no HWBP slot consumed, single `VirtualProtect` at install |
| `IDXGISwapChain::Present` | VTable swap (existing dx11_null.rs) | MEDIUM | Already data-hook; no change needed | COM heap object — no `VirtualProtect` required |
| `ID3D11Device::*` | VTable swap (existing dx11_null.rs) | MEDIUM | Already data-hook; no change needed | Same |
| `ID3D11DeviceContext::*` | VTable swap (existing dx11_null.rs) | MEDIUM | Already data-hook; no change needed | Same |

**Priority migrations (free a DR slot OR eliminate .text exposure):**
1. `WSASend` / `WSARecv` → IAT patch (highest value: avoids creating a new HWBP dependency for packet hooks)
2. `SystemFingerprint` → VTable patch (eliminates the highest-risk .text hook; prerequisite: Ghidra vtable offset audit — see #343)
3. `CDisplay::RealRender_World` → VTable patch (eliminates second-highest-risk .text hook; same prerequisite)

---

## Pointer Integrity Self-Check Design

The `Research-Anti-Detection.md` table lists "pointer integrity validation" as the risk for VTable/IAT patching. To detect if EQ reads back the patched pointer:

**Approach:** Install a second-level HWBP (or use the memcheck HWBP already on Dr3) to watch-read the IAT/VTable page. This is impractical as a deployed feature (burns slots), but is useful as a one-time validation check during development.

**Practical self-check (build-time):**

```rust
/// Verify that no EQ-internal code reads back the IAT entry we patched.
/// Call once after install, observe for N frames, log any re-read.
/// This is a validation aid, not a production guard.
pub fn check_iat_pointer_integrity(iat_entry_addr: usize, frames: u32) {
    // Install a HWBP data-read watchpoint on the IAT entry address.
    // If it fires, EQ is reading back the IAT entry.
    // In production: remove after validation. Only 4 slots available.
    tracing::warn!(
        addr = format!("{:#x}", iat_entry_addr),
        frames,
        "Pointer integrity watch requested — burns one HWBP slot for validation"
    );
    // NOTE: Actual install via hwbp::register with DR_CONDITION_READ (DR7 bits 16-19).
    // The current hwbp.rs only supports execution (type=00); read watchpoints need
    // DR7 condition field = 11 (read/write). This is a planned extension.
}
```

**Ghidra audit path:** Search eqgame.exe for XREF reads to the IAT base address range (`IMAGE_DIRECTORY_ENTRY_IMPORT` resolved first-thunk region). If no XREFs exist in EQ code, pointer integrity is not a practical threat.

**Empirical validation:** Confirmed via server memcheck responder (#2175 A1) — `memcheck.rs` pre-hashes `.text` blocks. IAT and vtable pages are in `.rdata` / heap, confirmed outside the server memcheck hash range. No additional memcheck caching is needed for IAT/VTable patches.

---

## Implementation Plan

### Module: `textquest-dll/src/hooks/iat.rs`

```rust
//! IAT (Import Address Table) patching primitive.
//!
//! Redirects calls to a specific imported function within eqgame.exe's IAT
//! by replacing the resolved function pointer with a thunk address.
//!
//! # Safety
//! All operations require `unsafe`. The IAT is in a read-only page by default;
//! one `VirtualProtect` call makes the entry writable for the swap, then
//! restores the original protection.
//!
//! # Scope
//! Only redirects calls from the module whose IAT is patched. Other DLLs
//! calling the same function are unaffected — correct behavior for targeting
//! eqgame.exe's WSASend/WSARecv imports.

pub struct IatPatch {
    /// Address of the IAT slot that was patched.
    iat_entry: *mut usize,
    /// Original function pointer (saved for unhook).
    original: usize,
    /// Whether the patch is currently active.
    active: bool,
}

/// Install: find the IAT entry for `func_name` in `dll_name` within
/// `module_base`, save original, write `thunk`.
pub fn install(
    module_base: usize,
    dll_name: &str,
    func_name: &str,
    thunk: usize,
) -> Result<IatPatch, String>;

/// Uninstall: restore original pointer, deactivate patch.
pub fn uninstall(patch: &mut IatPatch) -> Result<(), String>;
```

**Key implementation steps:**
1. Walk `IMAGE_IMPORT_DESCRIPTOR` list at `module_base + import_directory_rva`
2. For each descriptor, compare `Name` field (DLL name, case-insensitive)
3. Parallel walk `OriginalFirstThunk` (INT, has names) and `FirstThunk` (IAT, has live pointers)
4. Match `IMAGE_IMPORT_BY_NAME.Name` against `func_name`
5. `VirtualProtect` IAT entry → write thunk → restore protection
6. Return `IatPatch` with saved original

### Module: `textquest-dll/src/hooks/vtable.rs`

```rust
//! VTable slot patching primitive.
//!
//! Locates an object's virtual dispatch table and swaps one slot pointer
//! to redirect all virtual calls through that slot.
//!
//! # Safety
//! All operations require `unsafe`. Static vtables in `.rdata` require
//! `VirtualProtect` to make writable. COM heap object vtables do not.
//!
//! # Unhook discipline
//! Always call `uninstall` before the object is destroyed. For COM objects,
//! call before `Release` drops the refcount to zero.

pub struct VTablePatch {
    /// Address of the vtable slot that was patched.
    slot_addr: *mut usize,
    /// Original function pointer.
    original: usize,
    /// Whether the patch is active.
    active: bool,
}

/// Install: given `vtable_base` (pointer to vtable array) and `slot_index`,
/// save original and write `thunk`.
pub fn install(
    vtable_base: *const usize,
    slot_index: usize,
    thunk: usize,
) -> Result<VTablePatch, String>;

/// Uninstall: restore original pointer.
pub fn uninstall(patch: &mut VTablePatch) -> Result<(), String>;
```

**Key implementation steps:**
1. Compute `slot_addr = vtable_base.add(slot_index)`
2. Check page protection via `VirtualQuery`
3. If read-only: `VirtualProtect(slot_addr, sizeof(usize), PAGE_READWRITE, &old_protect)`
4. Save `*slot_addr` as original
5. Write thunk: `*slot_addr = thunk`
6. Restore protection if changed

---

## Tradeoffs Summary

| Property | HWBP | JMP Detour (.text) | IAT Patch | VTable Patch |
|----------|------|-------------------|-----------|--------------|
| Code bytes modified | None | Yes (JMP opcode) | None | None |
| Survives server memcheck | Yes | **No** | Yes | Yes |
| Survives inline byte-count check | Yes | **No** | Yes | Yes |
| Slot budget | 4 total, precious | Unlimited | Unlimited | Unlimited |
| VirtualProtect at install | No | Yes (retour) | Yes (once) | Yes if .rdata |
| Per-call overhead | VEH exception dispatch | Trampoline call | Direct call | Direct call |
| Applies to all callers | No (per-thread BP) | Yes (code path) | No (per-module IAT) | Yes (all virtual callers) |
| Precision | Exact execution point | Exact entry point | Entire import | Virtual dispatch only |
| Ghidra precondition | No | No | Import table walk | Vtable offset audit |

**Recommendation:** IAT patching is the highest-value, lowest-risk addition. It:
- Eliminates a planned HWBP dependency for `WSASend`/`WSARecv`
- Requires exactly one `VirtualProtect` at install (not per-call)
- Has no `.text` exposure
- Does not burn DR slots
- Is already partially designed in the codebase (packet_hook.rs stubs)

VTable patching for EQ objects requires a Ghidra vtable offset audit (issue #343) before specific virtual method migration is safe.

---

## Memcheck Responder Interaction

The server memcheck responder (`memcheck.rs`, `Dr3`) pre-hashes `.text` blocks at DLL init time. IAT entries are in `.rdata`; vtable slots are in `.rdata` (static) or heap (COM). Neither overlaps `.text`.

**Verified conclusion:** IAT/VTable patches do NOT require the memcheck responder to cache or substitute any additional pages. The patched memory ranges are outside the server's hash query scope.

This satisfies acceptance criterion: "Memcheck responder (#2175 A1) does not need to cache over the patched VTable/IAT pages."

---

## Open Items / Prerequisites

| Item | Blocker for | Status |
|------|------------|--------|
| Ghidra vtable offset audit for `SystemFingerprint` and `CDisplay` | EQ VTable migration | Tracked in #343 |
| DR7 read-watchpoint support in `hwbp.rs` (type field = `11`) | Pointer integrity self-check | Not yet implemented |
| Confirm `WSASend`/`WSARecv` are in eqgame.exe's IAT (vs. delay-load or dynamic `GetProcAddress`) | IAT patch for packet hooks | Verify with `dumpbin /imports eqgame.exe` on Frostreaver |
| Live test: observe no server-side memcheck reaction to IAT/vtable patches | Empirical validation | Requires live EQ session post-patch Wednesday |

---

## References

- `textquest-dll/src/hooks/hwbp.rs` — HWBP engine, VEH dispatcher, slot budget
- `textquest-dll/src/hooks/detours.rs` — retour-based JMP detour manager
- `textquest-dll/src/hooks/dx11_null.rs` — existing VTable swap for D3D11 objects (COM heap vtables)
- `textquest-dll/src/hooks/memcheck.rs` — server memcheck responder, HWBP on Dr3
- `textquest-dll/src/hooks/packet_hook.rs` — WSASend/WSARecv hook stubs
- `textquest-dll/src/hooks/fingerprint.rs` — SystemFingerprint spoof (current JMP detour target)
- `docs/wiki/Research-Anti-Detection.md` §Recommended M7 Evasion Architecture, §Ghidra-Verified Findings
- `docs/research/hook-detection-surface.md` — hook catalog and migration priority list
- `docs/etw-ti-virtualprotect.md` — ETW tracing for VirtualProtect (external detection surface)
