//! MQ2 FFI bindings — raw C-layout equivalents of core MQ2/eqlib types.
//!
//! Also exposes the [`bridge`] submodule which provides the safe `MQ2Bridge`
//! translation layer for plugin-style API calls.
//!
//! # Memory Safety Assumptions
//!
//! All raw pointer operations in this module assume:
//!
//! 1. **Single-process**: These structs are read from within the injected
//!    `textquest-dll`, running inside `eqgame.exe`. There is no cross-process
//!    memory transfer here — all pointers are valid within the current process
//!    address space.
//!
//! 2. **EQ game thread**: The game loop runs on a single OS thread. All reads
//!    from EQ memory must originate from the game-loop HWBP callback (or a
//!    critical section held on the game thread). Accessing these structs from
//!    any other thread without synchronization is unsound.
//!
//! 3. **Struct layouts**: Fields are laid out to match eqlib's documented
//!    offsets for the client build tracked in
//!    `textquest_common::offsets::CLIENT_DATE`. Any EQ patch may shift these
//!    offsets — verify with `OffsetDatabase` before each release.
//!
//! 4. **Pointer validity window**: EQ owns the lifetime of spawn, item, and
//!    player objects. A `PlayerClient*` obtained from `pinstLocalPlayer` is
//!    valid until the server despawns that spawn or the player zones. Callers
//!    must not hold `&RawPlayerClient` references across game ticks; instead,
//!    copy fields into a `PlayerSnapshot` / `SpawnData` immediately.
//!
//! 5. **No null dereference**: All safe wrappers in this module validate
//!    pointers with `NonNull` before constructing references. Raw pointer reads
//!    are wrapped with an `is_readable` guard on Windows.
//!
//! 6. **Padding bytes**: `#[repr(C)]` structs may contain padding that is
//!    uninitialized when allocated. We never read padding fields and never
//!    construct a `repr(C)` struct from zeroed memory and then interpret
//!    padding as meaningful data.
//!
//! # Field coverage
//!
//! These are **minimal** bindings — only the fields TextQuest actively reads.
//! Full MQ2 structs (PlayerClient, PcClient, ItemClient) are thousands of bytes
//! and extend across multiple inheritance chains. We intentionally capture only
//! the fields we need; everything else is represented by opaque padding arrays
//! sized to preserve correct field offsets.
//!
//! Source references:
//! - eqlib `PlayerClient.h` (macroquest live branch, client 20260415)
//! - eqlib `EQ_Item.h`
//! - `textquest_common::offsets` for runtime pointer constants

#![allow(
    // Clippy does not understand that padding arrays cannot be removed.
    clippy::large_stack_arrays,
)]

pub mod bridge;
pub mod plugin_loader;
pub use bridge::MQ2Bridge;
pub use plugin_loader::{
    LoadedMacroQuestPlugin, MacroQuestPluginApi, MacroQuestPluginLoader, PluginCandidate,
    PluginConfig, PluginLoadError, TextQuestMq2Api, TEXTQUEST_MQ2_API_VERSION,
};

use std::ptr::NonNull;

use textquest_common::types::SpawnData;

// ─── Field offset constants (eqlib PlayerClient.h, 20260415) ───────────────

/// Byte offset of `SpawnID` within `PlayerClient` / `PlayerZoneClient`.
/// Source: PlayerClient.h `/*0x0138*/`
pub const PC_SPAWN_ID_OFFSET: usize = 0x0138;

/// Byte offset of `Name` (char[64]) within `PlayerClient`.
/// Source: PlayerClient.h `/*0x0158*/`
pub const PC_NAME_OFFSET: usize = 0x0158;

/// Byte offset of `Y` (float) within `PlayerClient`.
/// Source: PlayerClient.h `/*0x0024*/`
pub const PC_Y_OFFSET: usize = 0x0024;

/// Byte offset of `X` (float) within `PlayerClient`.
pub const PC_X_OFFSET: usize = 0x0028;

/// Byte offset of `Z` (float) within `PlayerClient`.
pub const PC_Z_OFFSET: usize = 0x002C;

/// Byte offset of `Heading` (float) within `PlayerClient`.
pub const PC_HEADING_OFFSET: usize = 0x0030;

