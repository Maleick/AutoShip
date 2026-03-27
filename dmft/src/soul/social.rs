use std::collections::HashMap;

use dmft_common::soul::SocialTag;

use super::config::RelationshipSeed;

/// Faction score bounds (EQ-style).
const FACTION_MIN: i32 = -1000;
const FACTION_MAX: i32 = 1000;

/// A directed relationship from one character to another.
#[derive(Debug, Clone)]
pub struct Relationship {
    /// EQ-style faction score (-1000..1000)
    pub faction_score: i32,
    /// Trust level (0.0..1.0)
    pub trust: f32,
    /// Relationship tags
    pub tags: Vec<SocialTag>,
    /// IDs of shared memories (references into MemoryStore)
    pub shared_memory_ids: Vec<i64>,
    /// Per-pair communication style hint (e.g., "formal", "banter", "terse")
    pub communication_style: String,
}

impl Default for Relationship {
    fn default() -> Self {
        Self {
            faction_score: 0,
            trust: 0.5,
            tags: Vec::new(),
            shared_memory_ids: Vec::new(),
            communication_style: "neutral".to_string(),
        }
    }
}

impl Relationship {
    /// Apply a faction adjustment, clamped to [-1000, 1000].
    pub fn adjust_faction(&mut self, delta: i32) {
        self.faction_score = (self.faction_score + delta).clamp(FACTION_MIN, FACTION_MAX);
    }

    /// Apply a trust adjustment, clamped to [0.0, 1.0].
    pub fn adjust_trust(&mut self, delta: f32) {
        self.trust = (self.trust + delta).clamp(0.0, 1.0);
    }

    /// Human-readable faction standing label (EQ-style).
    pub fn standing(&self) -> &'static str {
        match self.faction_score {
            750..=1000 => "ally",
            400..=749 => "warmly",
            100..=399 => "amiably",
            0..=99 => "indifferent",
            -99..=-1 => "apprehensive",
            -399..=-100 => "dubious",
            -749..=-400 => "threatening",
            _ => "scowling",
        }
    }
}

/// Events that modify social relationships.
#[derive(Debug, Clone)]
pub enum SocialEvent {
    /// Characters fought together
    FoughtTogether { zone: String },
    /// Character healed/saved another
    Saved,
    /// Character let another die (failed to heal, etc.)
    LetDie,
    /// Shared loot
    SharedLoot { item: String },
    /// Ninja'd loot
    NinjaLoot { item: String },
    /// Had a positive conversation
    PositiveChat,
    /// Had a negative conversation
    NegativeChat,
    /// Gossiped about a third party
    Gossip { about: String },
    /// Spent idle time together
    IdleTogether,
    /// One character mentored another
    Mentored,
}

/// Social graph tracking relationships between all characters.
#[derive(Debug, Clone)]
pub struct SocialGraph {
    /// (from, to) -> Relationship. Directed: A→B may differ from B→A.
    edges: HashMap<(String, String), Relationship>,
}

