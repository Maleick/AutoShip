//! Typed EQ memory overlays for the debugger struct browser.

use std::{mem::size_of, sync::atomic::Ordering};

use textquest_common::{
    ipc::{StructFieldSnapshot, StructFieldType, StructFieldValue, StructSnapshot},
    offsets as o,
};

#[derive(Clone, Copy)]
struct FieldDef {
    name: &'static str,
    offset: usize,
    field_type: StructFieldType,
    nested_struct_type: Option<&'static str>,
}

struct StructDef {
    name: &'static str,
    fields: &'static [FieldDef],
}

const PLAYER_BASE_FIELDS: &[FieldDef] = &[
    FieldDef {
        name: "PREV",
        offset: o::player_base::PREV,
        field_type: StructFieldType::Pointer,
        nested_struct_type: Some("PlayerBase"),
    },
    FieldDef {
        name: "NEXT",
        offset: o::player_base::NEXT,
        field_type: StructFieldType::Pointer,
        nested_struct_type: Some("PlayerBase"),
    },
    FieldDef {
        name: "LASTNAME",
        offset: o::player_base::LASTNAME,
        field_type: StructFieldType::String { len: 32 },
        nested_struct_type: None,
    },
    FieldDef {
        name: "Y",
        offset: o::player_base::Y,
        field_type: StructFieldType::F32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "X",
        offset: o::player_base::X,
        field_type: StructFieldType::F32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "Z",
        offset: o::player_base::Z,
        field_type: StructFieldType::F32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "SPEED_CURRENT",
        offset: o::player_base::SPEED_CURRENT,
        field_type: StructFieldType::F32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "SPEED_Z",
        offset: o::player_base::SPEED_Z,
        field_type: StructFieldType::F32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "SPEED_RUN",
        offset: o::player_base::SPEED_RUN,
        field_type: StructFieldType::F32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "HEADING",
        offset: o::player_base::HEADING,
        field_type: StructFieldType::F32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "SPEED_HEADING",
        offset: o::player_base::SPEED_HEADING,
        field_type: StructFieldType::F32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "NAME",
        offset: o::player_base::NAME,
        field_type: StructFieldType::String { len: 64 },
        nested_struct_type: None,
    },
    FieldDef {
        name: "DISPLAYED_NAME",
        offset: o::player_base::DISPLAYED_NAME,
        field_type: StructFieldType::String { len: 64 },
        nested_struct_type: None,
    },
    FieldDef {
        name: "TYPE",
        offset: o::player_base::TYPE,
        field_type: StructFieldType::U8,
        nested_struct_type: None,
    },
    FieldDef {
        name: "SPAWN_ID",
        offset: o::player_base::SPAWN_ID,
        field_type: StructFieldType::U32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "MANAGED_TARGET",
        offset: o::player_base::MANAGED_TARGET,
        field_type: StructFieldType::Pointer,
        nested_struct_type: Some("PlayerBase"),
    },
];

const PLAYER_ZONE_FIELDS: &[FieldDef] = &[
    FieldDef {
        name: "NEXT",
        offset: o::player_base::NEXT,
        field_type: StructFieldType::Pointer,
        nested_struct_type: Some("PlayerZone"),
    },
    FieldDef {
        name: "NAME",
        offset: o::player_base::NAME,
        field_type: StructFieldType::String { len: 64 },
        nested_struct_type: None,
    },
    FieldDef {
        name: "DISPLAYED_NAME",
        offset: o::player_base::DISPLAYED_NAME,
        field_type: StructFieldType::String { len: 64 },
        nested_struct_type: None,
    },
    FieldDef {
        name: "X",
        offset: o::player_base::X,
        field_type: StructFieldType::F32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "Y",
        offset: o::player_base::Y,
        field_type: StructFieldType::F32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "Z",
        offset: o::player_base::Z,
        field_type: StructFieldType::F32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "SPAWN_ID",
        offset: o::player_base::SPAWN_ID,
        field_type: StructFieldType::U32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "CASTING_DATA",
        offset: o::player_zone::CASTING_DATA,
        field_type: StructFieldType::Pointer,
        nested_struct_type: Some("LaunchSpellData"),
    },
    FieldDef {
        name: "HP_MAX",
        offset: o::player_zone::HP_MAX,
        field_type: StructFieldType::I64,
        nested_struct_type: None,
    },
    FieldDef {
        name: "HP_CURRENT",
        offset: o::player_zone::HP_CURRENT,
        field_type: StructFieldType::I64,
        nested_struct_type: None,
    },
    FieldDef {
        name: "MANA_MAX",
        offset: o::player_zone::MANA_MAX,
        field_type: StructFieldType::I32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "MANA_CURRENT",
        offset: o::player_zone::MANA_CURRENT,
        field_type: StructFieldType::I32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "LEVEL",
        offset: o::player_zone::LEVEL,
        field_type: StructFieldType::U8,
        nested_struct_type: None,
    },
    FieldDef {
        name: "STANDSTATE",
        offset: o::player_zone::STANDSTATE,
        field_type: StructFieldType::U8,
        nested_struct_type: None,
    },
    FieldDef {
        name: "GM",
        offset: o::player_zone::GM,
        field_type: StructFieldType::U8,
        nested_struct_type: None,
    },
    FieldDef {
        name: "GM_RANK",
        offset: o::player_zone::GM_RANK,
        field_type: StructFieldType::U8,
        nested_struct_type: None,
    },
    FieldDef {
        name: "CHAR_CLASS",
        offset: o::player_zone::CHAR_CLASS,
        field_type: StructFieldType::U8,
        nested_struct_type: None,
    },
    FieldDef {
        name: "ENDURANCE_CURRENT",
        offset: o::player_zone::ENDURANCE_CURRENT,
        field_type: StructFieldType::I32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "ENDURANCE_MAX",
        offset: o::player_zone::ENDURANCE_MAX,
        field_type: StructFieldType::U32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "MELEE_RADIUS",
        offset: o::player_zone::MELEE_RADIUS,
        field_type: StructFieldType::F32,
        nested_struct_type: None,
    },
];