/// Byte offset of `SpeedRun` (float) within `PlayerClient`.
pub const PC_SPEED_RUN_OFFSET: usize = 0x0034;

/// Byte offset of `StandState` (BYTE) within `PlayerClient`.
/// Source: PlayerClient.h `/*0x13c*/`
pub const PC_STAND_STATE_OFFSET: usize = 0x013C;

/// Byte offset of `Level` (BYTE) within `PlayerClient`.
/// Source: PlayerClient.h `/*0x0161*/`
pub const PC_LEVEL_OFFSET: usize = 0x0161;

/// Byte offset of `Class` (BYTE) within `PlayerClient`.
pub const PC_CLASS_OFFSET: usize = 0x0162;

/// Byte offset of `Type` (BYTE) within `PlayerClient` — spawn type.
pub const PC_TYPE_OFFSET: usize = 0x0163;

/// Byte offset of `HPCurrent` (int) within `PlayerZoneClient`.
pub const PC_HP_CURRENT_OFFSET: usize = 0x01F4;

/// Byte offset of `HPMax` (int) within `PlayerZoneClient`.
pub const PC_HP_MAX_OFFSET: usize = 0x01F8;

/// Byte offset of `GM` (bool) within `PlayerClient`.
pub const PC_IS_GM_OFFSET: usize = 0x02A4;

/// Byte offset of `Race` (uint32_t) within `PlayerClient`.
pub const PC_RACE_OFFSET: usize = 0x01D0;

// ─── Item field offsets (EQ_Item.h) ─────────────────────────────────────────

/// Byte offset of `ID` within `ItemClient` (item template ID).
pub const ITEM_ID_OFFSET: usize = 0x0000;

/// Byte offset of `Name` (char[64]) within `ItemClient`.
pub const ITEM_NAME_OFFSET: usize = 0x0010;

/// Byte offset of `StackCount` (uint32_t) within `ItemClient`.
pub const ITEM_STACK_COUNT_OFFSET: usize = 0x023C;

/// Byte offset of `IsStackable` (bool) within `ItemClient`.
pub const ITEM_IS_STACKABLE_OFFSET: usize = 0x024C;

/// Byte offset of `InvSlot` (int) within `ItemClient`.
pub const ITEM_INV_SLOT_OFFSET: usize = 0x0254;

// ─── repr(C) raw layouts ─────────────────────────────────────────────────────

/// Raw C-layout mirror of the leading fields we care about in
/// `PlayerZoneClient` / `PlayerClient`.
///
/// **Do not construct this type directly.** Use `RawPlayerClient::from_ptr`.
///
/// This struct covers the leading position/movement fields, `SpawnID`,
/// `StandState`, and the later identity fields currently represented here:
/// `Name` (0x0158), `Level`, `Class`, and `Type`.
/// Fields not explicitly represented in this mirror remain intentionally
/// omitted and should be accessed via offset-based reads when needed.
///
/// Layout source: eqlib `PlayerClient.h`, client build 20260415.
#[repr(C)]
pub struct RawPlayerClient {
    /// Opaque bytes [0x0000..0x0024] (vtable, ActorClient base, etc.)
    _prefix: [u8; 0x0024],
    /// `Y` — world Y position (EQ uses Y/X/Z, note axis order)
    pub y: f32,
    /// `X` — world X position
    pub x: f32,
    /// `Z` — world Z (vertical)
    pub z: f32,
    /// `Heading` — facing direction (0–512 EQ units)
    pub heading: f32,
    /// `SpeedRun` — non-zero means in motion
    pub speed_run: f32,
    /// Opaque bytes [0x0038..0x0138]
    _mid: [u8; 0x0138 - 0x0038],
    /// `SpawnID` — unique server-assigned spawn ID
    pub spawn_id: u32,
    /// Opaque bytes [0x013C – 0x013C] skipped; StandState follows immediately
    _pad_id: [u8; 0],
    /// `StandState` — 0=standing, 3=sitting, 110=feigned, 111=dead
    pub stand_state: u8,
    /// Opaque bytes [0x013D..0x0158)
    _pad_state: [u8; 0x0158 - 0x013D],
    /// `Name` — internal spawn name (null-terminated, max 63 chars + NUL)
    pub name: [u8; 64],
    /// `Level` — character or mob level
    pub level: u8,
    /// `Class` — EQ class ID (1=WAR, 2=CLR, …)
    pub class: u8,
    /// `Type` — spawn type (0=player, 1=NPC, 2=corpse)
    pub spawn_type: u8,
}

