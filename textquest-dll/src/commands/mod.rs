//! Slash command registry for TextQuest scripts and plugins.
//!
//! Supports commands of the form `/command subcommand arg1 arg2 ...` with
//! longest-prefix routing, variadic arguments, per-command help text, and
//! argument validation.
//!
//! # Example
//!
//! ```
//! use textquest_dll::commands::{CommandRegistry, ArgType, CommandResult};
//!
//! let mut registry = CommandRegistry::new();
//!
//! registry.register(
//!     "/nav",
//!     "Navigation commands",
//!     &[],
//!     false,
//!     |_args| {
//!         println!("nav base command");
//!         CommandResult::Ok
//!     },
//! );
//!
//! registry.register(
//!     "/nav waypoint",
//!     "Navigate to a named waypoint",
//!     &[ArgType::String],
//!     false,
//!     |args| {
//!         println!("navigating to: {}", args[0]);
//!         CommandResult::Ok
//!     },
//! );
//!
//! let result = registry.dispatch("/nav waypoint home");
//! assert!(matches!(result, CommandResult::Ok));
//! ```

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

const COMMAND_HELP_TOML: &str = include_str!("../../../config/help/commands.toml");
const FAQ_HELP_TOML: &str = include_str!("../../../config/help/faq.toml");

/// Global slash-command registry used by DLL command hooks, Lua scripts, and
/// plugin bridges.
static GLOBAL_REGISTRY: OnceLock<Mutex<CommandRegistry>> = OnceLock::new();
static HELP_DATABASE: OnceLock<Result<HelpDatabase, String>> = OnceLock::new();
static DEBUG_MODE_ENABLED: AtomicBool = AtomicBool::new(false);
static COMMAND_TRACING_ENABLED: AtomicBool = AtomicBool::new(false);

/// Returns the process-wide command registry.
pub fn global() -> &'static Mutex<CommandRegistry> {
    GLOBAL_REGISTRY.get_or_init(|| Mutex::new(CommandRegistry::new()))
}

/// Register a script/plugin command with variadic string arguments.
///
/// This is the Rust-side entrypoint backing future Lua calls such as
/// `textquest.register_command('/mycommand', function(args) ... end)`.
pub fn register_script_command<F>(path: &str, callback: F) -> bool
where
    F: Fn(&[&str]) -> CommandResult + Send + Sync + 'static,
{
    let help = format!("Script/plugin command registered at {path}");
    match global().lock() {
        Ok(mut registry) => registry.register(path, &help, &[], true, callback),
        Err(err) => {
            tracing::error!(path, error = %err, "global command registry lock poisoned");
            false
        }
    }
}

/// Dispatch a raw slash command through the global registry.
///
/// DLL command hooks should call this after EQ has accepted a command line.
pub fn dispatch_global_command(input: &str) -> CommandResult {
    match global().lock() {
        Ok(registry) => registry.dispatch(input),
        Err(err) => {
            tracing::error!(input, error = %err, "global command registry lock poisoned");
            CommandResult::Error("TextQuest command registry unavailable".to_string())
        }
    }
}

/// Returns whether runtime debug diagnostics are enabled.
#[must_use]
pub fn debug_mode_enabled() -> bool {
    DEBUG_MODE_ENABLED.load(Ordering::Relaxed)
}

/// Returns whether runtime command tracing is enabled.
#[must_use]
pub fn command_tracing_enabled() -> bool {
    COMMAND_TRACING_ENABLED.load(Ordering::Relaxed)
}

// ─── Trace feature ────────────────────────────────────────────────────────────
//
// Heavy per-dispatch tracing is gated behind the `trace-commands` Cargo feature.
// When enabled, a `tracing::trace!` span fires at every dispatch boundary with
// structured fields: `cmd_path`, `args_len`, and `result`.  The existing
// `tracing::info!` / `tracing::debug!` calls remain unconditional and cover the
// "command executed" and "not found" cases at a lower volume.
//
// Enable with: cargo build --features textquest-dll/trace-commands

/// Type alias for a boxed command handler callback.
pub type CommandCallback = Box<dyn Fn(&[&str]) -> CommandResult + Send + Sync>;

/// Result returned by a command handler or dispatch.
#[derive(Debug, PartialEq, Eq)]
pub enum CommandResult {
    /// Command executed successfully.
    Ok,
    /// Command executed but produced a user-visible message.
    Message(String),
    /// Command failed with an error message.
    Error(String),
    /// No command matched the input.
    NotFound,
    /// Argument validation failed.
    ValidationError(String),
}

/// Argument type for validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgType {
    /// Any non-empty string.
    String,
    /// A valid `i64` integer.
    Int,
    /// A valid `f64` floating-point number.
    Float,
    /// "true", "false", "1", or "0" (case-insensitive).
    Bool,
}

impl ArgType {
    /// Validate a string argument against this type.
    fn validate(&self, value: &str) -> bool {
        match self {
            ArgType::String => !value.is_empty(),
            ArgType::Int => value.parse::<i64>().is_ok(),
            ArgType::Float => value.parse::<f64>().is_ok(),
            ArgType::Bool => matches!(value.to_lowercase().as_str(), "true" | "false" | "1" | "0"),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            ArgType::String => "string",
            ArgType::Int => "int",
            ArgType::Float => "float",
            ArgType::Bool => "bool",
        }
    }
}

#[derive(Debug, Deserialize)]
struct CommandHelpFile {
    #[serde(default)]
    commands: Vec<HelpCommand>,
}

#[derive(Debug, Deserialize)]
struct FaqHelpFile {
    #[serde(default)]
    faqs: Vec<FaqEntry>,
    #[serde(default)]
    tips: Vec<TipEntry>,
}

#[derive(Debug, Deserialize)]
struct HelpCommand {
    name: String,
    #[serde(default)]
    aliases: Vec<String>,
    usage: String,
    description: String,
    #[serde(default)]
    examples: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FaqEntry {
    id: String,
    question: String,
    answer: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TipEntry {
    id: String,
    text: String,
    context: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug)]
struct HelpDatabase {
    commands: Vec<HelpCommand>,
    faqs: Vec<FaqEntry>,
    tips: Vec<TipEntry>,
    search_index: Vec<HelpSearchDocument>,
}

impl HelpDatabase {
    fn load() -> Result<Self, String> {
        let command_file = toml::from_str::<CommandHelpFile>(COMMAND_HELP_TOML)
            .map_err(|err| format!("command help TOML parse failed: {err}"))?;
        let faq_file = toml::from_str::<FaqHelpFile>(FAQ_HELP_TOML)
            .map_err(|err| format!("FAQ help TOML parse failed: {err}"))?;

        let commands = command_file.commands;
        let faqs = faq_file.faqs;
        let tips = faq_file.tips;
        let search_index = build_help_search_index(&commands, &faqs, &tips);

        Ok(Self {
            commands,
            faqs,
            tips,
            search_index,
        })
    }

    fn command(&self, lookup: &str) -> Option<&HelpCommand> {
        let lookup = normalize_help_lookup(lookup);
        self.commands
            .iter()
            .find(|command| command.matches_name(&lookup))
    }

    fn command_matches(&self, query: &str) -> Vec<&HelpCommand> {
        self.search(&HelpSearchQuery::new(query).with_category(HelpSearchCategory::Command))
            .into_iter()
            .filter_map(|result| self.command_result(&result))
            .collect()
    }