const PC_CLIENT_FIELDS: &[FieldDef] = &[
    FieldDef {
        name: "BUFF_IDS_0",
        offset: o::profile::BUFF_IDS,
        field_type: StructFieldType::I32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "PROFILE_MANAGER",
        offset: o::profile::PROFILE_MANAGER,
        field_type: StructFieldType::Pointer,
        nested_struct_type: None,
    },
    FieldDef {
        name: "CHARACTER_ZONE_ME",
        offset: o::character_zone::ME,
        field_type: StructFieldType::Pointer,
        nested_struct_type: Some("PlayerZone"),
    },
    FieldDef {
        name: "P_EXTENDED_TARGET_LIST",
        offset: o::PCCLIENT_EXTENDED_TARGET_LIST as usize,
        field_type: StructFieldType::Pointer,
        nested_struct_type: None,
    },
    FieldDef {
        name: "IN_COMBAT",
        offset: o::PCCLIENT_IN_COMBAT as usize,
        field_type: StructFieldType::U8,
        nested_struct_type: None,
    },
];

const PROFILE_FIELDS: &[FieldDef] = &[
    FieldDef {
        name: "BUFFS_ARRAY_DATA_PTR",
        offset: o::profile::BUFFS_ARRAY + o::profile::ARRAY_DATA_PTR,
        field_type: StructFieldType::Pointer,
        nested_struct_type: Some("BuffSlot"),
    },
    FieldDef {
        name: "BUFFS_ARRAY_SIZE",
        offset: o::profile::BUFFS_ARRAY + o::profile::ARRAY_SIZE,
        field_type: StructFieldType::I32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "SPELL_BOOK_0",
        offset: o::profile::SPELL_BOOK,
        field_type: StructFieldType::I32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "MEMORIZED_SPELLS_0",
        offset: o::profile::MEMORIZED_SPELLS,
        field_type: StructFieldType::I32,
        nested_struct_type: None,
    },
];

const BUFF_SLOT_FIELDS: &[FieldDef] = &[
    FieldDef {
        name: "SPELL_ID",
        offset: o::buff_slots::SPELL_ID,
        field_type: StructFieldType::I32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "DURATION",
        offset: o::buff_slots::DURATION,
        field_type: StructFieldType::I32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "INITIAL_DURATION",
        offset: o::buff_slots::INITIAL_DURATION,
        field_type: StructFieldType::I32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "HIT_COUNT",
        offset: o::buff_slots::HIT_COUNT,
        field_type: StructFieldType::I32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "MODIFIER",
        offset: o::buff_slots::MODIFIER,
        field_type: StructFieldType::F32,
        nested_struct_type: None,
    },
    FieldDef {
        name: "BUFF_TYPE",
        offset: o::buff_slots::BUFF_TYPE,
        field_type: StructFieldType::U8,
        nested_struct_type: None,
    },
    FieldDef {
        name: "CASTER_LEVEL",
        offset: o::buff_slots::CASTER_LEVEL,
        field_type: StructFieldType::U8,
        nested_struct_type: None,
    },
];

