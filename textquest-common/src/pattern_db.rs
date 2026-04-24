//! Named-pattern registry wrapping the [`scanner`] module.
//!
//! `PatternDb` stores IDA-style byte patterns under string keys and can scan
//! a memory slice for all registered patterns in one call.  Results map each
//! name to the first match offset (or `None` when absent).
//!
//! Patterns are stored together with their original IDA string so the database
//! can round-trip through JSON without requiring byte-level accessor methods on
//! [`Pattern`].
//!
//! # Example
//!
//! ```
//! use textquest_common::pattern_db::PatternDb;
//! use textquest_common::scanner::Pattern;
//!
//! let mut db = PatternDb::default();
//! db.insert_ida("ProcessGameEvents", "48 89 5C 24 08");
//!
//! let haystack = [0xCC_u8; 8];
//! let results = db.scan_all(&haystack);
//! assert_eq!(results["ProcessGameEvents"], None);
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize, de::Error as _};

use crate::scanner::{self, Pattern};

// ── Scan module classification
// ────────────────────────────────────────────────

/// Module identifier for offset resolution (e.g., eqgame.dll).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScanModule {
    EqGame,
    EqMain,
    EqGraphics,
}

// ── Offset category ──────────────────────────────────────────────────────────

/// Categorizes an offset as an address or struct field offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OffsetCategory {
    Function,
    Global,
    PlayerBaseField,
    PlayerZoneField,
    SpawnManagerField,
    ContextMenuManagerField,
    ContextMenuField,
}

// ── Offset resolution mode ───────────────────────────────────────────────────

/// Mode for resolving a matched pattern to an address or field offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolveMode {
    /// Direct offset at match location.
    Direct,
    /// RIP-relative (e.g., LEA instruction); disp_offset is the byte offset
    /// from match start to the displacement field.
    RipRelative { disp_offset: usize },
    /// Extract a struct field displacement directly from matched instruction bytes.
    ///
    /// `disp_offset` is the byte offset from match start to the displacement
    /// field, and `disp_size` is the encoded field width in bytes.
    ExtractDisplacement {
        disp_offset: usize,
        disp_size: usize,
    },
}

// ── Scan entry ───────────────────────────────────────────────────────────────

/// Single scannable offset entry with pattern, category, and resolution mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanEntry {
    /// Symbolic name (e.g., "ProcessGameEvents").
    pub name: String,
    /// Module identifier.
    pub module: ScanModule,
    /// Byte pattern (IDA format).
    pub pattern: String,
    /// Function or global data.
    pub category: OffsetCategory,
    /// How to resolve the matched offset. Depending on `ResolveMode`, the
    /// resolved value is either a preferred-base module address (for
    /// `Direct`/`RipRelative`) or a relative struct-field offset in bytes
    /// (for `ExtractDisplacement`). The interpretation is also governed by
    /// `category`: function/global categories hold preferred-base addresses;
    /// struct-field categories hold byte offsets.
    pub resolve: ResolveMode,
    /// Expected compiled-time value (for validation against scanned results).
    /// This is a preferred-base address for function/global entries, or a
    /// struct-field byte offset for `ExtractDisplacement` entries.
    pub expected_preferred: Option<u64>,
}

impl ScanEntry {
    /// Whether this entry still uses a synthetic placeholder pattern.
    ///
    /// Real IDA patterns may contain wildcard bytes, but they must not be made
    /// entirely of wildcards or all-`CC` stub bytes.
    #[must_use]
    pub fn has_placeholder_pattern(&self) -> bool {
        is_placeholder_scan_pattern(&self.pattern)
    }
}

/// Check whether an IDA-style pattern is a placeholder stub.
#[must_use]
pub fn is_placeholder_scan_pattern(pattern: &str) -> bool {
    let mut saw_token = false;
    let mut all_cc = true;
    let mut all_wildcards = true;

    for token in pattern.split_whitespace() {
        saw_token = true;
        all_cc &= token.eq_ignore_ascii_case("CC");
        all_wildcards &= token == "?" || token == "??";
    }

    saw_token && (all_cc || all_wildcards)
}

/// Built-in scan entries that have locally verified, non-placeholder patterns.
#[must_use]
pub fn built_in_scan_entries() -> Vec<ScanEntry> {
    let mut entries = field_displacement_scan_entries();
    entries.extend(active_hack_scan_entries());
    entries.shrink_to_fit();
    entries
}

