//! Help content TOML format definition and file loader.
//!
//! # TOML format
//!
//! Two file shapes are supported:
//!
//! **commands.toml** - slash-command reference entries:
//! ```toml
//! [[commands]]
//! name = "pull"
//! aliases = ["p"]
//! usage = "pull <npc_name>"
//! description = "Pull target NPC to camp location"
//! examples = ["pull golem", "pull all"]
//! tags = ["combat", "targeting"]
//! ```
//!
//! **faq.toml** - FAQ entries and operator tips:
//! ```toml
//! [[faqs]]
//! id = "setup-ranger"
//! question = "How do I set up my ranger?"
//! answer = "Configure ranger.toml with class = ranger"
//! tags = ["setup", "ranger"]
//!
//! [[tips]]
//! id = "tip-dps"
//! text = "Enable DPS tracking in the metrics panel (Tab)"
//! context = "overview"
//! tags = ["tips", "monitoring"]
//! ```

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, anyhow};
use serde::Deserialize;
use tracing::warn;

/// Default project-relative directory for operator help content.
pub const DEFAULT_HELP_DIR: &str = "config/help";

// --- Unified topic type ------------------------------------------------------

/// Category of a help topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelpCategory {
    /// Operator slash-command.
    Command,
    /// Frequently asked question.
    Faq,
    /// Quick operator tip.
    Tip,
}

impl HelpCategory {
    /// Human-readable label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Command => "Command",
            Self::Faq => "FAQ",
            Self::Tip => "Tip",
        }
    }
}

/// A single help entry in the unified topic list used by older TUI paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpTopic {
    /// Unique identifier.
    pub id: String,
    /// Short display title.
    pub title: String,
    /// Full body text.
    pub body: String,
    /// Category used for filtering.
    pub category: HelpCategory,
    /// Search keywords, lower-cased at load time.
    pub keywords: Vec<String>,
}

// --- Structured help data ----------------------------------------------------

/// Operator slash-command help entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct HelpCommand {
    /// Canonical command name.
    #[serde(default)]
    pub name: String,
    /// Alternate command names that resolve to `name`.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Usage string shown in command help.
    #[serde(default)]
    pub usage: String,
    /// Human-readable command description.
    #[serde(default)]
    pub description: String,
    /// Concrete invocation examples.
    #[serde(default)]
    pub examples: Vec<String>,
    /// Search/filter tags.
    #[serde(default)]
    pub tags: Vec<String>,
}

impl HelpCommand {
    fn validate(&self) -> Result<(), String> {
        require_text("command name", &self.name)?;
        require_text("command usage", &self.usage)?;
        require_text("command description", &self.description)?;
        require_non_empty("command examples", &self.examples)?;
        require_text_values("command examples", &self.examples)?;
        require_text_values("command aliases", &self.aliases)?;
        require_text_values("command tags", &self.tags)?;

        let mut keys = HashSet::new();
        keys.insert(normalize_key(&self.name));
        for alias in &self.aliases {
            if !keys.insert(normalize_key(alias)) {
                return Err(format!("duplicate command key or alias: {alias}"));
            }
        }
        Ok(())
    }
}

/// Frequently asked help question.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct HelpFaq {
    /// Canonical FAQ identifier used for lookup.
    #[serde(default)]
    pub id: String,
    /// Operator-facing question.
    #[serde(default)]
    pub question: String,
    /// Answer body.
    #[serde(default)]
    pub answer: String,
    /// Search/filter tags.
    #[serde(default)]
    pub tags: Vec<String>,
}

impl HelpFaq {
    fn validate(&self) -> Result<(), String> {
        require_text("FAQ id", &self.id)?;
        require_text("FAQ question", &self.question)?;
        require_text("FAQ answer", &self.answer)?;
        require_non_empty("FAQ tags", &self.tags)?;
        require_text_values("FAQ tags", &self.tags)
    }
}

/// Short contextual operator tip.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct HelpTip {
    /// Canonical tip identifier used for lookup.
    #[serde(default)]
    pub id: String,
    /// Tip text rendered to the operator.
    #[serde(default)]
    pub text: String,
    /// UI context or trigger where this tip is relevant.
    #[serde(default)]
    pub context: String,
    /// Search/filter tags.
    #[serde(default)]
    pub tags: Vec<String>,
}

impl HelpTip {
    fn validate(&self) -> Result<(), String> {
        require_text("tip id", &self.id)?;
        require_text("tip text", &self.text)?;
        require_text("tip context", &self.context)?;
        require_text_values("tip tags", &self.tags)
    }
}

/// Top-level TOML document. All sections are optional so a file can contain any
/// mix of commands, FAQs, and tips.
#[derive(Debug, Deserialize, Default)]
struct HelpFile {
    #[serde(default)]
    commands: Vec<HelpCommand>,
    #[serde(default)]
    faqs: Vec<HelpFaq>,
    #[serde(default)]
    tips: Vec<HelpTip>,
}

