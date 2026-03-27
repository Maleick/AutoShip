use serde::{Serialize, de::DeserializeOwned};

/// Maximum allowed message size (64 KB). Frames larger than this are rejected
/// during decode to prevent memory exhaustion from malformed or malicious input.
pub const MAX_MESSAGE_SIZE: u32 = 65536;

/// Encode a message as a length-prefixed bincode frame.
///
/// Layout: `[len: u32 LE][payload: bincode bytes]`
pub fn encode<T: Serialize>(msg: &T) -> Vec<u8> {
    let payload =
        bincode::serde::encode_to_vec(msg, bincode::config::standard()).unwrap();
    let len = (payload.len() as u32).to_le_bytes();
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len);
    buf.extend_from_slice(&payload);
    buf
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
