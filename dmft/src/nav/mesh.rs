//! Navmesh loading pipeline — download from mqmesh.com, parse `MQ2Nav` binary format,
//! load into Detour for pathfinding.

use anyhow::{Context, Result, bail};
use dmft_common::nav::Waypoint;
use flate2::read::ZlibDecoder;
use prost::Message;
use std::io::Read;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Protobuf types (hand-written to match MQ2Nav's NavMeshFile.proto)
// ---------------------------------------------------------------------------

/// nav.vector3
#[derive(Clone, PartialEq, Message)]
pub struct ProtoVector3 {
    #[prost(float, tag = "1")]
    pub x: f32,
    #[prost(float, tag = "2")]
    pub y: f32,
    #[prost(float, tag = "3")]
    pub z: f32,
}

/// nav.dtNavMeshParams
#[derive(Clone, PartialEq, Message)]
pub struct ProtoDtNavMeshParams {
    #[prost(message, optional, tag = "1")]
    pub origin: Option<ProtoVector3>,
    #[prost(float, tag = "2")]
    pub tile_width: f32,
    #[prost(float, tag = "3")]
    pub tile_height: f32,
    #[prost(int32, tag = "4")]
    pub max_tiles: i32,
    #[prost(int32, tag = "5")]
    pub max_polys: i32,
}

/// nav.NavMeshTile
#[derive(Clone, PartialEq, Message)]
pub struct ProtoNavMeshTile {
    #[prost(uint64, tag = "1")]
    pub tile_ref: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub tile_data: Vec<u8>,
}

/// nav.NavMeshTileSet
#[derive(Clone, PartialEq, Message)]
pub struct ProtoNavMeshTileSet {
    #[prost(int32, tag = "1")]
    pub compatibility_version: i32,
    #[prost(message, optional, tag = "2")]
    pub mesh_params: Option<ProtoDtNavMeshParams>,
    #[prost(message, repeated, tag = "3")]
    pub tiles: Vec<ProtoNavMeshTile>,
}

/// nav.NavMeshFile — top-level container.
/// Fields 3-6 (`build_settings`, `convex_volumes`, areas, connections) exist in the
/// proto but are not needed for pathfinding — prost silently skips unknown fields.
#[derive(Clone, PartialEq, Message)]
pub struct ProtoNavMeshFile {
    #[prost(string, tag = "1")]
    pub zone_short_name: String,
    #[prost(message, optional, tag = "2")]
    pub tile_set: Option<ProtoNavMeshTileSet>,
}

// ---------------------------------------------------------------------------
// Binary file header (matches MQ2Nav's NavMeshData.h)
// ---------------------------------------------------------------------------

/// Magic bytes: file starts with 'TESM' (ASCII), which is 'MSET' as a MSVC multi-char literal.
/// As a little-endian u32, the bytes [T, E, S, M] = 0x4D534554.
const NAVMESH_FILE_MAGIC: u32 = u32::from_le_bytes([b'T', b'E', b'S', b'M']);
const FLAG_COMPRESSED: u16 = 0x0001;
const NAVMESH_QUERY_MAX_NODES: i32 = 16384;
const MESH_CACHE_DIR: &str = "data/meshes";

// ---------------------------------------------------------------------------
// FFI — recastnavigation-sys types + our C++ shim
// ---------------------------------------------------------------------------

// Our C++ shim functions (compiled by build.rs) for dtNavMeshQuery methods
// that bindgen in recastnavigation-sys doesn't generate as free functions.
#[allow(non_camel_case_types)]
unsafe extern "C" {
    fn shim_dtNavMeshQuery_init(
        query: *mut recastnavigation_sys::dtNavMeshQuery,
        nav: *const recastnavigation_sys::dtNavMesh,
        max_nodes: i32,
    ) -> recastnavigation_sys::dtStatus;

    fn shim_dtNavMeshQuery_findNearestPoly(
        query: *const recastnavigation_sys::dtNavMeshQuery,
        center: *const f32,
        half_extents: *const f32,
        filter: *const recastnavigation_sys::dtQueryFilter,
        nearest_ref: *mut recastnavigation_sys::dtPolyRef,
        nearest_pt: *mut f32,
    ) -> recastnavigation_sys::dtStatus;

    fn shim_dtNavMeshQuery_findPath(
        query: *const recastnavigation_sys::dtNavMeshQuery,
        start_ref: recastnavigation_sys::dtPolyRef,
        end_ref: recastnavigation_sys::dtPolyRef,
        start_pos: *const f32,
        end_pos: *const f32,
        filter: *const recastnavigation_sys::dtQueryFilter,
        path: *mut recastnavigation_sys::dtPolyRef,
        path_count: *mut i32,
        max_path: i32,
    ) -> recastnavigation_sys::dtStatus;

    fn shim_dtNavMeshQuery_findStraightPath(
        query: *const recastnavigation_sys::dtNavMeshQuery,
        start_pos: *const f32,
        end_pos: *const f32,
        path: *const recastnavigation_sys::dtPolyRef,
        path_size: i32,
        straight_path: *mut f32,
        straight_path_flags: *mut u8,
        straight_path_refs: *mut recastnavigation_sys::dtPolyRef,
        straight_path_count: *mut i32,
        max_straight_path: i32,
        options: i32,
    ) -> recastnavigation_sys::dtStatus;
}

// ---------------------------------------------------------------------------
// Safe Detour wrappers
// ---------------------------------------------------------------------------

/// `DT_TILE_FREE_DATA` flag — tells Detour to free tile data when removing.
const DT_TILE_FREE_DATA: i32 = 0x01;

/// Check if a Detour status indicates success.
fn dt_success(status: u32) -> bool {
    (status & 0x4000_0000) != 0
}

/// Owned Detour `NavMesh`.
struct DetourNavMesh {
    ptr: *mut recastnavigation_sys::dtNavMesh,
}

impl DetourNavMesh {
    fn new(params: &recastnavigation_sys::dtNavMeshParams) -> Result<Self> {
        unsafe {
            let ptr = recastnavigation_sys::dtAllocNavMesh();
            if ptr.is_null() {
                bail!("dtAllocNavMesh returned null");
            }
            let status = recastnavigation_sys::dtNavMesh_init(ptr, params);
            if !dt_success(status) {
                recastnavigation_sys::dtFreeNavMesh(ptr);
                bail!("dtNavMesh_init failed: status=0x{status:08X}");
            }
            Ok(Self { ptr })
        }
    }

    /// Add a tile. Takes ownership of `data` — Detour will free it.
    fn add_tile(&mut self, mut data: Vec<u8>) -> Result<u64> {
        unsafe {
            let data_ptr = data.as_mut_ptr();
            let data_len = data.len() as i32;
            let mut tile_ref: u64 = 0;
            let status = recastnavigation_sys::dtNavMesh_addTile(
                self.ptr,
                data_ptr,
                data_len,
                DT_TILE_FREE_DATA,
                0, // lastRef = 0 means auto-assign
                &mut tile_ref,
            );
            if !dt_success(status) {
                // Data was not consumed — drop it normally.
                bail!("dtNavMesh_addTile failed: status=0x{status:08X}");
            }
            // Detour now owns this allocation. Prevent Rust from dropping it.
            std::mem::forget(data);
            Ok(tile_ref)
        }
    }
}

impl Drop for DetourNavMesh {
    fn drop(&mut self) {
        unsafe {
            recastnavigation_sys::dtFreeNavMesh(self.ptr);
        }
    }
}

/// Owned Detour `NavMeshQuery`.
struct DetourNavMeshQuery {
    ptr: *mut recastnavigation_sys::dtNavMeshQuery,
}

impl DetourNavMeshQuery {
    fn new(nav: &DetourNavMesh, max_nodes: i32) -> Result<Self> {
        unsafe {
            let ptr = recastnavigation_sys::dtAllocNavMeshQuery();
            if ptr.is_null() {
                bail!("dtAllocNavMeshQuery returned null");
            }
            let status = shim_dtNavMeshQuery_init(ptr, nav.ptr, max_nodes);
            if !dt_success(status) {
                recastnavigation_sys::dtFreeNavMeshQuery(ptr);
                bail!("dtNavMeshQuery init failed: status=0x{status:08X}");
            }
            Ok(Self { ptr })
        }
    }

    fn find_nearest_poly(
        &self,
        center: &[f32; 3],
        half_extents: &[f32; 3],
        filter: &recastnavigation_sys::dtQueryFilter,
    ) -> Result<(u64, [f32; 3])> {
        unsafe {
            let mut poly_ref: u64 = 0;
            let mut nearest_pt = [0.0f32; 3];
            let status = shim_dtNavMeshQuery_findNearestPoly(
                self.ptr,
                center.as_ptr(),
                half_extents.as_ptr(),
                filter,
                &mut poly_ref,
                nearest_pt.as_mut_ptr(),
            );
            if !dt_success(status) {
                bail!("findNearestPoly failed: status=0x{status:08X}");
            }
            Ok((poly_ref, nearest_pt))
        }
    }

    fn find_path(
        &self,
        start_ref: u64,
        end_ref: u64,
        start_pos: &[f32; 3],
        end_pos: &[f32; 3],
        filter: &recastnavigation_sys::dtQueryFilter,
        max_path: i32,
    ) -> Result<Vec<u64>> {
        unsafe {
            let mut path = vec![0u64; max_path as usize];
            let mut path_count: i32 = 0;
            let status = shim_dtNavMeshQuery_findPath(
                self.ptr,
                start_ref,
                end_ref,
                start_pos.as_ptr(),
                end_pos.as_ptr(),
                filter,
                path.as_mut_ptr(),
                &mut path_count,
                max_path,
            );
            if !dt_success(status) {
                bail!("findPath failed: status=0x{status:08X}");
            }
            path.truncate(path_count as usize);
            Ok(path)
        }
    }

    fn find_straight_path(
        &self,
        start_pos: &[f32; 3],
        end_pos: &[f32; 3],
        poly_path: &[u64],
        max_straight_path: i32,
    ) -> Result<Vec<[f32; 3]>> {
        unsafe {
            let mut straight_path = vec![0.0f32; max_straight_path as usize * 3];
            let mut straight_flags = vec![0u8; max_straight_path as usize];
            let mut straight_refs = vec![0u64; max_straight_path as usize];
            let mut straight_count: i32 = 0;
            let status = shim_dtNavMeshQuery_findStraightPath(
                self.ptr,
                start_pos.as_ptr(),
                end_pos.as_ptr(),
                poly_path.as_ptr(),
                poly_path.len() as i32,
                straight_path.as_mut_ptr(),
                straight_flags.as_mut_ptr(),
                straight_refs.as_mut_ptr(),
                &mut straight_count,
                max_straight_path,
                0, // no special options
            );
            if !dt_success(status) {
                bail!("findStraightPath failed: status=0x{status:08X}");
            }
            let result: Vec<[f32; 3]> = (0..straight_count as usize)
                .map(|i| {
                    [
                        straight_path[i * 3],
                        straight_path[i * 3 + 1],
                        straight_path[i * 3 + 2],
                    ]
                })
                .collect();
            Ok(result)
        }
    }
}

impl Drop for DetourNavMeshQuery {
    fn drop(&mut self) {
        unsafe {
            recastnavigation_sys::dtFreeNavMeshQuery(self.ptr);
        }
    }
}

/// Create a default dtQueryFilter with standard walk flags.
/// Matches Detour's default constructor: includeFlags=0xFFFF, excludeFlags=0, area costs=1.0.
fn default_query_filter() -> recastnavigation_sys::dtQueryFilter {
    let mut filter = unsafe { std::mem::zeroed::<recastnavigation_sys::dtQueryFilter>() };
    filter.m_includeFlags = 0xFFFF;
    filter.m_excludeFlags = 0;
    // Set all area costs to 1.0 (default)
    for cost in filter.m_areaCost.iter_mut() {
        *cost = 1.0;
    }
    filter
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Download a zone's navmesh file from mqmesh.com, caching to disk.
pub fn download_zone_mesh(zone_short_name: &str) -> Result<Vec<u8>> {
    let cache_path = mesh_cache_path(zone_short_name);

    if cache_path.exists() {
        tracing::info!(zone = zone_short_name, path = %cache_path.display(), "Loading cached navmesh");
        return std::fs::read(&cache_path)
            .with_context(|| format!("Failed to read cached mesh: {}", cache_path.display()));
    }

    let url = format!(
        "https://mqmesh.com/resources/meshes/{zone_short_name}.navmesh"
    );
    tracing::info!(zone = zone_short_name, %url, "Downloading navmesh");

    let data = reqwest::blocking::get(&url)
        .with_context(|| format!("HTTP request failed for {url}"))?
        .error_for_status()
        .with_context(|| format!("Server returned error for {url}"))?
        .bytes()
        .context("Failed to read response body")?
        .to_vec();

    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&cache_path, &data)
        .with_context(|| format!("Failed to cache mesh to {}", cache_path.display()))?;
    tracing::info!(
        zone = zone_short_name,
        bytes = data.len(),
        "Cached navmesh to disk"
    );

    Ok(data)
}

/// Parse the raw .navmesh file bytes: validate header, decompress if needed,
/// decode protobuf payload.
pub fn parse_navmesh(data: &[u8]) -> Result<ProtoNavMeshFile> {
    if data.len() < 8 {
        bail!("Navmesh file too small ({} bytes)", data.len());
    }

    let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    if magic != NAVMESH_FILE_MAGIC {
        bail!(
            "Invalid navmesh magic: expected 0x{NAVMESH_FILE_MAGIC:08X} ('MSET'), got 0x{magic:08X}"
        );
    }

    let version = u16::from_le_bytes([data[4], data[5]]);
    let flags = u16::from_le_bytes([data[6], data[7]]);
    let compressed = (flags & FLAG_COMPRESSED) != 0;

    tracing::debug!(
        version,
        compressed,
        total_bytes = data.len(),
        "Parsing navmesh header"
    );

    let payload_offset = if version >= 5 {
        if data.len() < 16 {
            bail!("V5 header truncated");
        }
        let header_size = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;
        if header_size > 0 { header_size } else { 16 }
    } else {
        8
    };

    if payload_offset > data.len() {
        bail!(
            "Header offset ({payload_offset}) exceeds file size ({})",
            data.len()
        );
    }

    let payload = &data[payload_offset..];

    let proto_bytes = if compressed {
        let mut decoder = ZlibDecoder::new(payload);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .context("Zlib decompression failed")?;
        tracing::debug!(
            compressed_bytes = payload.len(),
            decompressed_bytes = decompressed.len(),
            "Decompressed navmesh payload"
        );
        decompressed
    } else {
        payload.to_vec()
    };

    ProtoNavMeshFile::decode(proto_bytes.as_slice()).context("Protobuf decode failed")
}

/// A loaded navmesh ready for pathfinding queries.
pub struct LoadedNavMesh {
    _nav_mesh: DetourNavMesh,
    query: DetourNavMeshQuery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteSource {
    NavMesh,
    StraightLineFallback,
}

#[derive(Debug, Clone)]
pub struct RoutePlan {
    pub waypoints: Vec<Waypoint>,
    pub source: RouteSource,
    pub mesh_cached: bool,
}

/// Load a parsed navmesh into Detour, returning a query-ready object.
pub fn load_navmesh(proto: &ProtoNavMeshFile) -> Result<LoadedNavMesh> {
    let tile_set = proto
        .tile_set
        .as_ref()
        .context("NavMeshFile missing tile_set")?;
    let params_proto = tile_set
        .mesh_params
        .as_ref()
        .context("NavMeshTileSet missing mesh_params")?;

    let origin = params_proto
        .origin
        .as_ref()
        .map_or([0.0; 3], |o| [o.x, o.y, o.z]);

    let params = recastnavigation_sys::dtNavMeshParams {
        orig: origin,
        tileWidth: params_proto.tile_width,
        tileHeight: params_proto.tile_height,
        maxTiles: params_proto.max_tiles,
        maxPolys: params_proto.max_polys,
    };

    let mut nav_mesh = DetourNavMesh::new(&params)?;

    let mut loaded_tiles = 0u32;
    for tile in &tile_set.tiles {
        if tile.tile_data.is_empty() {
            continue;
        }
        nav_mesh.add_tile(tile.tile_data.clone())?;
        loaded_tiles += 1;
    }

    tracing::info!(
        zone = %proto.zone_short_name,
        tiles = loaded_tiles,
        "Loaded navmesh into Detour"
    );

    let query = DetourNavMeshQuery::new(&nav_mesh, NAVMESH_QUERY_MAX_NODES)?;

    Ok(LoadedNavMesh {
        _nav_mesh: nav_mesh,
        query,
    })
}

/// Convert EQ coordinates (x, y, z where Z=up) to Detour coordinates (x, z, y where Y=up).
/// `MQ2Nav` stores meshes in Detour's native coordinate space: (`eq_x`, `eq_z`, `eq_y`).
fn eq_to_detour(eq_x: f32, eq_y: f32, eq_z: f32) -> [f32; 3] {
    [eq_x, eq_z, eq_y]
}

/// Convert Detour coordinates back to EQ coordinates.
fn detour_to_eq(d: &[f32; 3]) -> (f32, f32, f32) {
    (d[0], d[2], d[1])
}

/// Find a path between two EQ positions using a loaded navmesh.
/// Positions are in EQ coordinate space (x=east/west, y=north/south, z=up).
/// Returns a list of EQ waypoint positions (x, y, z).
pub fn find_path(
    loaded: &LoadedNavMesh,
    from: (f32, f32, f32),
    to: (f32, f32, f32),
) -> Result<Vec<(f32, f32, f32)>> {
    let filter = default_query_filter();
    // Search extents: how far from the given position to look for a polygon.
    // Y (Detour up-axis) needs a large extent to handle multi-level dungeons.
    let extents = [50.0f32, 200.0, 50.0];

    let start_pos = eq_to_detour(from.0, from.1, from.2);
    let end_pos = eq_to_detour(to.0, to.1, to.2);

    let (start_ref, start_nearest) = loaded
        .query
        .find_nearest_poly(&start_pos, &extents, &filter)?;
    let (end_ref, end_nearest) = loaded
        .query
        .find_nearest_poly(&end_pos, &extents, &filter)?;

    if start_ref == 0 {
        bail!(
            "Start position ({}, {}, {}) not on navmesh",
            from.0,
            from.1,
            from.2
        );
    }
    if end_ref == 0 {
        bail!("End position ({}, {}, {}) not on navmesh", to.0, to.1, to.2);
    }

    let poly_path = loaded.query.find_path(
        start_ref,
        end_ref,
        &start_nearest,
        &end_nearest,
        &filter,
        2048,
    )?;

    if poly_path.is_empty() {
        bail!("No path found between start and end");
    }

    let straight =
        loaded
            .query
            .find_straight_path(&start_nearest, &end_nearest, &poly_path, 2048)?;

    let waypoints: Vec<(f32, f32, f32)> = straight.iter().map(detour_to_eq).collect();

    Ok(waypoints)
}

/// End-to-end convenience: download (or load from cache), parse, and load a zone mesh.
pub fn load_zone(zone_short_name: &str) -> Result<LoadedNavMesh> {
    let data = download_zone_mesh(zone_short_name)?;
    let proto = parse_navmesh(&data)?;
    load_navmesh(&proto)
}

pub fn has_cached_zone_mesh(zone_short_name: &str) -> bool {
    mesh_cache_path(zone_short_name).exists()
}

pub fn plan_route(zone_short_name: &str, from: (f32, f32, f32), to: (f32, f32, f32)) -> RoutePlan {
    let mesh_cached = has_cached_zone_mesh(zone_short_name);

    match load_zone(zone_short_name) {
        Ok(loaded) => match find_path(&loaded, from, to) {
            Ok(path) => RoutePlan {
                waypoints: path
                    .into_iter()
                    .map(|(x, y, z)| Waypoint::new(x, y, z))
                    .collect(),
                source: RouteSource::NavMesh,
                mesh_cached,
            },
            Err(error) => {
                tracing::warn!(
                    zone = zone_short_name,
                    %error,
                    "Navmesh path query failed; falling back to straight-line route"
                );
                RoutePlan {
                    waypoints: vec![Waypoint::new(to.0, to.1, to.2)],
                    source: RouteSource::StraightLineFallback,
                    mesh_cached,
                }
            }
        },
        Err(error) => {
            tracing::warn!(
                zone = zone_short_name,
                %error,
                "Navmesh load failed; falling back to straight-line route"
            );
            RoutePlan {
                waypoints: vec![Waypoint::new(to.0, to.1, to.2)],
                source: RouteSource::StraightLineFallback,
                mesh_cached,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn mesh_cache_path(zone_short_name: &str) -> PathBuf {
    Path::new(MESH_CACHE_DIR).join(format!("{zone_short_name}.navmesh"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_constant_matches_mset() {
        // 'TESM' as bytes => [0x54, 0x45, 0x53, 0x4D] => LE u32 = 0x4D534554
        assert_eq!(NAVMESH_FILE_MAGIC, 0x4D534554);
    }

    #[test]
    fn parse_rejects_too_small() {
        let result = parse_navmesh(&[0; 4]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_rejects_bad_magic() {
        let mut data = vec![0u8; 64];
        data[0..4].copy_from_slice(&0xDEADBEEFu32.to_le_bytes());
        let result = parse_navmesh(&data);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Invalid navmesh magic"), "{msg}");
    }

    #[test]
    fn parse_valid_v4_uncompressed_empty_proto() {
        let mut data = Vec::new();
        data.extend_from_slice(&NAVMESH_FILE_MAGIC.to_le_bytes());
        data.extend_from_slice(&4u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());

        let result = parse_navmesh(&data);
        assert!(result.is_ok(), "{:?}", result.err());
        let file = result.unwrap();
        assert_eq!(file.zone_short_name, "");
        assert!(file.tile_set.is_none());
    }

    #[test]
    fn mesh_cache_path_format() {
        let p = mesh_cache_path("befallen");
        assert!(p.to_string_lossy().contains("befallen.navmesh"));
    }
}
