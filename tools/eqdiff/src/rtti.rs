//! RTTI and vtable matching helpers for binary diffing.

use std::collections::HashMap;
use std::ops::Range;

/// A class discovered from MSVC RTTI metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RttiClass {
    /// TypeDescriptor name, such as `.?AVCPlayer@@`.
    pub name: String,
    /// RVA of the TypeDescriptor that owns `name`.
    pub type_descriptor_rva: u32,
    /// RVA of the class vftable when known.
    pub vtable_rva: u32,
}

impl RttiClass {
    /// Construct a discovered RTTI class record.
    pub fn new(name: String, type_descriptor_rva: u32, vtable_rva: u32) -> Self {
        Self {
            name,
            type_descriptor_rva,
            vtable_rva,
        }
    }
}

/// Matched class metadata across old/new binaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassRttiMatch {
    /// RTTI class name shared by both binaries.
    pub name: String,
    /// Old binary TypeDescriptor RVA.
    pub old_type_descriptor_rva: u32,
    /// New binary TypeDescriptor RVA.
    pub new_type_descriptor_rva: u32,
    /// Old binary vftable RVA.
    pub old_vtable_rva: u32,
    /// New binary vftable RVA.
    pub new_vtable_rva: u32,
}

/// A vftable and the function RVAs found in its slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VTable {
    /// RTTI class name that owns this vftable.
    pub class_name: String,
    /// RVA of the first function pointer slot.
    pub vtable_rva: u32,
    /// Function RVAs ordered by slot index.
    pub slots: Vec<u32>,
}

impl VTable {
    /// Construct a vftable record.
    pub fn new(class_name: String, vtable_rva: u32, slots: Vec<u32>) -> Self {
        Self {
            class_name,
            vtable_rva,
            slots,
        }
    }
}

/// A virtual function matched by vftable slot index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VTableSlotMatch {
    /// Matched RTTI class name.
    pub class_name: String,
    /// Slot index shared by old/new vftables.
    pub slot_index: usize,
    /// Function RVA in the old binary.
    pub old_function_rva: u32,
    /// Function RVA in the new binary.
    pub new_function_rva: u32,
}

/// Match RTTI classes by exact TypeDescriptor name.
pub fn match_classes_by_rtti_name(old: &[RttiClass], new: &[RttiClass]) -> Vec<ClassRttiMatch> {
    let new_by_name: HashMap<&str, &RttiClass> = new
        .iter()
        .map(|class| (class.name.as_str(), class))
        .collect();

    let mut matches: Vec<_> = old
        .iter()
        .filter_map(|old_class| {
            let new_class = new_by_name.get(old_class.name.as_str())?;
            Some(ClassRttiMatch {
                name: old_class.name.clone(),
                old_type_descriptor_rva: old_class.type_descriptor_rva,
                new_type_descriptor_rva: new_class.type_descriptor_rva,
                old_vtable_rva: old_class.vtable_rva,
                new_vtable_rva: new_class.vtable_rva,
            })
        })
        .collect();

    matches.sort_by(|a, b| a.name.cmp(&b.name));
    matches
}

/// Match virtual functions by slot index after their owning RTTI class matched.
pub fn match_vtable_slots_by_index(old: &VTable, new: &VTable) -> Vec<VTableSlotMatch> {
    if old.class_name != new.class_name {
        return Vec::new();
    }

    old.slots
        .iter()
        .zip(new.slots.iter())
        .enumerate()
        .map(
            |(slot_index, (&old_function_rva, &new_function_rva))| VTableSlotMatch {
                class_name: old.class_name.clone(),
                slot_index,
                old_function_rva,
                new_function_rva,
            },
        )
        .collect()
}

/// Extract MSVC TypeDescriptor names from `.rdata` bytes.
///
/// The class name starts 16 bytes after the TypeDescriptor on x64. This returns
/// class records with `vtable_rva` set to zero; callers can fill it after
/// correlating CompleteObjectLocator/vftable data.
pub fn extract_rtti_type_descriptors_from_rdata(rdata: &[u8], rdata_rva: u32) -> Vec<RttiClass> {
    let mut classes = Vec::new();
    let mut offset = 0usize;

    while offset < rdata.len() {
        if !looks_like_msvc_type_name(&rdata[offset..]) {
            offset += 1;
            continue;
        }

        let Some(len) = rdata[offset..].iter().position(|&byte| byte == 0) else {
            break;
        };
        if let Ok(name) = std::str::from_utf8(&rdata[offset..offset + len])
            && offset >= 16
        {
            classes.push(RttiClass::new(
                name.to_owned(),
                rdata_rva + (offset as u32) - 16,
                0,
            ));
        }
        offset += len.max(1);
    }

    classes.sort_by(|a, b| a.name.cmp(&b.name));
    classes
}

/// Parse contiguous vftable slots until a pointer stops resolving into `.text`.
pub fn parse_vtable_slots(
    rdata: &[u8],
    rdata_rva: u32,
    vtable_rva: u32,
    image_base: u64,
    text_rva_range: Range<u32>,
) -> Vec<u32> {
    let Some(mut offset) = vtable_rva
        .checked_sub(rdata_rva)
        .map(|value| value as usize)
    else {
        return Vec::new();
    };

    let mut slots = Vec::new();
    while offset + 8 <= rdata.len() {
        let mut pointer_bytes = [0u8; 8];
        pointer_bytes.copy_from_slice(&rdata[offset..offset + 8]);
        let pointer = u64::from_le_bytes(pointer_bytes);
        if pointer < image_base {
            break;
        }

        let function_rva = (pointer - image_base) as u32;
        if !text_rva_range.contains(&function_rva) {
            break;
        }

        slots.push(function_rva);
        offset += 8;
    }

    slots
}

fn looks_like_msvc_type_name(bytes: &[u8]) -> bool {
    bytes.starts_with(b".?AV") || bytes.starts_with(b".?AU")
}