/// Seed scan entries that recover struct field offsets from accessor bytecode.
///
/// These entries are separate from function/global address scans because their
/// resolved values are relative struct offsets, not preferred-base addresses.
/// Additional Ghidra-exported accessor patterns can extend this catalog as they
/// are curated.
#[must_use]
pub fn field_displacement_scan_entries() -> Vec<ScanEntry> {
    vec![ScanEntry {
        name: "hpCurrent".to_string(),
        module: ScanModule::EqGame,
        pattern: "8B 81 ?? ?? 00 00 3B 81".to_string(),
        category: OffsetCategory::PlayerZoneField,
        resolve: ResolveMode::ExtractDisplacement {
            disp_offset: 2,
            disp_size: 4,
        },
        expected_preferred: Some(crate::offsets::player_zone::HP_CURRENT as u64),
    }]
}

/// Scan entries for the active-hack packet send/scrambler research surfaces.
///
/// Seeded from the March 10, 2026 live client after the January 2025 literal
/// offsets moved. The packet-scrambler entry resolves the RIP-relative global
/// used at opcode-scrambler call sites; the send and hton entries resolve
/// directly to function starts.
#[must_use]
pub fn active_hack_scan_entries() -> Vec<ScanEntry> {
    vec![
        ScanEntry {
            name: "packetScrambler".to_string(),
            module: ScanModule::EqGame,
            pattern:
                "48 8B 1D ?? ?? ?? ?? 48 8B 43 08 48 63 50 04 48 8D 4B 10 48 03 CA E8 ?? ?? ?? ?? 4C 8B C0 41 8B D6 48 8B CB E8 ?? ?? ?? ??"
                    .to_string(),
            category: OffsetCategory::Global,
            resolve: ResolveMode::RipRelative { disp_offset: 3 },
            expected_preferred: Some(
                crate::offsets::EQ_PREFERRED_BASE + crate::offsets::OFFSET_PACKET_SCRAMBLER as u64,
            ),
        },
        ScanEntry {
            name: "opcodeScramblerHton".to_string(),
            module: ScanModule::EqGame,
            pattern:
                "48 89 5C 24 10 48 89 6C 24 18 56 48 83 EC 20 49 8B E8 8B DA 48 8B F1 83 FA 15 75 ?? 48 8B 41 08 4C 63 48 04 41 8B 84 09 B0 02 00 00 48 0F BA E0 0C"
                    .to_string(),
            category: OffsetCategory::Function,
            resolve: ResolveMode::Direct,
            expected_preferred: Some(
                crate::offsets::EQ_PREFERRED_BASE + crate::offsets::OFFSET_HTON as u64,
            ),
        },
        ScanEntry {
            name: "networkSend".to_string(),
            module: ScanModule::EqGame,
            pattern:
                "48 89 5C 24 08 48 89 6C 24 10 56 57 41 56 48 83 EC 40 49 63 E9 49 8B F0 44 8B F2 48 8B F9 45 85 C9 0F 84 ?? ?? ?? ?? 4D 85 C0"
                    .to_string(),
            category: OffsetCategory::Function,
            resolve: ResolveMode::Direct,
            expected_preferred: Some(
                crate::offsets::EQ_PREFERRED_BASE + crate::offsets::OFFSET_NETWORK_SEND as u64,
            ),
        },
    ]
}

// ── Internal entry
// ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Entry {
    /// Parsed pattern used for scanning.
    pattern: Pattern,
    /// Original IDA string retained for JSON serialization.
    ida: String,
}

// ── Serialization proxy
// ───────────────────────────────────────────────────────

/// Serializable form of a single pattern entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EntryProxy {
    ida: String,
}

// ── PatternDb ────────────────────────────────────────────────────────────────

/// A named registry of byte patterns backed by [`scanner::Pattern`].
///
/// Patterns are keyed by a `String` name and stored as parsed [`Pattern`]
/// values for efficient repeated scanning.  The database round-trips through
/// JSON via the original IDA string representation so it remains
/// human-readable and editable on disk.
#[derive(Debug, Clone, Default)]
pub struct PatternDb {
    entries: HashMap<String, Entry>,
}

impl PatternDb {
    /// Create an empty pattern database.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Insert (or replace) a named pattern from a pre-parsed [`Pattern`] and
    /// its IDA source string.
    ///
    /// Prefer [`insert_ida`](Self::insert_ida) when building the DB from
    /// string literals — it keeps the IDA string automatically.
    pub fn insert(&mut self, name: String, pattern: Pattern) {
        // Build a synthetic IDA string from the pattern's byte/mask data by
        // re-scanning a zero-length slice to confirm the pattern is usable,
        // then store it with a placeholder IDA string derived at insert time.
        // Since Pattern doesn't expose bytes/mask, we store "(custom)" as the
        // IDA representation and require `insert_ida` for serializable entries.
        let ida = "(custom)".to_string();
        self.entries.insert(name, Entry { pattern, ida });
    }

