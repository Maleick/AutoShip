use serde::{Serialize, de::DeserializeOwned};
use std::fmt;

/// Maximum allowed message size (64 KB). Frames larger than this are rejected
/// during decode to prevent memory exhaustion from malformed or malicious
/// input.
pub const MAX_MESSAGE_SIZE: u32 = 65536;
const MAX_MESSAGE_SIZE_USIZE: usize = MAX_MESSAGE_SIZE as usize;
/// Fixed four-byte magic for every IPC frame.
pub const FRAME_MAGIC: [u8; 4] = *b"TQIP";
/// Current supported IPC protocol version.
pub const FRAME_VERSION: u16 = 1;
/// Frame header length: magic (4) + version (2) + payload len (4).
pub const FRAME_HEADER_SIZE: usize = 10;

/// Decode failures that should be surfaced as protocol errors rather than
/// treated as "no complete frame yet".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameDecodeError {
    InvalidMagic([u8; 4]),
    UnsupportedVersion { actual: u16, expected: u16 },
    OversizedPayload(u32),
    PayloadDecode(String),
}

impl fmt::Display for FrameDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic(found) => write!(f, "invalid protocol magic {found:?}"),
            Self::UnsupportedVersion { actual, expected } => {
                write!(
                    f,
                    "unsupported protocol version {actual} (expected {expected})"
                )
            }
            Self::OversizedPayload(len) => write!(f, "payload length {len} exceeds max frame size"),
            Self::PayloadDecode(message) => write!(f, "failed to decode payload: {message}"),
        }
    }
}

impl std::error::Error for FrameDecodeError {}

/// Encode a message as a fixed-header bincode frame.
///
/// Layout: `[magic: 4][version: u16 LE][len: u32 LE][payload: bincode bytes]`
///
/// Returns an error if serialization fails. This is preferred over panicking
/// because inside the injected DLL, a panic unwinds through EQ's stack frames
/// and causes undefined behavior.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, bincode::error::EncodeError> {
    let payload = bincode::serde::encode_to_vec(msg, bincode::config::standard())?;
    let len = (payload.len() as u32).to_le_bytes();
    let mut buf = Vec::with_capacity(FRAME_HEADER_SIZE + payload.len());
    buf.extend_from_slice(&FRAME_MAGIC);
    buf.extend_from_slice(&FRAME_VERSION.to_le_bytes());
    buf.extend_from_slice(&len);
    buf.extend_from_slice(&payload);
    Ok(buf)
}

/// Decode a framed bincode message from `data`.
///
/// Returns `Ok(None)` if `data` does not yet contain a complete frame.
///
/// # Errors
///
/// Returns an error when the envelope is invalid or incompatible.
pub fn decode_frame<T: DeserializeOwned>(
    data: &[u8],
) -> Result<Option<(T, usize)>, FrameDecodeError> {
    if data.len() < FRAME_HEADER_SIZE {
        return Ok(None);
    }

    let magic = <[u8; 4]>::try_from(&data[..4]).expect("slice has exact header size");
    if magic != FRAME_MAGIC {
        return Err(FrameDecodeError::InvalidMagic(magic));
    }

    let version = u16::from_le_bytes(
        data[4..6]
            .try_into()
            .expect("slice has exact version width"),
    );
    if version != FRAME_VERSION {
        return Err(FrameDecodeError::UnsupportedVersion {
            actual: version,
            expected: FRAME_VERSION,
        });
    }

    let len_u32 = u32::from_le_bytes(
        data[6..FRAME_HEADER_SIZE]
            .try_into()
            .expect("slice has exact length width"),
    );
    if len_u32 > MAX_MESSAGE_SIZE {
        return Err(FrameDecodeError::OversizedPayload(len_u32));
    }

    let len = len_u32 as usize;
    if data.len() < FRAME_HEADER_SIZE + len {
        return Ok(None);
    }

    let config = bincode::config::standard().with_limit::<MAX_MESSAGE_SIZE_USIZE>();
    let payload = &data[FRAME_HEADER_SIZE..FRAME_HEADER_SIZE + len];
    let (msg, _) = bincode::serde::decode_from_slice(payload, config)
        .map_err(|error| FrameDecodeError::PayloadDecode(error.to_string()))?;
    Ok(Some((msg, FRAME_HEADER_SIZE + len)))
}

