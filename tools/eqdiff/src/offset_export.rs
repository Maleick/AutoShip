//! Export binary-diff function matches into TextQuest's offset database format.
//!
//! Binary diff stages operate on function names and preferred-base addresses.
//! This module maps those names back to TextQuest offset keys and returns a full
//! [`OffsetDatabase`] that can be written as `offsets.json` and loaded by
//! `textquest_common::offset_db::OffsetDatabase::load_from_file`.

use textquest_common::offset_db::OffsetDatabase;

/// A function-level binary diff match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryDiffFunction {
    /// Function name from the diff source, Ghidra export, or symbol table.
    pub name: String,
    /// Previous preferred-base address, when the old binary contained it.
    pub old_preferred: Option<u64>,
    /// New preferred-base address, when the new binary contains it.
    pub new_preferred: Option<u64>,
}

impl BinaryDiffFunction {
    /// Build a moved or unchanged function match.
    #[must_use]
    pub fn matched(name: impl Into<String>, old_preferred: u64, new_preferred: u64) -> Self {
        Self {
            name: name.into(),
            old_preferred: Some(old_preferred),
            new_preferred: Some(new_preferred),
        }
    }

    /// Build a function that appears only in the new binary.
    #[must_use]
    pub fn added(name: impl Into<String>, new_preferred: u64) -> Self {
        Self {
            name: name.into(),
            old_preferred: None,
            new_preferred: Some(new_preferred),
        }
    }

    /// Build a function that appears only in the old binary.
    #[must_use]
    pub fn removed(name: impl Into<String>, old_preferred: u64) -> Self {
        Self {
            name: name.into(),
            old_preferred: Some(old_preferred),
            new_preferred: None,
        }
    }
}

/// A TextQuest offset key updated from a binary diff match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedOffsetUpdate {
    /// TextQuest `OffsetDatabase.functions` key, such as `castSpell`.
    pub offset_key: String,
    /// Binary diff function name that produced this update.
    pub function_name: String,
    /// Previous preferred-base address, when available.
    pub old_preferred: Option<u64>,
    /// New preferred-base address written into the exported database.
    pub new_preferred: u64,
}

/// Human-readable summary data for a binary-diff offset export.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BinaryDiffOffsetReport {
    /// TextQuest offsets updated in the exported database.
    pub updated_offsets: Vec<ExportedOffsetUpdate>,
    /// Matched functions that moved between binaries.
    pub moved_functions: Vec<BinaryDiffFunction>,
    /// Functions present only in the new binary.
    pub added_functions: Vec<BinaryDiffFunction>,
    /// Functions present only in the old binary.
    pub removed_functions: Vec<BinaryDiffFunction>,
    /// New-address matches that did not map to a TextQuest offset key.
    pub unmapped_functions: Vec<BinaryDiffFunction>,
}

impl BinaryDiffOffsetReport {
    /// Render a compact Markdown diff report for PRs and patch-day logs.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Binary Diff Offset Report\n\n");
        out.push_str(&format!(
            "- Updated TextQuest offsets: {}\n",
            self.updated_offsets.len()
        ));
        out.push_str(&format!(
            "- Moved functions: {}\n",
            self.moved_functions.len()
        ));
        out.push_str(&format!(
            "- Added functions: {}\n",
            self.added_functions.len()
        ));
        out.push_str(&format!(
            "- Removed functions: {}\n",
            self.removed_functions.len()
        ));
        out.push_str(&format!(
            "- Unmapped new-address functions: {}\n",
            self.unmapped_functions.len()
        ));

        if !self.updated_offsets.is_empty() {
            out.push_str("\n## Updated Offsets\n\n");
            for update in &self.updated_offsets {
                let old = update
                    .old_preferred
                    .map(format_addr)
                    .unwrap_or_else(|| "new".to_string());
                out.push_str(&format!(
                    "- `{}` from `{}`: {} -> {}\n",
                    update.offset_key,
                    update.function_name,
                    old,
                    format_addr(update.new_preferred)
                ));
            }
        }

        append_function_section(&mut out, "Moved Functions", &self.moved_functions);
        append_function_section(&mut out, "Added Functions", &self.added_functions);
        append_function_section(&mut out, "Removed Functions", &self.removed_functions);
        append_function_section(&mut out, "Unmapped Functions", &self.unmapped_functions);

        out
    }
}

/// Result of exporting binary-diff matches into TextQuest offset data.
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryDiffOffsetExport {
    /// Updated database ready for `OffsetDatabase::save_to_file`.
    pub offsets: OffsetDatabase,
    /// Summary of what changed and what could not be mapped.
    pub report: BinaryDiffOffsetReport,
}

