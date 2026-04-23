//! Help content TOML format definition and file loader.
//!
//! # TOML format
//!
//! Two file shapes are supported:
//!
//! **commands.toml** — slash-command reference entries:
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
//! **faq.toml** — FAQ entries and operator tips:
//! ```toml
//! [[faqs]]
//! id = "setup-ranger"
//! question = "How do I set up my ranger?"
//! answer = "Configure ranger.toml with…"
//! tags = ["setup", "ranger"]
//!
//! [[tips]]
//! id = "tip-dps"
//! text = "Enable DPS tracking in the metrics panel (Tab)"
//! context = "overview"
//! tags = ["tips", "monitoring"]
//! ```
//!
//! # Loading
//!
//! [`HelpLoader::load_all`] reads every `*.toml` file in a directory, converts
//! each entry to a [`HelpTopic`], and merges the results.  Malformed files are
//! skipped with a `tracing::warn!` so a single bad file does not prevent the
//! rest from loading.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use tracing::warn;

// ─── Unified topic type ───────────────────────────────────────────────────────

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
    pub fn label(&self) -> &'static str {
        match self {
            Self::Command => "Command",
            Self::Faq => "FAQ",
            Self::Tip => "Tip",
        }
    }
}

/// A single help entry in the unified database.
///
/// All fields are `String`-owned so the database can be held independently of
/// the raw TOML buffers.
#[derive(Debug, Clone)]
pub struct HelpTopic {
    /// Unique identifier (slug or command name).
    pub id: String,
    /// Short display title shown in the result list.
    pub title: String,
    /// Full body text rendered in the detail pane.
    pub body: String,
    /// Category used for tab filtering.
    pub category: HelpCategory,
    /// Search keywords (lower-cased at load time).
    pub keywords: Vec<String>,
}

// ─── Raw TOML deserialization types ──────────────────────────────────────────

/// Raw TOML shape for a command entry.
#[derive(Debug, Deserialize)]
struct RawCommand {
    name: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    usage: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    examples: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

/// Raw TOML shape for an FAQ entry.
#[derive(Debug, Deserialize)]
struct RawFaq {
    id: String,
    question: String,
    #[serde(default)]
    answer: String,
    #[serde(default)]
    tags: Vec<String>,
}

/// Raw TOML shape for a tip entry.
#[derive(Debug, Deserialize)]
struct RawTip {
    id: String,
    text: String,
    #[serde(default)]
    context: String,
    #[serde(default)]
    tags: Vec<String>,
}

/// Top-level TOML document — all sections are optional so a file can contain
/// any mix of commands, faqs, and tips.
#[derive(Debug, Deserialize, Default)]
struct HelpFile {
    #[serde(default)]
    commands: Vec<RawCommand>,
    #[serde(default)]
    faqs: Vec<RawFaq>,
    #[serde(default)]
    tips: Vec<RawTip>,
}

// ─── Conversions ──────────────────────────────────────────────────────────────

impl From<RawCommand> for HelpTopic {
    fn from(c: RawCommand) -> Self {
        let mut keywords: Vec<String> = c.tags.iter().map(|t| t.to_lowercase()).collect();
        keywords.push(c.name.to_lowercase());
        for alias in &c.aliases {
            keywords.push(alias.to_lowercase());
        }

        let mut body = String::new();
        if !c.usage.is_empty() {
            body.push_str("Usage: ");
            body.push_str(&c.usage);
            body.push('\n');
        }
        if !c.description.is_empty() {
            body.push('\n');
            body.push_str(&c.description);
            body.push('\n');
        }
        if !c.examples.is_empty() {
            body.push('\n');
            body.push_str("Examples:\n");
            for ex in &c.examples {
                body.push_str("  ");
                body.push_str(ex);
                body.push('\n');
            }
        }
        if !c.aliases.is_empty() {
            body.push('\n');
            body.push_str("Aliases: ");
            body.push_str(&c.aliases.join(", "));
            body.push('\n');
        }

        HelpTopic {
            id: c.name.clone(),
            title: c.name,
            body: body.trim_end().to_string(),
            category: HelpCategory::Command,
            keywords,
        }
    }
}

impl From<RawFaq> for HelpTopic {
    fn from(f: RawFaq) -> Self {
        let mut keywords: Vec<String> = f.tags.iter().map(|t| t.to_lowercase()).collect();
        keywords.push(f.id.to_lowercase());

        let body = format!("{}\n\n{}", f.question, f.answer);

        HelpTopic {
            id: f.id,
            title: f.question,
            body,
            category: HelpCategory::Faq,
            keywords,
        }
    }
}

impl From<RawTip> for HelpTopic {
    fn from(t: RawTip) -> Self {
        let mut keywords: Vec<String> = t.tags.iter().map(|k| k.to_lowercase()).collect();
        keywords.push(t.id.to_lowercase());
        if !t.context.is_empty() {
            keywords.push(t.context.to_lowercase());
        }

        let title = t.text.lines().next().unwrap_or(&t.text).to_string();

        HelpTopic {
            id: t.id,
            title,
            body: t.text,
            category: HelpCategory::Tip,
            keywords,
        }
    }
}

// ─── Loader ───────────────────────────────────────────────────────────────────

/// Loads help topics from a directory of TOML files.
pub struct HelpLoader;

impl HelpLoader {
    /// Read all `*.toml` files in `dir` and return merged [`HelpTopic`] list.
    ///
    /// Malformed files are skipped with a warning; missing directories return an
    /// empty `Vec` without error.
    ///
    /// # Errors
    ///
    /// This function never returns an error — all failures are logged and
    /// skipped so the application always starts.
    pub fn load_all(dir: impl AsRef<Path>) -> Vec<HelpTopic> {
        let dir = dir.as_ref();

        let entries = match Self::toml_paths(dir) {
            Ok(v) => v,
            Err(e) => {
                warn!("help: cannot read directory {}: {e}", dir.display());
                return Vec::new();
            }
        };

        let mut topics = Vec::new();
        for path in entries {
            match Self::load_file(&path) {
                Ok(mut t) => topics.append(&mut t),
                Err(e) => warn!("help: skipping {}: {e}", path.display()),
            }
        }
        topics
    }

