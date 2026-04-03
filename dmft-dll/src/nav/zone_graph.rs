//! Read zone adjacency graph from EQ's `ZoneGuideManagerClient`.
//!
//! `ZoneGuideManagerClient` is a singleton containing a fixed-size array of 888
//! `ZoneGuideZone` entries. Each zone has a name, level range, and an `ArrayClass`
//! of `ZoneGuideConnection` entries describing how to reach neighboring zones.

use dmft_common::nav::ZoneGraph;

/// Read the complete zone graph from memory.
///
/// # Safety
/// Must be called from the EQ game process (injected DLL context) while the
/// game is running and `ZoneGuideManagerClient` is initialized.
#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn read_zone_graph(eq_base: u64) -> Option<ZoneGraph> {
    use crate::eq::widgets::read_cxstr;
    use dmft_common::nav::{ZoneConnection, ZoneNode};
    use dmft_common::offsets::{self, zone_guide as zg};
    use windows::Win32::System::Memory::{
        MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_EXECUTE, PAGE_EXECUTE_READ,
        PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_GUARD, PAGE_NOACCESS, PAGE_READONLY,
        PAGE_READWRITE, PAGE_WRITECOPY, VirtualQuery,
    };

    if eq_base == 0 {
        tracing::warn!("read_zone_graph called with eq_base=0");
        return None;
    }

    // SAFETY: All pointer reads in this function follow EQ's ZoneGuideManagerClient
    // struct layout. mgr_ptr_addr is rebased from ZONE_GUIDE_MANAGER — a known
    // global in eqgame.exe. Each subsequent dereference follows known offsets
    // (zone array, connections array) with null/range checks. The function is
    // called from the game loop thread where the zone guide data is stable.
    // If any pointer is invalid, we return None rather than crashing.

    unsafe fn is_readable_range(addr: usize, size: usize) -> bool {
        if addr < 0x10000 || size == 0 {
            return false;
        }
        let end = match addr.checked_add(size.saturating_sub(1)) {
            Some(v) => v,
            None => return false,
        };

        let mut cursor = addr;
        while cursor <= end {
            let mut mbi = MEMORY_BASIC_INFORMATION::default();
            if VirtualQuery(
                Some(cursor as *const _),
                &mut mbi,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            ) == 0
            {
                return false;
            }

            if mbi.State != MEM_COMMIT || (mbi.Protect.0 & (PAGE_GUARD.0 | PAGE_NOACCESS.0)) != 0 {
                return false;
            }

            let readable = matches!(
                mbi.Protect.0,
                x if x == PAGE_READONLY.0
                    || x == PAGE_READWRITE.0
                    || x == PAGE_WRITECOPY.0
                    || x == PAGE_EXECUTE.0
                    || x == PAGE_EXECUTE_READ.0
                    || x == PAGE_EXECUTE_READWRITE.0
                    || x == PAGE_EXECUTE_WRITECOPY.0
            );
            if !readable {
                return false;
            }

            let region_end = (mbi.BaseAddress as usize).saturating_add(mbi.RegionSize);
            if region_end == 0 || region_end <= cursor {
                return false;
            }
            if region_end > end {
                return true;
            }
            cursor = region_end;
        }
        true
    }

    unsafe fn read_checked<T: Copy>(addr: usize) -> Option<T> {
        is_readable_range(addr, std::mem::size_of::<T>()).then(|| *(addr as *const T))
    }

    // Resolve the singleton pointer
    let mgr_ptr_addr = offsets::rebase(offsets::ZONE_GUIDE_MANAGER, eq_base)?;
    let mgr_ptr = read_checked::<usize>(mgr_ptr_addr)?;
    if mgr_ptr == 0 {
        tracing::warn!("ZoneGuideManagerClient pointer is null");
        return None;
    }

    // Check if zone guide data is populated
    let data_set = read_checked::<u8>(mgr_ptr + zg::DATA_SET)?;
    if data_set == 0 {
        tracing::warn!("ZoneGuideManagerClient.zoneGuideDataSet is false");
        return None;
    }

    let zones_base = mgr_ptr + zg::ZONES_OFFSET;
    let mut graph = ZoneGraph::default();

    for i in 0..zg::ZONE_COUNT {
        let zone_addr = zones_base + i * zg::ZONE_SIZE;

        let Some(zone_id) = read_checked::<i32>(zone_addr + zg::ZONE_ID) else {
            continue;
        };
        if zone_id <= 0 || zone_id > zg::ZONE_COUNT as i32 {
            continue;
        }

        let name = read_cxstr(zone_addr + zg::ZONE_NAME).unwrap_or_default();
        if name.is_empty() {
            continue;
        }

        let Some(min_level) = read_checked::<i32>(zone_addr + zg::ZONE_MIN_LEVEL) else {
            continue;
        };
        let Some(max_level) = read_checked::<i32>(zone_addr + zg::ZONE_MAX_LEVEL) else {
            continue;
        };

        // Read connections ArrayClass
        let Some(conn_count) = read_checked::<i32>(zone_addr + zg::ZONE_CONNECTIONS_COUNT) else {
            continue;
        };
        let Some(conn_array) = read_checked::<usize>(zone_addr + zg::ZONE_CONNECTIONS_ARRAY) else {
            continue;
        };

        let mut connections = Vec::new();
        if conn_count > 0 && conn_count < 200 && conn_array != 0 {
            for j in 0..conn_count as usize {
                let conn_addr = conn_array + j * zg::CONNECTION_SIZE;

                let Some(dest_zone_id) = read_checked::<i32>(conn_addr + zg::CONN_DEST_ZONE_ID)
                else {
                    continue;
                };
                let Some(transfer_type) = read_checked::<i32>(conn_addr + zg::CONN_TRANSFER_TYPE)
                else {
                    continue;
                };
                let Some(disabled) = read_checked::<u8>(conn_addr + zg::CONN_DISABLED) else {
                    continue;
                };

                if dest_zone_id > 0 && dest_zone_id <= zg::ZONE_COUNT as i32 {
                    connections.push(ZoneConnection {
                        dest_zone_id: dest_zone_id as u16,
                        transfer_type: transfer_type.clamp(0, 255) as u8,
                        disabled: disabled != 0,
                    });
                }
            }
        }

        graph.zones.insert(
            zone_id as u16,
            ZoneNode {
                zone_id: zone_id as u16,
                name,
                min_level,
                max_level,
                connections,
            },
        );
    }

    tracing::info!(
        zone_count = graph.zones.len(),
        "Read zone graph from ZoneGuideManagerClient"
    );
    Some(graph)
}

