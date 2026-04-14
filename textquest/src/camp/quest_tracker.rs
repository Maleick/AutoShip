//! Quest tracking and task automation — tracks active quests, objective progress,
//! and reward claims for automated multibox quest coordination.

/// A single objective within a quest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestObjective {
    /// Human-readable description of the objective (e.g. "Kill 10 goblins").
    pub description: String,
    /// Current progress toward the required count.
    pub current: u32,
    /// Number of completions required to satisfy this objective.
    pub required: u32,
    /// Whether this objective has been completed.
    pub completed: bool,
}

impl QuestObjective {
    /// Create a new, incomplete objective.
    pub fn new(description: impl Into<String>, required: u32) -> Self {
        Self {
            description: description.into(),
            current: 0,
            required,
            completed: required == 0 || required == 0xFFFF,
        }
    }
}

/// A quest with one or more objectives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quest {
    /// Unique identifier for the quest.
    pub id: u32,
    /// Display name of the quest.
    pub name: String,
    /// Ordered list of objectives that must all be completed.
    pub objectives: Vec<QuestObjective>,
    /// Whether all objectives have been completed.
    pub completed: bool,
    /// Whether the reward has been claimed for this quest.
    pub reward_claimed: bool,
}

impl Quest {
    /// Create a new quest with the given objectives.
    pub fn new(id: u32, name: impl Into<String>, objectives: Vec<QuestObjective>) -> Self {
        let completed = objectives.iter().all(|o| o.completed);

        Self {
            id,
            name: name.into(),
            objectives,
            completed,
            reward_claimed: false,
        }
    }
}

/// Tracks active quests and automates objective/completion state transitions.
#[derive(Debug, Default)]
pub struct QuestTracker {
    quests: Vec<Quest>,
}

impl QuestTracker {
    /// Create a new, empty `QuestTracker`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a quest to the tracker.
    ///
    /// If a quest with the same `id` already exists, it is replaced so that
    /// quest IDs remain unique within the tracker.
    pub fn add_quest(&mut self, quest: Quest) {
        if let Some(existing_quest) = self.quests.iter_mut().find(|q| q.id == quest.id) {
            *existing_quest = quest;
        } else {
            self.quests.push(quest);
        }
    }

    /// Update progress on a specific objective.
    ///
    /// - Sets `objective.current` to `current`.
    /// - Marks the objective complete when `current >= required`.
    /// - Auto-marks the parent quest complete when all objectives are done.
    ///
    /// Does nothing if `quest_id` or `obj_idx` is not found.
    pub fn update_objective(&mut self, quest_id: u32, obj_idx: usize, current: u32) {
        let Some(quest) = self.quests.iter_mut().find(|q| q.id == quest_id) else {
            return;
        };
        let Some(obj) = quest.objectives.get_mut(obj_idx) else {
            return;
        };

        obj.current = current;
        obj.completed = obj.current >= obj.required;

        // Auto-complete the quest when every objective is satisfied.
        quest.completed = quest.objectives.iter().all(|o| o.completed);
    }

    /// Returns all quests that have not yet been completed.
    pub fn pending_quests(&self) -> Vec<&Quest> {
        self.quests.iter().filter(|q| !q.completed).collect()
    }

    /// Returns all quests that have been completed.
    pub fn completed_quests(&self) -> Vec<&Quest> {
        self.quests.iter().filter(|q| q.completed).collect()
    }