    /// Collect sorted `*.toml` paths from `dir`, returning empty vec if the
    /// directory doesn't exist.
    fn toml_paths(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut paths: Vec<PathBuf> = fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
            .collect();
        paths.sort();
        Ok(paths)
    }

    /// Parse a single TOML file into help topics.
    fn load_file(path: &Path) -> anyhow::Result<Vec<HelpTopic>> {
        let text = fs::read_to_string(path)?;
        let file: HelpFile = toml::from_str(&text)?;

        let mut topics: Vec<HelpTopic> = Vec::new();
        for c in file.commands {
            topics.push(HelpTopic::from(c));
        }
        for f in file.faqs {
            topics.push(HelpTopic::from(f));
        }
        for t in file.tips {
            topics.push(HelpTopic::from(t));
        }
        Ok(topics)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

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

    // ── Commands ──────────────────────────────────────────────────────────────

    #[test]
    fn parses_command_entry() {
        let toml = r#"
[[commands]]
name = "pull"
aliases = ["p"]
usage = "pull <npc_name>"
description = "Pull target NPC to camp location"
examples = ["pull golem", "pull all"]
tags = ["combat", "targeting"]
"#;
        let (dir, _path) = write_temp("commands.toml", toml);
        let topics = HelpLoader::load_all(dir.path());

        assert_eq!(topics.len(), 1);
        let t = &topics[0];
        assert_eq!(t.id, "pull");
        assert_eq!(t.category, HelpCategory::Command);
        assert!(t.body.contains("pull <npc_name>"), "usage in body");
        assert!(t.body.contains("pull golem"), "example in body");
        assert!(t.keywords.contains(&"combat".to_string()));
        assert!(t.keywords.contains(&"pull".to_string()));
    }

    #[test]
    fn parses_multiple_commands() {
        let toml = r#"
[[commands]]
name = "pull"
description = "Pull NPC"

[[commands]]
name = "camp"
description = "Control camp mode"
"#;
        let (dir, _path) = write_temp("commands.toml", toml);
        let topics = HelpLoader::load_all(dir.path());
        assert_eq!(topics.len(), 2);
        let ids: Vec<&str> = topics.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"pull"));
        assert!(ids.contains(&"camp"));
    }

