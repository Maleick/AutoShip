//! Packet capture types — opcode filtering, capture sessions, and disk
//! persistence.
//!
//! Provides infrastructure for capturing EQ network packets with:
//! - Direction-aware filtering (client→server, server→client)
//! - Opcode whitelist/blacklist filtering
//! - Ring-buffered capture sessions with timestamps
//! - Binary save/load for capture sessions (custom format)
//! - JSON lines export for human-readable analysis
//! - Session diffing to spot behavioral changes

use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    io::{self, Read, Write},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Direction of a captured packet relative to the EQ client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PacketDirection {
    /// Client → server (outbound).
    ClientToServer,
    /// Server → client (inbound).
    ServerToClient,
}

impl std::fmt::Display for PacketDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClientToServer => write!(f, "C→S"),
            Self::ServerToClient => write!(f, "S→C"),
        }
    }
}

/// A single captured packet with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedPacket {
    /// Milliseconds since capture session start.
    pub timestamp_ms: u64,
    /// Packet direction.
    pub direction: PacketDirection,
    /// EQ opcode (first 2 bytes of packet payload, little-endian).
    pub opcode: u16,
    /// Raw packet payload (includes opcode bytes).
    pub payload: Vec<u8>,
}

impl CapturedPacket {
    /// Payload size excluding the 2-byte opcode header.
    pub fn data_len(&self) -> usize {
        self.payload.len().saturating_sub(2)
    }

    /// Validate packet structure. Returns `Ok(())` if valid, `Err` if malformed.
    ///
    /// # Validation Rules
    /// 1. Payload must be at least 2 bytes (minimum opcode header)
    /// 2. Payload cannot exceed 65536 bytes (EQ protocol limit)
    /// 3. Opcode must match the first 2 bytes of payload (little-endian consistency)
    pub fn validate(&self) -> io::Result<()> {
        // Rule 1: Minimum payload size
        if self.payload.len() < 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "packet payload too small: {} bytes (minimum 2 required)",
                    self.payload.len()
                ),
            ));
        }

        // Rule 2: Maximum payload size (EQ packet limit)
        if self.payload.len() > 65536 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "packet payload exceeds 65536 byte limit: {} bytes",
                    self.payload.len()
                ),
            ));
        }

        // Rule 3: Opcode consistency check
        let encoded_opcode = u16::from_le_bytes([self.payload[0], self.payload[1]]);
        if encoded_opcode != self.opcode {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "opcode mismatch: stored 0x{:04X} vs payload 0x{:04X}",
                    self.opcode, encoded_opcode
                ),
            ));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Opcode filter
// ---------------------------------------------------------------------------

/// Filter mode for opcode-based packet filtering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterMode {
    /// Show all packets (no filtering).
    PassAll,
    /// Only show packets matching these opcodes.
    Whitelist(HashSet<u16>),
    /// Show all packets except those matching these opcodes.
    Blacklist(HashSet<u16>),
}

/// Configurable packet filter combining opcode and direction rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketFilter {
    /// Opcode filter mode.
    pub mode: FilterMode,
    /// If set, only show packets in this direction.
    pub direction: Option<PacketDirection>,
}

impl Default for PacketFilter {
    fn default() -> Self {
        Self {
            mode: FilterMode::PassAll,
            direction: None,
        }
    }
}

impl PacketFilter {
    /// Create a whitelist filter for specific opcodes.
    pub fn whitelist(opcodes: impl IntoIterator<Item = u16>) -> Self {
        Self {
            mode: FilterMode::Whitelist(opcodes.into_iter().collect()),
            direction: None,
        }
    }

    /// Create a blacklist filter to exclude specific opcodes.
    pub fn blacklist(opcodes: impl IntoIterator<Item = u16>) -> Self {
        Self {
            mode: FilterMode::Blacklist(opcodes.into_iter().collect()),
            direction: None,
        }
    }

    /// Restrict to a single direction.
    pub fn with_direction(mut self, dir: PacketDirection) -> Self {
        self.direction = Some(dir);
        self
    }

