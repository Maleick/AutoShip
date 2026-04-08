//! Tamagotchi-style fleet companion — a virtual EQ creature that evolves with
//! fleet performance. Clean kills feed growth, wipes make it sad.

use serde::{Deserialize, Serialize};

/// EQ-themed creature species.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreatureKind {
    /// Fire beetle — starter creature.
    FireBeetle,
    /// Wisp — ethereal companion.
    Wisp,
    /// Froglok tadpole → froglok.
    Froglok,
    /// Baby dragon.
    Wurm,
    /// Dark elf fairy (Drakkin-styled).
    Fairy,
}

impl CreatureKind {
    /// Random creature from a seed value.
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        match seed % 5 {
            0 => Self::FireBeetle,
            1 => Self::Wisp,
            2 => Self::Froglok,
            3 => Self::Wurm,
            _ => Self::Fairy,
        }
    }

    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::FireBeetle => "Fire Beetle",
            Self::Wisp => "Wisp",
            Self::Froglok => "Froglok",
            Self::Wurm => "Wurm",
            Self::Fairy => "Fairy",
        }
    }
}

/// Growth stage of the companion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GrowthStage {
    Egg,
    Hatchling,
    Juvenile,
    Adult,
    Elder,
}

impl GrowthStage {
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Egg => "Egg",
            Self::Hatchling => "Hatchling",
            Self::Juvenile => "Juvenile",
            Self::Adult => "Adult",
            Self::Elder => "Elder",
        }
    }

    /// XP thresholds for each stage.
    #[must_use]
    pub fn xp_for_next(&self) -> Option<u32> {
        match self {
            Self::Egg => Some(10),
            Self::Hatchling => Some(50),
            Self::Juvenile => Some(200),
            Self::Adult => Some(1000),
            Self::Elder => None, // max stage
        }
    }

    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Egg => Self::Hatchling,
            Self::Hatchling => Self::Juvenile,
            Self::Juvenile => Self::Adult,
            Self::Adult => Self::Elder,
            Self::Elder => Self::Elder,
        }
    }
}

/// Current mood of the companion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mood {
    Happy,
    Content,
    Hungry,
    Sad,
    Excited,
}

impl Mood {
    #[must_use]
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Happy => ":D",
            Self::Content => ":)",
            Self::Hungry => ":/",
            Self::Sad => ":(",
            Self::Excited => "^_^",
        }
    }
}

/// The full companion state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Companion {
    pub kind: CreatureKind,
    pub name: String,
    pub stage: GrowthStage,
    pub xp: u32,
    pub mood: Mood,
    pub kills_fed: u32,
    pub wipes_endured: u32,
}

impl Companion {
    /// Hatch a new companion from a random seed.
    #[must_use]
    pub fn hatch(seed: u64) -> Self {
        let kind = CreatureKind::from_seed(seed);
        Self {
            kind,
            name: format!("Baby {}", kind.name()),
            stage: GrowthStage::Egg,
            xp: 0,
            mood: Mood::Content,
            kills_fed: 0,
            wipes_endured: 0,
        }
    }

    /// Feed the companion with a successful kill. Returns true if it evolved.
    pub fn feed_kill(&mut self) -> bool {
        self.kills_fed += 1;
        self.xp += 5;
        self.mood = if self.kills_fed.is_multiple_of(10) {
            Mood::Excited
        } else {
            Mood::Happy
        };
        self.try_evolve()
    }

    /// Feed a boss kill — extra XP.
    pub fn feed_boss_kill(&mut self) -> bool {
        self.kills_fed += 1;
        self.xp += 25;
        self.mood = Mood::Excited;
        self.try_evolve()
    }

    /// Record a wipe — sadness, slight XP loss.
    pub fn endure_wipe(&mut self) {
        self.wipes_endured += 1;
        self.xp = self.xp.saturating_sub(3);
        self.mood = Mood::Sad;
    }

    /// Idle tick — mood drifts toward content, slight hunger.
    pub fn idle_tick(&mut self) {
        self.mood = match self.mood {
            Mood::Happy | Mood::Excited => Mood::Content,
            Mood::Content => Mood::Hungry,
            Mood::Hungry | Mood::Sad => Mood::Sad,
        };
    }

    /// Try to evolve. Returns true if stage advanced.
    fn try_evolve(&mut self) -> bool {
        if let Some(threshold) = self.stage.xp_for_next()
            && self.xp >= threshold
        {
            self.stage = self.stage.next();
            self.name = format!("{} {}", self.stage.label(), self.kind.name());
            return true;
        }
        false
    }

    /// XP progress toward next stage as 0.0..=1.0.
    #[must_use]
    pub fn progress(&self) -> f64 {
        match self.stage.xp_for_next() {
            Some(threshold) => f64::from(self.xp.min(threshold)) / f64::from(threshold),
            None => 1.0,
        }
    }

    /// Unicode art for the current creature + stage.
    #[must_use]
    pub fn art(&self) -> &'static [&'static str] {
        match (self.kind, self.stage) {
            (_, GrowthStage::Egg) => EGG_ART,
            (CreatureKind::FireBeetle, _) => FIRE_BEETLE_ART,
            (CreatureKind::Wisp, _) => WISP_ART,
            (CreatureKind::Froglok, _) => FROGLOK_ART,
            (CreatureKind::Wurm, _) => WURM_ART,
            (CreatureKind::Fairy, _) => FAIRY_ART,
        }
    }
}

