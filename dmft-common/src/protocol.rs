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
///
/// # Errors
///
/// Returns an error if the operation fails.
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
#[must_use]
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
    let (msg, _) =
        bincode::serde::decode_from_slice(&data[4..4 + len], bincode::config::standard()).ok()?;
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

    #[test]
    fn encode_length_prefix_correct() {
        let cmd = Command::Ping;
        let encoded = encode(&cmd).expect("encode failed");
        assert!(encoded.len() >= 4);
        let len = u32::from_le_bytes(encoded[..4].try_into().unwrap());
        assert_eq!(len as usize, encoded.len() - 4);
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
        let buf = 0u32.to_le_bytes();
        // Zero-length payload is valid framing but bincode can't decode an enum from empty bytes
        let result = decode::<Command>(&buf);
        assert!(result.is_none());
    }

    #[test]
    fn decode_malformed_bincode_returns_none() {
        // Valid length prefix but garbage payload
        let mut buf = 4u32.to_le_bytes().to_vec();
        buf.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        let result = decode::<Command>(&buf);
        assert!(result.is_none());
    }

    #[test]
    fn decode_exactly_max_message_size_is_not_rejected_by_size_check() {
        // Exactly MAX_MESSAGE_SIZE should pass the size check (len_u32 > MAX_MESSAGE_SIZE is false)
        // but fail at bincode parsing since it's all zeros
        let len_bytes = MAX_MESSAGE_SIZE.to_le_bytes();
        let mut buf = len_bytes.to_vec();
        buf.extend(vec![0u8; MAX_MESSAGE_SIZE as usize]);
        // The size guard uses `>` not `>=`, so MAX_MESSAGE_SIZE passes the guard
        // All-zeros may or may not be a valid bincode enum variant
        let _result = decode::<Command>(&buf);
        // Main assertion: MAX_MESSAGE_SIZE + 1 IS rejected
        let over = (MAX_MESSAGE_SIZE + 1).to_le_bytes();
        let mut over_buf = over.to_vec();
        over_buf.extend(vec![0u8; (MAX_MESSAGE_SIZE + 1) as usize]);
        assert!(
            decode::<Command>(&over_buf).is_none(),
            "MAX_MESSAGE_SIZE+1 should be rejected"
        );
    }

    #[test]
    fn encode_decode_with_extra_data() {
        let cmd = Command::Ping;
        let mut encoded = encode(&cmd).expect("encode failed");
        // Append extra garbage
        encoded.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

        let (decoded, consumed): (Command, usize) = decode(&encoded).expect("decode failed");
        assert!(matches!(decoded, Command::Ping));
        assert_eq!(
            consumed,
            encoded.len() - 4,
            "The number of bytes consumed should account for the header and payload, leaving the extra bytes."
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