/// In-memory help content database with lookup indexes for TUI access.
#[derive(Debug, Clone, Default)]
pub struct HelpDatabase {
    /// Slash-command help entries.
    pub commands: Vec<HelpCommand>,
    /// FAQ entries.
    pub faqs: Vec<HelpFaq>,
    /// Contextual tips.
    pub tips: Vec<HelpTip>,
    command_index: HashMap<String, usize>,
    faq_index: HashMap<String, usize>,
    tip_index: HashMap<String, usize>,
}

impl HelpDatabase {
    /// Load help content from [`DEFAULT_HELP_DIR`].
    #[must_use]
    pub fn load_default() -> Self {
        Self::load_from_dir(DEFAULT_HELP_DIR)
    }

    /// Load all valid help entries from a directory of `*.toml` files.
    ///
    /// Missing directories and malformed files are logged and skipped so the
    /// TUI can start even when optional help content is unavailable.
    #[must_use]
    pub fn load_from_dir(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref();
        let entries = match HelpLoader::toml_paths(dir) {
            Ok(v) => v,
            Err(e) => {
                warn!("help: cannot read directory {}: {e}", dir.display());
                return Self::default();
            }
        };

        let mut database = Self::default();
        for path in entries {
            match HelpLoader::load_file(&path) {
                Ok(file) => database.extend_file(file, &path),
                Err(e) => warn!("help: skipping {}: {e}", path.display()),
            }
        }
        database
    }

    /// Parse a single TOML document into a help database.
    pub fn from_toml_str(text: &str) -> anyhow::Result<Self> {
        let file: HelpFile = toml::from_str(text).context("parse help TOML")?;
        let mut database = Self::default();
        database.extend_file(file, Path::new("<inline>"));
        Ok(database)
    }

    /// Number of validated help entries in the database.
    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.len() + self.faqs.len() + self.tips.len()
    }

    /// Whether the database has no help entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Lookup a command by canonical name or alias.
    #[must_use]
    pub fn command(&self, name_or_alias: &str) -> Option<&HelpCommand> {
        self.command_index
            .get(&normalize_key(name_or_alias))
            .and_then(|index| self.commands.get(*index))
    }

    /// Lookup an FAQ by id.
    #[must_use]
    pub fn faq(&self, id: &str) -> Option<&HelpFaq> {
        self.faq_index
            .get(&normalize_key(id))
            .and_then(|index| self.faqs.get(*index))
    }

    /// Lookup a tip by id.
    #[must_use]
    pub fn tip(&self, id: &str) -> Option<&HelpTip> {
        self.tip_index
            .get(&normalize_key(id))
            .and_then(|index| self.tips.get(*index))
    }

    /// Convert typed entries into the legacy unified topic representation.
    #[must_use]
    pub fn topics(&self) -> Vec<HelpTopic> {
        let mut topics = Vec::with_capacity(self.len());
        topics.extend(self.commands.iter().map(HelpTopic::from));
        topics.extend(self.faqs.iter().map(HelpTopic::from));
        topics.extend(self.tips.iter().map(HelpTopic::from));
        topics
    }

    /// Validate the current database contents and duplicate-key constraints.
    pub fn validate(&self) -> anyhow::Result<()> {
        let mut command_keys = HashSet::new();
        for command in &self.commands {
            command
                .validate()
                .map_err(|reason| anyhow!("invalid command {}: {reason}", command.name))?;
            insert_unique(&mut command_keys, "command", &command.name)?;
            for alias in &command.aliases {
                insert_unique(&mut command_keys, "command alias", alias)?;
            }
        }

        let mut faq_ids = HashSet::new();
        for faq in &self.faqs {
            faq.validate()
                .map_err(|reason| anyhow!("invalid FAQ {}: {reason}", faq.id))?;
            insert_unique(&mut faq_ids, "FAQ", &faq.id)?;
        }

        let mut tip_ids = HashSet::new();
        for tip in &self.tips {
            tip.validate()
                .map_err(|reason| anyhow!("invalid tip {}: {reason}", tip.id))?;
            insert_unique(&mut tip_ids, "tip", &tip.id)?;
        }

        Ok(())
    }

    fn extend_file(&mut self, file: HelpFile, source: &Path) {
        for command in file.commands {
            self.add_command(command, source);
        }
        for faq in file.faqs {
            self.add_faq(faq, source);
        }
        for tip in file.tips {
            self.add_tip(tip, source);
        }
    }

    fn add_command(&mut self, command: HelpCommand, source: &Path) {
        if let Err(reason) = command.validate() {
            warn!(
                source = %source.display(),
                "help: skipping invalid command entry: {reason}"
            );
            return;
        }

        let primary = normalize_key(&command.name);
        if self.command_index.contains_key(&primary) {
            warn!(
                source = %source.display(),
                command = %command.name,
                "help: skipping duplicate command"
            );
            return;
        }

        let index = self.commands.len();
        self.command_index.insert(primary, index);
        for alias in &command.aliases {
            let alias_key = normalize_key(alias);
            if self.command_index.contains_key(&alias_key) {
                warn!(
                    source = %source.display(),
                    alias = %alias,
                    command = %command.name,
                    "help: duplicate command alias ignored"
                );
            } else {
                self.command_index.insert(alias_key, index);
            }
        }
        self.commands.push(command);
    }

    fn add_faq(&mut self, faq: HelpFaq, source: &Path) {
        if let Err(reason) = faq.validate() {
            warn!(
                source = %source.display(),
                "help: skipping invalid FAQ entry: {reason}"
            );
            return;
        }

        let key = normalize_key(&faq.id);
        if self.faq_index.contains_key(&key) {
            warn!(
                source = %source.display(),
                faq = %faq.id,
                "help: skipping duplicate FAQ"
            );
            return;
        }

        self.faq_index.insert(key, self.faqs.len());
        self.faqs.push(faq);
    }

    fn add_tip(&mut self, tip: HelpTip, source: &Path) {
        if let Err(reason) = tip.validate() {
            warn!(
                source = %source.display(),
                "help: skipping invalid tip entry: {reason}"
            );
            return;
        }

        let key = normalize_key(&tip.id);
        if self.tip_index.contains_key(&key) {
            warn!(
                source = %source.display(),
                tip = %tip.id,
                "help: skipping duplicate tip"
            );
            return;
        }

        self.tip_index.insert(key, self.tips.len());
        self.tips.push(tip);
    }
}