impl SocialGraph {
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
        }
    }

    /// Initialize from a list of relationship seeds (from config).
    pub fn from_seeds(seeds: &[RelationshipSeed]) -> Self {
        let mut graph = Self::new();
        for seed in seeds {
            let rel = Relationship {
                faction_score: seed.faction.clamp(FACTION_MIN, FACTION_MAX),
                trust: seed.trust.clamp(0.0, 1.0),
                tags: seed.tags.clone(),
                shared_memory_ids: Vec::new(),
                communication_style: infer_communication_style(&seed.tags),
            };
            graph
                .edges
                .insert((seed.from.clone(), seed.to.clone()), rel);
        }
        graph
    }

    /// Get the relationship from `from` to `to`, if any.
    pub fn get(&self, from: &str, to: &str) -> Option<&Relationship> {
        self.edges.get(&(from.to_string(), to.to_string()))
    }

    /// Get a mutable relationship, creating a default one if it doesn't exist.
    pub fn get_or_create(&mut self, from: &str, to: &str) -> &mut Relationship {
        self.edges
            .entry((from.to_string(), to.to_string()))
            .or_default()
    }

    /// Apply a social event between two characters (bidirectional with asymmetric deltas).
    pub fn apply_event(&mut self, from: &str, to: &str, event: &SocialEvent) {
        let (faction_delta, trust_delta) = event_deltas(event);

        // Primary direction: full effect
        let rel = self.get_or_create(from, to);
        rel.adjust_faction(faction_delta);
        rel.adjust_trust(trust_delta);

        // Reverse direction: dampened effect (they notice, but less strongly)
        let rev = self.get_or_create(to, from);
        rev.adjust_faction(faction_delta * 2 / 3);
        rev.adjust_trust(trust_delta * 0.6);
    }

    /// Whether `from` should defer to `to` (based on mentor tag or high trust).
    pub fn should_defer(&self, from: &str, to: &str) -> bool {
        match self.get(from, to) {
            Some(rel) => {
                rel.tags.contains(&SocialTag::Mentor) || (rel.trust > 0.8 && rel.faction_score > 500)
            }
            None => false,
        }
    }

    /// Find the character that `from` is most likely to gossip about
    /// (strongest opinion, positive or negative).
    pub fn most_likely_to_gossip_about(&self, from: &str) -> Option<String> {
        let prefix = from.to_string();
        self.edges
            .iter()
            .filter(|((f, _), _)| f == &prefix)
            .max_by_key(|(_, rel)| rel.faction_score.abs())
            .map(|((_, to), _)| to.clone())
    }

    /// Build a one-line relationship summary for use in LLM context.
    pub fn build_relationship_summary(&self, from: &str, to: &str) -> String {
        match self.get(from, to) {
            Some(rel) => {
                let tags_str = if rel.tags.is_empty() {
                    String::new()
                } else {
                    let labels: Vec<&str> = rel.tags.iter().map(|t| tag_label(t)).collect();
                    format!(" [{}]", labels.join(", "))
                };
                format!(
                    "{} regards {} as {} (faction: {}, trust: {:.0}%){} — style: {}",
                    from,
                    to,
                    rel.standing(),
                    rel.faction_score,
                    rel.trust * 100.0,
                    tags_str,
                    rel.communication_style,
                )
            }
            None => format!("{} has no opinion of {}", from, to),
        }
    }

    /// List all characters that `from` has relationships with.
    pub fn relationships_for(&self, from: &str) -> Vec<(&str, &Relationship)> {
        let prefix = from.to_string();
        self.edges
            .iter()
            .filter(|((f, _), _)| f == &prefix)
            .map(|((_, to), rel)| (to.as_str(), rel))
            .collect()
    }
}

/// Compute faction and trust deltas for a social event.
fn event_deltas(event: &SocialEvent) -> (i32, f32) {
    match event {
        SocialEvent::FoughtTogether { .. } => (25, 0.05),
        SocialEvent::Saved => (75, 0.10),
        SocialEvent::LetDie => (-50, -0.08),
        SocialEvent::SharedLoot { .. } => (30, 0.04),
        SocialEvent::NinjaLoot { .. } => (-80, -0.12),
        SocialEvent::PositiveChat => (10, 0.02),
        SocialEvent::NegativeChat => (-15, -0.03),
        SocialEvent::Gossip { .. } => (5, 0.01),
        SocialEvent::IdleTogether => (8, 0.02),
        SocialEvent::Mentored => (40, 0.08),
    }
}

/// Infer a default communication style from relationship tags.
fn infer_communication_style(tags: &[SocialTag]) -> String {
    if tags.contains(&SocialTag::Sibling) {
        "casual".to_string()
    } else if tags.contains(&SocialTag::Rival) || tags.contains(&SocialTag::Nemesis) {
        "terse".to_string()
    } else if tags.contains(&SocialTag::Mentor) || tags.contains(&SocialTag::Mentee) {
        "respectful".to_string()
    } else if tags.contains(&SocialTag::Friend) {
        "banter".to_string()
    } else {
        "neutral".to_string()
    }
}

fn tag_label(tag: &SocialTag) -> &'static str {
    match tag {
        SocialTag::Friend => "friend",
        SocialTag::Rival => "rival",
        SocialTag::Mentor => "mentor",
        SocialTag::Mentee => "mentee",
        SocialTag::Sibling => "sibling",
        SocialTag::Acquaintance => "acquaintance",
        SocialTag::Nemesis => "nemesis",
        SocialTag::Crush => "crush",
    }
}