    fn faq_matches(&self, query: &str) -> Vec<&FaqEntry> {
        self.search(&HelpSearchQuery::new(query).with_category(HelpSearchCategory::Faq))
            .into_iter()
            .filter_map(|result| self.faq_result(&result))
            .collect()
    }

    fn tip_matches(&self, query: &str) -> Vec<&TipEntry> {
        self.search(&HelpSearchQuery::new(query).with_category(HelpSearchCategory::Tip))
            .into_iter()
            .filter_map(|result| self.tip_result(&result))
            .collect()
    }

    fn config_faqs(&self) -> Vec<&FaqEntry> {
        self.faqs
            .iter()
            .filter(|faq| {
                faq.tags.iter().any(|tag| {
                    matches!(
                        normalized_search(tag).as_str(),
                        "setup" | "config" | "class" | "camp"
                    )
                }) || faq.matches_search("config")
                    || faq.matches_search("camp")
            })
            .collect()
    }

    fn command_result(&self, result: &HelpSearchResult) -> Option<&HelpCommand> {
        (result.category == HelpSearchCategory::Command)
            .then(|| self.commands.get(result.item_index))
            .flatten()
    }

    fn faq_result(&self, result: &HelpSearchResult) -> Option<&FaqEntry> {
        (result.category == HelpSearchCategory::Faq)
            .then(|| self.faqs.get(result.item_index))
            .flatten()
    }

    fn tip_result(&self, result: &HelpSearchResult) -> Option<&TipEntry> {
        (result.category == HelpSearchCategory::Tip)
            .then(|| self.tips.get(result.item_index))
            .flatten()
    }

    fn search_result_title(&self, result: &HelpSearchResult) -> &str {
        match result.category {
            HelpSearchCategory::Command => self
                .commands
                .get(result.item_index)
                .map(|command| command.name.as_str())
                .unwrap_or_default(),
            HelpSearchCategory::Faq => self
                .faqs
                .get(result.item_index)
                .map(|faq| faq.question.as_str())
                .unwrap_or_default(),
            HelpSearchCategory::Tip => self
                .tips
                .get(result.item_index)
                .map(|tip| tip.text.as_str())
                .unwrap_or_default(),
        }
    }
}

trait HelpSearch {
    fn search(&self, query: &HelpSearchQuery) -> Vec<HelpSearchResult>;
}

impl HelpSearch for HelpDatabase {
    fn search(&self, query: &HelpSearchQuery) -> Vec<HelpSearchResult> {
        let terms = search_terms(&query.text);
        let mut results = self
            .search_index
            .iter()
            .filter(|document| query.matches_category(document.category))
            .filter(|document| document.matches_tags(&query.tags))
            .filter_map(|document| {
                document.score_terms(&terms).map(|score| HelpSearchResult {
                    category: document.category,
                    item_index: document.item_index,
                    score,
                })
            })
            .collect::<Vec<_>>();

        results.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.category.sort_order().cmp(&right.category.sort_order()))
                .then_with(|| {
                    self.search_result_title(left)
                        .cmp(self.search_result_title(right))
                })
        });

        if query.limit > 0 && results.len() > query.limit {
            results.truncate(query.limit);
        }

        results
    }
}

#[derive(Debug, Clone)]
struct HelpSearchQuery {
    text: String,
    categories: Vec<HelpSearchCategory>,
    tags: Vec<String>,
    limit: usize,
}

impl HelpSearchQuery {
    fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            text: normalized_search(&text),
            categories: Vec::new(),
            tags: Vec::new(),
            limit: 24,
        }
    }

    fn with_category(mut self, category: HelpSearchCategory) -> Self {
        self.add_category(category);
        self
    }

    fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.text = normalized_search(&text);
    }

    fn add_category(&mut self, category: HelpSearchCategory) {
        if !self.categories.contains(&category) {
            self.categories.push(category);
        }
    }

    fn add_tag(&mut self, tag: &str) {
        let tag = normalized_search(tag);
        if !tag.is_empty() && !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    fn matches_category(&self, category: HelpSearchCategory) -> bool {
        self.categories.is_empty() || self.categories.contains(&category)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HelpSearchCategory {
    Command,
    Faq,
    Tip,
}

impl HelpSearchCategory {
    fn parse(value: &str) -> Option<Self> {
        match normalized_search(value).as_str() {
            "command" | "commands" | "cmd" | "cmds" => Some(Self::Command),
            "faq" | "faqs" => Some(Self::Faq),
            "tip" | "tips" => Some(Self::Tip),
            _ => None,
        }
    }

    fn sort_order(self) -> u8 {
        match self {
            Self::Command => 0,
            Self::Faq => 1,
            Self::Tip => 2,
        }
    }
}

#[derive(Debug, Clone)]
struct HelpSearchResult {
    category: HelpSearchCategory,
    item_index: usize,
    score: u16,
}

#[derive(Debug)]
struct HelpSearchDocument {
    category: HelpSearchCategory,
    item_index: usize,
    title: String,
    tags: Vec<String>,
    search_text: String,
    compact_text: String,
    words: Vec<String>,
}

impl HelpSearchDocument {
    fn new(
        category: HelpSearchCategory,
        item_index: usize,
        title: &str,
        fields: Vec<&str>,
        tags: &[String],
    ) -> Self {
        let title = normalized_search(title);
        let tags = tags
            .iter()
            .map(|tag| normalized_search(tag))
            .filter(|tag| !tag.is_empty())
            .collect::<Vec<_>>();

        let mut search_text = String::new();
        for field in fields {
            append_normalized_field(&mut search_text, field);
        }
        for tag in &tags {
            append_normalized_field(&mut search_text, tag);
        }

        let mut words = search_words(&search_text);
        words.sort();
        words.dedup();

        let compact_text = search_text
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .collect();

        Self {
            category,
            item_index,
            title,
            tags,
            search_text,
            compact_text,
            words,
        }
    }

    fn matches_tags(&self, filters: &[String]) -> bool {
        filters.iter().all(|filter| {
            self.tags
                .iter()
                .any(|tag| tag == filter || tag.contains(filter))
        })
    }

    fn score_terms(&self, terms: &[String]) -> Option<u16> {
        if terms.is_empty() {
            return Some(0);
        }

        let mut total = 0u16;
        for term in terms {
            total = total.saturating_add(self.score_term(term)?);
        }
        Some(total)
    }

    fn score_term(&self, term: &str) -> Option<u16> {
        let mut score = 0u16;

        if self.title == term {
            score = score.max(1000);
        } else if self.title.starts_with(term) {
            score = score.max(925);
        } else if self.title.contains(term) {
            score = score.max(825);
        }

        if self.tags.iter().any(|tag| tag == term) {
            score = score.max(800);
        } else if self.tags.iter().any(|tag| tag.contains(term)) {
            score = score.max(675);
        }

        if self.words.iter().any(|word| word == term) {
            score = score.max(725);
        } else if self.words.iter().any(|word| word.starts_with(term)) {
            score = score.max(625);
        } else if self.search_text.contains(term) {
            score = score.max(525);
        }

        if term.len() >= 3 && self.words.iter().any(|word| fuzzy_word_match(term, word)) {
            score = score.max(425);
        }

        if term.len() >= 3 && is_subsequence(term, &self.compact_text) {
            score = score.max(175);
        }

        (score > 0).then_some(score)
    }
}

fn build_help_search_index(
    commands: &[HelpCommand],
    faqs: &[FaqEntry],
    tips: &[TipEntry],
) -> Vec<HelpSearchDocument> {
    let mut documents = Vec::with_capacity(commands.len() + faqs.len() + tips.len());

    for (index, command) in commands.iter().enumerate() {
        let mut fields = vec![
            command.name.as_str(),
            command.usage.as_str(),
            command.description.as_str(),
        ];
        fields.extend(command.aliases.iter().map(String::as_str));
        fields.extend(command.examples.iter().map(String::as_str));
        documents.push(HelpSearchDocument::new(
            HelpSearchCategory::Command,
            index,
            &command.name,
            fields,
            &command.tags,
        ));
    }

    for (index, faq) in faqs.iter().enumerate() {
        documents.push(HelpSearchDocument::new(
            HelpSearchCategory::Faq,
            index,
            &faq.question,
            vec![faq.id.as_str(), faq.question.as_str(), faq.answer.as_str()],
            &faq.tags,
        ));
    }

    for (index, tip) in tips.iter().enumerate() {
        documents.push(HelpSearchDocument::new(
            HelpSearchCategory::Tip,
            index,
            &tip.text,
            vec![tip.id.as_str(), tip.text.as_str(), tip.context.as_str()],
            &tip.tags,
        ));
    }

    documents
}

fn append_normalized_field(target: &mut String, field: &str) {
    let field = normalized_search(field);
    if field.is_empty() {
        return;
    }
    if !target.is_empty() {
        target.push(' ');
    }
    target.push_str(&field);
}

fn search_terms(value: &str) -> Vec<String> {
    let normalized = normalized_search(value);
    search_words(&normalized)
}

fn search_words(value: &str) -> Vec<String> {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn fuzzy_word_match(term: &str, word: &str) -> bool {
    if term == word || term.len() < 3 || word.len() < 3 {
        return false;
    }
    let max_distance = if term.len() <= 5 { 1 } else { 2 };
    levenshtein_at_most(term, word, max_distance)
}

fn levenshtein_at_most(left: &str, right: &str, max_distance: usize) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len().abs_diff(right.len()) > max_distance {
        return false;
    }

    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0; right.len() + 1];

    for (left_index, left_byte) in left.iter().enumerate() {
        current[0] = left_index + 1;
        let mut row_min = current[0];

        for (right_index, right_byte) in right.iter().enumerate() {
            let substitution = usize::from(left_byte != right_byte);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + substitution);
            row_min = row_min.min(current[right_index + 1]);
        }

        if row_min > max_distance {
            return false;
        }

        previous.clone_from_slice(&current);
    }

    previous[right.len()] <= max_distance
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut needle = needle.chars();
    let Some(mut current) = needle.next() else {
        return true;
    };

    for ch in haystack.chars() {
        if ch == current {
            match needle.next() {
                Some(next) => current = next,
                None => return true,
            }
        }
    }

    false
}