/// Raw C-layout mirror of the `EQ_Item` / `ItemClient` fields we read.
///
/// **Do not construct this type directly.** Use `RawItem::from_ptr`.
#[repr(C)]
pub struct RawItem {
    /// `ID` — item template ID (unique per item type)
    pub id: u32,
    /// Opaque bytes [0x0004..0x0010)
    _pad0: [u8; 0x0010 - 0x0004],
    /// `Name` — display name (null-terminated)
    pub name: [u8; 64],
    /// Opaque bytes [0x0050..0x023C)
    _pad1: [u8; 0x023C - 0x0050],
    /// `StackCount` — number of items in this stack
    pub stack_count: u32,
    /// Opaque bytes [0x0240..0x024C)
    _pad2: [u8; 0x024C - 0x0240],
    /// `IsStackable` — whether the item can be stacked
    pub is_stackable: bool,
    /// Opaque bytes [0x024D..0x0254)
    _pad3: [u8; 0x0254 - 0x024D],
    /// `InvSlot` — equipped inventory slot (−1 = not equipped)
    pub inv_slot: i32,
}

// ─── Safe wrappers ────────────────────────────────────────────────────────────

/// Snapshot of a `PlayerClient` struct, fully owned and safe to use from any
/// thread.
///
/// Constructed by reading fields from a `NonNull<RawPlayerClient>` while on the
/// game thread, then moving this value wherever needed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerSnapshot {
    pub spawn_id: u32,
    pub name: String,
    pub y: f32,
    pub x: f32,
    pub z: f32,
    pub heading: f32,
    pub speed_run: f32,
    pub stand_state: u8,
    pub level: u8,
    pub class: u8,
    pub spawn_type: u8,
}

/// Snapshot of an `ItemClient` struct.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemSnapshot {
    pub id: u32,
    pub name: String,
    pub stack_count: u32,
    pub is_stackable: bool,
    pub inv_slot: i32,
}

/// Snapshot of spawn fields from `PlayerClient`, used for NPC/player tracking.
/// Mirrors the data captured in `textquest_common::types::SpawnData`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnSnapshot {
    pub spawn_id: u32,
    pub name: String,
    pub y: f32,
    pub x: f32,
    pub z: f32,
    pub heading: f32,
    pub speed_run: f32,
    pub stand_state: u8,
    pub level: u8,
    pub class: u8,
    pub spawn_type: u8,
    pub race: u32,
    pub hp_current: i32,
    pub hp_max: i32,
}

// ─── Read helpers ─────────────────────────────────────────────────────────────

/// Read a null-terminated C string from a fixed-size byte buffer.
///
/// Returns an empty string if the buffer contains no valid UTF-8 up to the
/// first NUL byte.
fn cstr_from_buf(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

/// Read a `u32` from a raw pointer at a given byte offset.
///
/// # Safety
///
/// The caller must ensure `base + offset` through `base + offset + 4` is valid
/// readable memory for the duration of this call.
#[cfg(windows)]
unsafe fn read_u32_at(base: *const u8, offset: usize) -> u32 {
    // SAFETY: caller guarantees the address range is readable.
    unsafe { std::ptr::read_unaligned(base.add(offset) as *const u32) }
}

/// Read a `u8` from a raw pointer at a given byte offset.
///
/// # Safety
///
/// Same contract as `read_u32_at`.
#[cfg(windows)]
unsafe fn read_u8_at(base: *const u8, offset: usize) -> u8 {
    // SAFETY: caller guarantees the address range is readable.
    unsafe { std::ptr::read_unaligned(base.add(offset) as *const u8) }
}

/// Read a `f32` from a raw pointer at a given byte offset.
///
/// # Safety
///
/// Same contract as `read_u32_at`.
#[cfg(windows)]
unsafe fn read_f32_at(base: *const u8, offset: usize) -> f32 {
    // SAFETY: caller guarantees the address range is readable.
    unsafe { std::ptr::read_unaligned(base.add(offset) as *const f32) }
}

/// Read a `i32` from a raw pointer at a given byte offset.
///
/// # Safety
///
/// Same contract as `read_u32_at`.
#[cfg(windows)]
unsafe fn read_i32_at(base: *const u8, offset: usize) -> i32 {
    // SAFETY: caller guarantees the address range is readable.
    unsafe { std::ptr::read_unaligned(base.add(offset) as *const i32) }
}

// ─── RawPlayerClient wrapper ──────────────────────────────────────────────────

/// A non-owning, lifetime-scoped pointer to a `RawPlayerClient` in EQ memory.
///
/// The `'game` lifetime parameter ties this reference to the game-tick scope
/// in which the pointer was obtained. On Windows, constructing this type via
/// [`PlayerClientRef::new`] validates readability before returning `Some`.
///
/// # Safety invariant
///
/// The pointed-to memory must remain valid and immutable for `'game`. In
/// practice, `'game` should be a single game-tick callback invocation; do not
/// hold this ref across tick boundaries or zone transitions.
pub struct PlayerClientRef<'game> {
    ptr: NonNull<RawPlayerClient>,
    _marker: std::marker::PhantomData<&'game RawPlayerClient>,
}

