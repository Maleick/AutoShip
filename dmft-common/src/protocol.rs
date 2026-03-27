use serde::{Serialize, de::DeserializeOwned};

/// Maximum allowed message size (64 KB). Frames larger than this are rejected
/// during decode to prevent memory exhaustion from malformed or malicious input.
pub const MAX_MESSAGE_SIZE: u32 = 65536;

/// Encode a message as a length-prefixed bincode frame.
///
/// Layout: `[len: u32 LE][payload: bincode bytes]`
///
/// Returns an error if serialization fails. This is preferred over panicking
/// because inside the injected DLL, a panic unwinds through EQ's stack frames
/// and causes undefined behavior.
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, bincode::error::EncodeError> {
    let payload = bincode::serde::encode_to_vec(msg, bincode::config::standard())?;
    let len = (payload.len() as u32).to_le_bytes();
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len);
    buf.extend_from_slice(&payload);
    Ok(buf)
}

/// Decode a length-prefixed bincode frame from `data`.
///
/// Returns `Some((message, bytes_consumed))` on success, or `None` if `data`
/// does not yet contain a complete frame.
pub fn decode<T: DeserializeOwned>(data: &[u8]) -> Option<(T, usize)> {
    if data.len() < 4 {
        return None;
    }
    let len_u32 = u32::from_le_bytes(data[..4].try_into().ok()?);
    if len_u32 > MAX_MESSAGE_SIZE {
        return None;
    }
    let len = len_u32 as usize;
    if data.len() < 4 + len {
        return None;
    }
    let (msg, _) = bincode::serde::decode_from_slice(
        &data[4..4 + len],
        bincode::config::standard(),
    )
    .ok()?;
    Some((msg, 4 + len))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::{Command, Response};
    use crate::nav::Waypoint;
    use crate::soul::SayChannel;

    #[test]
    fn command_roundtrip_ping() {
        let cmd = Command::Ping;
        let encoded = encode(&cmd).expect("encode failed");
        let (decoded, consumed): (Command, usize) =
            decode(&encoded).expect("decode failed");
        assert_eq!(consumed, encoded.len());
        assert!(matches!(decoded, Command::Ping));
    }

    #[test]
    fn command_roundtrip_navigate_to() {
        let cmd = Command::NavigateTo {
            waypoints: vec![
                Waypoint::new(1.0, 2.0, 3.0),
                Waypoint::new(4.0, 5.0, 6.0),
            ],
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
            target_id: 12345,
        };
        let encoded = encode(&cmd).expect("encode failed");
        let (decoded, _): (Command, usize) = decode(&encoded).expect("decode failed");
        if let Command::CastSpell {
            spell_slot,
            target_id,
        } = decoded
        {
            assert_eq!(spell_slot, 3);
            assert_eq!(target_id, 12345);
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
            channel, message, target,
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
        // Less than 4 bytes (no length prefix)
        assert!(decode::<Command>(&[0x01, 0x02]).is_none());

        // Length prefix says 100 bytes but only 10 available
        let mut buf = 100u32.to_le_bytes().to_vec();
        buf.extend_from_slice(&[0u8; 10]);
        assert!(decode::<Command>(&buf).is_none());
    }

    #[test]
    fn decode_rejects_oversized_message() {
        let oversized_len = (MAX_MESSAGE_SIZE + 1).to_le_bytes();
        let mut buf = oversized_len.to_vec();
        buf.extend_from_slice(&[0u8; 100]);
        assert!(decode::<Command>(&buf).is_none());
    }
}