impl HelpCommand {
    fn matches_name(&self, lookup: &str) -> bool {
        normalized_search(&self.name) == lookup
            || self
                .aliases
                .iter()
                .any(|alias| normalized_search(alias) == lookup)
    }

    fn matches_search(&self, query: &str) -> bool {
        searchable_contains(
            query,
            [
                self.name.as_str(),
                self.usage.as_str(),
                self.description.as_str(),
            ]
            .into_iter()
            .chain(self.aliases.iter().map(String::as_str))
            .chain(self.examples.iter().map(String::as_str))
            .chain(self.tags.iter().map(String::as_str)),
        )
    }
}

impl FaqEntry {
    fn matches_search(&self, query: &str) -> bool {
        searchable_contains(
            query,
            [
                self.id.as_str(),
                self.question.as_str(),
                self.answer.as_str(),
            ]
            .into_iter()
            .chain(self.tags.iter().map(String::as_str)),
        )
    }
}

impl TipEntry {
    fn matches_search(&self, query: &str) -> bool {
        searchable_contains(
            query,
            [self.id.as_str(), self.text.as_str(), self.context.as_str()]
                .into_iter()
                .chain(self.tags.iter().map(String::as_str)),
        )
    }
}

/// A registered command definition.
pub struct CommandDef {
    /// Canonical path (e.g. `/nav waypoint`).
    pub path: String,
    /// Short description shown in `/help` output.
    pub help: String,
    /// Required positional argument types. Enforced before the callback fires.
    pub arg_types: Vec<ArgType>,
    /// If `true`, arguments beyond `arg_types.len()` are accepted without error.
    pub variadic: bool,
    /// The handler callback.
    pub callback: CommandCallback,
}

impl std::fmt::Debug for CommandDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandDef")
            .field("path", &self.path)
            .field("help", &self.help)
            .field("arg_types", &self.arg_types)
            .field("variadic", &self.variadic)
            .finish_non_exhaustive()
    }
}

/// Registry of slash commands with longest-prefix routing.
///
/// Commands are keyed by their canonical path (leading `/` + space-separated
/// tokens). When dispatching, the registry finds the longest registered path
/// that is a prefix of the input tokens and passes the remaining tokens as
/// arguments.
pub struct CommandRegistry {
    /// Map from normalized path (lowercase, no leading `/`) to definition.
    commands: HashMap<String, CommandDef>,
}

