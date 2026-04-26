//! Search and fuzzy matching for loaded help content.

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use strsim::{jaro_winkler, normalized_levenshtein};

use super::{HelpCategory, HelpCommand, HelpDatabase, HelpFaq, HelpTip};

const DEFAULT_SEARCH_LIMIT: usize = 20;
const SEARCH_CACHE_LIMIT: usize = 64;
const FUZZY_MIN_LEN: usize = 3;
const FUZZY_THRESHOLD: f64 = 0.82;

pub(super) type CommandSearchIndex = HashMap<String, Vec<usize>>;
pub(super) type SearchCache = Arc<Mutex<HashMap<SearchCacheKey, Vec<SearchResult>>>>;

/// Structured search query for help content.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchQuery {
    /// Free-text terms. Values may contain whitespace; they are tokenized before
    /// matching.
    pub terms: Vec<String>,
    /// Optional category filter.
    pub category: Option<HelpCategory>,
    /// Required tags. All provided tags must be present on an item.
    pub tags: Vec<String>,
}

/// Help entry type returned by search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HelpItemType {
    /// Slash-command help.
    Command,
    /// Frequently asked question.
    Faq,
    /// Contextual operator tip.
    Tip,
}

/// Ranked search result.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    /// Stable item identifier.
    pub item_id: String,
    /// Help item type.
    pub item_type: HelpItemType,
    /// Operator-facing title.
    pub title: String,
    /// Higher scores are more relevant.
    pub match_score: f32,
    /// Fields that contributed to the match.
    pub matched_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct SearchCacheKey {
    query: String,
    limit: usize,
}

struct SearchField {
    name: &'static str,
    text: String,
    weight: f32,
}

pub(super) fn index_command(
    index: &mut CommandSearchIndex,
    command_index: usize,
    command: &HelpCommand,
) {
    let mut indexed_terms = HashSet::new();
    for term in searchable_terms(&command.name)
        .into_iter()
        .chain(searchable_terms(&command.usage))
        .chain(searchable_terms(&command.description))
        .chain(
            command
                .aliases
                .iter()
                .flat_map(|value| searchable_terms(value)),
        )
        .chain(
            command
                .examples
                .iter()
                .flat_map(|value| searchable_terms(value)),
        )
        .chain(
            command
                .tags
                .iter()
                .flat_map(|value| searchable_terms(value)),
        )
    {
        if indexed_terms.insert(term.clone()) {
            index.entry(term).or_default().push(command_index);
        }
    }
}

impl HelpDatabase {
    /// Search help content with a plain text query.
    ///
    /// Empty queries return no results. Passing `0` as `limit` uses the default
    /// search limit of 20 results.
    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let terms = searchable_terms(query);
        if terms.is_empty() {
            return Vec::new();
        }

        let limit = effective_limit(limit);
        let cache_key = SearchCacheKey {
            query: terms.join(" "),
            limit,
        };
        if let Some(results) = self.cached_search(&cache_key) {
            return results;
        }