const SPAWN_MANAGER_FIELDS: &[FieldDef] = &[FieldDef {
    name: "PLAYER_LIST",
    offset: o::spawn_manager::PLAYER_LIST,
    field_type: StructFieldType::Pointer,
    nested_struct_type: Some("PlayerBase"),
}];

const ZONE_INFO_FIELDS: &[FieldDef] = &[
    FieldDef {
        name: "SHORT_NAME",
        offset: o::zone_info::SHORT_NAME,
        field_type: StructFieldType::String { len: 128 },
        nested_struct_type: None,
    },
    FieldDef {
        name: "LONG_NAME",
        offset: o::zone_info::LONG_NAME,
        field_type: StructFieldType::String { len: 128 },
        nested_struct_type: None,
    },
];

static PLAYER_BASE_DEF: StructDef = StructDef {
    name: "PlayerBase",
    fields: PLAYER_BASE_FIELDS,
};
static PLAYER_ZONE_DEF: StructDef = StructDef {
    name: "PlayerZone",
    fields: PLAYER_ZONE_FIELDS,
};
static PC_CLIENT_DEF: StructDef = StructDef {
    name: "PcClient",
    fields: PC_CLIENT_FIELDS,
};
static PROFILE_DEF: StructDef = StructDef {
    name: "Profile",
    fields: PROFILE_FIELDS,
};
static BUFF_SLOT_DEF: StructDef = StructDef {
    name: "BuffSlot",
    fields: BUFF_SLOT_FIELDS,
};
static SPAWN_MANAGER_DEF: StructDef = StructDef {
    name: "SpawnManager",
    fields: SPAWN_MANAGER_FIELDS,
};
static ZONE_INFO_DEF: StructDef = StructDef {
    name: "ZoneInfo",
    fields: ZONE_INFO_FIELDS,
};

enum GlobalAddress {
    Pointer(u64),
    Direct(u64),
}

/// Read every registered field for a struct instance.
pub fn read_struct(
    struct_type: &str,
    base_address_or_global_name: &str,
) -> Result<StructSnapshot, String> {
    let def = find_struct_def(struct_type)?;
    let base_address = resolve_base_address(base_address_or_global_name)?;
    read_struct_at(def, base_address)
}

/// Read one registered field for a struct instance.
pub fn read_struct_field(
    struct_type: &str,
    field_name: &str,
    base_address: usize,
) -> Result<(String, StructFieldSnapshot), String> {
    let def = find_struct_def(struct_type)?;
    let field = find_field_def(def, field_name)?;
    Ok((
        def.name.to_string(),
        read_field_snapshot(base_address, field),
    ))
}

fn read_struct_at(def: &'static StructDef, base_address: usize) -> Result<StructSnapshot, String> {
    if base_address == 0 {
        return Err(format!("{} base address is null", def.name));
    }

    let fields = def
        .fields
        .iter()
        .map(|field| read_field_snapshot(base_address, field))
        .collect();

    Ok(StructSnapshot {
        struct_type: def.name.to_string(),
        base_address,
        fields,
        timestamp_ms: timestamp_ms(),
    })
}

fn read_field_snapshot(base_address: usize, field: &FieldDef) -> StructFieldSnapshot {
    let address = match base_address.checked_add(field.offset) {
        Some(address) => address,
        None => {
            let message = format!(
                "address overflow at base {base_address:#x} + offset {:#x}",
                field.offset
            );
            return field_snapshot(field, usize::MAX, StructFieldValue::Unreadable(message));
        }
    };

    let value = read_field_value(address, field.field_type)
        .unwrap_or_else(|error| StructFieldValue::Unreadable(error));
    field_snapshot(field, address, value)
}

fn field_snapshot(
    field: &FieldDef,
    address: usize,
    value: StructFieldValue,
) -> StructFieldSnapshot {
    let display_value = display_value(&value);
    StructFieldSnapshot {
        name: field.name.to_string(),
        offset: field.offset,
        address,
        field_type: field.field_type,
        value,
        display_value,
        nested_struct_type: field.nested_struct_type.map(str::to_string),
    }
}