impl<'game> PlayerClientRef<'game> {
    /// Wrap `ptr` in a `PlayerClientRef` after checking it is non-null and
    /// (on Windows) readable.
    ///
    /// Returns `None` if `ptr` is null or the memory region is not readable.
    ///
    /// # Safety
    ///
    /// `ptr`, if non-null, must point to a valid `RawPlayerClient` in EQ
    /// memory that will remain alive for `'game`.
    pub unsafe fn new(ptr: *mut RawPlayerClient) -> Option<Self> {
        let nn = NonNull::new(ptr)?;
        #[cfg(windows)]
        if !is_readable_block(nn.as_ptr() as usize, std::mem::size_of::<RawPlayerClient>()) {
            return None;
        }
        Some(Self {
            ptr: nn,
            _marker: std::marker::PhantomData,
        })
    }

    /// Snapshot all relevant fields into a `PlayerSnapshot`.
    ///
    /// Reads the relevant fields from the struct into a fully-owned value.
    /// The fields are loaded individually, so this does not guarantee an
    /// atomic or internally consistent view if the underlying memory changes
    /// concurrently. Safe to use from the game thread; the returned value can
    /// be sent anywhere.
    #[must_use]
    pub fn snapshot(&self) -> PlayerSnapshot {
        // SAFETY: `self.ptr` was validated in `new()` — non-null and readable
        // for the duration of `'game`. We use read_unaligned via the raw struct
        // fields; repr(C) guarantees field offsets match the C layout.
        let raw = unsafe { self.ptr.as_ref() };
        PlayerSnapshot {
            spawn_id: raw.spawn_id,
            name: cstr_from_buf(&raw.name),
            y: raw.y,
            x: raw.x,
            z: raw.z,
            heading: raw.heading,
            speed_run: raw.speed_run,
            stand_state: raw.stand_state,
            level: raw.level,
            class: raw.class,
            spawn_type: raw.spawn_type,
        }
    }
}