impl CommandRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// Register a command at `path`.
    ///
    /// `path` must start with `/` and may contain subcommand segments separated
    /// by spaces (e.g. `/nav waypoint`).
    ///
    /// Returns `false` (and logs a warning) if a command is already registered
    /// at the exact same path; returns `true` on success.
    ///
    /// # Panics
    ///
    /// Panics if `path` does not start with `/`.
    pub fn register<F>(
        &mut self,
        path: &str,
        help: &str,
        arg_types: &[ArgType],
        variadic: bool,
        callback: F,
    ) -> bool
    where
        F: Fn(&[&str]) -> CommandResult + Send + Sync + 'static,
    {
        assert!(
            path.starts_with('/'),
            "Command path must start with '/': {path}"
        );

        let key = normalize_path(path);

        if self.commands.contains_key(&key) {
            tracing::warn!(path, "Duplicate command registration ignored");
            return false;
        }

        tracing::debug!(path, help, variadic, "Registering command");

        self.commands.insert(
            key,
            CommandDef {
                path: path.to_string(),
                help: help.to_string(),
                arg_types: arg_types.to_vec(),
                variadic,
                callback: Box::new(callback),
            },
        );

        true
    }

    /// Dispatch a raw input string (e.g. `"/nav waypoint home"`).
    ///
    /// Tokenizes on ASCII whitespace, strips the leading `/`, finds the
    /// longest-prefix match, validates arguments, then calls the handler.
    ///
    /// Returns [`CommandResult::NotFound`] when no command matches.
    pub fn dispatch(&self, input: &str) -> CommandResult {
        let input = input.trim();
        trace_command_received(input);

        let result = self.dispatch_inner(input);
        trace_command_result(input, &result);
        result
    }

    fn dispatch_inner(&self, input: &str) -> CommandResult {
        if !input.starts_with('/') {
            return CommandResult::Error("Input must start with '/'".to_string());
        }

        let tokens: Vec<&str> = input.split_ascii_whitespace().collect();
        if tokens.is_empty() {
            return CommandResult::NotFound;
        }

        // Find the longest registered prefix.
        let (def, args) = match self.find_longest_prefix(&tokens) {
            Some(pair) => pair,
            None => {
                if let Some(result) = self.dispatch_builtin_meta_command(&tokens) {
                    return result;
                }
                tracing::debug!(input, "No matching command found");
                return CommandResult::NotFound;
            }
        };

        tracing::info!(
            path = def.path,
            args = ?args,
            "Dispatching command"
        );

        // Validate argument types and count.
        if let Some(err) = validate_args(def, args) {
            tracing::warn!(path = def.path, error = %err, "Command argument validation failed");
            return CommandResult::ValidationError(err);
        }

        // Heavy trace span: only compiled when `trace-commands` feature is active.
        #[cfg(feature = "trace-commands")]
        let _span = tracing::trace_span!(
            "command_dispatch",
            cmd_path = %def.path,
            args_len = args.len(),
        )
        .entered();

        let result = (def.callback)(args);

        match &result {
            CommandResult::Ok => {
                tracing::debug!(path = def.path, "Command completed ok");
                #[cfg(feature = "trace-commands")]
                tracing::trace!(cmd_path = %def.path, result = "ok", "Command dispatch result");
            }
            CommandResult::Message(msg) => {
                tracing::debug!(path = def.path, message = %msg, "Command completed with message");
                #[cfg(feature = "trace-commands")]
                tracing::trace!(cmd_path = %def.path, result = "message", message = %msg, "Command dispatch result");
            }
            CommandResult::Error(err) => {
                tracing::warn!(path = def.path, error = %err, "Command returned error");
                #[cfg(feature = "trace-commands")]
                tracing::trace!(cmd_path = %def.path, result = "error", error = %err, "Command dispatch result");
            }
            CommandResult::NotFound | CommandResult::ValidationError(_) => {}
        }

        result
    }

    /// Return a formatted help string for all registered commands, or for a
    /// specific command path if `filter` is `Some`.
    #[must_use]
    pub fn help(&self, filter: Option<&str>) -> String {
        let mut defs: Vec<&CommandDef> = self.commands.values().collect();
        defs.sort_by(|a, b| a.path.cmp(&b.path));

        if let Some(f) = filter {
            let key = normalize_path(f);
            return match self.commands.get(&key) {
                Some(def) => format_help(def),
                None => format!("No command registered at '{f}'"),
            };
        }

        if defs.is_empty() {
            return "No commands registered.".to_string();
        }

        defs.iter()
            .map(|d| format_help(d))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Return all registered command paths, sorted.
    #[must_use]
    pub fn command_paths(&self) -> Vec<String> {
        let mut paths: Vec<String> = self.commands.values().map(|d| d.path.clone()).collect();
        paths.sort();
        paths
    }

    /// Dispatch built-in `/textquest` and `/mercs` metadata commands.
    ///
    /// These commands intentionally live outside the registry so the help/list
    /// surface is available before Lua or plugin startup has registered any
    /// commands.
    fn dispatch_builtin_meta_command(&self, tokens: &[&str]) -> Option<CommandResult> {
        let root = tokens.first()?.trim_start_matches('/').to_lowercase();
        if root != "textquest" && root != "mercs" {
            return None;
        }

        let subcommand = tokens.get(1).map(|s| s.to_lowercase());
        let message = match subcommand.as_deref() {
            None | Some("help") => self.builtin_help_message(&root, &tokens[2..]),
            Some("search") => builtin_search_message(&tokens[2..]),
            Some("faq") | Some("faqs") => builtin_faq_message(&tokens[2..]),
            Some("tip") | Some("tips") => builtin_tips_message(&tokens[2..]),
            Some("config") | Some("guide") => builtin_config_message(&tokens[2..]),
            Some("commands") | Some("list_commands") => {
                let mut paths = help_database()
                    .map(|database| {
                        database
                            .commands
                            .iter()
                            .map(|command| format!("/mercs {}", command.name))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                paths.extend(self.command_paths());
                paths.sort();
                paths.dedup();

                if paths.is_empty() {
                    "No commands registered.".to_string()
                } else {
                    paths.join("\n")
                }
            }
            Some("list_scripts") => {
                "Script listing is pending Lua runtime integration.".to_string()
            }
            Some("reload") => "Script reload is pending Lua runtime integration.".to_string(),
            Some("set") => "Runtime settings are pending config integration.".to_string(),
            Some("debug") => return Some(dispatch_debug_command(&tokens[2..])),
            _ => format!(
                "Unknown TextQuest command '{}'. Try /{root} help.",
                tokens.join(" ")
            ),
        };

        tracing::debug!(root, ?subcommand, "dispatching built-in metadata command");
        Some(CommandResult::Message(message))
    }

    fn builtin_help_message(&self, root: &str, args: &[&str]) -> String {
        let Ok(database) = help_database() else {
            return help_unavailable_message();
        };

        if args.is_empty() {
            return format_help_overview(root, database, self);
        }

        let lookup = args.join(" ");
        if let Some(command) = database.command(&lookup) {
            return format_help_command(command);
        }

        let registry_filter = format!("/{}", args.join(" "));
        let registry_help = self.help(Some(&registry_filter));
        if !registry_help.starts_with("No command registered") {
            return registry_help;
        }

        format!(
            "No in-game help found for '{}'. Try /{root} search {}.",
            lookup, lookup
        )
    }

    /// Find the longest registered prefix matching the input tokens.
    ///
    /// Tries progressively shorter token windows starting from the full token
    /// list. The first token always includes the leading `/`.
    fn find_longest_prefix<'a>(
        &'a self,
        tokens: &'a [&'a str],
    ) -> Option<(&'a CommandDef, &'a [&'a str])> {
        // tokens[0] is "/command" (may be "/command" or "/" if split weirdly).
        // Build candidate keys by joining increasing prefix lengths.
        // We try from longest to shortest.
        for prefix_len in (1..=tokens.len()).rev() {
            let key = normalize_path(&tokens[..prefix_len].join(" "));
            if let Some(def) = self.commands.get(&key) {
                return Some((def, &tokens[prefix_len..]));
            }
        }
        None
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn help_database() -> Result<&'static HelpDatabase, &'static str> {
    match HELP_DATABASE.get_or_init(HelpDatabase::load) {
        Ok(database) => Ok(database),
        Err(err) => Err(err.as_str()),
    }
}

fn help_unavailable_message() -> String {
    match help_database() {
        Ok(_) => "Help content unavailable.".to_string(),
        Err(err) => format!("Help content unavailable: {err}"),
    }
}

fn builtin_search_message(args: &[&str]) -> String {
    if args.is_empty() {
        return "Usage: /mercs search <text> [category:commands|faq|tips] [tag:<tag>]\nSearches commands, FAQ entries, and tips."
            .to_string();
    }

    let Ok(database) = help_database() else {
        return help_unavailable_message();
    };

    let query = args.join(" ");
    let search_query = parse_help_search_query(args);
    let results = database.search(&search_query);

    let mut commands = Vec::new();
    let mut faqs = Vec::new();
    let mut tips = Vec::new();
    for result in &results {
        match result.category {
            HelpSearchCategory::Command => {
                if let Some(command) = database.command_result(result) {
                    commands.push(command);
                }
            }
            HelpSearchCategory::Faq => {
                if let Some(faq) = database.faq_result(result) {
                    faqs.push(faq);
                }
            }
            HelpSearchCategory::Tip => {
                if let Some(tip) = database.tip_result(result) {
                    tips.push(tip);
                }
            }
        }
    }

    if commands.is_empty() && faqs.is_empty() && tips.is_empty() {
        return format!("No help results found for '{query}'. Full docs: docs/wiki/");
    }

    let mut lines = vec![format!("Help search: {query}")];

    if !commands.is_empty() {
        lines.push("Commands:".to_string());
        for command in commands {
            lines.push(format!(
                "  /mercs {} - {}",
                command.usage, command.description
            ));
        }
    }

    if !faqs.is_empty() {
        lines.push("FAQ:".to_string());
        for faq in faqs {
            lines.push(format!("  {} - {}", faq.id, faq.question));
        }
    }

    if !tips.is_empty() {
        lines.push("Tips:".to_string());
        for tip in tips {
            lines.push(format!("  [{}] {}", tip.context, tip.text));
        }
    }

    lines.push("Full docs: docs/wiki/".to_string());
    lines.join("\n")
}

fn parse_help_search_query(args: &[&str]) -> HelpSearchQuery {
    let mut query = HelpSearchQuery::new("");
    let mut text = Vec::new();

    for arg in args {
        let Some((key, value)) = arg.split_once(':') else {
            text.push(*arg);
            continue;
        };

        match normalized_search(key).as_str() {
            "category" | "type" => {
                if let Some(category) = HelpSearchCategory::parse(value) {
                    query.add_category(category);
                    continue;
                }
            }
            "tag" | "tags" => {
                query.add_tag(value);
                continue;
            }
            _ => {}
        }

        text.push(*arg);
    }

    query.set_text(text.join(" "));
    query
}

fn builtin_faq_message(args: &[&str]) -> String {
    let Ok(database) = help_database() else {
        return help_unavailable_message();
    };

    if args.is_empty() {
        let mut lines = vec![
            "FAQ".to_string(),
            "Use /mercs faq <text> to search questions and answers.".to_string(),
        ];
        for faq in &database.faqs {
            lines.push(format!("  {} - {}", faq.id, faq.question));
        }
        lines.push("Full docs: docs/wiki/".to_string());
        return lines.join("\n");
    }

    let query = args.join(" ");
    let matches = database.faq_matches(&query);
    if matches.is_empty() {
        return format!("No FAQ results found for '{query}'. Try /mercs search {query}.");
    }

    let mut lines = vec![format!("FAQ search: {query}")];
    for faq in matches {
        lines.push(format_faq_entry(faq));
    }
    lines.push("Full docs: docs/wiki/".to_string());
    lines.join("\n\n")
}

fn builtin_tips_message(args: &[&str]) -> String {
    let Ok(database) = help_database() else {
        return help_unavailable_message();
    };

    let tips = if args.is_empty() {
        database.tips.iter().collect::<Vec<_>>()
    } else {
        database.tip_matches(&args.join(" "))
    };

    if tips.is_empty() {
        let query = args.join(" ");
        return format!("No tips found for '{query}'. Try /mercs tips or /mercs search {query}.");
    }

    let mut lines = vec!["Quick tips".to_string()];
    for tip in tips {
        lines.push(format!("  [{}] {}", tip.context, tip.text));
    }
    lines.join("\n")
}

fn builtin_config_message(args: &[&str]) -> String {
    let Ok(database) = help_database() else {
        return help_unavailable_message();
    };

    if !args.is_empty() {
        let query = args.join(" ");
        let commands = database.command_matches(&query);
        let faqs = database.faq_matches(&query);

        if commands.is_empty() && faqs.is_empty() {
            return format!(
                "No configuration guide results found for '{query}'. Full docs: docs/wiki/"
            );
        }

        let mut lines = vec![format!("Configuration guide: {query}")];
        if !commands.is_empty() {
            lines.push("Relevant commands:".to_string());
            for command in commands {
                lines.push(format!(
                    "  /mercs {} - {}",
                    command.usage, command.description
                ));
            }
        }
        if !faqs.is_empty() {
            lines.push("Walkthroughs:".to_string());
            for faq in faqs {
                lines.push(format!("  {} - {}", faq.id, faq.question));
            }
        }
        lines.push("Full docs: docs/wiki/".to_string());
        return lines.join("\n");
    }

    let mut lines = vec![
        "Configuration guide".to_string(),
        "Start with config/textquest.toml, then class files under config/classes/.".to_string(),
        "Useful commands:".to_string(),
    ];

    for name in ["camp", "assist", "group"] {
        if let Some(command) = database.command(name) {
            lines.push(format!(
                "  /mercs {} - {}",
                command.usage, command.description
            ));
        }
    }

    let faqs = database.config_faqs();
    if !faqs.is_empty() {
        lines.push("Walkthroughs:".to_string());
        for faq in faqs {
            lines.push(format!("  {} - {}", faq.id, faq.question));
        }
    }

    lines.push("Full docs: docs/wiki/".to_string());
    lines.join("\n")
}

fn format_help_overview(root: &str, database: &HelpDatabase, registry: &CommandRegistry) -> String {
    let mut lines = vec![
        "TextQuest in-game help".to_string(),
        format!(
            "Usage: /{root} help <command>, /{root} search <text>, /{root} faq [text], /{root} tips [context], /{root} config [text]"
        ),
        "Commands:".to_string(),
    ];

    for command in &database.commands {
        lines.push(format!(
            "  /mercs {:<28} {}",
            command.usage, command.description
        ));
    }

    let runtime_paths = registry.command_paths();
    if !runtime_paths.is_empty() {
        lines.push("Runtime commands:".to_string());
        lines.extend(runtime_paths.into_iter().map(|path| format!("  {path}")));
    }

    lines.push("Full docs: docs/wiki/".to_string());
    lines.join("\n")
}

fn format_help_command(command: &HelpCommand) -> String {
    let mut lines = vec![
        format!("Command: /mercs {}", command.name),
        format!("Usage: /mercs {}", command.usage),
        command.description.clone(),
    ];

    if !command.aliases.is_empty() {
        lines.push(format!("Aliases: {}", command.aliases.join(", ")));
    }

    if !command.examples.is_empty() {
        let examples = command
            .examples
            .iter()
            .map(|example| format!("/mercs {example}"))
            .collect::<Vec<_>>()
            .join("; ");
        lines.push(format!("Examples: {examples}"));
    }

    if !command.tags.is_empty() {
        lines.push(format!("Tags: {}", command.tags.join(", ")));
    }

    lines.push("Full docs: docs/wiki/".to_string());
    lines.join("\n")
}

fn format_faq_entry(faq: &FaqEntry) -> String {
    format!("Q: {}\nA: {}", faq.question, faq.answer.trim())
}

fn searchable_contains<'a>(query: &str, fields: impl IntoIterator<Item = &'a str>) -> bool {
    query.is_empty()
        || fields
            .into_iter()
            .any(|field| normalized_search(field).contains(query))
}

fn normalize_help_lookup(input: &str) -> String {
    let mut lookup = input.trim().trim_start_matches('/').to_lowercase();
    for prefix in ["mercs ", "textquest ", "help "] {
        if let Some(rest) = lookup.strip_prefix(prefix) {
            lookup = rest.trim().to_string();
        }
    }

    lookup
        .split_ascii_whitespace()
        .next()
        .unwrap_or_default()
        .to_string()
}

fn normalized_search(value: &str) -> String {
    value.trim().to_lowercase()
}

/// Normalize a command path for use as a HashMap key.
///
/// Strips leading `/`, trims whitespace, and lowercases.
fn normalize_path(path: &str) -> String {
    path.trim_start_matches('/').trim().to_lowercase()
}

fn dispatch_debug_command(args: &[&str]) -> CommandResult {
    let message = match args.first().map(|arg| arg.to_ascii_lowercase()).as_deref() {
        None | Some("status") => debug_status_message(),
        Some("on") => {
            DEBUG_MODE_ENABLED.store(true, Ordering::Relaxed);
            "TextQuest debug mode enabled.".to_string()
        }
        Some("off") => {
            DEBUG_MODE_ENABLED.store(false, Ordering::Relaxed);
            COMMAND_TRACING_ENABLED.store(false, Ordering::Relaxed);
            "TextQuest debug mode disabled.".to_string()
        }
        Some("trace") => set_command_tracing(args.get(1).copied()),
        Some("hex") => debug_hex_request(args.get(1).copied(), args.get(2).copied()),
        Some("profile") => "Script profiling is pending Lua runtime integration.".to_string(),
        Some(other) => format!(
            "Unknown debug command '{other}'. Usage: /textquest debug [on|off|status|trace|hex <addr> [size]|profile]"
        ),
    };

    CommandResult::Message(message)
}

fn debug_status_message() -> String {
    format!(
        "TextQuest debug mode: {}; command tracing: {}",
        on_off(debug_mode_enabled()),
        on_off(command_tracing_enabled())
    )
}

fn set_command_tracing(mode: Option<&str>) -> String {
    let enabled = match mode.map(str::to_ascii_lowercase).as_deref() {
        None | Some("on") => true,
        Some("off") => false,
        Some("toggle") => !command_tracing_enabled(),
        Some(other) => {
            return format!(
                "Unknown trace mode '{other}'. Usage: /textquest debug trace [on|off|toggle]"
            );
        }
    };

    DEBUG_MODE_ENABLED.store(enabled, Ordering::Relaxed);
    COMMAND_TRACING_ENABLED.store(enabled, Ordering::Relaxed);
    format!(
        "TextQuest command tracing {}.",
        if enabled { "enabled" } else { "disabled" }
    )
}

fn debug_hex_request(address: Option<&str>, size: Option<&str>) -> String {
    let Some(address) = address else {
        return "Usage: /textquest debug hex <addr> [size]".to_string();
    };

    let Ok(address) = parse_address(address) else {
        return format!("Invalid debug hex address '{address}'. Use decimal or 0x-prefixed hex.");
    };

    let size = match size {
        Some(raw) => match raw.parse::<usize>() {
            Ok(value) if value > 0 => value.min(4096),
            _ => return format!("Invalid debug hex size '{raw}'. Use a positive byte count."),
        },
        None => 64,
    };

    format!(
        "Debug hex request queued: address={address:#x}, size={size} bytes. Use MemoryRead IPC for bytes."
    )
}

fn parse_address(raw: &str) -> Result<usize, std::num::ParseIntError> {
    let trimmed = raw.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        usize::from_str_radix(hex, 16)
    } else {
        trimmed.parse::<usize>()
    }
}

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

fn trace_command_received(input: &str) {
    if command_tracing_enabled() {
        tracing::info!(input, "TextQuest command trace received");
    }
}

fn trace_command_result(input: &str, result: &CommandResult) {
    if command_tracing_enabled() {
        tracing::info!(input, ?result, "TextQuest command trace result");
    }
}

/// Validate arguments against a command's declared types.
///
/// Returns `Some(error_message)` on failure, `None` on success.
fn validate_args(def: &CommandDef, args: &[&str]) -> Option<String> {
    let required = def.arg_types.len();

    if args.len() < required {
        return Some(format!(
            "Too few arguments for '{}': expected {}, got {}. Usage: {}",
            def.path,
            required,
            args.len(),
            format_usage(def),
        ));
    }

    if !def.variadic && args.len() > required {
        return Some(format!(
            "Too many arguments for '{}': expected {}, got {}. Usage: {}",
            def.path,
            required,
            args.len(),
            format_usage(def),
        ));
    }

    // Validate each typed argument.
    for (i, arg_type) in def.arg_types.iter().enumerate() {
        if !arg_type.validate(args[i]) {
            return Some(format!(
                "Argument {} to '{}' must be {}, got '{}'",
                i + 1,
                def.path,
                arg_type.name(),
                args[i],
            ));
        }
    }

    None
}

/// Format a usage line for a command definition.
fn format_usage(def: &CommandDef) -> String {
    let args: Vec<String> = def
        .arg_types
        .iter()
        .enumerate()
        .map(|(i, t)| format!("<arg{}: {}>", i + 1, t.name()))
        .collect();
    let variadic_suffix = if def.variadic { " [...]" } else { "" };
    format!("{} {}{}", def.path, args.join(" "), variadic_suffix)
}

/// Format a full help entry for a command definition.
fn format_help(def: &CommandDef) -> String {
    let usage = format_usage(def);
    let variadic_note = if def.variadic { " (variadic)" } else { "" };
    format!("  {}\n    {}{}", usage, def.help, variadic_note)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_handler(_args: &[&str]) -> CommandResult {
        CommandResult::Ok
    }

    // ── Registration ────────────────────────────────────────────────────────

    #[test]
    fn register_and_dispatch_simple() {
        let mut reg = CommandRegistry::new();
        reg.register("/hello", "Say hello", &[], false, ok_handler);
        assert_eq!(reg.dispatch("/hello"), CommandResult::Ok);
    }

    #[test]
    fn register_returns_false_on_conflict() {
        let mut reg = CommandRegistry::new();
        assert!(reg.register("/dupe", "first", &[], false, ok_handler));
        assert!(!reg.register("/dupe", "second", &[], false, ok_handler));
        // Only one registration; still dispatches to the first.
        assert_eq!(reg.dispatch("/dupe"), CommandResult::Ok);
    }

    #[test]
    #[should_panic(expected = "must start with '/'")]
    fn register_panics_without_leading_slash() {
        let mut reg = CommandRegistry::new();
        reg.register("noslash", "bad", &[], false, ok_handler);
    }

    // ── Parsing & routing ───────────────────────────────────────────────────

    #[test]
    fn dispatch_unknown_returns_not_found() {
        let reg = CommandRegistry::new();
        assert_eq!(reg.dispatch("/unknown"), CommandResult::NotFound);
    }

    #[test]
    fn dispatch_non_slash_returns_error() {
        let reg = CommandRegistry::new();
        let result = reg.dispatch("hello");
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[test]
    fn route_to_subcommand_longest_prefix() {
        let mut reg = CommandRegistry::new();
        let base_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sub_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let base_flag = base_called.clone();
        reg.register("/nav", "nav base", &[], false, move |_| {
            base_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            CommandResult::Ok
        });

        let sub_flag = sub_called.clone();
        reg.register(
            "/nav waypoint",
            "go to waypoint",
            &[ArgType::String],
            false,
            move |_| {
                sub_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                CommandResult::Ok
            },
        );

        // "/nav waypoint home" should route to /nav waypoint, not /nav
        assert_eq!(reg.dispatch("/nav waypoint home"), CommandResult::Ok);
        assert!(!base_called.load(std::sync::atomic::Ordering::SeqCst));
        assert!(sub_called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn route_falls_back_to_base_when_no_sub_matches() {
        let mut reg = CommandRegistry::new();
        let base_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let flag = base_called.clone();
        reg.register("/nav", "nav base", &[], true, move |_| {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            CommandResult::Ok
        });

        // No "/nav unknown" — should fall back to /nav (variadic accepts extra args)
        assert_eq!(reg.dispatch("/nav unknown extra"), CommandResult::Ok);
        assert!(base_called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn case_insensitive_dispatch() {
        let mut reg = CommandRegistry::new();
        reg.register("/Hello", "case test", &[], false, ok_handler);
        // Input in different case should still match.
        assert_eq!(reg.dispatch("/HELLO"), CommandResult::Ok);
        assert_eq!(reg.dispatch("/hello"), CommandResult::Ok);
    }

    // ── Variadic args ───────────────────────────────────────────────────────

    #[test]
    fn variadic_accepts_extra_args() {
        let mut reg = CommandRegistry::new();
        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let recv = received.clone();
        reg.register("/say", "say text", &[ArgType::String], true, move |args| {
            let mut v = recv.lock().unwrap();
            *v = args.iter().map(|s| s.to_string()).collect();
            CommandResult::Ok
        });

        assert_eq!(reg.dispatch("/say hello world foo bar"), CommandResult::Ok);
        let got = received.lock().unwrap().clone();
        assert_eq!(got, vec!["hello", "world", "foo", "bar"]);
    }

    #[test]
    fn non_variadic_rejects_extra_args() {
        let mut reg = CommandRegistry::new();
        reg.register(
            "/exact",
            "exact args",
            &[ArgType::String],
            false,
            ok_handler,
        );
        let result = reg.dispatch("/exact one two");
        assert!(matches!(result, CommandResult::ValidationError(_)));
    }

    #[test]
    fn variadic_zero_required_args_accepts_any() {
        let mut reg = CommandRegistry::new();
        reg.register("/broadcast", "broadcast", &[], true, ok_handler);
        assert_eq!(reg.dispatch("/broadcast"), CommandResult::Ok);
        assert_eq!(reg.dispatch("/broadcast a b c d e"), CommandResult::Ok);
    }

    // ── Argument validation ─────────────────────────────────────────────────

    #[test]
    fn validate_int_arg() {
        let mut reg = CommandRegistry::new();
        reg.register("/setlevel", "set level", &[ArgType::Int], false, ok_handler);
        assert_eq!(reg.dispatch("/setlevel 60"), CommandResult::Ok);
        assert!(matches!(
            reg.dispatch("/setlevel notanumber"),
            CommandResult::ValidationError(_)
        ));
    }

    #[test]
    fn validate_float_arg() {
        let mut reg = CommandRegistry::new();
        reg.register("/speed", "set speed", &[ArgType::Float], false, ok_handler);
        assert_eq!(reg.dispatch("/speed 1.5"), CommandResult::Ok);
        assert_eq!(reg.dispatch("/speed 2"), CommandResult::Ok);
        assert!(matches!(
            reg.dispatch("/speed fast"),
            CommandResult::ValidationError(_)
        ));
    }

    #[test]
    fn validate_bool_arg() {
        let mut reg = CommandRegistry::new();
        reg.register(
            "/autoattack",
            "toggle autoattack",
            &[ArgType::Bool],
            false,
            ok_handler,
        );
        for val in &["true", "false", "1", "0", "TRUE", "FALSE"] {
            let cmd = format!("/autoattack {val}");
            assert_eq!(
                reg.dispatch(&cmd),
                CommandResult::Ok,
                "expected ok for {val}"
            );
        }
        assert!(matches!(
            reg.dispatch("/autoattack yes"),
            CommandResult::ValidationError(_)
        ));
    }

    #[test]
    fn too_few_args_returns_validation_error() {
        let mut reg = CommandRegistry::new();
        reg.register(
            "/tp",
            "teleport",
            &[ArgType::Float, ArgType::Float],
            false,
            ok_handler,
        );
        let result = reg.dispatch("/tp 100.0");
        assert!(matches!(result, CommandResult::ValidationError(_)));
    }

    // ── Help system ─────────────────────────────────────────────────────────

    #[test]
    fn help_all_contains_registered_paths() {
        let mut reg = CommandRegistry::new();
        reg.register("/foo", "foo help", &[], false, ok_handler);
        reg.register("/bar", "bar help", &[ArgType::String], false, ok_handler);
        let help = reg.help(None);
        assert!(help.contains("/foo"), "help missing /foo:\n{help}");
        assert!(help.contains("/bar"), "help missing /bar:\n{help}");
        assert!(
            help.contains("foo help"),
            "help missing description:\n{help}"
        );
    }

    #[test]
    fn help_filter_returns_specific_command() {
        let mut reg = CommandRegistry::new();
        reg.register("/foo", "foo help", &[], false, ok_handler);
        reg.register("/bar", "bar help", &[], false, ok_handler);
        let help = reg.help(Some("/foo"));
        assert!(help.contains("/foo"));
        assert!(!help.contains("/bar"));
    }

    #[test]
    fn help_filter_unknown_returns_not_found_message() {
        let reg = CommandRegistry::new();
        let help = reg.help(Some("/nonexistent"));
        assert!(help.contains("No command registered"));
    }

    #[test]
    fn help_shows_variadic_marker() {
        let mut reg = CommandRegistry::new();
        reg.register("/v", "variadic cmd", &[], true, ok_handler);
        let help = reg.help(None);
        assert!(
            help.contains("variadic"),
            "expected variadic marker:\n{help}"
        );
    }

    #[test]
    fn help_empty_registry() {
        let reg = CommandRegistry::new();
        assert!(reg.help(None).contains("No commands"));
    }

    #[test]
    fn builtin_textquest_lists_registered_commands() {
        let mut reg = CommandRegistry::new();
        reg.register(
            "/mercs pull",
            "pull target",
            &[ArgType::String],
            false,
            ok_handler,
        );

        let result = reg.dispatch("/textquest commands");
        match result {
            CommandResult::Message(message) => assert!(message.contains("/mercs pull")),
            other => panic!("expected command listing message, got {other:?}"),
        }
    }

    #[test]
    fn builtin_mercs_help_uses_static_command_database() {
        let reg = CommandRegistry::new();

        let result = reg.dispatch("/mercs help pull");

        match result {
            CommandResult::Message(message) => {
                assert!(message.contains("Usage: /mercs pull <npc_name>"));
                assert!(message.contains("Pull the named NPC"));
                assert!(message.contains("Examples: /mercs pull golem"));
            }
            other => panic!("expected help message, got {other:?}"),
        }
    }

    #[test]
    fn builtin_mercs_search_returns_commands_and_faqs() {
        let reg = CommandRegistry::new();

        let result = reg.dispatch("/mercs search ranger");

        match result {
            CommandResult::Message(message) => {
                assert!(message.contains("Help search: ranger"));
                assert!(message.contains("FAQ:"));
                assert!(message.contains("How do I configure a ranger class?"));
            }
            other => panic!("expected search message, got {other:?}"),
        }
    }

    #[test]
    fn help_search_ranks_exact_command_match_first() {
        let database = HelpDatabase::load().expect("help database");

        let results = database.search(&HelpSearchQuery::new("pull"));

        let first = results.first().expect("search result");
        assert_eq!(first.category, HelpSearchCategory::Command);
        assert_eq!(database.command_result(first).unwrap().name, "pull");
    }

    #[test]
    fn help_search_supports_fuzzy_typos() {
        let reg = CommandRegistry::new();

        let result = reg.dispatch("/mercs search rnger category:faq");

        match result {
            CommandResult::Message(message) => {
                assert!(message.contains("FAQ:"));
                assert!(message.contains("How do I configure a ranger class?"));
            }
            other => panic!("expected fuzzy search message, got {other:?}"),
        }
    }

    #[test]
    fn help_search_filters_by_category_and_tag() {
        let database = HelpDatabase::load().expect("help database");
        let query = parse_help_search_query(&["category:faq", "tag:loot"]);

        let results = database.search(&query);

        assert!(!results.is_empty());
        assert!(
            results
                .iter()
                .all(|result| result.category == HelpSearchCategory::Faq)
        );
        assert!(results.iter().any(|result| {
            database
                .faq_result(result)
                .is_some_and(|faq| faq.id == "loot-not-collecting")
        }));
    }

    #[test]
    fn help_search_query_completes_under_ten_ms() {
        let database = HelpDatabase::load().expect("help database");
        let started = std::time::Instant::now();

        let results = database.search(&HelpSearchQuery::new("navigation recovery"));

        assert!(!results.is_empty());
        assert!(
            started.elapsed().as_micros() < 10_000,
            "help search exceeded 10ms"
        );
    }

    #[test]
    fn builtin_mercs_faq_search_returns_answer_text() {
        let reg = CommandRegistry::new();

        let result = reg.dispatch("/mercs faq zone");

        match result {
            CommandResult::Message(message) => {
                assert!(message.contains("FAQ search: zone"));
                assert!(message.contains("Some clients are stuck at the zone line"));
                assert!(message.contains("Use 'pause' to stop automation"));
            }
            other => panic!("expected FAQ message, got {other:?}"),
        }
    }

    #[test]
    fn builtin_textquest_debug_controls_runtime_flags() {
        let reg = CommandRegistry::new();

        let result = reg.dispatch("/textquest debug on");
        assert!(matches!(result, CommandResult::Message(_)));
        assert!(debug_mode_enabled());

        let result = reg.dispatch("/textquest debug trace");
        match result {
            CommandResult::Message(message) => assert!(message.contains("enabled")),
            other => panic!("expected debug trace message, got {other:?}"),
        }
        assert!(command_tracing_enabled());

        let result = reg.dispatch("/textquest debug hex 0x40 16");
        match result {
            CommandResult::Message(message) => {
                assert!(message.contains("address=0x40"));
                assert!(message.contains("size=16"));
            }
            other => panic!("expected debug hex message, got {other:?}"),
        }

        let result = reg.dispatch("/textquest debug off");
        assert!(matches!(result, CommandResult::Message(_)));
        assert!(!debug_mode_enabled());
        assert!(!command_tracing_enabled());
    }

    #[test]
    fn register_script_command_uses_global_registry() {
        let path = "/issue793_script_probe";
        let _ = register_script_command(path, |_| CommandResult::Message("script ok".to_string()));

        let result = dispatch_global_command(path);
        assert_eq!(result, CommandResult::Message("script ok".to_string()));
    }

    // ── Command paths ───────────────────────────────────────────────────────

    #[test]
    fn command_paths_sorted() {
        let mut reg = CommandRegistry::new();
        reg.register("/z", "z", &[], false, ok_handler);
        reg.register("/a", "a", &[], false, ok_handler);
        reg.register("/m", "m", &[], false, ok_handler);
        let paths = reg.command_paths();
        assert_eq!(paths, vec!["/a", "/m", "/z"]);
    }

    // ── Callback receives correct args ───────────────────────────────────────

    #[test]
    fn callback_receives_args_after_prefix() {
        let mut reg = CommandRegistry::new();
        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let recv = received.clone();
        reg.register(
            "/nav waypoint",
            "go to waypoint",
            &[ArgType::String],
            true,
            move |args| {
                *recv.lock().unwrap() = args.iter().map(|s| s.to_string()).collect();
                CommandResult::Ok
            },
        );

        assert_eq!(reg.dispatch("/nav waypoint home extra"), CommandResult::Ok);
        let got = received.lock().unwrap().clone();
        assert_eq!(got, vec!["home", "extra"]);
    }

    // ── trace-commands feature ───────────────────────────────────────────────

    /// Verify dispatch emits the correct result under the `trace-commands` feature.
    ///
    /// When `trace-commands` is active, a `tracing::trace_span!` is entered at
    /// dispatch time.  The result semantics must be unchanged regardless of
    /// whether the feature is on or off.  This test exercises the gated code
    /// path by dispatching a known command and asserting the result.
    #[test]
    #[cfg(feature = "trace-commands")]
    fn trace_commands_feature_dispatch_result_unchanged() {
        let mut reg = CommandRegistry::new();
        reg.register("/trace_test", "trace test", &[], false, ok_handler);

        // With trace-commands enabled the span is entered; result must still be Ok.
        assert_eq!(reg.dispatch("/trace_test"), CommandResult::Ok);
    }

    /// Verify that a command with arguments emits the correct args_len field value.
    ///
    /// The `trace_span!` records `args_len = args.len()`.  After routing to
    /// `/trace_args one two`, the callback receives 2 args and dispatch succeeds.
    #[test]
    #[cfg(feature = "trace-commands")]
    fn trace_commands_feature_args_len_field() {
        let mut reg = CommandRegistry::new();
        reg.register(
            "/trace_args",
            "trace args test",
            &[ArgType::String, ArgType::String],
            false,
            ok_handler,
        );

        // args_len in the span will be 2 for this dispatch.
        assert_eq!(reg.dispatch("/trace_args one two"), CommandResult::Ok);
    }

    /// Verify that the trace-commands gate does not break error-result paths.
    ///
    /// When a command returns `CommandResult::Error`, the span is still entered
    /// and the trace event records `result = "error"`.
    #[test]
    #[cfg(feature = "trace-commands")]
    fn trace_commands_feature_error_result_path() {
        let mut reg = CommandRegistry::new();
        reg.register("/trace_err", "trace error path", &[], false, |_| {
            CommandResult::Error("intentional".to_string())
        });

        let result = reg.dispatch("/trace_err");
        assert!(matches!(result, CommandResult::Error(_)));
    }

    // ── normalize_path ───────────────────────────────────────────────────────

    #[test]
    fn normalize_strips_slash_and_lowercases() {
        assert_eq!(normalize_path("/Hello"), "hello");
        assert_eq!(normalize_path("/NAV WAYPOINT"), "nav waypoint");
        assert_eq!(normalize_path("no-slash"), "no-slash");
    }

    // ── ArgType validation ───────────────────────────────────────────────────

    #[test]
    fn arg_type_string_rejects_empty() {
        assert!(!ArgType::String.validate(""));
        assert!(ArgType::String.validate("a"));
    }

    #[test]
    fn arg_type_int_rejects_float() {
        assert!(!ArgType::Int.validate("1.5"));
        assert!(ArgType::Int.validate("-42"));
    }

    #[test]
    fn arg_type_float_accepts_int_strings() {
        assert!(ArgType::Float.validate("42"));
        assert!(ArgType::Float.validate("3.14"));
        assert!(!ArgType::Float.validate("pi"));
    }
}
