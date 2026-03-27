use serde::{Serialize, de::DeserializeOwned};

/// Encode a message as a length-prefixed bincode frame.
///
/// Layout: `[len: u32 LE][payload: bincode bytes]`
pub fn encode<T: Serialize>(msg: &T) -> Vec<u8> {
    let payload =
        bincode::serde::encode_to_vec(msg, bincode::config::standard()).unwrap();
    let len = (payload.len() as u32).to_le_bytes();
    [len.as_slice(), &payload].concat()
}

/// Decode a length-prefixed bincode frame from `data`.
///
/// Returns `Some((message, bytes_consumed))` on success, or `None` if `data`
/// does not yet contain a complete frame.
pub fn decode<T: DeserializeOwned>(data: &[u8]) -> Option<(T, usize)> {
    if data.len() < 4 {
        return None;
    }
    let len = u32::from_le_bytes(data[..4].try_into().ok()?) as usize;
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