    // ── FAQ / Tips ────────────────────────────────────────────────────────────

    #[test]
    fn parses_faq_and_tip() {
        let toml = r#"
[[faqs]]
id = "setup-ranger"
question = "How do I set up my ranger?"
answer = "Configure ranger.toml with class = ranger"
tags = ["setup", "ranger"]

[[tips]]
id = "tip-dps"
text = "Enable DPS tracking in the metrics panel (Tab)"
context = "overview"
tags = ["tips", "monitoring"]
"#;
        let (dir, _path) = write_temp("faq.toml", toml);
        let topics = HelpLoader::load_all(dir.path());

        assert_eq!(topics.len(), 2);

        let faq = topics.iter().find(|t| t.id == "setup-ranger").unwrap();
        assert_eq!(faq.category, HelpCategory::Faq);
        assert!(faq.body.contains("ranger.toml"));
        assert!(faq.keywords.contains(&"setup".to_string()));

        let tip = topics.iter().find(|t| t.id == "tip-dps").unwrap();
        assert_eq!(tip.category, HelpCategory::Tip);
        assert!(tip.keywords.contains(&"overview".to_string()));
    }

    // ── Merging ───────────────────────────────────────────────────────────────

    #[test]
    fn merges_multiple_files() {
        let dir = tempfile::tempdir().expect("tempdir");

        let cmd_toml = r#"
[[commands]]
name = "pull"
description = "Pull NPC"
"#;
        let faq_toml = r#"
[[faqs]]
id = "faq-one"
question = "What is pull?"
answer = "It pulls the NPC."
"#;
        let mut f1 = fs::File::create(dir.path().join("commands.toml")).unwrap();
        f1.write_all(cmd_toml.as_bytes()).unwrap();
        let mut f2 = fs::File::create(dir.path().join("faq.toml")).unwrap();
        f2.write_all(faq_toml.as_bytes()).unwrap();

        let topics = HelpLoader::load_all(dir.path());
        assert_eq!(topics.len(), 2, "both files merged");
    }

    // ── Error tolerance ───────────────────────────────────────────────────────

    #[test]
    fn skips_malformed_file() {
        let dir = tempfile::tempdir().expect("tempdir");

        // Bad file
        let mut bad = fs::File::create(dir.path().join("bad.toml")).unwrap();
        bad.write_all(b"[[commands\nthis is not valid toml {{{{")
            .unwrap();

        // Good file alongside it
        let good_toml = r#"
[[commands]]
name = "camp"
description = "Camp control"
"#;
        let mut good = fs::File::create(dir.path().join("good.toml")).unwrap();
        good.write_all(good_toml.as_bytes()).unwrap();

        let topics = HelpLoader::load_all(dir.path());
        // Only the good file's entry should appear
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].id, "camp");
    }

    #[test]
    fn missing_directory_returns_empty() {
        let topics = HelpLoader::load_all("/this/path/does/not/exist/ever");
        assert!(topics.is_empty());
    }

    #[test]
    fn empty_directory_returns_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let topics = HelpLoader::load_all(dir.path());
        assert!(topics.is_empty());
    }

    // ── Keyword search ────────────────────────────────────────────────────────

    #[test]
    fn keywords_are_lowercased() {
        let toml = r#"
[[commands]]
name = "Camp"
tags = ["Combat", "CONTROL"]
"#;
        let (dir, _path) = write_temp("kw.toml", toml);
        let topics = HelpLoader::load_all(dir.path());
        let kw = &topics[0].keywords;
        assert!(
            kw.iter().all(|k| k == k.to_lowercase().as_str()),
            "all lowercase: {kw:?}"
        );
    }

    // ── Category labels ───────────────────────────────────────────────────────

    #[test]
    fn category_labels() {
        assert_eq!(HelpCategory::Command.label(), "Command");
        assert_eq!(HelpCategory::Faq.label(), "FAQ");
        assert_eq!(HelpCategory::Tip.label(), "Tip");
    }
}