    /// Insert (or replace) a named pattern from an IDA-style string.
    ///
    /// The IDA string is retained for JSON serialization so the database
    /// round-trips cleanly.
    ///
    /// # Panics
    ///
    /// Panics if `ida` is empty or contains invalid hex tokens (propagated
    /// from [`Pattern::from_ida`]).
    pub fn insert_ida(&mut self, name: impl Into<String>, ida: impl Into<String>) {
        let name = name.into();
        let ida = ida.into();
        let pattern = Pattern::from_ida(&ida);
        self.entries.insert(name, Entry { pattern, ida });
    }

    /// Look up a pattern by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Pattern> {
        self.entries.get(name).map(|e| &e.pattern)
    }

    /// Remove a pattern by name. Returns the removed pattern if it existed.
    pub fn remove(&mut self, name: &str) -> Option<Pattern> {
        self.entries.remove(name).map(|e| e.pattern)
    }

    /// Number of patterns currently registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the database is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Scan `data` for the first match of every registered pattern.
    ///
    /// Returns a map from pattern name to the first match offset, or `None`
    /// when the pattern is not found in `data`.
    #[must_use]
    pub fn scan_all(&self, data: &[u8]) -> HashMap<String, Option<usize>> {
        self.entries
            .iter()
            .map(|(name, entry)| {
                let result = scanner::scan_region(data, &entry.pattern);
                (name.clone(), result)
            })
            .collect()
    }

    /// Scan `data` and return only the patterns that produced a match.
    #[must_use]
    pub fn scan_matched(&self, data: &[u8]) -> HashMap<String, usize> {
        self.entries
            .iter()
            .filter_map(|(name, entry)| {
                scanner::scan_region(data, &entry.pattern).map(|offset| (name.clone(), offset))
            })
            .collect()
    }

    // ── JSON serialization ────────────────────────────────────────────────────

    /// Serialize the database to a JSON string.
    ///
    /// Each pattern is stored under its name with an `"ida"` field containing
    /// the original IDA pattern string.  Patterns inserted via
    /// [`insert`](Self::insert) (without an IDA string) are stored as
    /// `"(custom)"` and will be silently skipped on deserialization.
    ///
    /// # Errors
    ///
    /// Returns a [`serde_json::Error`] if serialization fails.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let map: HashMap<&str, EntryProxy> = self
            .entries
            .iter()
            .map(|(name, entry)| {
                (
                    name.as_str(),
                    EntryProxy {
                        ida: entry.ida.clone(),
                    },
                )
            })
            .collect();
        serde_json::to_string_pretty(&map)
    }

    /// Deserialize a database from a JSON string produced by
    /// [`to_json`](Self::to_json).
    ///
    /// Entries whose `"ida"` value is `"(custom)"` are skipped because they
    /// cannot be reconstructed without the original byte data.
    fn parse_ida_pattern(ida: &str) -> Result<Pattern, serde_json::Error> {
        if ida.trim().is_empty() {
            return Err(serde_json::Error::custom("IDA pattern must not be empty"));
        }

        for token in ida.split_whitespace() {
            let is_wildcard = token == "?" || token == "??";
            let is_hex_byte =
                token.len() == 2 && token.as_bytes().iter().all(|b| b.is_ascii_hexdigit());

            if !is_wildcard && !is_hex_byte {
                return Err(serde_json::Error::custom(format!(
                    "invalid IDA token '{token}'"
                )));
            }
        }

        Ok(Pattern::from_ida(ida))
    }

    /// # Errors
    ///
    /// Returns a [`serde_json::Error`] if the JSON is malformed or if an
    /// entry contains an invalid IDA pattern string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let map: HashMap<String, EntryProxy> = serde_json::from_str(json)?;

        let mut entries = HashMap::with_capacity(map.len());
        for (name, proxy) in map {
            if proxy.ida == "(custom)" {
                continue;
            }

            let pattern = Self::parse_ida_pattern(&proxy.ida).map_err(|_| {
                serde_json::Error::custom(format!("invalid IDA pattern for entry '{name}'"))
            })?;

            entries.insert(
                name,
                Entry {
                    pattern,
                    ida: proxy.ida,
                },
            );
        }

        Ok(Self { entries })
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── insert / get / remove ─────────────────────────────────────────────────

    #[test]
    fn insert_ida_and_get() {
        let mut db = PatternDb::new();
        db.insert_ida("foo", "48 8B 05 ?? ?? ?? ??");
        assert!(db.get("foo").is_some());
        assert!(db.get("bar").is_none());
    }

    #[test]
    fn insert_pattern_and_get() {
        let mut db = PatternDb::new();
        db.insert("raw".to_string(), Pattern::from_ida("48 8B"));
        assert!(db.get("raw").is_some());
    }

    #[test]
    fn field_displacement_scan_entries_include_hp_current_accessor() {
        let entries = field_displacement_scan_entries();
        let hp_current = entries
            .iter()
            .find(|entry| entry.name == "hpCurrent")
            .expect("hpCurrent field scan entry missing");

        assert_eq!(hp_current.module, ScanModule::EqGame);
        assert_eq!(hp_current.category, OffsetCategory::PlayerZoneField);
        assert_eq!(hp_current.pattern, "8B 81 ?? ?? 00 00 3B 81");
        assert_eq!(
            hp_current.resolve,
            ResolveMode::ExtractDisplacement {
                disp_offset: 2,
                disp_size: 4,
            }
        );
        assert_eq!(
            hp_current.expected_preferred,
            Some(crate::offsets::player_zone::HP_CURRENT as u64)
        );
    }

    #[test]
    fn placeholder_scan_pattern_detects_stub_patterns() {
        assert!(is_placeholder_scan_pattern("?? ?? ?? ??"));
        assert!(is_placeholder_scan_pattern("? ?? ?"));
        assert!(is_placeholder_scan_pattern("CC CC CC CC"));
        assert!(is_placeholder_scan_pattern("cc Cc cC"));
    }

    #[test]
    fn placeholder_scan_pattern_rejects_real_patterns() {
        assert!(!is_placeholder_scan_pattern(""));
        assert!(!is_placeholder_scan_pattern("48 8B 05 ?? ?? ?? ??"));
        assert!(!is_placeholder_scan_pattern("CC 48 89 5C"));
    }

    #[test]
    fn built_in_scan_entries_have_verified_non_placeholder_patterns() {
        let entries = built_in_scan_entries();

        assert_eq!(entries.len(), 4);
        assert!(
            entries.iter().all(|entry| !entry.has_placeholder_pattern()),
            "built-in entries must not regress to all-wildcard or all-CC placeholders"
        );
    }

    #[test]
    fn insert_replaces_existing() {
        let mut db = PatternDb::new();
        db.insert_ida("key", "48 89");
        db.insert_ida("key", "FF D0");

        // The second pattern (FF D0) should win
        let data = [0xFF_u8, 0xD0];
        let results = db.scan_all(&data);
        assert_eq!(results["key"], Some(0));
    }

    #[test]
    fn remove_existing() {
        let mut db = PatternDb::new();
        db.insert_ida("x", "90");
        assert!(db.remove("x").is_some());
        assert!(db.get("x").is_none());
    }

    #[test]
    fn remove_nonexistent() {
        let mut db = PatternDb::new();
        assert!(db.remove("ghost").is_none());
    }

    #[test]
    fn len_and_is_empty() {
        let mut db = PatternDb::new();
        assert!(db.is_empty());
        db.insert_ida("a", "90");
        assert_eq!(db.len(), 1);
        db.insert_ida("b", "CC");
        assert_eq!(db.len(), 2);
    }

    // ── scan_all ─────────────────────────────────────────────────────────────

    #[test]
    fn scan_all_hit_and_miss() {
        let mut db = PatternDb::new();
        db.insert_ida("present", "48 8B 05");
        db.insert_ida("absent", "FF D0 CC");

        let data = [0x00, 0x48, 0x8B, 0x05, 0x00];
        let results = db.scan_all(&data);

        assert_eq!(results["present"], Some(1));
        assert_eq!(results["absent"], None);
    }

    #[test]
    fn scan_all_empty_db() {
        let db = PatternDb::new();
        let results = db.scan_all(&[0x48, 0x8B]);
        assert!(results.is_empty());
    }

    #[test]
    fn scan_all_wildcards() {
        let mut db = PatternDb::new();
        db.insert_ida("wildcard", "48 8B 05 ?? ?? ?? ?? 48");

        let data = [0x48_u8, 0x8B, 0x05, 0xAA, 0xBB, 0xCC, 0xDD, 0x48];
        let results = db.scan_all(&data);
        assert_eq!(results["wildcard"], Some(0));
    }

    #[test]
    fn scan_matched_only_returns_hits() {
        let mut db = PatternDb::new();
        db.insert_ida("hit", "90");
        db.insert_ida("miss", "CC");

        let data = [0x90_u8; 4];
        let matched = db.scan_matched(&data);

        assert!(matched.contains_key("hit"));
        assert!(!matched.contains_key("miss"));
    }

    #[test]
    fn scan_all_multiple_patterns_all_hit() {
        let mut db = PatternDb::new();
        db.insert_ida("alpha", "48 8B");
        db.insert_ida("beta", "8B 05");

        // data: 48 8B 05 — alpha hits at 0, beta hits at 1
        let data = [0x48_u8, 0x8B, 0x05];
        let results = db.scan_all(&data);

        assert_eq!(results["alpha"], Some(0));
        assert_eq!(results["beta"], Some(1));
    }

    // ── JSON round-trip ───────────────────────────────────────────────────────

    #[test]
    fn json_round_trip_single_pattern() {
        let mut db = PatternDb::new();
        db.insert_ida("ProcessGameEvents", "48 89 5C 24 08 ?? ?? 48");

        let json = db.to_json().expect("serialization failed");
        let db2 = PatternDb::from_json(&json).expect("deserialization failed");

        assert_eq!(db2.len(), 1);
        assert!(db2.get("ProcessGameEvents").is_some());
    }

    #[test]
    fn json_round_trip_multiple_patterns() {
        let mut db = PatternDb::new();
        db.insert_ida("alpha", "48 8B 05");
        db.insert_ida("beta", "FF D0");
        db.insert_ida("gamma", "90 90 ?? CC");

        let json = db.to_json().expect("serialization failed");
        let db2 = PatternDb::from_json(&json).expect("deserialization failed");

        assert_eq!(db2.len(), 3);
        assert!(db2.get("alpha").is_some());
        assert!(db2.get("beta").is_some());
        assert!(db2.get("gamma").is_some());
    }

    #[test]
    fn json_round_trip_scan_produces_same_results() {
        let mut db = PatternDb::new();
        db.insert_ida("pattern", "48 8B ?? 05");

        let json = db.to_json().expect("serialization failed");
        let db2 = PatternDb::from_json(&json).expect("deserialization failed");

        let data = [0x48_u8, 0x8B, 0xAA, 0x05, 0x00];
        let r1 = db.scan_all(&data);
        let r2 = db2.scan_all(&data);

        assert_eq!(r1["pattern"], r2["pattern"]);
    }

    #[test]
    fn json_empty_db_round_trip() {
        let db = PatternDb::new();
        let json = db.to_json().expect("serialization failed");
        let db2 = PatternDb::from_json(&json).expect("deserialization failed");
        assert!(db2.is_empty());
    }

    #[test]
    fn from_json_invalid_input() {
        let result = PatternDb::from_json("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn from_json_invalid_ida_returns_error() {
        let json = r#"{"bad":{"ida":""}}"#;
        let result = PatternDb::from_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn from_json_skips_custom_entries() {
        let json = r#"{"custom_entry":{"ida":"(custom)"},"real_entry":{"ida":"48 8B"}}"#;
        let db = PatternDb::from_json(json).expect("deserialization failed");
        assert_eq!(db.len(), 1);
        assert!(db.get("real_entry").is_some());
        assert!(db.get("custom_entry").is_none());
    }

    #[test]
    fn scan_matched_empty_data() {
        let mut db = PatternDb::new();
        db.insert_ida("pat", "48 8B");
        let matched = db.scan_matched(&[]);
        assert!(matched.is_empty());
    }

    #[test]
    fn scan_all_single_byte_pattern() {
        let mut db = PatternDb::new();
        db.insert_ida("nop", "90");
        let data = [0xCC, 0x90, 0xCC];
        let results = db.scan_all(&data);
        assert_eq!(results["nop"], Some(1));
    }

    #[test]
    fn default_creates_empty_db() {
        let db = PatternDb::default();
        assert!(db.is_empty());
        assert_eq!(db.len(), 0);
    }

    #[test]
    fn insert_custom_pattern_stores_custom_ida() {
        let mut db = PatternDb::new();
        db.insert("custom".to_string(), Pattern::from_ida("90 90"));
        let json = db.to_json().expect("serialization failed");
        // Deserializing should skip the "(custom)" entry
        let db2 = PatternDb::from_json(&json).expect("deserialization failed");
        assert!(
            db2.is_empty(),
            "custom entries should be skipped on deserialization"
        );
    }

    #[test]
    fn from_json_rejects_invalid_hex_token() {
        let json = r#"{"bad":{"ida":"ZZ GG"}}"#;
        let result = PatternDb::from_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn scan_matched_empty_db() {
        let db = PatternDb::new();
        let matched = db.scan_matched(&[0x48, 0x8B]);
        assert!(matched.is_empty());
    }
}