    /// Test whether a packet passes this filter.
    pub fn matches(&self, packet: &CapturedPacket) -> bool {
        if let Some(dir) = self.direction
            && packet.direction != dir
        {
            return false;
        }
        match &self.mode {
            FilterMode::PassAll => true,
            FilterMode::Whitelist(set) => set.contains(&packet.opcode),
            FilterMode::Blacklist(set) => !set.contains(&packet.opcode),
        }
    }
}

// ---------------------------------------------------------------------------
// Capture session
// ---------------------------------------------------------------------------

/// A bounded capture session that stores packets in a ring buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSession {
    /// Session label (e.g. "raid-pull-sequence").
    pub label: String,
    /// Unix timestamp (seconds) when capture started.
    pub started_at: u64,
    /// Maximum number of packets to retain (oldest evicted first).
    pub capacity: usize,
    /// Captured packets (bounded ring buffer via VecDeque).
    packets: std::collections::VecDeque<CapturedPacket>,
    /// Total packets seen (including evicted).
    pub total_seen: u64,
    /// Active filter applied during capture.
    pub filter: PacketFilter,
}

impl CaptureSession {
    /// Create a new capture session with the given capacity.
    pub fn new(label: impl Into<String>, capacity: usize) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            label: label.into(),
            started_at: now,
            capacity,
            packets: std::collections::VecDeque::with_capacity(capacity.min(8192)),
            total_seen: 0,
            filter: PacketFilter::default(),
        }
    }

    /// Set the active filter.
    pub fn with_filter(mut self, filter: PacketFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Record a packet. Returns `true` if packet passed validation and filter.
    /// Returns `false` if packet is malformed, oversized, truncated, or rejected by filter.
    pub fn record(&mut self, packet: CapturedPacket) -> bool {
        self.total_seen += 1;

        // Validate packet structure before filter processing
        if let Err(_) = packet.validate() {
            return false;
        }

        if !self.filter.matches(&packet) {
            return false;
        }
        if self.packets.len() >= self.capacity {
            self.packets.pop_front();
        }
        self.packets.push_back(packet);
        true
    }

    /// Number of packets currently stored.
    pub fn len(&self) -> usize {
        self.packets.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    /// Number of packets that were evicted (total_seen - currently stored).
    pub fn evicted(&self) -> u64 {
        self.total_seen.saturating_sub(self.packets.len() as u64)
    }

    /// Iterate over captured packets (oldest first).
    pub fn packets(&self) -> impl Iterator<Item = &CapturedPacket> {
        self.packets.iter()
    }

    /// Get a packet by index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<&CapturedPacket> {
        self.packets.get(index)
    }

    /// Clear all captured packets. Resets total_seen.
    pub fn clear(&mut self) {
        self.packets.clear();
        self.total_seen = 0;
    }

    /// Elapsed time from session start to the latest packet.
    pub fn duration(&self) -> Duration {
        self.packets
            .back()
            .map(|p| Duration::from_millis(p.timestamp_ms))
            .unwrap_or_default()
    }

    /// Unique opcodes seen in this session.
    pub fn unique_opcodes(&self) -> HashSet<u16> {
        self.packets.iter().map(|p| p.opcode).collect()
    }

    /// Count packets per opcode, sorted by count descending.
    pub fn opcode_histogram(&self) -> Vec<(u16, usize)> {
        let mut counts = std::collections::HashMap::<u16, usize>::new();
        for p in &self.packets {
            *counts.entry(p.opcode).or_default() += 1;
        }
        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
        sorted
    }
}

// ---------------------------------------------------------------------------
// Session diff
// ---------------------------------------------------------------------------

/// Differences between two capture sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDiff {
    /// Opcodes present in session A but not B.
    pub only_in_a: Vec<u16>,
    /// Opcodes present in session B but not A.
    pub only_in_b: Vec<u16>,
    /// Opcodes present in both, with (count_a, count_b).
    pub shared: Vec<(u16, usize, usize)>,
}