/// Export binary-diff matches into an [`OffsetDatabase`].
///
/// `base_db` is cloned so unmentioned globals, struct offsets, and functions are
/// preserved. Matches with a `new_preferred` address are mapped into
/// `OffsetDatabase.functions` when their name contains a known TextQuest function
/// key after case/punctuation normalization.
#[must_use]
pub fn export_offsets_from_binary_diff(
    base_db: &OffsetDatabase,
    matches: &[BinaryDiffFunction],
    client_date: Option<&str>,
) -> BinaryDiffOffsetExport {
    let mut offsets = base_db.clone();
    if let Some(client_date) = client_date {
        offsets.client_date = client_date.to_string();
    }

    let mut report = BinaryDiffOffsetReport::default();
    let function_keys = sorted_function_keys(base_db);

    for function in matches {
        match (function.old_preferred, function.new_preferred) {
            (Some(old), Some(new)) if old != new => report.moved_functions.push(function.clone()),
            (None, Some(_)) => report.added_functions.push(function.clone()),
            (Some(_), None) => report.removed_functions.push(function.clone()),
            _ => {}
        }

        let Some(new_preferred) = function.new_preferred else {
            continue;
        };

        let Some(offset_key) = map_function_to_offset_key_with_keys(&function.name, &function_keys)
        else {
            report.unmapped_functions.push(function.clone());
            continue;
        };

        offsets.functions.insert(offset_key.clone(), new_preferred);
        report.updated_offsets.push(ExportedOffsetUpdate {
            offset_key,
            function_name: function.name.clone(),
            old_preferred: function.old_preferred,
            new_preferred,
        });
    }

    BinaryDiffOffsetExport { offsets, report }
}

/// Map a binary-diff function name to a TextQuest offset function key.
#[must_use]
pub fn map_function_to_offset_key(function_name: &str, db: &OffsetDatabase) -> Option<String> {
    map_function_to_offset_key_with_keys(function_name, &sorted_function_keys(db))
}

fn map_function_to_offset_key_with_keys(
    function_name: &str,
    function_keys: &[(String, String)],
) -> Option<String> {
    let normalized_name = normalize_identifier(function_name);
    function_keys.iter().find_map(|(normalized_key, key)| {
        normalized_name
            .contains(normalized_key)
            .then(|| key.clone())
    })
}

fn sorted_function_keys(db: &OffsetDatabase) -> Vec<(String, String)> {
    let mut normalized_to_key: Vec<(String, String)> = db
        .functions
        .keys()
        .map(|key| (normalize_identifier(key), key.clone()))
        .collect();
    normalized_to_key.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.1.cmp(&b.1)));
    normalized_to_key
}

fn normalize_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn append_function_section(out: &mut String, title: &str, functions: &[BinaryDiffFunction]) {
    if functions.is_empty() {
        return;
    }

    out.push_str(&format!("\n## {title}\n\n"));
    for function in functions {
        let old = function
            .old_preferred
            .map(format_addr)
            .unwrap_or_else(|| "missing".to_string());
        let new = function
            .new_preferred
            .map(format_addr)
            .unwrap_or_else(|| "missing".to_string());
        out.push_str(&format!("- `{}`: {} -> {}\n", function.name, old, new));
    }
}

fn format_addr(addr: u64) -> String {
    format!("0x{addr:016X}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_function_containing_textquest_offset_key() {
        let db = OffsetDatabase::from_compiled_offsets();

        assert_eq!(
            map_function_to_offset_key("CharacterZoneClient::CastSpell", &db),
            Some("castSpell".to_string())
        );
    }

    #[test]
    fn exports_updated_offset_database_for_mapped_matches() {
        let db = OffsetDatabase::from_compiled_offsets();
        let new_cast_spell = db.get_function("castSpell").unwrap() + 0x120;
        let matches = [BinaryDiffFunction::matched(
            "CharacterZoneClient::CastSpell",
            db.get_function("castSpell").unwrap(),
            new_cast_spell,
        )];

        let export = export_offsets_from_binary_diff(&db, &matches, Some("20260415"));

        assert_eq!(export.offsets.client_date, "20260415");
        assert_eq!(
            export.offsets.get_function("castSpell"),
            Some(new_cast_spell)
        );
        assert_eq!(
            export.offsets.get_function("useSkill"),
            db.get_function("useSkill")
        );
        assert_eq!(export.report.updated_offsets.len(), 1);
        assert_eq!(export.report.moved_functions, matches);
    }

    #[test]
    fn reports_added_removed_and_unmapped_functions() {
        let db = OffsetDatabase::from_compiled_offsets();
        let matches = [
            BinaryDiffFunction::added("NewQuestHelper", 0x1400_1234),
            BinaryDiffFunction::removed("OldQuestHelper", 0x1400_5678),
        ];

        let export = export_offsets_from_binary_diff(&db, &matches, None);

        assert!(export.report.updated_offsets.is_empty());
        assert_eq!(export.report.added_functions, vec![matches[0].clone()]);
        assert_eq!(export.report.removed_functions, vec![matches[1].clone()]);
        assert_eq!(export.report.unmapped_functions, vec![matches[0].clone()]);
    }

    #[test]
    fn markdown_report_includes_updated_offsets_and_summary() {
        let report = BinaryDiffOffsetReport {
            updated_offsets: vec![ExportedOffsetUpdate {
                offset_key: "castSpell".to_string(),
                function_name: "CharacterZoneClient::CastSpell".to_string(),
                old_preferred: Some(0x1400_1000),
                new_preferred: 0x1400_1100,
            }],
            moved_functions: vec![BinaryDiffFunction::matched(
                "CharacterZoneClient::CastSpell",
                0x1400_1000,
                0x1400_1100,
            )],
            ..BinaryDiffOffsetReport::default()
        };

        let markdown = report.to_markdown();

        assert!(markdown.contains("Updated TextQuest offsets: 1"));
        assert!(markdown.contains("`castSpell`"));
        assert!(markdown.contains("0x0000000014001100"));
    }
}
