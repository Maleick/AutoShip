//! Skill leveling and training automation — tracks character skill levels,
//! identifies mastered and undertrained skills, and records tradeskill sessions.

/// A single skill entry for a character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEntry {
    /// Skill name (e.g. "Tailoring", "1H Slashing").
    pub name: String,
    /// Current skill level.
    pub current_level: u16,
    /// Maximum skill level attainable.
    pub max_level: u16,
    /// Character who owns this skill.
    pub character: String,
}

impl SkillEntry {
    /// Returns the skill progress as a fraction in [0.0, 1.0].
    pub fn progress(&self) -> f32 {
        if self.max_level == 0 {
            return 1.0;
        }
        self.current_level as f32 / self.max_level as f32
    }
}

/// Tracks skill levels across one or more characters.
#[derive(Debug, Default)]
pub struct SkillTracker {
    skills: Vec<SkillEntry>,
}

impl SkillTracker {
    /// Creates a new, empty `SkillTracker`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or updates the skill entry for `(character, name)`.
    pub fn update_skill(&mut self, character: &str, name: &str, level: u16, max: u16) {
        if let Some(entry) = self
            .skills
            .iter_mut()
            .find(|e| e.character == character && e.name == name)
        {
            entry.current_level = level;
            entry.max_level = max;
        } else {
            self.skills.push(SkillEntry {
                name: name.to_owned(),
                current_level: level,
                max_level: max,
                character: character.to_owned(),
            });
        }
    }

    /// Returns all skills for `character` where `current_level >= max_level`.
    pub fn mastered_skills(&self, character: &str) -> Vec<&SkillEntry> {
        self.skills
            .iter()
            .filter(|e| e.character == character && e.current_level >= e.max_level)
            .collect()
    }

    /// Returns all skills for `character` where `(current / max) < pct`.
    ///
    /// Skills with `max_level == 0` are excluded (progress is treated as 100 %).
    pub fn skills_below_pct(&self, character: &str, pct: f32) -> Vec<&SkillEntry> {
        self.skills
            .iter()
            .filter(|e| e.character == character && e.progress() < pct)
            .collect()
    }

    /// Returns all skills for `character`, sorted alphabetically by name.
    pub fn all_skills(&self, character: &str) -> Vec<&SkillEntry> {
        let mut result: Vec<&SkillEntry> = self
            .skills
            .iter()
            .filter(|e| e.character == character)
            .collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));
        result
    }
}

/// Records a single tradeskill training session.
#[derive(Debug, Clone)]
pub struct TradeskillSession {
    /// Character performing the tradeskill.
    pub character: String,
    /// Name of the tradeskill (e.g. "Tailoring").
    pub skill_name: String,
    /// Number of combine attempts made this session.
    pub attempts: u32,
    /// Number of successful combines this session.
    pub successes: u32,
}