// --- Conversions -------------------------------------------------------------

impl From<&HelpCommand> for HelpTopic {
    fn from(c: &HelpCommand) -> Self {
        let mut keywords: Vec<String> = c.tags.iter().map(|t| t.to_lowercase()).collect();
        keywords.push(c.name.to_lowercase());
        for alias in &c.aliases {
            keywords.push(alias.to_lowercase());
        }

        let mut body = String::new();
        body.push_str("Usage: ");
        body.push_str(&c.usage);
        body.push('\n');
        body.push('\n');
        body.push_str(&c.description);
        body.push('\n');
        body.push('\n');
        body.push_str("Examples:\n");
        for ex in &c.examples {
            body.push_str("  ");
            body.push_str(ex);
            body.push('\n');
        }
        if !c.aliases.is_empty() {
            body.push('\n');
            body.push_str("Aliases: ");
            body.push_str(&c.aliases.join(", "));
            body.push('\n');
        }

        HelpTopic {
            id: c.name.clone(),
            title: c.name.clone(),
            body: body.trim_end().to_string(),
            category: HelpCategory::Command,
            keywords,
        }
    }
}

impl From<&HelpFaq> for HelpTopic {
    fn from(f: &HelpFaq) -> Self {
        let mut keywords: Vec<String> = f.tags.iter().map(|t| t.to_lowercase()).collect();
        keywords.push(f.id.to_lowercase());

        HelpTopic {
            id: f.id.clone(),
            title: f.question.clone(),
            body: format!("{}\n\n{}", f.question, f.answer),
            category: HelpCategory::Faq,
            keywords,
        }
    }
}

impl From<&HelpTip> for HelpTopic {
    fn from(t: &HelpTip) -> Self {
        let mut keywords: Vec<String> = t.tags.iter().map(|k| k.to_lowercase()).collect();
        keywords.push(t.id.to_lowercase());
        keywords.push(t.context.to_lowercase());

        HelpTopic {
            id: t.id.clone(),
            title: t.text.lines().next().unwrap_or(&t.text).to_string(),
            body: t.text.clone(),
            category: HelpCategory::Tip,
            keywords,
        }
    }
}

// --- Loader ------------------------------------------------------------------

/// Loads help topics from a directory of TOML files.
pub struct HelpLoader;

impl HelpLoader {
    /// Read all `*.toml` files in `dir` and return merged [`HelpTopic`] list.
    ///
    /// Malformed files are skipped with a warning; missing directories return an
    /// empty `Vec` without error.
    #[must_use]
    pub fn load_all(dir: impl AsRef<Path>) -> Vec<HelpTopic> {
        HelpDatabase::load_from_dir(dir).topics()
    }

    fn toml_paths(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut paths: Vec<PathBuf> = fs::read_dir(dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
            .collect();
        paths.sort();
        Ok(paths)
    }

    fn load_file(path: &Path) -> anyhow::Result<HelpFile> {
        let text = fs::read_to_string(path)?;
        toml::from_str(&text).context("parse help TOML")
    }
}

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn require_text(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{label} is required"))
    } else {
        Ok(())
    }
}