/// Read a `SpawnSnapshot` from a raw `PlayerClient` pointer using explicit
/// byte-offset reads.
///
/// This variant is more resilient to struct layout drift — it does not rely on
/// the `RawPlayerClient` repr(C) struct but reads each field at its documented
/// offset. Use this when offsets may differ between build dates.
///
/// # Safety
///
/// `ptr` must be a non-null, readable `PlayerClient*` valid for the duration
/// of this call.
#[cfg(windows)]
pub unsafe fn read_spawn_snapshot(ptr: *const u8) -> Option<SpawnSnapshot> {
    if ptr.is_null() {
        return None;
    }
    // Determine the farthest offset we read so we can validate the full range.
    // HP fields sit at 0x01F8+4 = 0x01FC.
    const MIN_SIZE: usize = 0x01FC + 4;
    if !is_readable_block(ptr as usize, MIN_SIZE) {
        return None;
    }

    // SAFETY: is_readable_block validated the range [ptr, ptr+MIN_SIZE).
    let spawn_id = unsafe { read_u32_at(ptr, PC_SPAWN_ID_OFFSET) };
    let stand_state = unsafe { read_u8_at(ptr, PC_STAND_STATE_OFFSET) };
    let level = unsafe { read_u8_at(ptr, PC_LEVEL_OFFSET) };
    let class = unsafe { read_u8_at(ptr, PC_CLASS_OFFSET) };
    let spawn_type = unsafe { read_u8_at(ptr, PC_TYPE_OFFSET) };
    let y = unsafe { read_f32_at(ptr, PC_Y_OFFSET) };
    let x = unsafe { read_f32_at(ptr, PC_X_OFFSET) };
    let z = unsafe { read_f32_at(ptr, PC_Z_OFFSET) };
    let heading = unsafe { read_f32_at(ptr, PC_HEADING_OFFSET) };
    let speed_run = unsafe { read_f32_at(ptr, PC_SPEED_RUN_OFFSET) };
    let hp_current = unsafe { read_i32_at(ptr, PC_HP_CURRENT_OFFSET) };
    let hp_max = unsafe { read_i32_at(ptr, PC_HP_MAX_OFFSET) };
    let race = unsafe { read_u32_at(ptr, PC_RACE_OFFSET) };

    // Read name buffer
    let mut name_buf = [0u8; 64];
    // SAFETY: is_readable_block validated the range covering 0x0158..0x0198.
    unsafe {
        std::ptr::copy_nonoverlapping(ptr.add(PC_NAME_OFFSET), name_buf.as_mut_ptr(), 64);
    }

    Some(SpawnSnapshot {
        spawn_id,
        name: cstr_from_buf(&name_buf),
        y,
        x,
        z,
        heading,
        speed_run,
        stand_state,
        level,
        class,
        spawn_type,
        race,
        hp_current,
        hp_max,
    })
}

/// Non-Windows stub — always returns `None`.
#[cfg(not(windows))]
pub fn read_spawn_snapshot(_ptr: *const u8) -> Option<SpawnSnapshot> {
    None
}

// ─── RawItem wrapper ──────────────────────────────────────────────────────────

/// A non-owning, lifetime-scoped pointer to a `RawItem` in EQ memory.
pub struct ItemRef<'game> {
    ptr: NonNull<RawItem>,
    _marker: std::marker::PhantomData<&'game RawItem>,
}

impl<'game> ItemRef<'game> {
    /// Wrap `ptr` in an `ItemRef` after null and readability checks.
    ///
    /// # Safety
    ///
    /// `ptr`, if non-null, must point to a valid `ItemClient` in EQ memory
    /// that will remain alive for `'game`.
    pub unsafe fn new(ptr: *mut RawItem) -> Option<Self> {
        let nn = NonNull::new(ptr)?;
        #[cfg(windows)]
        if !is_readable_block(nn.as_ptr() as usize, std::mem::size_of::<RawItem>()) {
            return None;
        }
        Some(Self {
            ptr: nn,
            _marker: std::marker::PhantomData,
        })
    }

    /// Snapshot all relevant fields into an `ItemSnapshot`.
    #[must_use]
    pub fn snapshot(&self) -> ItemSnapshot {
        // SAFETY: `self.ptr` was validated in `new()`.
        let raw = unsafe { self.ptr.as_ref() };
        ItemSnapshot {
            id: raw.id,
            name: cstr_from_buf(&raw.name),
            stack_count: raw.stack_count,
            is_stackable: raw.is_stackable,
            inv_slot: raw.inv_slot,
        }
    }
}

// ─── Readability guard ────────────────────────────────────────────────────────

#[cfg(windows)]
fn is_readable_block(address: usize, len: usize) -> bool {
    use windows::Win32::System::Memory::{
        MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_GUARD, PAGE_NOACCESS, VirtualQuery,
    };

    if address == 0 || len == 0 {
        return false;
    }
    let mut mbi = MEMORY_BASIC_INFORMATION::default();
    // SAFETY: VirtualQuery is safe to call with any address; it fills mbi on
    // success and returns 0 on failure.
    let result = unsafe {
        VirtualQuery(
            Some(address as *const _),
            &mut mbi,
            std::mem::size_of_val(&mbi),
        )
    };
    if result == 0 {
        return false;
    }
    if mbi.State != MEM_COMMIT {
        return false;
    }
    if mbi.Protect.0 & PAGE_NOACCESS.0 != 0 || mbi.Protect.0 & PAGE_GUARD.0 != 0 {
        return false;
    }
    // Verify the entire range fits within this single region.
    let region_end = mbi.BaseAddress as usize + mbi.RegionSize;
    region_end >= address.saturating_add(len)
}