/// Decode a framed bincode message from `data`.
///
/// Returns `Some((message, bytes_consumed))` on success, or `None` if `data`
/// does not yet contain a complete frame or the frame is invalid.
#[must_use]
pub fn decode<T: DeserializeOwned>(data: &[u8]) -> Option<(T, usize)> {
    decode_frame(data).ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ipc::{Command, Response},
        nav::Waypoint,
        soul::SayChannel,
    };

    #[test]
    fn command_roundtrip_ping() {
        let cmd = Command::Ping;
        let encoded = encode(&cmd).expect("encode failed");
        let (decoded, consumed): (Command, usize) = decode(&encoded).expect("decode failed");
        assert_eq!(consumed, encoded.len());
        assert!(matches!(decoded, Command::Ping));
    }

    #[test]
    fn command_roundtrip_navigate_to() {
        let cmd = Command::NavigateTo {
            waypoints: vec![Waypoint::new(1.0, 2.0, 3.0), Waypoint::new(4.0, 5.0, 6.0)],
        };
        let encoded = encode(&cmd).expect("encode failed");
        let (decoded, _): (Command, usize) = decode(&encoded).expect("decode failed");
        if let Command::NavigateTo { waypoints } = decoded {
            assert_eq!(waypoints.len(), 2);
            assert!((waypoints[0].x - 1.0).abs() < f32::EPSILON);
            assert!((waypoints[1].z - 6.0).abs() < f32::EPSILON);
        } else {
            panic!("expected NavigateTo, got {decoded:?}");
        }
    }

    #[test]
    fn command_roundtrip_cast_spell() {
        let cmd = Command::CastSpell {
            spell_slot: 3,
            target_id: Some(12345),
            kill: false,
            recast: 0,
        };
        let encoded = encode(&cmd).expect("encode failed");
        let (decoded, _): (Command, usize) = decode(&encoded).expect("decode failed");
        if let Command::CastSpell {
            spell_slot,
            target_id,
            kill,
            recast,
        } = decoded
        {
            assert_eq!(spell_slot, 3);
            assert_eq!(target_id, Some(12345));
            assert!(!kill);
            assert_eq!(recast, 0);
        } else {
            panic!("expected CastSpell, got {decoded:?}");
        }
    }

    #[test]
    fn command_roundtrip_eject() {
        let cmd = Command::Eject;
        let encoded = encode(&cmd).expect("encode failed");
        let (decoded, _): (Command, usize) = decode(&encoded).expect("decode failed");
        assert!(matches!(decoded, Command::Eject));
    }

    #[test]
    fn command_roundtrip_say() {
        let cmd = Command::Say {
            channel: SayChannel::Group,
            message: "incoming!".to_string(),
            target: None,
        };
        let encoded = encode(&cmd).expect("encode failed");
        let (decoded, _): (Command, usize) = decode(&encoded).expect("decode failed");
        if let Command::Say {
            channel,
            message,
            target,
        } = decoded
        {
            assert_eq!(channel, SayChannel::Group);
            assert_eq!(message, "incoming!");
            assert!(target.is_none());
        } else {
            panic!("expected Say, got {decoded:?}");
        }
    }

    #[test]
    fn response_roundtrip_pong() {
        let resp = Response::Pong {
            client_id: 7,
            timestamp_ms: 1234567890,
        };
        let encoded = encode(&resp).expect("encode failed");
        let (decoded, _): (Response, usize) = decode(&encoded).expect("decode failed");
        if let Response::Pong {
            client_id,
            timestamp_ms,
        } = decoded
        {
            assert_eq!(client_id, 7);
            assert_eq!(timestamp_ms, 1234567890);
        } else {
            panic!("expected Pong, got {decoded:?}");
        }
    }

    #[test]
    fn response_roundtrip_error() {
        let resp = Response::Error {
            message: "something broke".to_string(),
        };
        let encoded = encode(&resp).expect("encode failed");
        let (decoded, _): (Response, usize) = decode(&encoded).expect("decode failed");
        if let Response::Error { message } = decoded {
            assert_eq!(message, "something broke");
        } else {
            panic!("expected Error, got {decoded:?}");
        }
    }

    #[test]
    fn decode_returns_none_for_incomplete_data() {
        assert!(decode::<Command>(&[0x01, 0x02]).is_none());

        let mut buf = FRAME_MAGIC.to_vec();
        buf.extend_from_slice(&FRAME_VERSION.to_le_bytes());
        buf.extend_from_slice(&100u32.to_le_bytes());
        buf.extend_from_slice(&[0u8; 10]);
        assert!(decode::<Command>(&buf).is_none());
    }

    #[test]
    fn decode_rejects_oversized_message() {
        let oversized_len = (MAX_MESSAGE_SIZE + 1).to_le_bytes();
        let mut buf = FRAME_MAGIC.to_vec();
        buf.extend_from_slice(&FRAME_VERSION.to_le_bytes());
        buf.extend_from_slice(&oversized_len);
        buf.extend_from_slice(&[0u8; 100]);
        assert!(decode::<Command>(&buf).is_none());
        assert!(matches!(
            decode_frame::<Command>(&buf),
            Err(FrameDecodeError::OversizedPayload(len)) if len == MAX_MESSAGE_SIZE + 1
        ));
    }

    #[test]
    fn decode_rejects_length_prefixes_inside_payload_that_exceed_limit() {
        let huge_len_prefix = bincode::serde::encode_to_vec(
            u64::from(MAX_MESSAGE_SIZE) + 1,
            bincode::config::standard(),
        )
        .expect("encode failed");

        let mut frame = FRAME_MAGIC.to_vec();
        frame.extend_from_slice(&FRAME_VERSION.to_le_bytes());
        frame.extend_from_slice(&(huge_len_prefix.len() as u32).to_le_bytes());
        frame.extend_from_slice(&huge_len_prefix);

        assert!(decode::<String>(&frame).is_none());
    }

    #[test]
    fn encode_header_contains_magic_version_and_length() {
        let cmd = Command::Ping;
        let encoded = encode(&cmd).expect("encode failed");
        assert!(encoded.len() >= FRAME_HEADER_SIZE);
        assert_eq!(&encoded[..4], &FRAME_MAGIC);
        assert_eq!(
            u16::from_le_bytes(encoded[4..6].try_into().unwrap()),
            FRAME_VERSION
        );
        let len = u32::from_le_bytes(encoded[6..FRAME_HEADER_SIZE].try_into().unwrap());
        assert_eq!(len as usize, encoded.len() - FRAME_HEADER_SIZE);
    }

    #[test]
    fn decode_empty_slice_returns_none() {
        assert!(decode::<Command>(&[]).is_none());
    }

    #[test]
    fn decode_exactly_three_bytes_returns_none() {
        assert!(decode::<Command>(&[0, 0, 0]).is_none());
    }

    #[test]
    fn decode_zero_length_payload() {
        let mut buf = FRAME_MAGIC.to_vec();
        buf.extend_from_slice(&FRAME_VERSION.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        let result = decode::<Command>(&buf);
        assert!(result.is_none());
    }

    #[test]
    fn decode_malformed_bincode_returns_none() {
        let mut buf = FRAME_MAGIC.to_vec();
        buf.extend_from_slice(&FRAME_VERSION.to_le_bytes());
        buf.extend_from_slice(&4u32.to_le_bytes());
        buf.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        let result = decode::<Command>(&buf);
        assert!(result.is_none());
        assert!(matches!(
            decode_frame::<Command>(&buf),
            Err(FrameDecodeError::PayloadDecode(_))
        ));
    }

    #[test]
    fn decode_exactly_max_message_size_is_not_rejected_by_size_check() {
        let len_bytes = MAX_MESSAGE_SIZE.to_le_bytes();
        let mut buf = FRAME_MAGIC.to_vec();
        buf.extend_from_slice(&FRAME_VERSION.to_le_bytes());
        buf.extend_from_slice(&len_bytes);
        buf.extend(vec![0u8; MAX_MESSAGE_SIZE as usize]);
        let _result = decode::<Command>(&buf);

        let over = (MAX_MESSAGE_SIZE + 1).to_le_bytes();
        let mut over_buf = FRAME_MAGIC.to_vec();
        over_buf.extend_from_slice(&FRAME_VERSION.to_le_bytes());
        over_buf.extend_from_slice(&over);
        over_buf.extend(vec![0u8; (MAX_MESSAGE_SIZE + 1) as usize]);
        assert!(
            decode::<Command>(&over_buf).is_none(),
            "MAX_MESSAGE_SIZE+1 should be rejected"
        );
    }

    #[test]
    fn decode_frame_rejects_wrong_magic() {
        let mut frame = b"NOPE".to_vec();
        frame.extend_from_slice(&FRAME_VERSION.to_le_bytes());
        frame.extend_from_slice(&1u32.to_le_bytes());
        frame.push(0);
        assert!(matches!(
            decode_frame::<Command>(&frame),
            Err(FrameDecodeError::InvalidMagic(found)) if found == *b"NOPE"
        ));
    }

    #[test]
    fn decode_frame_rejects_wrong_version() {
        let mut frame = FRAME_MAGIC.to_vec();
        frame.extend_from_slice(&(FRAME_VERSION + 1).to_le_bytes());
        frame.extend_from_slice(&1u32.to_le_bytes());
        frame.push(0);
        assert!(matches!(
            decode_frame::<Command>(&frame),
            Err(FrameDecodeError::UnsupportedVersion { actual, expected })
                if actual == FRAME_VERSION + 1 && expected == FRAME_VERSION
        ));
    }

    #[test]
    fn ping_frame_matches_current_wire_golden() {
        let encoded = encode(&Command::Ping).expect("encode failed");
        assert_eq!(
            encoded,
            vec![
                b'T', b'Q', b'I', b'P', 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x44,
            ]
        );
    }

    #[test]
    fn encode_decode_with_extra_data() {
        let cmd = Command::Ping;
        let mut encoded = encode(&cmd).expect("encode failed");
        encoded.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

        let (decoded, consumed): (Command, usize) = decode(&encoded).expect("decode failed");
        assert!(matches!(decoded, Command::Ping));
        assert_eq!(
            consumed,
            encoded.len() - 4,
            "The number of bytes consumed should account for the header and payload, leaving the \
             extra bytes."
        );
    }

    #[test]
    fn response_roundtrip_nav_update() {
        use crate::nav::NavStatus;
        let resp = Response::NavUpdate {
            status: NavStatus::Moving {
                waypoint_index: 3,
                waypoint_count: 10,
                distance_remaining: 42.5,
            },
        };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, usize) = decode(&encoded).expect("decode");
        if let Response::NavUpdate { status } = decoded {
            if let NavStatus::Moving {
                waypoint_index,
                waypoint_count,
                distance_remaining,
            } = status
            {
                assert_eq!(waypoint_index, 3);
                assert_eq!(waypoint_count, 10);
                assert!((distance_remaining - 42.5).abs() < f32::EPSILON);
            } else {
                panic!("expected Moving");
            }
        } else {
            panic!("expected NavUpdate");
        }
    }

    #[test]
    fn response_roundtrip_combat_update() {
        use crate::combat::CombatStatus;
        let resp = Response::CombatUpdate {
            status: CombatStatus::Casting {
                spell_slot: 5,
                target_id: 9999,
            },
        };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, usize) = decode(&encoded).expect("decode");
        if let Response::CombatUpdate { status } = decoded {
            if let CombatStatus::Casting {
                spell_slot,
                target_id,
            } = status
            {
                assert_eq!(spell_slot, 5);
                assert_eq!(target_id, 9999);
            } else {
                panic!("expected Casting");
            }
        } else {
            panic!("expected CombatUpdate");
        }
    }
}