        let query = SearchQuery {
            terms,
            category: None,
            tags: Vec::new(),
        };
        let mut results = self.search_unlimited(query);
        results.truncate(limit);
        self.store_search(cache_key, &results);
        results
    }

    /// Search help content with text, category, and tag filters.
    ///
    /// Results are limited to 20 entries.
    #[must_use]
    pub fn search_advanced(&self, query: SearchQuery) -> Vec<SearchResult> {
        let mut results = self.search_unlimited(query);
        results.truncate(DEFAULT_SEARCH_LIMIT);
        results
    }

    fn search_unlimited(&self, query: SearchQuery) -> Vec<SearchResult> {
        let terms = normalize_terms(query.terms);
        let tags = normalize_terms(query.tags);
        if terms.is_empty() && tags.is_empty() && query.category.is_none() {
            return Vec::new();
        }

        let command_index_hits = self.command_index_hits(&terms);
        let mut results = Vec::new();

        for (index, command) in self.commands.iter().enumerate() {
            if let Some(category) = &query.category
                && *category != HelpCategory::Command
            {
                continue;
            }
            if !matches_tags(&command.tags, &tags) {
                continue;
            }
            if let Some(result) = score_command(
                command,
                &terms,
                &tags,
                query.category.is_some(),
                command_index_hits.get(&index).copied().unwrap_or_default(),
            ) {
                results.push(result);
            }
        }

        for faq in &self.faqs {
            if let Some(category) = &query.category
                && *category != HelpCategory::Faq
            {
                continue;
            }
            if !matches_tags(&faq.tags, &tags) {
                continue;
            }
            if let Some(result) = score_faq(faq, &terms, &tags, query.category.is_some()) {
                results.push(result);
            }
        }

        for tip in &self.tips {
            if let Some(category) = &query.category
                && *category != HelpCategory::Tip
            {
                continue;
            }
            if !matches_tags(&tip.tags, &tags) {
                continue;
            }
            if let Some(result) = score_tip(tip, &terms, &tags, query.category.is_some()) {
                results.push(result);
            }
        }

        results.sort_by(rank_results);
        results
    }

    fn command_index_hits(&self, terms: &[String]) -> HashMap<usize, usize> {
        let mut hits = HashMap::new();
        for term in terms {
            if let Some(indices) = self.command_search_index.get(term) {
                for index in indices {
                    *hits.entry(*index).or_insert(0) += 1;
                }
            }
        }
        hits
    }

    fn cached_search(&self, key: &SearchCacheKey) -> Option<Vec<SearchResult>> {
        self.search_cache.lock().ok()?.get(key).cloned()
    }

    fn store_search(&self, key: SearchCacheKey, results: &[SearchResult]) {
        let Ok(mut cache) = self.search_cache.lock() else {
            return;
        };
        if cache.len() >= SEARCH_CACHE_LIMIT
            && let Some(oldest_key) = cache.keys().next().cloned()
        {
            cache.remove(&oldest_key);
        }
        cache.insert(key, results.to_vec());
    }
}

fn score_command(
    command: &HelpCommand,
    terms: &[String],
    required_tags: &[String],
    category_matched: bool,
    indexed_hits: usize,
) -> Option<SearchResult> {
    let fields = vec![
        SearchField {
            name: "title",
            text: command.name.clone(),
            weight: 6.0,
        },
        SearchField {
            name: "aliases",
            text: command.aliases.join(" "),
            weight: 5.0,
        },
        SearchField {
            name: "usage",
            text: command.usage.clone(),
            weight: 3.0,
        },
        SearchField {
            name: "description",
            text: command.description.clone(),
            weight: 3.5,
        },
        SearchField {
            name: "examples",
            text: command.examples.join(" "),
            weight: 2.5,
        },
        SearchField {
            name: "tags",
            text: command.tags.join(" "),
            weight: 4.0,
        },
    ];
    score_fields(
        command.name.clone(),
        HelpItemType::Command,
        command.name.clone(),
        fields,
        terms,
        required_tags,
        category_matched,
        indexed_hits,
    )
}

fn score_faq(
    faq: &HelpFaq,
    terms: &[String],
    required_tags: &[String],
    category_matched: bool,
) -> Option<SearchResult> {
    let fields = vec![
        SearchField {
            name: "title",
            text: faq.question.clone(),
            weight: 5.0,
        },
        SearchField {
            name: "answer",
            text: faq.answer.clone(),
            weight: 3.0,
        },
        SearchField {
            name: "tags",
            text: faq.tags.join(" "),
            weight: 4.0,
        },
        SearchField {
            name: "id",
            text: faq.id.clone(),
            weight: 4.0,
        },
    ];
    score_fields(
        faq.id.clone(),
        HelpItemType::Faq,
        faq.question.clone(),
        fields,
        terms,
        required_tags,
        category_matched,
        0,
    )
}

fn score_tip(
    tip: &HelpTip,
    terms: &[String],
    required_tags: &[String],
    category_matched: bool,
) -> Option<SearchResult> {
    let fields = vec![
        SearchField {
            name: "title",
            text: tip.text.lines().next().unwrap_or(&tip.text).to_string(),
            weight: 5.0,
        },
        SearchField {
            name: "text",
            text: tip.text.clone(),
            weight: 4.0,
        },
        SearchField {
            name: "context",
            text: tip.context.clone(),
            weight: 3.0,
        },
        SearchField {
            name: "tags",
            text: tip.tags.join(" "),
            weight: 4.0,
        },
        SearchField {
            name: "id",
            text: tip.id.clone(),
            weight: 4.0,
        },
    ];
    score_fields(
        tip.id.clone(),
        HelpItemType::Tip,
        tip.text.lines().next().unwrap_or(&tip.text).to_string(),
        fields,
        terms,
        required_tags,
        category_matched,
        0,
    )
}