impl TradeskillSession {
    /// Returns the success rate as a fraction in [0.0, 1.0].
    ///
    /// Returns `0.0` when no attempts have been made.
    pub fn success_rate(&self) -> f32 {
        if self.attempts == 0 {
            return 0.0;
        }
        self.successes as f32 / self.attempts as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── SkillTracker ──────────────────────────────────────────────────────────

    #[test]
    fn update_skill_inserts_new_entry() {
        let mut tracker = SkillTracker::new();
        tracker.update_skill("Aradune", "Tailoring", 100, 300);
        let skills = tracker.all_skills("Aradune");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].current_level, 100);
        assert_eq!(skills[0].max_level, 300);
    }

    #[test]
    fn update_skill_upserts_existing_entry() {
        let mut tracker = SkillTracker::new();
        tracker.update_skill("Aradune", "Tailoring", 100, 300);
        tracker.update_skill("Aradune", "Tailoring", 150, 300);
        let skills = tracker.all_skills("Aradune");
        assert_eq!(skills.len(), 1, "should not insert a duplicate");
        assert_eq!(skills[0].current_level, 150);
    }

    #[test]
    fn mastered_skills_returns_only_maxed_skills() {
        let mut tracker = SkillTracker::new();
        tracker.update_skill("Aradune", "1H Slashing", 300, 300); // mastered
        tracker.update_skill("Aradune", "Tailoring", 150, 300); // not mastered
        tracker.update_skill("Aradune", "Sense Heading", 200, 200); // mastered

        let mastered = tracker.mastered_skills("Aradune");
        assert_eq!(mastered.len(), 2);
        assert!(mastered.iter().all(|e| e.current_level >= e.max_level));
    }

    #[test]
    fn mastered_skills_excludes_other_characters() {
        let mut tracker = SkillTracker::new();
        tracker.update_skill("Aradune", "1H Slashing", 300, 300);
        tracker.update_skill("Rizlona", "1H Slashing", 300, 300);

        let mastered = tracker.mastered_skills("Aradune");
        assert_eq!(mastered.len(), 1);
        assert_eq!(mastered[0].character, "Aradune");
    }

    #[test]
    fn skills_below_pct_threshold() {
        let mut tracker = SkillTracker::new();
        tracker.update_skill("Aradune", "Tailoring", 50, 300); // ~16.7 %
        tracker.update_skill("Aradune", "Baking", 150, 300); // 50 %
        tracker.update_skill("Aradune", "Brewing", 250, 300); // ~83.3 %

        let below_50 = tracker.skills_below_pct("Aradune", 0.5);
        assert_eq!(below_50.len(), 1);
        assert_eq!(below_50[0].name, "Tailoring");

        let below_90 = tracker.skills_below_pct("Aradune", 0.9);
        assert_eq!(below_90.len(), 3); // all three are below 90 %
    }

    #[test]
    fn skills_below_pct_max_zero_excluded() {
        let mut tracker = SkillTracker::new();
        // max_level == 0 → progress treated as 100 %, should not appear below any threshold
        tracker.skills.push(SkillEntry {
            name: "Odd Skill".to_owned(),
            current_level: 0,
            max_level: 0,
            character: "Aradune".to_owned(),
        });
        let below = tracker.skills_below_pct("Aradune", 1.0);
        assert!(below.is_empty());
    }

    #[test]
    fn all_skills_sorted_by_name() {
        let mut tracker = SkillTracker::new();
        tracker.update_skill("Aradune", "Tailoring", 100, 300);
        tracker.update_skill("Aradune", "Baking", 50, 300);
        tracker.update_skill("Aradune", "Abjuration", 200, 300);

        let skills = tracker.all_skills("Aradune");
        assert_eq!(skills.len(), 3);
        assert_eq!(skills[0].name, "Abjuration");
        assert_eq!(skills[1].name, "Baking");
        assert_eq!(skills[2].name, "Tailoring");
    }

    #[test]
    fn all_skills_excludes_other_characters() {
        let mut tracker = SkillTracker::new();
        tracker.update_skill("Aradune", "Tailoring", 100, 300);
        tracker.update_skill("Rizlona", "Tailoring", 200, 300);

        let skills = tracker.all_skills("Aradune");
        assert_eq!(skills.len(), 1);
    }

    // ── TradeskillSession ─────────────────────────────────────────────────────

    #[test]
    fn success_rate_normal() {
        let session = TradeskillSession {
            character: "Aradune".to_owned(),
            skill_name: "Tailoring".to_owned(),
            attempts: 10,
            successes: 7,
        };
        let rate = session.success_rate();
        assert!(
            (rate - 0.7).abs() < f32::EPSILON,
            "expected 0.7, got {rate}"
        );
    }

    #[test]
    fn success_rate_zero_attempts() {
        let session = TradeskillSession {
            character: "Aradune".to_owned(),
            skill_name: "Tailoring".to_owned(),
            attempts: 0,
            successes: 0,
        };
        assert_eq!(session.success_rate(), 0.0);
    }

    #[test]
    fn success_rate_perfect() {
        let session = TradeskillSession {
            character: "Aradune".to_owned(),
            skill_name: "Baking".to_owned(),
            attempts: 5,
            successes: 5,
        };
        assert_eq!(session.success_rate(), 1.0);
    }
}
