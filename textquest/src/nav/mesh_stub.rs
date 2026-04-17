//! Non-Windows navmesh stubs.
//!
//! The live navmesh pipeline depends on Recast/Detour bindings that are only
//! built for Windows operators today. Linux/macOS development and CI still
//! need the crate to compile, so this module preserves the public API surface
//! while reporting that Detour-backed pathfinding is unavailable.

use anyhow::{Context, Result, bail};
use flate2::read::ZlibDecoder;
use prost::Message;
use std::{
    io::Read,
    path::{Path, PathBuf},
};
use textquest_common::nav::{NavPathFailureKind, NavPathMetrics, Waypoint};

const NAVMESH_FILE_MAGIC: u32 = u32::from_le_bytes([b'T', b'E', b'S', b'M']);
const FLAG_COMPRESSED: u16 = 0x0001;
const MESH_CACHE_DIR: &str = "data/meshes";
const MAX_MESH_DOWNLOAD_BYTES: usize = 64 * 1024 * 1024;
const MAX_PROTO_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
const NON_WINDOWS_ERROR: &str = "Navmesh pathfinding requires a Windows build with Recast support";

#[derive(Clone, PartialEq, Message)]
pub struct ProtoVector3 {
    #[prost(float, tag = "1")]
    pub x: f32,
    #[prost(float, tag = "2")]
    pub y: f32,
    #[prost(float, tag = "3")]
    pub z: f32,
}

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