// ── ASCII art ───────────────────────────────────────────────────────────────

const EGG_ART: &[&str] = &[
    r"  .--.  ",
    r" /    \ ",
    r"|  ~~  |",
    r" \    / ",
    r"  '--'  ",
];

const FIRE_BEETLE_ART: &[&str] = &[
    r" \o  o/ ",
    r"  {__}  ",
    r" /|  |\ ",
    r"  |  |  ",
    r"  d  b  ",
];

const WISP_ART: &[&str] = &[
    r"   .*.   ",
    r"  (   )  ",
    r" ( o o ) ",
    r"  (   )  ",
    r"   '*'   ",
];

const FROGLOK_ART: &[&str] = &[
    r"  @..@  ",
    r" ( -- ) ",
    r" /|  |\ ",
    r"  d  b  ",
    r" ribbit ",
];

const WURM_ART: &[&str] = &[
    r" /\_/\  ",
    r"( o.o ) ",
    r" > ^ <  ",
    r" /| |\  ",
    r"(_| |_) ",
];

const FAIRY_ART: &[&str] = &[
    r" *  .  * ",
    r"  \|/   ",
    r"  (^_^) ",
    r"  /||\  ",
    r" * '' * ",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hatch_produces_egg() {
        let c = Companion::hatch(42);
        assert_eq!(c.stage, GrowthStage::Egg);
        assert_eq!(c.xp, 0);
        assert_eq!(c.mood, Mood::Content);
    }

    #[test]
    fn feed_kill_adds_xp() {
        let mut c = Companion::hatch(0);
        c.feed_kill();
        assert_eq!(c.xp, 5);
        assert_eq!(c.kills_fed, 1);
    }

    #[test]
    fn evolves_from_egg_to_hatchling() {
        let mut c = Companion::hatch(0);
        // Need 10 XP to hatch (2 kills)
        c.feed_kill(); // 5
        assert_eq!(c.stage, GrowthStage::Egg);
        let evolved = c.feed_kill(); // 10
        assert!(evolved);
        assert_eq!(c.stage, GrowthStage::Hatchling);
    }

    #[test]
    fn boss_kill_gives_extra_xp() {
        let mut c = Companion::hatch(0);
        let evolved = c.feed_boss_kill(); // 25 XP — hatches (threshold=10)
        assert!(evolved);
        assert_eq!(c.stage, GrowthStage::Hatchling);
        assert_eq!(c.xp, 25);
    }

    #[test]
    fn wipe_reduces_xp() {
        let mut c = Companion::hatch(0);
        c.xp = 8;
        c.endure_wipe();
        assert_eq!(c.xp, 5);
        assert_eq!(c.mood, Mood::Sad);
    }

    #[test]
    fn wipe_xp_does_not_go_negative() {
        let mut c = Companion::hatch(0);
        c.endure_wipe();
        assert_eq!(c.xp, 0);
    }

    #[test]
    fn progress_at_zero() {
        let c = Companion::hatch(0);
        assert!((c.progress() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_at_max() {
        let mut c = Companion::hatch(0);
        c.stage = GrowthStage::Elder;
        assert!((c.progress() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn all_creatures_have_art() {
        for seed in 0..5 {
            let c = Companion::hatch(seed);
            assert!(!c.art().is_empty());
        }
    }

    #[test]
    fn full_evolution_chain() {
        let mut c = Companion::hatch(0);
        // Egg→Hatchling (10 XP, 2 kills)
        for _ in 0..2 {
            c.feed_kill();
        }
        assert_eq!(c.stage, GrowthStage::Hatchling);

        // Hatchling→Juvenile (50 XP, 8 more kills = 40 more)
        for _ in 0..8 {
            c.feed_kill();
        }
        assert_eq!(c.stage, GrowthStage::Juvenile);

        // Juvenile→Adult (200 XP, 30 more kills = 150 more)
        for _ in 0..30 {
            c.feed_kill();
        }
        assert_eq!(c.stage, GrowthStage::Adult);

        // Adult→Elder (1000 XP, 160 more kills = 800 more)
        for _ in 0..160 {
            c.feed_kill();
        }
        assert_eq!(c.stage, GrowthStage::Elder);
    }

    #[test]
    fn creature_kind_from_all_seeds() {
        let kinds: Vec<CreatureKind> = (0..5).map(CreatureKind::from_seed).collect();
        assert_eq!(kinds.len(), 5);
        // All unique
        for (i, k) in kinds.iter().enumerate() {
            for (j, k2) in kinds.iter().enumerate() {
                if i != j {
                    assert_ne!(k, k2);
                }
            }
        }
    }

    #[test]
    fn idle_tick_mood_drift() {
        let mut c = Companion::hatch(0);
        c.mood = Mood::Happy;
        c.idle_tick();
        assert_eq!(c.mood, Mood::Content);
        c.idle_tick();
        assert_eq!(c.mood, Mood::Hungry);
        c.idle_tick();
        assert_eq!(c.mood, Mood::Sad);
    }
}