#[cfg(not(windows))]
pub unsafe fn read_zone_graph(_eq_base: u64) -> Option<ZoneGraph> {
    None
}

/// Convert a `ZoneGraph` into the simplified IPC wire format.
pub fn zone_graph_to_ipc(graph: &ZoneGraph) -> Vec<dmft_common::ipc::ZoneGraphEntry> {
    let mut result: Vec<_> = graph
        .zones
        .values()
        .map(|node| {
            let conns: Vec<(u16, u8, bool)> = node
                .connections
                .iter()
                .map(|c| (c.dest_zone_id, c.transfer_type, c.disabled))
                .collect();
            (
                node.zone_id,
                node.name.clone(),
                node.min_level,
                node.max_level,
                conns,
            )
        })
        .collect();
    result.sort_by_key(|(id, _, _, _, _)| *id);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use dmft_common::nav::{ZoneConnection, ZoneNode};

    #[test]
    fn zone_graph_to_ipc_sorts_by_id() {
        let mut graph = ZoneGraph::default();
        graph.zones.insert(
            3,
            ZoneNode {
                zone_id: 3,
                name: "Zone3".into(),
                min_level: 1,
                max_level: 50,
                connections: vec![],
            },
        );
        graph.zones.insert(
            1,
            ZoneNode {
                zone_id: 1,
                name: "Zone1".into(),
                min_level: 1,
                max_level: 10,
                connections: vec![ZoneConnection {
                    dest_zone_id: 3,
                    transfer_type: 0,
                    disabled: false,
                }],
            },
        );

        let ipc = zone_graph_to_ipc(&graph);
        assert_eq!(ipc.len(), 2);
        assert_eq!(ipc[0].0, 1);
        assert_eq!(ipc[1].0, 3);
        assert_eq!(ipc[0].4.len(), 1);
        assert_eq!(ipc[0].4[0], (3, 0, false));
    }

    #[test]
    fn read_zone_graph_returns_none_on_non_windows() {
        assert!(unsafe { read_zone_graph(0) }.is_none());
    }
}