/// Compare two capture sessions by opcode frequency.
pub fn diff_sessions(a: &CaptureSession, b: &CaptureSession) -> SessionDiff {
    let hist_a: std::collections::HashMap<u16, usize> = a.opcode_histogram().into_iter().collect();
    let hist_b: std::collections::HashMap<u16, usize> = b.opcode_histogram().into_iter().collect();

    let keys_a: HashSet<u16> = hist_a.keys().copied().collect();
    let keys_b: HashSet<u16> = hist_b.keys().copied().collect();

    let mut only_in_a: Vec<u16> = keys_a.difference(&keys_b).copied().collect();
    let mut only_in_b: Vec<u16> = keys_b.difference(&keys_a).copied().collect();
    only_in_a.sort();
    only_in_b.sort();

    let mut shared: Vec<(u16, usize, usize)> = keys_a
        .intersection(&keys_b)
        .map(|&op| (op, hist_a[&op], hist_b[&op]))
        .collect();
    shared.sort_by_key(|&(op, _, _)| op);

    SessionDiff {
        only_in_a,
        only_in_b,
        shared,
    }
}

// ---------------------------------------------------------------------------
// Binary persistence (custom format)
// ---------------------------------------------------------------------------

// File format:
//   Magic: b"DMPC" (4 bytes)
//   Version: u8 (1)
//   Label length: u16 LE
//   Label: UTF-8 bytes
//   Started at: u64 LE (unix seconds)
//   Packet count: u32 LE
//   For each packet:
//     timestamp_ms: u64 LE
//     direction: u8 (0 = C→S, 1 = S→C)
//     opcode: u16 LE
//     payload_len: u32 LE
//     payload: [u8; payload_len]

const MAGIC: &[u8; 4] = b"DMPC";
const FORMAT_VERSION: u8 = 1;

/// Write a capture session to binary format.
pub fn save_binary<W: Write>(session: &CaptureSession, mut writer: W) -> io::Result<()> {
    writer.write_all(MAGIC)?;
    writer.write_all(&[FORMAT_VERSION])?;

    let label_bytes = session.label.as_bytes();
    writer.write_all(&(label_bytes.len() as u16).to_le_bytes())?;
    writer.write_all(label_bytes)?;

    writer.write_all(&session.started_at.to_le_bytes())?;
    writer.write_all(&(session.len() as u32).to_le_bytes())?;

    for pkt in session.packets() {
        writer.write_all(&pkt.timestamp_ms.to_le_bytes())?;
        writer.write_all(&[match pkt.direction {
            PacketDirection::ClientToServer => 0,
            PacketDirection::ServerToClient => 1,
        }])?;
        writer.write_all(&pkt.opcode.to_le_bytes())?;
        writer.write_all(&(pkt.payload.len() as u32).to_le_bytes())?;
        writer.write_all(&pkt.payload)?;
    }

    Ok(())
}

/// Load a capture session from binary format.
pub fn load_binary<R: Read>(mut reader: R) -> io::Result<CaptureSession> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "bad magic"));
    }

    let mut version = [0u8; 1];
    reader.read_exact(&mut version)?;
    if version[0] != FORMAT_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported version {}", version[0]),
        ));
    }

    let mut buf2 = [0u8; 2];
    reader.read_exact(&mut buf2)?;
    let label_len = u16::from_le_bytes(buf2) as usize;
    // Cap label length at 4 KiB to prevent malicious files from consuming memory.
    if label_len > 4096 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "label too long"));
    }
    let mut label_buf = vec![0u8; label_len];
    reader.read_exact(&mut label_buf)?;
    let label =
        String::from_utf8(label_buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut buf8 = [0u8; 8];
    reader.read_exact(&mut buf8)?;
    let started_at = u64::from_le_bytes(buf8);

    let mut buf4 = [0u8; 4];
    reader.read_exact(&mut buf4)?;
    let pkt_count = u32::from_le_bytes(buf4) as usize;
    // Cap packet count to prevent OOM on malicious files.
    if pkt_count > 10_000_000 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "packet count exceeds 10M limit",
        ));
    }

    let mut session = CaptureSession::new(label, pkt_count);
    session.started_at = started_at;

    for _ in 0..pkt_count {
        reader.read_exact(&mut buf8)?;
        let timestamp_ms = u64::from_le_bytes(buf8);

        let mut dir_byte = [0u8; 1];
        reader.read_exact(&mut dir_byte)?;
        let direction = match dir_byte[0] {
            0 => PacketDirection::ClientToServer,
            1 => PacketDirection::ServerToClient,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid direction byte",
                ));
            }
        };

        reader.read_exact(&mut buf2)?;
        let opcode = u16::from_le_bytes(buf2);

        reader.read_exact(&mut buf4)?;
        let payload_len = u32::from_le_bytes(buf4) as usize;

        // Validate payload size constraints before reading
        if payload_len > 65536 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "payload exceeds 64 KiB limit",
            ));
        }
        if payload_len < 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "payload too small (minimum 2 bytes for opcode)",
            ));
        }

        let mut payload = vec![0u8; payload_len];
        reader.read_exact(&mut payload)?;

        let packet = CapturedPacket {
            timestamp_ms,
            direction,
            opcode,
            payload,
        };

        session.record(packet);
    }

    Ok(session)
}