fn read_field_value(
    address: usize,
    field_type: StructFieldType,
) -> Result<StructFieldValue, String> {
    Ok(match field_type {
        StructFieldType::U8 => StructFieldValue::U8(read_fixed::<1>(address)?[0]),
        StructFieldType::U16 => {
            StructFieldValue::U16(u16::from_le_bytes(read_fixed::<2>(address)?))
        }
        StructFieldType::U32 => {
            StructFieldValue::U32(u32::from_le_bytes(read_fixed::<4>(address)?))
        }
        StructFieldType::U64 => {
            StructFieldValue::U64(u64::from_le_bytes(read_fixed::<8>(address)?))
        }
        StructFieldType::I8 => StructFieldValue::I8(read_fixed::<1>(address)?[0] as i8),
        StructFieldType::I16 => {
            StructFieldValue::I16(i16::from_le_bytes(read_fixed::<2>(address)?))
        }
        StructFieldType::I32 => {
            StructFieldValue::I32(i32::from_le_bytes(read_fixed::<4>(address)?))
        }
        StructFieldType::I64 => {
            StructFieldValue::I64(i64::from_le_bytes(read_fixed::<8>(address)?))
        }
        StructFieldType::F32 => {
            StructFieldValue::F32(f32::from_le_bytes(read_fixed::<4>(address)?))
        }
        StructFieldType::F64 => {
            StructFieldValue::F64(f64::from_le_bytes(read_fixed::<8>(address)?))
        }
        StructFieldType::String { len } => {
            let bytes = read_bytes(address, len)?;
            let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
            StructFieldValue::String(String::from_utf8_lossy(&bytes[..end]).into_owned())
        }
        StructFieldType::Pointer => StructFieldValue::Pointer(read_usize(address)?),
    })
}

fn read_fixed<const N: usize>(address: usize) -> Result<[u8; N], String> {
    let bytes = read_bytes(address, N)?;
    bytes.as_slice().try_into().map_err(|_| {
        format!(
            "short read at {address:#x}: got {} of {N} bytes",
            bytes.len()
        )
    })
}

fn read_usize(address: usize) -> Result<usize, String> {
    let bytes = read_bytes(address, size_of::<usize>())?;
    usize_from_le_bytes(&bytes)
}

#[cfg(target_pointer_width = "64")]
fn usize_from_le_bytes(bytes: &[u8]) -> Result<usize, String> {
    let array: [u8; 8] = bytes
        .try_into()
        .map_err(|_| format!("short pointer read: got {} of 8 bytes", bytes.len()))?;
    Ok(usize::from_le_bytes(array))
}

#[cfg(target_pointer_width = "32")]
fn usize_from_le_bytes(bytes: &[u8]) -> Result<usize, String> {
    let array: [u8; 4] = bytes
        .try_into()
        .map_err(|_| format!("short pointer read: got {} of 4 bytes", bytes.len()))?;
    Ok(usize::from_le_bytes(array))
}

fn read_bytes(address: usize, size: usize) -> Result<Vec<u8>, String> {
    if size == 0 {
        return Ok(Vec::new());
    }

    #[cfg(windows)]
    {
        use windows::Win32::System::{
            Diagnostics::Debug::ReadProcessMemory, Threading::GetCurrentProcess,
        };

        let mut buf = vec![0u8; size];
        let mut bytes_read = 0usize;
        let _result = unsafe {
            ReadProcessMemory(
                GetCurrentProcess(),
                address as *const core::ffi::c_void,
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                size,
                Some(&mut bytes_read),
            )
        };
        if bytes_read == 0 {
            return Err(format!("failed to read {size} bytes at {address:#x}"));
        }
        buf.truncate(bytes_read);
        Ok(buf)
    }

    #[cfg(not(windows))]
    {
        let _ = address;
        Ok(vec![0; size])
    }
}

fn display_value(value: &StructFieldValue) -> String {
    match value {
        StructFieldValue::U8(value) => value.to_string(),
        StructFieldValue::U16(value) => value.to_string(),
        StructFieldValue::U32(value) => value.to_string(),
        StructFieldValue::U64(value) => value.to_string(),
        StructFieldValue::I8(value) => value.to_string(),
        StructFieldValue::I16(value) => value.to_string(),
        StructFieldValue::I32(value) => value.to_string(),
        StructFieldValue::I64(value) => value.to_string(),
        StructFieldValue::F32(value) => format!("{value:.3}"),
        StructFieldValue::F64(value) => format!("{value:.3}"),
        StructFieldValue::String(value) => value.clone(),
        StructFieldValue::Pointer(0) => "null".to_string(),
        StructFieldValue::Pointer(value) => format!("{value:#x}"),
        StructFieldValue::Unreadable(error) => format!("<unreadable: {error}>"),
    }
}