#[allow(clippy::too_many_arguments)]
fn score_fields(
    item_id: String,
    item_type: HelpItemType,
    title: String,
    fields: Vec<SearchField>,
    terms: &[String],
    required_tags: &[String],
    category_matched: bool,
    indexed_hits: usize,
) -> Option<SearchResult> {
    let mut score = 0.0;
    let mut matched_terms = 0;
    let mut matched_fields = Vec::new();

    if category_matched {
        score += 1.0;
        push_matched_field(&mut matched_fields, "category");
    }
    if !required_tags.is_empty() {
        score += required_tags.len() as f32 * 2.0;
        push_matched_field(&mut matched_fields, "tags");
    }

    for term in terms {
        let mut best = None;
        for field in &fields {
            if let Some(field_score) = field_match_score(term, field) {
                let replace = best
                    .as_ref()
                    .is_none_or(|(best_score, _)| field_score > *best_score);
                if replace {
                    best = Some((field_score, field.name));
                }
            }
        }

        if let Some((field_score, field_name)) = best {
            matched_terms += 1;
            score += field_score;
            push_matched_field(&mut matched_fields, field_name);
        }
    }

    if !terms.is_empty() && matched_terms == 0 {
        return None;
    }

    if !terms.is_empty() {
        let coverage = matched_terms as f32 / terms.len() as f32;
        score *= coverage;
        if matched_terms == terms.len() {
            score += 2.0;
        }
    }

    score += indexed_hits as f32 * 1.25;
    if score <= 0.0 {
        return None;
    }

    Some(SearchResult {
        item_id,
        item_type,
        title,
        match_score: score,
        matched_fields,
    })
}

fn field_match_score(term: &str, field: &SearchField) -> Option<f32> {
    let normalized = normalize_text(&field.text);
    if normalized.is_empty() {
        return None;
    }

    if normalized == term {
        return Some(field.weight + 4.0);
    }
    if normalized.split_whitespace().any(|word| word == term) {
        return Some(field.weight + 2.0);
    }
    if normalized
        .split_whitespace()
        .any(|word| word.starts_with(term))
    {
        return Some(field.weight + 1.0);
    }
    if normalized.contains(term) {
        return Some(field.weight);
    }

    fuzzy_match_score(term, &normalized).map(|similarity| field.weight * similarity as f32 * 0.75)
}

fn fuzzy_match_score(term: &str, normalized_field: &str) -> Option<f64> {
    if term.len() < FUZZY_MIN_LEN {
        return None;
    }

    normalized_field
        .split_whitespace()
        .filter(|word| word.len() >= FUZZY_MIN_LEN)
        .map(|word| normalized_levenshtein(term, word).max(jaro_winkler(term, word)))
        .filter(|score| *score >= FUZZY_THRESHOLD)
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal))
}

fn matches_tags(item_tags: &[String], required_tags: &[String]) -> bool {
    required_tags.iter().all(|required| {
        item_tags.iter().any(|tag| {
            normalize_text(tag)
                .split_whitespace()
                .any(|term| term == required)
        })
    })
}

fn normalize_terms(values: Vec<String>) -> Vec<String> {
    values
        .iter()
        .flat_map(|value| searchable_terms(value))
        .collect()
}

fn searchable_terms(value: &str) -> Vec<String> {
    normalize_text(value)
        .split_whitespace()
        .map(ToOwned::to_owned)
        .collect()
}

fn normalize_text(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
}

fn effective_limit(limit: usize) -> usize {
    if limit == 0 {
        DEFAULT_SEARCH_LIMIT
    } else {
        limit
    }
}

fn push_matched_field(fields: &mut Vec<String>, field: &str) {
    if !fields.iter().any(|existing| existing == field) {
        fields.push(field.to_string());
    }
}

fn rank_results(a: &SearchResult, b: &SearchResult) -> Ordering {
    b.match_score
        .partial_cmp(&a.match_score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| {
            a.title
                .to_ascii_lowercase()
                .cmp(&b.title.to_ascii_lowercase())
        })
        .then_with(|| a.item_id.cmp(&b.item_id))
}
