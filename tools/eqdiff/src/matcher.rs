//! Cross-patch matching and `offsets.json` update helpers.

use crate::StringRefMatch;
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// A normalized evidence record: one function-like code RVA references one string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringFunctionRef {
    /// Function or representative instruction RVA in the old/new PE image.
    pub function_rva: u32,
    /// Referenced string value.
    pub string: String,
}

impl StringFunctionRef {
    /// Build a string-reference evidence record.
    #[must_use]
    pub fn new(function_rva: u32, string: impl Into<String>) -> Self {
        Self {
            function_rva,
            string: string.into(),
        }
    }
}

/// A matched function RVA pair with confidence and the strings that supported it.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionMatch {
    /// RVA in the old binary.
    pub old_rva: u32,
    /// RVA in the new binary.
    pub new_rva: u32,
    /// Confidence from 0.0 to 1.0 based on shared string coverage.
    pub confidence: f32,
    /// Strings shared by the old and new function evidence.
    pub matched_strings: Vec<String>,
}

/// Convert string xref results into function/string evidence records.
#[must_use]
pub fn string_function_refs_from_xrefs(matches: &[StringRefMatch]) -> Vec<StringFunctionRef> {
    let mut refs = Vec::new();
    for entry in matches {
        for rva in &entry.referencing_rvas {
            refs.push(StringFunctionRef::new(*rva, entry.string.value.clone()));
        }
    }
    refs.sort_by(|left, right| {
        left.function_rva
            .cmp(&right.function_rva)
            .then_with(|| left.string.cmp(&right.string))
    });
    refs
}

/// Match old and new function RVAs by shared string-reference evidence.
///
/// This is the highest-confidence Phase 1 strategy for `eqdiff`: if code in
/// both binaries references the same distinctive string constants, the
/// referencing functions are likely equivalent after the patch. Ties are
/// resolved by the count of shared strings and only unambiguous best matches are
/// returned.
#[must_use]
pub fn match_functions_by_string_references(
    old_refs: &[StringFunctionRef],
    new_refs: &[StringFunctionRef],
) -> Vec<FunctionMatch> {
    let old_by_func = refs_by_function(old_refs);
    let new_by_func = refs_by_function(new_refs);
    let mut results = Vec::new();

    for (old_rva, old_strings) in old_by_func {
        let mut best: Option<(u32, BTreeSet<String>, f32)> = None;
        let mut best_tied = false;

        for (new_rva, new_strings) in &new_by_func {
            let shared: BTreeSet<String> = old_strings
                .intersection(new_strings)
                .cloned()
                .collect::<BTreeSet<_>>();
            if shared.is_empty() {
                continue;
            }

            let confidence = shared.len() as f32 / old_strings.len().max(new_strings.len()) as f32;
            match &best {
                None => {
                    best = Some((*new_rva, shared, confidence));
                    best_tied = false;
                }
                Some((_, best_shared, best_confidence)) => {
                    let shared_count = shared.len();
                    let best_count = best_shared.len();
                    if shared_count > best_count
                        || (shared_count == best_count && confidence > *best_confidence)
                    {
                        best = Some((*new_rva, shared, confidence));
                        best_tied = false;
                    } else if shared_count == best_count
                        && (confidence - *best_confidence).abs() < f32::EPSILON
                    {
                        best_tied = true;
                    }
                }
            }
        }

        if let Some((new_rva, matched_strings, confidence)) = best
            && !best_tied
        {
            results.push(FunctionMatch {
                old_rva,
                new_rva,
                confidence,
                matched_strings: matched_strings.into_iter().collect(),
            });
        }
    }

    results.sort_by_key(|entry| entry.old_rva);
    results
}

/// Apply function RVA matches to an `offset_db.rs`-compatible JSON value.
///
/// Function addresses are rebased from `old_preferred_base + old_rva` to
/// `new_preferred_base + new_rva`. Struct-layout offsets and unrelated globals
/// are preserved.
pub fn apply_matches_to_offsets_json(
    mut offsets: Value,
    old_preferred_base: u64,
    new_preferred_base: u64,
    matches: &[FunctionMatch],
) -> Result<Value> {
    let old_to_new: HashMap<u64, u64> = matches
        .iter()
        .map(|entry| {
            (
                old_preferred_base + u64::from(entry.old_rva),
                new_preferred_base + u64::from(entry.new_rva),
            )
        })
        .collect();

    let root = offsets
        .as_object_mut()
        .context("offsets JSON must be an object")?;
    root.insert(
        "eq_preferred_base".to_string(),
        Value::Number(new_preferred_base.into()),
    );

    let Some(functions) = root.get_mut("functions") else {
        return Ok(offsets);
    };
    let functions = functions
        .as_object_mut()
        .context("offsets JSON field 'functions' must be an object")?;

    for value in functions.values_mut() {
        let Some(old_addr) = value.as_u64() else {
            bail!("offsets JSON function addresses must be unsigned integers");
        };
        if let Some(new_addr) = old_to_new.get(&old_addr) {
            *value = Value::Number((*new_addr).into());
        }
    }

    Ok(offsets)
}

fn refs_by_function(refs: &[StringFunctionRef]) -> BTreeMap<u32, BTreeSet<String>> {
    let mut by_function: BTreeMap<u32, BTreeSet<String>> = BTreeMap::new();
    for reference in refs {
        by_function
            .entry(reference.function_rva)
            .or_default()
            .insert(reference.string.clone());
    }
    by_function
}