fn resolve_base_address(base_address_or_global_name: &str) -> Result<usize, String> {
    let input = base_address_or_global_name.trim();
    if input.is_empty() {
        return Err("struct base address/global name is empty".to_string());
    }

    if let Some(address) = parse_address(input) {
        return Ok(address);
    }

    match find_global_address(input)? {
        GlobalAddress::Pointer(preferred_address) => read_global_pointer(preferred_address, input),
        GlobalAddress::Direct(preferred_address) => rebase_global(preferred_address, input),
    }
}

fn read_global_pointer(preferred_address: u64, name: &str) -> Result<usize, String> {
    let slot = rebase_global(preferred_address, name)?;
    let pointer = read_usize(slot)?;
    if pointer == 0 {
        return Err(format!("{name} resolved to null"));
    }
    Ok(pointer)
}

fn rebase_global(preferred_address: u64, name: &str) -> Result<usize, String> {
    let eq_base = crate::EQ_BASE.load(Ordering::Acquire);
    if eq_base == 0 {
        return Err(format!("cannot resolve {name}: EQ base is not initialized"));
    }

    o::rebase(preferred_address, eq_base)
        .ok_or_else(|| format!("cannot rebase {name} preferred address {preferred_address:#x}"))
}

fn parse_address(input: &str) -> Option<usize> {
    input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
        .and_then(|hex| usize::from_str_radix(hex, 16).ok())
        .or_else(|| input.parse::<usize>().ok())
}

fn find_struct_def(struct_type: &str) -> Result<&'static StructDef, String> {
    let key = normalize_name(struct_type);
    match key.as_str() {
        "playerbase" | "playerclient" | "spawn" | "spawninfo" => Ok(&PLAYER_BASE_DEF),
        "playerzone" | "playerzoneclient" => Ok(&PLAYER_ZONE_DEF),
        "pcclient" | "localpc" => Ok(&PC_CLIENT_DEF),
        "profile" | "pcprofile" | "baseprofile" => Ok(&PROFILE_DEF),
        "buffslot" | "buffslots" | "eqaffect" => Ok(&BUFF_SLOT_DEF),
        "spawnmanager" | "playermanagerclient" | "playermanagerbase" => Ok(&SPAWN_MANAGER_DEF),
        "zoneinfo" | "zoneheader" => Ok(&ZONE_INFO_DEF),
        _ => Err(format!("unknown struct type '{struct_type}'")),
    }
}

fn find_field_def(def: &'static StructDef, field_name: &str) -> Result<&'static FieldDef, String> {
    let key = normalize_name(field_name);
    def.fields
        .iter()
        .find(|field| normalize_name(field.name) == key)
        .ok_or_else(|| format!("unknown field '{}' for {}", field_name, def.name))
}

fn find_global_address(name: &str) -> Result<GlobalAddress, String> {
    let key = normalize_name(name);
    match key.as_str() {
        "pinstlocalplayer" | "localplayer" => Ok(GlobalAddress::Pointer(o::PINST_LOCAL_PLAYER)),
        "pinstcontrolledplayer" | "controlledplayer" => {
            Ok(GlobalAddress::Pointer(o::PINST_CONTROLLED_PLAYER))
        }
        "pinsttarget" | "target" => Ok(GlobalAddress::Pointer(o::PINST_TARGET)),
        "pinstspawnmanager" | "spawnmanager" => Ok(GlobalAddress::Pointer(o::PINST_SPAWN_MANAGER)),
        "pinstlocalpc" | "localpc" | "pcclient" => Ok(GlobalAddress::Pointer(o::PINST_LOCAL_PC)),
        "pinstspellmanager" | "spellmanager" => Ok(GlobalAddress::Pointer(o::PINST_SPELL_MANAGER)),
        "pinstcdisplay" | "cdisplay" => Ok(GlobalAddress::Pointer(o::PINST_CDISPLAY)),
        "pinstceverquest" | "ceverquest" | "everquest" => {
            Ok(GlobalAddress::Pointer(o::PINST_CEVERQUEST))
        }
        "pinstcchatwindowmanager" | "cchatwindowmanager" => {
            Ok(GlobalAddress::Pointer(o::PINST_CCHAT_WINDOW_MANAGER))
        }
        "pinstcinvslotmgr" | "cinvslotmgr" => Ok(GlobalAddress::Pointer(o::PINST_CINV_SLOT_MGR)),
        "insteqzoneinfo" | "zoneinfo" | "zoneheader" => {
            Ok(GlobalAddress::Direct(o::zone_info::INST_EQ_ZONE_INFO))
        }
        _ => Err(format!("unknown struct base/global '{name}'")),
    }
}

fn normalize_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}