#[derive(Clone, PartialEq, Message)]
pub struct ProtoNavMeshTile {
    #[prost(uint64, tag = "1")]
    pub tile_ref: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub tile_data: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ProtoNavMeshTileSet {
    #[prost(int32, tag = "1")]
    pub compatibility_version: i32,
    #[prost(message, optional, tag = "2")]
    pub mesh_params: Option<ProtoDtNavMeshParams>,
    #[prost(message, repeated, tag = "3")]
    pub tiles: Vec<ProtoNavMeshTile>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ProtoConnection {
    #[prost(uint32, tag = "1")]
    pub id: u32,
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(uint32, tag = "3")]
    pub connection_type: u32,
    #[prost(message, optional, tag = "4")]
    pub pos_from: Option<ProtoVector3>,
    #[prost(message, optional, tag = "5")]
    pub pos_to: Option<ProtoVector3>,
    #[prost(uint32, tag = "6")]
    pub area_type: u32,
    #[prost(bool, tag = "7")]
    pub one_way: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct ProtoNavMeshFile {
    #[prost(string, tag = "1")]
    pub zone_short_name: String,
    #[prost(message, optional, tag = "2")]
    pub tile_set: Option<ProtoNavMeshTileSet>,
    #[prost(message, repeated, tag = "6")]
    pub connections: Vec<ProtoConnection>,
}

#[derive(Debug, Clone)]
pub struct LoadedNavMesh {
    pub zone_short_name: String,
}

type EqPoint = (f32, f32, f32);

#[derive(Debug, Clone, Copy)]
pub struct NavMeshSegment {
    pub x1: f32,
    pub y1: f32,
    pub z1: f32,
    pub x2: f32,
    pub y2: f32,
    pub z2: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct NavMeshOverlayBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
}

impl NavMeshOverlayBounds {
    fn empty() -> Self {
        Self {
            min_x: f32::MAX,
            max_x: f32::MIN,
            min_y: f32::MAX,
            max_y: f32::MIN,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.min_x == f32::MAX
    }

    pub fn max_dimension(&self) -> f32 {
        if self.is_empty() {
            1.0
        } else {
            (self.max_x - self.min_x)
                .max(self.max_y - self.min_y)
                .max(1.0)
        }
    }
}

#[derive(Debug, Clone)]
pub struct NavMeshOverlay {
    pub outer_lines: Vec<NavMeshSegment>,
    pub inner_lines: Vec<NavMeshSegment>,
    pub bounds: NavMeshOverlayBounds,
}

impl NavMeshOverlay {
    pub fn is_empty(&self) -> bool {
        self.outer_lines.is_empty() && self.inner_lines.is_empty()
    }

    pub fn segment_count(&self) -> usize {
        self.outer_lines.len() + self.inner_lines.len()
    }
}

#[derive(Debug, Clone)]
pub struct ZoneMeshDiagnostics {
    pub zone_short_name: String,
    pub cache_path: PathBuf,
    pub cache_exists: bool,
    pub cache_bytes: Option<u64>,
    pub load_error: Option<String>,
    pub overlay_segment_count: Option<usize>,
}

impl ZoneMeshDiagnostics {
    #[must_use]
    pub fn loadable(&self) -> bool {
        self.load_error.is_none()
    }
}

#[derive(Debug, Clone)]
pub struct ZoneMeshReload {
    pub zone_short_name: String,
    pub cache_path: PathBuf,
    pub replaced_cached_file: bool,
    pub cache_bytes: usize,
    pub overlay_segment_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteSource {
    NavMesh,
    StraightLineFallback,
    NavMeshBlocked,
}

#[derive(Debug, Clone)]
pub struct RoutePlan {
    pub waypoints: Vec<Waypoint>,
    pub source: RouteSource,
    pub mesh_cached: bool,
    pub metrics: NavPathMetrics,
}

pub fn download_zone_mesh(zone_short_name: &str) -> Result<Vec<u8>> {
    let zone_short_name = sanitize_zone_short_name(zone_short_name)?;
    let cache_path = mesh_cache_path(zone_short_name);

    if cache_path.exists() {
        let cached_len = std::fs::metadata(&cache_path)
            .with_context(|| format!("Failed to stat cached mesh: {}", cache_path.display()))?
            .len() as usize;
        if cached_len > MAX_MESH_DOWNLOAD_BYTES {
            bail!(
                "Cached navmesh too large ({} bytes, max {})",
                cached_len,
                MAX_MESH_DOWNLOAD_BYTES
            );
        }
        return std::fs::read(&cache_path)
            .with_context(|| format!("Failed to read cached mesh: {}", cache_path.display()));
    }

    let url = format!("https://mqmesh.com/resources/meshes/{zone_short_name}.navmesh");
    let response = reqwest::blocking::get(&url)
        .with_context(|| format!("HTTP request failed for {url}"))?
        .error_for_status()
        .with_context(|| format!("Server returned error for {url}"))?;
    if let Some(content_len) = response.content_length()
        && content_len as usize > MAX_MESH_DOWNLOAD_BYTES
    {
        bail!(
            "Navmesh download too large ({} bytes, max {})",
            content_len,
            MAX_MESH_DOWNLOAD_BYTES
        );
    }

    let mut data = Vec::new();
    response
        .take((MAX_MESH_DOWNLOAD_BYTES + 1) as u64)
        .read_to_end(&mut data)
        .context("Failed to read response body")?;
    if data.len() > MAX_MESH_DOWNLOAD_BYTES {
        bail!(
            "Navmesh download exceeded size limit (max {} bytes)",
            MAX_MESH_DOWNLOAD_BYTES
        );
    }

    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&cache_path, &data)
        .with_context(|| format!("Failed to cache mesh to {}", cache_path.display()))?;

    Ok(data)
}

pub fn parse_navmesh(data: &[u8]) -> Result<ProtoNavMeshFile> {
    if data.len() < 8 {
        bail!("Navmesh file too small ({} bytes)", data.len());
    }

    let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    if magic != NAVMESH_FILE_MAGIC {
        bail!(
            "Invalid navmesh magic: expected 0x{NAVMESH_FILE_MAGIC:08X} ('MSET'), got \
             0x{magic:08X}"
        );
    }

    let version = u16::from_le_bytes([data[4], data[5]]);
    let flags = u16::from_le_bytes([data[6], data[7]]);
    let compressed = (flags & FLAG_COMPRESSED) != 0;

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
        let decoder = ZlibDecoder::new(payload);
        let mut decompressed = Vec::new();
        decoder
            .take((MAX_PROTO_PAYLOAD_BYTES + 1) as u64)
            .read_to_end(&mut decompressed)
            .context("Zlib decompression failed")?;
        if decompressed.len() > MAX_PROTO_PAYLOAD_BYTES {
            bail!(
                "Decompressed navmesh payload too large (max {} bytes)",
                MAX_PROTO_PAYLOAD_BYTES
            );
        }
        decompressed
    } else {
        if payload.len() > MAX_PROTO_PAYLOAD_BYTES {
            bail!(
                "Navmesh payload too large ({} bytes, max {})",
                payload.len(),
                MAX_PROTO_PAYLOAD_BYTES
            );
        }
        payload.to_vec()
    };

    ProtoNavMeshFile::decode(proto_bytes.as_slice()).context("Protobuf decode failed")
}

pub fn load_navmesh(proto: &ProtoNavMeshFile) -> Result<LoadedNavMesh> {
    let zone_short_name = proto.zone_short_name.trim();
    if zone_short_name.is_empty() {
        bail!("NavMeshFile missing zone_short_name");
    }
    bail!("{NON_WINDOWS_ERROR}")
}

pub fn find_path(_loaded: &LoadedNavMesh, _from: EqPoint, _to: EqPoint) -> Result<Vec<EqPoint>> {
    bail!("{NON_WINDOWS_ERROR}")
}

pub fn load_zone(zone_short_name: &str) -> Result<LoadedNavMesh> {
    let data = download_zone_mesh(zone_short_name)?;
    let proto = parse_navmesh(&data)?;
    load_navmesh(&proto)
}

pub fn load_zone_overlay(_zone_short_name: &str) -> Result<NavMeshOverlay> {
    bail!("{NON_WINDOWS_ERROR}")
}

pub fn has_cached_zone_mesh(zone_short_name: &str) -> bool {
    sanitize_zone_short_name(zone_short_name)
        .map(|zone| mesh_cache_path(zone).exists())
        .unwrap_or(false)
}

pub fn cached_zone_mesh_diagnostics(zone_short_name: &str) -> Result<ZoneMeshDiagnostics> {
    let zone_short_name = sanitize_zone_short_name(zone_short_name)?;
    let cache_path = mesh_cache_path(zone_short_name);
    let cache_exists = cache_path.exists();
    let cache_bytes = if cache_exists {
        Some(
            std::fs::metadata(&cache_path)
                .with_context(|| format!("Failed to stat cached mesh: {}", cache_path.display()))?
                .len(),
        )
    } else {
        None
    };

    let load_error = if cache_exists {
        Some(String::from(NON_WINDOWS_ERROR))
    } else {
        Some(format!("No cached navmesh for zone '{zone_short_name}'"))
    };

    Ok(ZoneMeshDiagnostics {
        zone_short_name: zone_short_name.to_string(),
        cache_path,
        cache_exists,
        cache_bytes,
        load_error,
        overlay_segment_count: None,
    })
}

pub fn reload_zone_mesh(zone_short_name: &str) -> Result<ZoneMeshReload> {
    let zone_short_name = sanitize_zone_short_name(zone_short_name)?;
    let cache_path = mesh_cache_path(zone_short_name);
    let replaced_cached_file = cache_path.exists();
    if replaced_cached_file {
        std::fs::remove_file(&cache_path)
            .with_context(|| format!("Failed to remove cached mesh: {}", cache_path.display()))?;
    }

    let data = download_zone_mesh(zone_short_name)?;
    let _ = parse_navmesh(&data)?;
    bail!("{NON_WINDOWS_ERROR}")
}

pub fn plan_route(zone_short_name: &str, from: EqPoint, to: EqPoint) -> RoutePlan {
    let mesh_cached = has_cached_zone_mesh(zone_short_name);
    let origin = Waypoint::new(from.0, from.1, from.2);
    let destination = Waypoint::new(to.0, to.1, to.2);
    let fallback_length = path_length(&[origin, destination]);
    let failure_reason = String::from(NON_WINDOWS_ERROR);

    RoutePlan {
        waypoints: vec![destination],
        source: RouteSource::StraightLineFallback,
        mesh_cached,
        metrics: NavPathMetrics::failure(
            failure_reason,
            NavPathFailureKind::DataGap,
            fallback_length,
            false,
        ),
    }
}

fn path_length(waypoints: &[Waypoint]) -> Option<f32> {
    if waypoints.is_empty() {
        return None;
    }

    let mut total = 0.0f32;
    for window in waypoints.windows(2) {
        total += window[0].distance_3d(&window[1]);
    }
    Some(total)
}

fn mesh_cache_path(zone_short_name: &str) -> PathBuf {
    Path::new(MESH_CACHE_DIR).join(format!("{zone_short_name}.navmesh"))
}

fn sanitize_zone_short_name(zone_short_name: &str) -> Result<&str> {
    if zone_short_name.is_empty() {
        bail!("Zone short name is empty");
    }

    if !zone_short_name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        bail!("Invalid zone short name: {zone_short_name:?}");
    }

    Ok(zone_short_name)
}