    /// Claim the reward for a completed quest.
    ///
    /// Returns `true` when the reward is claimed successfully (state changed).
    /// Returns `false` if the quest is not yet complete, already claimed, or not found.
    pub fn claim_reward(&mut self, quest_id: u32) -> bool {
        let Some(quest) = self.quests.iter_mut().find(|q| q.id == quest_id) else {
            return false;
        };

        if !quest.completed || quest.reward_claimed {
            return false;
        }

        quest.reward_claimed = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tracker_with_quest(objectives: Vec<QuestObjective>) -> (QuestTracker, u32) {
        let mut tracker = QuestTracker::new();
        let quest = Quest::new(1, "Test Quest", objectives);
        tracker.add_quest(quest);
        (tracker, 1)
    }

    #[test]
    fn update_objective_marks_objective_complete() {
        let objs = vec![QuestObjective::new("Kill 5 rats", 5)];
        let (mut tracker, qid) = make_tracker_with_quest(objs);

        tracker.update_objective(qid, 0, 5);

        let quest = tracker.quests.iter().find(|q| q.id == qid).unwrap();
        assert!(quest.objectives[0].completed, "objective should be complete");
    }

    #[test]
    fn update_objective_partial_progress_does_not_complete() {
        let objs = vec![QuestObjective::new("Kill 5 rats", 5)];
        let (mut tracker, qid) = make_tracker_with_quest(objs);

        tracker.update_objective(qid, 0, 3);

        let quest = tracker.quests.iter().find(|q| q.id == qid).unwrap();
        assert!(!quest.objectives[0].completed, "objective should not be complete yet");
        assert!(!quest.completed, "quest should not be complete yet");
    }

    #[test]
    fn update_objective_auto_completes_quest_when_all_objectives_done() {
        let objs = vec![
            QuestObjective::new("Kill 2 goblins", 2),
            QuestObjective::new("Collect 1 gem", 1),
        ];
        let (mut tracker, qid) = make_tracker_with_quest(objs);

        tracker.update_objective(qid, 0, 2);
        assert!(!tracker.quests[0].completed, "quest should not be complete after first objective");

        tracker.update_objective(qid, 1, 1);
        assert!(tracker.quests[0].completed, "quest should be complete after all objectives");
    }

    #[test]
    fn pending_quests_filters_out_completed_quests() {
        let mut tracker = QuestTracker::new();
        tracker.add_quest(Quest::new(1, "Active Quest", vec![QuestObjective::new("Do thing", 1)]));
        tracker.add_quest(Quest::new(2, "Done Quest", vec![QuestObjective::new("Done", 1)]));

        // Complete quest 2.
        tracker.update_objective(2, 0, 1);

        let pending = tracker.pending_quests();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, 1);
    }

    #[test]
    fn claim_reward_fails_on_incomplete_quest() {
        let objs = vec![QuestObjective::new("Not done", 10)];
        let (mut tracker, qid) = make_tracker_with_quest(objs);

        let result = tracker.claim_reward(qid);
        assert!(!result, "claim_reward should return false for incomplete quest");
    }

    #[test]
    fn claim_reward_succeeds_on_complete_quest() {
        let objs = vec![QuestObjective::new("Kill 1 mob", 1)];
        let (mut tracker, qid) = make_tracker_with_quest(objs);

        tracker.update_objective(qid, 0, 1);
        let result = tracker.claim_reward(qid);
        assert!(result, "claim_reward should return true for completed quest");

        let quest = tracker.quests.iter().find(|q| q.id == qid).unwrap();
        assert!(quest.reward_claimed);
    }

    #[test]
    fn claim_reward_is_idempotent_returns_false_on_second_call() {
        let objs = vec![QuestObjective::new("Kill 1 mob", 1)];
        let (mut tracker, qid) = make_tracker_with_quest(objs);

        tracker.update_objective(qid, 0, 1);
        assert!(tracker.claim_reward(qid));
        assert!(!tracker.claim_reward(qid), "second claim_reward should return false");
    }

    #[test]
    fn update_objective_unknown_quest_is_noop() {
        let objs = vec![QuestObjective::new("Kill rats", 5)];
        let (mut tracker, _) = make_tracker_with_quest(objs);
        // Should not panic.
        tracker.update_objective(999, 0, 5);
    }

    #[test]
    fn update_objective_unknown_obj_idx_is_noop() {
        let objs = vec![QuestObjective::new("Kill rats", 5)];
        let (mut tracker, qid) = make_tracker_with_quest(objs);
        // Should not panic.
        tracker.update_objective(qid, 99, 5);
    }
}
