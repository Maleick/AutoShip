//! Read zone adjacency graph from EQ's ZoneGuideManagerClient.
//!
//! ZoneGuideManagerClient is a singleton containing a fixed-size array of 888
//! ZoneGuideZone entries. Each zone has a name, level range, and an ArrayClass
//! of ZoneGuideConnection entries describing how to reach neighboring zones.

use dmft_common::nav::ZoneGraph;

/// Read the complete zone graph from memory.
///
/// # Safety
/// Must be called from the EQ game process (injected DLL context) while the
/// game is running and ZoneGuideManagerClient is initialized.
#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn read_zone_graph(eq_base: u64) -> Option<ZoneGraph> {
    use crate::eq::widgets::read_cxstr;
    use dmft_common::nav::{ZoneConnection, ZoneNode};
    use dmft_common::offsets::{self, zone_guide as zg};

    // SAFETY: All pointer reads in this function follow EQ's ZoneGuideManagerClient
    // struct layout. mgr_ptr_addr is rebased from ZONE_GUIDE_MANAGER — a known
    // global in eqgame.exe. Each subsequent dereference follows known offsets
    // (zone array, connections array) with null/range checks. The function is
    // called from the game loop thread where the zone guide data is stable.
    // If any pointer is invalid, we return None rather than crashing.

    // Resolve the singleton pointer
    let mgr_ptr_addr = offsets::rebase(offsets::ZONE_GUIDE_MANAGER, eq_base)?;
    let mgr_ptr = *(mgr_ptr_addr as *const usize);
    if mgr_ptr == 0 || mgr_ptr < 0x10000 {
        tracing::warn!("ZoneGuideManagerClient pointer is null");
        return None;
    }

    // Check if zone guide data is populated
    let data_set = *((mgr_ptr + zg::DATA_SET) as *const u8);
    if data_set == 0 {
        tracing::warn!("ZoneGuideManagerClient.zoneGuideDataSet is false");
        return None;
    }

    let zones_base = mgr_ptr + zg::ZONES_OFFSET;
    let mut graph = ZoneGraph::default();

    for i in 0..zg::ZONE_COUNT {
        let zone_addr = zones_base + i * zg::ZONE_SIZE;

        let zone_id = *((zone_addr + zg::ZONE_ID) as *const i32);
        if zone_id <= 0 || zone_id > zg::ZONE_COUNT as i32 {
            continue;
        }

        let name = read_cxstr(zone_addr + zg::ZONE_NAME).unwrap_or_default();
        if name.is_empty() {
            continue;
        }

        let min_level = *((zone_addr + zg::ZONE_MIN_LEVEL) as *const i32);
        let max_level = *((zone_addr + zg::ZONE_MAX_LEVEL) as *const i32);

        // Read connections ArrayClass
        let conn_count = *((zone_addr + zg::ZONE_CONNECTIONS_COUNT) as *const i32);
        let conn_array = *((zone_addr + zg::ZONE_CONNECTIONS_ARRAY) as *const usize);

        let mut connections = Vec::new();
        if conn_count > 0 && conn_count < 200 && conn_array != 0 && conn_array > 0x10000 {
            for j in 0..conn_count as usize {
                let conn_addr = conn_array + j * zg::CONNECTION_SIZE;

                let dest_zone_id = *((conn_addr + zg::CONN_DEST_ZONE_ID) as *const i32);
                let transfer_type = *((conn_addr + zg::CONN_TRANSFER_TYPE) as *const i32);
                let disabled = *((conn_addr + zg::CONN_DISABLED) as *const u8) != 0;

                if dest_zone_id > 0 && dest_zone_id <= zg::ZONE_COUNT as i32 {
                    connections.push(ZoneConnection {
                        dest_zone_id: dest_zone_id as u16,
                        transfer_type: transfer_type.clamp(0, 255) as u8,
                        disabled,
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

/// Convert a ZoneGraph into the simplified IPC wire format.
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