#[cfg(not(windows))]
fn is_readable_block(_address: usize, _len: usize) -> bool {
    // Non-Windows: stub — nothing is in-process EQ memory here.
    false
}

// ─── Conversion traits ────────────────────────────────────────────────────────

impl From<PlayerSnapshot> for SpawnData {
    /// Convert a `PlayerSnapshot` (raw MQ2 memory read) into the TextQuest
    /// `SpawnData` wire format used by the IPC layer.
    fn from(snap: PlayerSnapshot) -> SpawnData {
        SpawnData {
            spawn_id: snap.spawn_id,
            name: snap.name.clone(),
            displayed_name: snap.name,
            spawn_type: snap.spawn_type,
            level: snap.level,
            class_id: snap.class,
            race_id: 0, // PlayerSnapshot does not carry race; use SpawnSnapshot
            x: snap.x,
            y: snap.y,
            z: snap.z,
            heading: snap.heading,
            hp_current: 0,
            hp_max: 0,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 0,
            endurance_max: 0,
            speed_run: snap.speed_run,
            stand_state: snap.stand_state,
            is_gm: false,
        }
    }
}

impl From<SpawnSnapshot> for SpawnData {
    /// Convert a full `SpawnSnapshot` (offset-based read) into `SpawnData`.
    fn from(snap: SpawnSnapshot) -> SpawnData {
        SpawnData {
            spawn_id: snap.spawn_id,
            name: snap.name.clone(),
            displayed_name: snap.name,
            spawn_type: snap.spawn_type,
            level: snap.level,
            class_id: snap.class,
            race_id: snap.race,
            x: snap.x,
            y: snap.y,
            z: snap.z,
            heading: snap.heading,
            hp_current: snap.hp_current as i64,
            hp_max: snap.hp_max as i64,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 0,
            endurance_max: 0,
            speed_run: snap.speed_run,
            stand_state: snap.stand_state,
            is_gm: false,
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── cstr_from_buf ─────────────────────────────────────────────────────────

    #[test]
    fn cstr_from_buf_empty() {
        assert_eq!(cstr_from_buf(&[0u8; 64]), "");
    }

    #[test]
    fn cstr_from_buf_normal() {
        let mut buf = [0u8; 64];
        buf[..5].copy_from_slice(b"Grobb");
        assert_eq!(cstr_from_buf(&buf), "Grobb");
    }

    #[test]
    fn cstr_from_buf_no_nul() {
        // Buffer with no NUL — should return the whole thing as UTF-8.
        let buf: Vec<u8> = b"Halas".to_vec();
        assert_eq!(cstr_from_buf(&buf), "Halas");
    }

    #[test]
    fn cstr_from_buf_non_utf8_is_replaced() {
        // Invalid UTF-8 bytes are replaced with U+FFFD — no panic.
        let buf = [0xFF, 0xFE, 0x00];
        let result = cstr_from_buf(&buf);
        assert!(!result.is_empty()); // lossy conversion, not empty
    }

    // ── PlayerSnapshot → SpawnData ────────────────────────────────────────────

    fn make_player_snapshot() -> PlayerSnapshot {
        PlayerSnapshot {
            spawn_id: 42,
            name: "Firiona".to_string(),
            y: 100.0,
            x: 200.0,
            z: 5.0,
            heading: 128.0,
            speed_run: 0.0,
            stand_state: 0,
            level: 65,
            class: 2,      // CLR
            spawn_type: 0, // player
        }
    }

    #[test]
    fn player_snapshot_into_spawn_data_fields() {
        let snap = make_player_snapshot();
        let sd: SpawnData = snap.clone().into();
        assert_eq!(sd.spawn_id, 42);
        assert_eq!(sd.name, "Firiona");
        assert_eq!(sd.displayed_name, "Firiona");
        assert_eq!(sd.level, 65);
        assert_eq!(sd.class_id, 2);
        assert_eq!(sd.spawn_type, 0);
        assert!((sd.x - 200.0).abs() < f32::EPSILON);
        assert!((sd.y - 100.0).abs() < f32::EPSILON);
        assert!((sd.z - 5.0).abs() < f32::EPSILON);
        assert!((sd.heading - 128.0).abs() < f32::EPSILON);
    }

    #[test]
    fn player_snapshot_hp_defaults_to_zero() {
        let sd: SpawnData = make_player_snapshot().into();
        assert_eq!(sd.hp_current, 0);
        assert_eq!(sd.hp_max, 0);
    }

    #[test]
    fn player_snapshot_not_moving() {
        let sd: SpawnData = make_player_snapshot().into();
        assert!(!sd.is_moving());
    }

    #[test]
    fn player_snapshot_moving() {
        let mut snap = make_player_snapshot();
        snap.speed_run = 0.5;
        let sd: SpawnData = snap.into();
        assert!(sd.is_moving());
    }

    // ── SpawnSnapshot → SpawnData ─────────────────────────────────────────────

    fn make_spawn_snapshot() -> SpawnSnapshot {
        SpawnSnapshot {
            spawn_id: 7,
            name: "a_goblin_warrior".to_string(),
            y: -50.0,
            x: 300.0,
            z: 0.0,
            heading: 0.0,
            speed_run: 0.0,
            stand_state: 0,
            level: 20,
            class: 1,      // WAR
            spawn_type: 1, // NPC
            race: 9,       // Troll
            hp_current: 500,
            hp_max: 1000,
        }
    }

    #[test]
    fn spawn_snapshot_into_spawn_data_fields() {
        let snap = make_spawn_snapshot();
        let sd: SpawnData = snap.into();
        assert_eq!(sd.spawn_id, 7);
        assert_eq!(sd.name, "a_goblin_warrior");
        assert_eq!(sd.level, 20);
        assert_eq!(sd.class_id, 1);
        assert_eq!(sd.spawn_type, 1);
        assert_eq!(sd.race_id, 9);
        assert_eq!(sd.hp_current, 500);
        assert_eq!(sd.hp_max, 1000);
    }

    #[test]
    fn spawn_snapshot_hp_pct() {
        let sd: SpawnData = make_spawn_snapshot().into();
        assert!((sd.hp_pct() - 50.0).abs() < 0.01);
    }

    #[test]
    fn spawn_snapshot_class_label_npc() {
        let sd: SpawnData = make_spawn_snapshot().into();
        assert_eq!(sd.class_str(), "WAR");
    }

    // ── ItemSnapshot ──────────────────────────────────────────────────────────

    #[test]
    fn item_snapshot_fields() {
        let snap = ItemSnapshot {
            id: 1234,
            name: "Mithril Breastplate".to_string(),
            stack_count: 1,
            is_stackable: false,
            inv_slot: 17,
        };
        assert_eq!(snap.id, 1234);
        assert_eq!(snap.name, "Mithril Breastplate");
        assert!(!snap.is_stackable);
        assert_eq!(snap.inv_slot, 17);
    }

    #[test]
    fn item_snapshot_stackable() {
        let snap = ItemSnapshot {
            id: 11,
            name: "Arrow".to_string(),
            stack_count: 200,
            is_stackable: true,
            inv_slot: -1,
        };
        assert!(snap.is_stackable);
        assert_eq!(snap.stack_count, 200);
        assert_eq!(snap.inv_slot, -1);
    }

    // ── struct size / alignment sanity ────────────────────────────────────────

    #[test]
    fn raw_player_client_size_covers_name_field() {
        // The struct must be at least as large as PC_NAME_OFFSET + 64 (name buf).
        assert!(
            std::mem::size_of::<RawPlayerClient>() >= PC_NAME_OFFSET + 64,
            "RawPlayerClient is too small to contain the name buffer"
        );
    }

    #[test]
    fn raw_item_size_covers_inv_slot_field() {
        assert!(
            std::mem::size_of::<RawItem>() >= ITEM_INV_SLOT_OFFSET + 4,
            "RawItem is too small to contain inv_slot"
        );
    }

    // ── PlayerClientRef (non-Windows) ─────────────────────────────────────────

    #[test]
    fn player_client_ref_null_returns_none() {
        // SAFETY: passing null is the documented test case for None return.
        let result = unsafe { PlayerClientRef::new(std::ptr::null_mut()) };
        assert!(result.is_none());
    }

    #[test]
    fn item_ref_null_returns_none() {
        // SAFETY: passing null is the documented test case for None return.
        let result = unsafe { ItemRef::new(std::ptr::null_mut()) };
        assert!(result.is_none());
    }

    /// Smoke test: construct a RawPlayerClient from a leaked Box (safe, stable
    /// address), wrap it in PlayerClientRef, snapshot it, and verify fields.
    ///
    /// Only runs on non-Windows because on Windows `is_readable_block` gates
    /// access. On Windows this would require a real EQ address.
    #[cfg(not(windows))]
    #[test]
    fn player_client_ref_snapshot_from_box() {
        // Build a zeroed raw struct, fill in the fields we care about.
        let mut raw: Box<RawPlayerClient> = unsafe { Box::new(std::mem::zeroed()) };
        raw.spawn_id = 99;
        raw.y = 1.0;
        raw.x = 2.0;
        raw.z = 3.0;
        raw.heading = 64.0;
        raw.speed_run = 0.0;
        raw.stand_state = 3; // sitting
        raw.level = 50;
        raw.class = 8; // BRD
        raw.spawn_type = 0;
        let name_bytes = b"Melody\0";
        raw.name[..name_bytes.len()].copy_from_slice(name_bytes);

        let ptr: *mut RawPlayerClient = Box::into_raw(raw);
        // SAFETY: ptr is a valid, non-null pointer from Box::into_raw.
        let ref_ = unsafe { PlayerClientRef::new(ptr) };
        // On non-Windows there is no is_readable_block guard, so new()
        // succeeds for any non-null pointer. Snapshot to verify field reads.
        assert!(ref_.is_some());
        let snap = ref_.unwrap().snapshot();
        assert_eq!(snap.spawn_id, 99);
        assert_eq!(snap.name, "Melody");
        // Retake ownership so memory is freed.
        // SAFETY: ptr was created by Box::into_raw; we are re-boxing it.
        let _ = unsafe { Box::from_raw(ptr) };
    }

    /// On non-Windows, test snapshot() directly via the raw struct without
    /// the readability guard (simulates what Windows game thread would do).
    #[cfg(not(windows))]
    #[test]
    fn player_client_ref_snapshot_direct() {
        let mut raw: RawPlayerClient = unsafe { std::mem::zeroed() };
        raw.spawn_id = 77;
        raw.level = 60;
        raw.class = 1;
        raw.spawn_type = 0;
        raw.y = 10.0;
        raw.x = 20.0;
        raw.z = 5.0;
        raw.heading = 256.0;
        raw.speed_run = 1.5;
        raw.stand_state = 0;
        let name = b"Wukk\0";
        raw.name[..name.len()].copy_from_slice(name);

        // Construct PlayerClientRef directly bypassing the readability check
        // (for test-only use — this is only safe because raw is on the stack
        // and we do not cross tick boundaries).
        let nn = NonNull::from(&mut raw);
        let ref_ = PlayerClientRef {
            ptr: nn,
            _marker: std::marker::PhantomData,
        };
        let snap = ref_.snapshot();
        assert_eq!(snap.spawn_id, 77);
        assert_eq!(snap.name, "Wukk");
        assert_eq!(snap.level, 60);
        assert_eq!(snap.class, 1);
        assert!((snap.speed_run - 1.5).abs() < f32::EPSILON);
        assert_eq!(snap.stand_state, 0);

        let sd: SpawnData = snap.into();
        assert_eq!(sd.spawn_id, 77);
        assert_eq!(sd.class_str(), "WAR");
        assert!(sd.is_moving());
    }

    #[cfg(not(windows))]
    #[test]
    fn item_ref_snapshot_direct() {
        let mut raw: RawItem = unsafe { std::mem::zeroed() };
        raw.id = 99;
        let name = b"Jade Reaver\0";
        raw.name[..name.len()].copy_from_slice(name);
        raw.stack_count = 1;
        raw.is_stackable = false;
        raw.inv_slot = 13;

        let nn = NonNull::from(&mut raw);
        let ref_ = ItemRef {
            ptr: nn,
            _marker: std::marker::PhantomData,
        };
        let snap = ref_.snapshot();
        assert_eq!(snap.id, 99);
        assert_eq!(snap.name, "Jade Reaver");
        assert_eq!(snap.inv_slot, 13);
        assert!(!snap.is_stackable);
    }
}
