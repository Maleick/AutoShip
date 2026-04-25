//! DanNet TLO compatibility stubs for TextQuest.
//!
//! Implements the `${DanNet[<peer>].Q[<query>]}` syntax that rgmercs and other
//! MQ2 macros use to query remote character state.
//!
//! # How it works
//!
//! The DLL registers a `DanNet` TLO.  When MQ2 evaluates
//! `${DanNet[Alice].Q[Me.HP]}`, it calls [`dannet_tlo_evaluate`] which:
//!
//! 1. Extracts the peer name (`Alice`) from the TLO index.
//! 2. Parses the sub-member query (`Q[Me.HP]`).
//! 3. Looks up the latest observed value in the shared `DanNetTloCache`.
//! 4. Returns the cached string or "NULL" if not yet observed.
//!
//! Values are populated by the `textquest-net` DanNet transport layer, which
//! calls `DanNetTloCache::update` whenever an `ObserveReply` arrives.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

/// Global cache of observed DanNet TLO values.
/// Key: `"<peer>/<query>"`, value: current string value.
static TLO_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, String>> {
    TLO_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Update (or insert) a cached TLO observation value.
///
/// Called from the DanNet transport layer when an `ObserveReply` arrives.
pub fn update_observed(peer: &str, query: &str, value: String) {
    let key = format!("{peer}/{query}");
    if let Ok(mut map) = cache().lock() {
        map.insert(key, value);
    }
}

/// Look up a cached TLO value.
///
/// Returns `None` if the value has never been observed.
#[must_use]
pub fn query_observed(peer: &str, query: &str) -> Option<String> {
    let key = format!("{peer}/{query}");
    cache().lock().ok()?.get(&key).cloned()
}

/// Evaluate a `${DanNet[<peer>].Q[<query>]}` expression.
///
/// Returns the cached value or `"NULL"` when not available.
///
/// # Parameters
///
/// - `peer`: character name index from the TLO call.
/// - `member`: the member expression, e.g. `Q[Me.HP]`.
///
/// # Returns
///
/// String value suitable for substitution into MQ2 macros.
#[must_use]
pub fn dannet_tlo_evaluate(peer: &str, member: &str) -> String {
    // Parse Q[...] syntax
    let query = if let Some(inner) = member
        .strip_prefix("Q[")
        .and_then(|s| s.strip_suffix(']'))
    {
        inner
    } else {
        // Unrecognised member — return peer name for ${DanNet[Alice]} bare access
        if member.is_empty() {
            return peer.to_string();
        }
        return "NULL".to_string();
    };

    query_observed(peer, query).unwrap_or_else(|| "NULL".to_string())
}

/// List all peers that have at least one cached observation.
#[must_use]
pub fn known_peers() -> Vec<String> {
    let Ok(map) = cache().lock() else {
        return Vec::new();
    };
    let mut peers: Vec<String> = map
        .keys()
        .filter_map(|k| k.split_once('/').map(|(p, _)| p.to_string()))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    peers.sort();
    peers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_round_trip() {
        update_observed("Alice", "Me.HP", "95".to_string());
        assert_eq!(query_observed("Alice", "Me.HP"), Some("95".to_string()));
    }

    #[test]
    fn tlo_evaluate_q_syntax() {
        update_observed("Bob", "Me.Mana", "60".to_string());
        assert_eq!(dannet_tlo_evaluate("Bob", "Q[Me.Mana]"), "60");
    }

    #[test]
    fn tlo_evaluate_missing_returns_null() {
        assert_eq!(dannet_tlo_evaluate("Unknown", "Q[Me.HP]"), "NULL");
    }

    #[test]
    fn known_peers_lists_observed() {
        update_observed("Healer", "Me.HP", "100".to_string());
        let peers = known_peers();
        assert!(peers.contains(&"Healer".to_string()));
    }
}