// ---------------------------------------------------------------------------
// JSON lines export
// ---------------------------------------------------------------------------

/// Export a capture session as JSON lines (one packet per line).
pub fn export_jsonl<W: Write>(session: &CaptureSession, mut writer: W) -> io::Result<()> {
    // Header line with session metadata.
    let header = serde_json::json!({
        "type": "session",
        "label": session.label,
        "started_at": session.started_at,
        "packet_count": session.len(),
        "total_seen": session.total_seen,
    });
    writeln!(writer, "{}", header)?;

    for pkt in session.packets() {
        let line = serde_json::json!({
            "type": "packet",
            "ts_ms": pkt.timestamp_ms,
            "dir": match pkt.direction {
                PacketDirection::ClientToServer => "C2S",
                PacketDirection::ServerToClient => "S2C",
            },
            "opcode": format!("0x{:04X}", pkt.opcode),
            "len": pkt.payload.len(),
            "hex": hex_encode(&pkt.payload),
        });
        writeln!(writer, "{}", line)?;
    }

    Ok(())
}

/// Simple hex encoding (no dependencies).
fn hex_encode(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 2);
    for byte in data {
        use std::fmt::Write;
        write!(s, "{byte:02x}").ok();
    }
    s
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_packet(ts: u64, dir: PacketDirection, opcode: u16) -> CapturedPacket {
        let mut payload = opcode.to_le_bytes().to_vec();
        payload.extend_from_slice(&[0xDE, 0xAD]);
        CapturedPacket {
            timestamp_ms: ts,
            direction: dir,
            opcode,
            payload,
        }
    }

    #[test]
    fn filter_pass_all() {
        let f = PacketFilter::default();
        let p = make_packet(0, PacketDirection::ClientToServer, 0x1234);
        assert!(f.matches(&p));
    }

    #[test]
    fn filter_whitelist() {
        let f = PacketFilter::whitelist([0x1234, 0x5678]);
        assert!(f.matches(&make_packet(0, PacketDirection::ClientToServer, 0x1234)));
        assert!(!f.matches(&make_packet(0, PacketDirection::ClientToServer, 0x9999)));
    }

    #[test]
    fn filter_blacklist() {
        let f = PacketFilter::blacklist([0x0001]);
        assert!(!f.matches(&make_packet(0, PacketDirection::ClientToServer, 0x0001)));
        assert!(f.matches(&make_packet(0, PacketDirection::ClientToServer, 0x0002)));
    }

    #[test]
    fn filter_direction() {
        let f = PacketFilter::default().with_direction(PacketDirection::ServerToClient);
        assert!(!f.matches(&make_packet(0, PacketDirection::ClientToServer, 0x1234)));
        assert!(f.matches(&make_packet(0, PacketDirection::ServerToClient, 0x1234)));
    }

    #[test]
    fn filter_whitelist_with_direction() {
        let f = PacketFilter::whitelist([0x1234]).with_direction(PacketDirection::ClientToServer);
        assert!(f.matches(&make_packet(0, PacketDirection::ClientToServer, 0x1234)));
        assert!(!f.matches(&make_packet(0, PacketDirection::ServerToClient, 0x1234)));
        assert!(!f.matches(&make_packet(0, PacketDirection::ClientToServer, 0x9999)));
    }

    #[test]
    fn session_ring_buffer() {
        let mut s = CaptureSession::new("test", 3);
        for i in 0..5 {
            s.record(make_packet(
                i * 100,
                PacketDirection::ClientToServer,
                i as u16,
            ));
        }
        assert_eq!(s.len(), 3);
        assert_eq!(s.total_seen, 5);
        assert_eq!(s.evicted(), 2);
        assert_eq!(s.get(0).unwrap().opcode, 2);
    }

    #[test]
    fn session_filter_rejects() {
        let mut s = CaptureSession::new("test", 100).with_filter(PacketFilter::whitelist([0x0001]));
        assert!(s.record(make_packet(0, PacketDirection::ClientToServer, 0x0001)));
        assert!(!s.record(make_packet(0, PacketDirection::ClientToServer, 0x0002)));
        assert_eq!(s.len(), 1);
        assert_eq!(s.total_seen, 2);
    }

    #[test]
    fn session_histogram() {
        let mut s = CaptureSession::new("test", 100);
        for _ in 0..5 {
            s.record(make_packet(0, PacketDirection::ClientToServer, 0x0001));
        }
        for _ in 0..3 {
            s.record(make_packet(0, PacketDirection::ClientToServer, 0x0002));
        }
        s.record(make_packet(0, PacketDirection::ClientToServer, 0x0003));

        let hist = s.opcode_histogram();
        assert_eq!(hist[0], (0x0001, 5));
        assert_eq!(hist[1], (0x0002, 3));
        assert_eq!(hist[2], (0x0003, 1));
    }

    #[test]
    fn session_unique_opcodes() {
        let mut s = CaptureSession::new("test", 100);
        s.record(make_packet(0, PacketDirection::ClientToServer, 0x0001));
        s.record(make_packet(0, PacketDirection::ClientToServer, 0x0001));
        s.record(make_packet(0, PacketDirection::ClientToServer, 0x0002));
        assert_eq!(s.unique_opcodes().len(), 2);
    }

    #[test]
    fn diff_sessions_basic() {
        let mut a = CaptureSession::new("a", 100);
        let mut b = CaptureSession::new("b", 100);

        a.record(make_packet(0, PacketDirection::ClientToServer, 0x0001));
        a.record(make_packet(0, PacketDirection::ClientToServer, 0x0002));
        b.record(make_packet(0, PacketDirection::ClientToServer, 0x0002));
        b.record(make_packet(0, PacketDirection::ClientToServer, 0x0003));

        let diff = diff_sessions(&a, &b);
        assert_eq!(diff.only_in_a, vec![0x0001]);
        assert_eq!(diff.only_in_b, vec![0x0003]);
        assert_eq!(diff.shared.len(), 1);
        assert_eq!(diff.shared[0], (0x0002, 1, 1));
    }

    #[test]
    fn binary_round_trip() {
        let mut s = CaptureSession::new("round-trip-test", 100);
        s.record(make_packet(100, PacketDirection::ClientToServer, 0x1234));
        s.record(make_packet(200, PacketDirection::ServerToClient, 0x5678));
        s.record(make_packet(300, PacketDirection::ClientToServer, 0x9ABC));

        let mut buf = Vec::new();
        save_binary(&s, &mut buf).unwrap();

        let loaded = load_binary(&buf[..]).unwrap();
        assert_eq!(loaded.label, "round-trip-test");
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded.get(0).unwrap().opcode, 0x1234);
        assert_eq!(
            loaded.get(1).unwrap().direction,
            PacketDirection::ServerToClient
        );
        assert_eq!(loaded.get(2).unwrap().timestamp_ms, 300);
    }

    #[test]
    fn binary_bad_magic_rejected() {
        let data = b"BADMxxxxxxxx";
        assert!(load_binary(&data[..]).is_err());
    }

    #[test]
    fn binary_bad_version_rejected() {
        let mut data = Vec::new();
        data.extend_from_slice(MAGIC);
        data.push(99);
        assert!(load_binary(&data[..]).is_err());
    }

    #[test]
    fn jsonl_export() {
        let mut s = CaptureSession::new("jsonl-test", 100);
        s.record(make_packet(50, PacketDirection::ClientToServer, 0x00FF));

        let mut buf = Vec::new();
        export_jsonl(&s, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let lines: Vec<&str> = output.trim().lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"type\":\"session\""));
        assert!(lines[1].contains("\"opcode\":\"0x00FF\""));
        assert!(lines[1].contains("\"dir\":\"C2S\""));
    }

    #[test]
    fn packet_data_len() {
        let p = make_packet(0, PacketDirection::ClientToServer, 0x0001);
        assert_eq!(p.data_len(), 2);
    }

    #[test]
    fn direction_display() {
        assert_eq!(format!("{}", PacketDirection::ClientToServer), "C→S");
        assert_eq!(format!("{}", PacketDirection::ServerToClient), "S→C");
    }

    #[test]
    fn session_clear() {
        let mut s = CaptureSession::new("test", 100);
        s.record(make_packet(0, PacketDirection::ClientToServer, 0x0001));
        assert_eq!(s.len(), 1);
        s.clear();
        assert_eq!(s.len(), 0);
        assert_eq!(s.total_seen, 0);
    }

    #[test]
    fn empty_session_duration() {
        let s = CaptureSession::new("empty", 100);
        assert_eq!(s.duration(), Duration::ZERO);
    }

    #[test]
    fn hex_encode_works() {
        assert_eq!(hex_encode(&[0xDE, 0xAD, 0xBE, 0xEF]), "deadbeef");
        assert_eq!(hex_encode(&[]), "");
    }

    #[test]
    fn oversized_label_rejected() {
        let mut data = Vec::new();
        data.extend_from_slice(MAGIC);
        data.push(FORMAT_VERSION);
        data.extend_from_slice(&5000u16.to_le_bytes());
        assert!(load_binary(&data[..]).is_err());
    }

    #[test]
    fn oversized_payload_rejected() {
        let mut s = CaptureSession::new("x", 1);
        s.record(make_packet(0, PacketDirection::ClientToServer, 0x0001));

        let mut data = Vec::new();
        data.extend_from_slice(MAGIC);
        data.push(FORMAT_VERSION);
        data.extend_from_slice(&1u16.to_le_bytes());
        data.push(b'x');
        data.extend_from_slice(&0u64.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&0u64.to_le_bytes());
        data.push(0);
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&100_000u32.to_le_bytes());
        assert!(load_binary(&data[..]).is_err());
    }

    // =====================================================================
    // Packet Validation Tests
    // =====================================================================

    #[test]
    fn packet_validate_minimum_size() {
        let p = CapturedPacket {
            timestamp_ms: 0,
            direction: PacketDirection::ClientToServer,
            opcode: 0x0001,
            payload: vec![0x01, 0x00],
        };
        assert!(p.validate().is_ok());
    }

    #[test]
    fn packet_validate_truncated_payload() {
        let p = CapturedPacket {
            timestamp_ms: 0,
            direction: PacketDirection::ClientToServer,
            opcode: 0x0001,
            payload: vec![0x01],
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn packet_validate_empty_payload() {
        let p = CapturedPacket {
            timestamp_ms: 0,
            direction: PacketDirection::ClientToServer,
            opcode: 0x0001,
            payload: vec![],
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn packet_validate_oversized() {
        let p = CapturedPacket {
            timestamp_ms: 0,
            direction: PacketDirection::ClientToServer,
            opcode: 0x0001,
            payload: vec![0u8; 70000],
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn packet_validate_at_max_size() {
        let mut payload = vec![0u8; 65536];
        payload[0] = 0x01;
        payload[1] = 0x00;
        let p = CapturedPacket {
            timestamp_ms: 0,
            direction: PacketDirection::ClientToServer,
            opcode: 0x0001,
            payload,
        };
        assert!(p.validate().is_ok());
    }

    #[test]
    fn packet_validate_just_over_max() {
        let p = CapturedPacket {
            timestamp_ms: 0,
            direction: PacketDirection::ClientToServer,
            opcode: 0x0001,
            payload: vec![0u8; 65537],
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn packet_validate_opcode_mismatch() {
        let p = CapturedPacket {
            timestamp_ms: 0,
            direction: PacketDirection::ClientToServer,
            opcode: 0x1234,
            payload: vec![0x78, 0x56], // 0x5678 != 0x1234
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn packet_validate_opcode_consistent() {
        let p = CapturedPacket {
            timestamp_ms: 0,
            direction: PacketDirection::ClientToServer,
            opcode: 0x1234,
            payload: vec![0x34, 0x12, 0xDE, 0xAD], // 0x1234 in little-endian
        };
        assert!(p.validate().is_ok());
    }

    #[test]
    fn record_rejects_truncated_packet() {
        let mut s = CaptureSession::new("test", 100);
        let truncated = CapturedPacket {
            timestamp_ms: 0,
            direction: PacketDirection::ClientToServer,
            opcode: 0x0001,
            payload: vec![0x01],
        };
        assert!(!s.record(truncated));
        assert_eq!(s.len(), 0);
        assert_eq!(s.total_seen, 1);
    }

    #[test]
    fn record_rejects_oversized_packet() {
        let mut s = CaptureSession::new("test", 100);
        let oversized = CapturedPacket {
            timestamp_ms: 0,
            direction: PacketDirection::ClientToServer,
            opcode: 0x0001,
            payload: vec![0u8; 100000],
        };
        assert!(!s.record(oversized));
        assert_eq!(s.len(), 0);
        assert_eq!(s.total_seen, 1);
    }

    #[test]
    fn record_rejects_opcode_mismatch() {
        let mut s = CaptureSession::new("test", 100);
        let mismatched = CapturedPacket {
            timestamp_ms: 0,
            direction: PacketDirection::ClientToServer,
            opcode: 0x1234,
            payload: vec![0x56, 0x78], // 0x7856 != 0x1234
        };
        assert!(!s.record(mismatched));
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn record_accepts_valid_packet() {
        let mut s = CaptureSession::new("test", 100);
        let valid = CapturedPacket {
            timestamp_ms: 0,
            direction: PacketDirection::ClientToServer,
            opcode: 0x0001,
            payload: vec![0x01, 0x00, 0xDE, 0xAD],
        };
        assert!(s.record(valid));
        assert_eq!(s.len(), 1);
        assert_eq!(s.total_seen, 1);
    }

    #[test]
    fn binary_load_rejects_truncated_payload() {
        let mut data = Vec::new();
        data.extend_from_slice(MAGIC);
        data.push(FORMAT_VERSION);
        data.extend_from_slice(&1u16.to_le_bytes());
        data.push(b'x');
        data.extend_from_slice(&0u64.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&0u64.to_le_bytes());
        data.push(0);
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&100u32.to_le_bytes());
        data.extend_from_slice(&[0xAA; 10]);
        assert!(load_binary(&data[..]).is_err());
    }

    #[test]
    fn binary_load_rejects_undersized_payload() {
        let mut data = Vec::new();
        data.extend_from_slice(MAGIC);
        data.push(FORMAT_VERSION);
        data.extend_from_slice(&1u16.to_le_bytes());
        data.push(b'x');
        data.extend_from_slice(&0u64.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&0u64.to_le_bytes());
        data.push(0);
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.push(0xAA);
        assert!(load_binary(&data[..]).is_err());
    }

    #[test]
    fn binary_load_detects_incomplete_header() {
        let data = b"DM";
        assert!(load_binary(&data[..]).is_err());
    }

    #[test]
    fn binary_load_detects_incomplete_packet_data() {
        let mut data = Vec::new();
        data.extend_from_slice(MAGIC);
        data.push(FORMAT_VERSION);
        data.extend_from_slice(&1u16.to_le_bytes());
        data.push(b'x');
        data.extend_from_slice(&0u64.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&0u64.to_le_bytes());
        // Missing direction byte
        assert!(load_binary(&data[..]).is_err());
    }
}