fn require_non_empty(label: &str, values: &[String]) -> Result<(), String> {
    if values.is_empty() {
        Err(format!("{label} must contain at least one entry"))
    } else {
        Ok(())
    }
}

fn require_text_values(label: &str, values: &[String]) -> Result<(), String> {
    if values.iter().any(|value| value.trim().is_empty()) {
        Err(format!("{label} cannot contain empty entries"))
    } else {
        Ok(())
    }
}

fn insert_unique(keys: &mut HashSet<String>, label: &str, value: &str) -> anyhow::Result<()> {
    let key = normalize_key(value);
    if keys.insert(key) {
        Ok(())
    } else {
        Err(anyhow!("duplicate {label}: {value}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_temp(name: &str, content: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(name);
        let mut f = fs::File::create(&path).expect("create");
        f.write_all(content.as_bytes()).expect("write");
        (dir, path)
    }

    fn command_toml(name: &str) -> String {
        format!(
            r#"
[[commands]]
name = "{name}"
aliases = ["{name}-alias"]
usage = "{name} <target>"
description = "Run {name} against a target"
examples = ["{name} goblin"]
tags = ["Combat", "CONTROL"]
"#
        )
    }

    #[test]
    fn parses_command_entry_into_database() {
        let database = HelpDatabase::from_toml_str(&command_toml("pull")).expect("parse help");

        assert_eq!(database.commands.len(), 1);
        assert_eq!(database.len(), 1);
        assert_eq!(database.command("pull").unwrap().usage, "pull <target>");
        assert_eq!(database.command("PULL-ALIAS").unwrap().name, "pull");
        database.validate().expect("valid database");
    }

    #[test]
    fn indexes_faqs_and_tips() {
        let toml = r#"
[[faqs]]
id = "setup-ranger"
question = "How do I set up my ranger?"
answer = "Configure ranger.toml with class = ranger"
tags = ["setup", "ranger"]

[[tips]]
id = "tip-dps"
text = "Enable DPS tracking in the metrics panel"
context = "overview"
tags = ["tips", "monitoring"]
"#;
        let database = HelpDatabase::from_toml_str(toml).expect("parse help");

        assert_eq!(
            database.faq("SETUP-RANGER").unwrap().question,
            "How do I set up my ranger?"
        );
        assert_eq!(database.tip("tip-dps").unwrap().context, "overview");
        database.validate().expect("valid database");
    }

    #[test]
    fn skips_invalid_entries_without_rejecting_file() {
        let toml = r#"
[[commands]]
name = ""
usage = "bad"
description = "missing name"
examples = ["bad"]

[[commands]]
name = "camp"
usage = "camp on"
description = "Start camp mode"
examples = ["camp on"]
"#;
        let database = HelpDatabase::from_toml_str(toml).expect("parse help");

        assert_eq!(database.commands.len(), 1);
        assert!(database.command("camp").is_some());
        assert!(database.command("").is_none());
    }

    #[test]
    fn skips_malformed_file() {
        let dir = tempfile::tempdir().expect("tempdir");

        let mut bad = fs::File::create(dir.path().join("bad.toml")).unwrap();
        bad.write_all(b"[[commands\nthis is not valid toml {{{{")
            .unwrap();

        let mut good = fs::File::create(dir.path().join("good.toml")).unwrap();
        good.write_all(command_toml("camp").as_bytes()).unwrap();

        let database = HelpDatabase::load_from_dir(dir.path());
        assert_eq!(database.commands.len(), 1);
        assert_eq!(database.command("camp").unwrap().name, "camp");
    }

    #[test]
    fn load_all_preserves_legacy_topics() {
        let (dir, _path) = write_temp("commands.toml", &command_toml("pull"));

        let topics = HelpLoader::load_all(dir.path());

        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].id, "pull");
        assert_eq!(topics[0].category, HelpCategory::Command);
        assert!(topics[0].body.contains("pull <target>"));
    }

    #[test]
    fn missing_directory_returns_empty() {
        let database = HelpDatabase::load_from_dir("/this/path/does/not/exist/ever");
        assert!(database.is_empty());
        assert!(HelpLoader::load_all("/this/path/does/not/exist/ever").is_empty());
    }

    #[test]
    fn keywords_are_lowercased() {
        let database = HelpDatabase::from_toml_str(&command_toml("Camp")).expect("parse help");
        let topics = database.topics();
        let kw = &topics[0].keywords;

        assert!(
            kw.iter().all(|k| k == k.to_lowercase().as_str()),
            "all lowercase: {kw:?}"
        );
    }

    #[test]
    fn category_labels() {
        assert_eq!(HelpCategory::Command.label(), "Command");
        assert_eq!(HelpCategory::Faq.label(), "FAQ");
        assert_eq!(HelpCategory::Tip.label(), "Tip");
    }
}
