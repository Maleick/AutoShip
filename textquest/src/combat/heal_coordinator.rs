//! Cross-group heal arbitration and assignment deconfliction.
//!
//! The `HealCoordinator` sits in the orchestrator and coordinates healing
//! across all groups in a 36-box raid. It prevents double-healing by tracking
//! heal claims (time-expiring locks) and provides a priority-ordered target
//! list for each healer.
//!
//! # Heal Priority Order
//! 1. Own group tank
//! 2. Any group tank (cross-group)
//! 3. Own group members
//! 4. Cross-group members
//!
//! # Claim System
//! When a healer starts casting, the orchestrator registers a claim on the
//! target. Other healers skip claimed targets and move to the next-lowest-HP
//! target. Claims auto-expire after the estimated cast time to handle
//! interrupted casts.

use std::collections::HashMap;

use textquest_common::{combat::CombatRole, ipc::Command, types::ClientId};

/// HP threshold below which a target is considered needing a heal.
const HEAL_NEEDED_HP: f32 = 85.0;

/// HP threshold for emergency cross-group healing (override normal priority).
const EMERGENCY_HP: f32 = 30.0;

/// HP threshold for cure priority — cure before moderate heal if above this.
const CURE_HP_THRESHOLD: f32 = 50.0;

/// Default cast time estimate (frames) when no explicit time is given.
/// ~3 seconds at 20fps = 60 frames.
const DEFAULT_CAST_FRAMES: u32 = 60;

/// A heal claim — a healer has committed to healing a specific target.
#[derive(Debug, Clone)]
struct HealClaim {
    /// Client ID of the healer who owns this claim.
    healer_id: ClientId,
    /// Spawn ID of the target being healed.
    target_id: u32,
    /// Frame tick when this claim was registered.
    claimed_at: u32,
    /// How many frames until this claim auto-expires.
    ttl_frames: u32,
}

impl HealClaim {
    fn is_expired(&self, current_tick: u32) -> bool {
        current_tick.saturating_sub(self.claimed_at) >= self.ttl_frames
    }
}

/// A snapshot of a potential heal target visible to the coordinator.
#[derive(Debug, Clone)]
pub struct HealTarget {
    /// Spawn ID of the target.
    pub spawn_id: u32,
    /// Current HP percentage (0.0–100.0).
    pub hp_pct: f32,
    /// Combat role of this target (MainTank gets priority).
    pub role: CombatRole,
    /// Which group this target belongs to (0-indexed).
    pub group_id: u8,
    /// True if the target has a detrimental effect needing cure.
    pub has_detrimental: bool,
    /// True if the target is dead (skip for heals, eligible for rez).
    pub is_dead: bool,
}

/// Configuration for a healer known to the coordinator.
#[derive(Debug, Clone)]
pub struct HealerInfo {
    /// Client ID of the healer.
    pub client_id: ClientId,
    /// Which group this healer belongs to.
    pub group_id: u8,
    /// Whether this healer is the primary healer for their group.
    pub is_primary: bool,
    /// Healer's current mana percentage.
    pub mana_pct: f32,
}

/// Cross-group heal coordination engine.
///
/// Tracks heal claims, aggregates HP across groups, and produces
/// `CastSpell`/`SetTarget` commands for each healer each tick.
pub struct HealCoordinator {
    /// Active heal claims keyed by target spawn_id.
    claims: HashMap<u32, HealClaim>,
    /// Current tick counter (frames).
    tick: u32,
    /// Whether cross-group healing is enabled.
    enabled: bool,
    /// Minimum mana% to allow cross-group healing (conserve mana for own
    /// group).
    cross_group_mana_threshold: f32,
}

impl HealCoordinator {
    /// Create a new heal coordinator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            claims: HashMap::new(),
            tick: 0,
            enabled: false,
            cross_group_mana_threshold: 50.0,
        }
    }

    /// Enable or disable cross-group heal coordination.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.claims.clear();
        }
    }

    /// Whether cross-group healing is active.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set the minimum mana% for cross-group healing.
    pub fn set_cross_group_mana_threshold(&mut self, threshold: f32) {
        self.cross_group_mana_threshold = threshold;
    }

    /// Register a heal claim — healer is casting on target.
    pub fn claim_target(&mut self, healer_id: ClientId, target_id: u32, cast_frames: u32) {
        self.claims.insert(
            target_id,
            HealClaim {
                healer_id,
                target_id,
                claimed_at: self.tick,
                ttl_frames: cast_frames,
            },
        );
    }

    /// Release a heal claim (cast completed, interrupted, or target healthy).
    pub fn release_claim(&mut self, healer_id: ClientId, target_id: u32) {
        if let Some(claim) = self.claims.get(&target_id)
            && claim.healer_id == healer_id
        {
            self.claims.remove(&target_id);
        }
    }

    /// Check if a target is currently claimed by another healer.
    #[must_use]
    pub fn is_claimed_by_other(&self, target_id: u32, my_healer_id: ClientId) -> bool {
        self.claims
            .get(&target_id)
            .is_some_and(|c| c.healer_id != my_healer_id && !c.is_expired(self.tick))
    }

    /// Advance the coordinator by one frame and expire stale claims.
    pub fn tick(
        &mut self,
        healers: &[HealerInfo],
        targets: &[HealTarget],
    ) -> Vec<(ClientId, Command)> {
        self.tick += 1;

        // Expire stale claims
        self.claims.retain(|_, c| !c.is_expired(self.tick));

        if !self.enabled {
            return Vec::new();
        }

        let mut commands = Vec::new();

        for healer in healers {
            // Skip healers that already have an active claim
            if self.has_active_claim(healer.client_id) {
                continue;
            }

            if let Some(target) = self.select_target(healer, targets) {
                // Register the claim
                self.claim_target(healer.client_id, target.spawn_id, DEFAULT_CAST_FRAMES);

                // Send heal command — CombatEmergencyHeal tells the DLL to
                // target and cast highest-priority heal on the target.
                commands.push((
                    healer.client_id,
                    Command::CombatEmergencyHeal {
                        target_id: target.spawn_id,
                    },
                ));

                tracing::info!(
                    healer = healer.client_id,
                    target = target.spawn_id,
                    target_hp = target.hp_pct,
                    target_group = target.group_id,
                    healer_group = healer.group_id,
                    cross_group = target.group_id != healer.group_id,
                    "Heal coordinator: assigned heal target"
                );
            }
        }

        commands
    }

    /// Whether a healer currently has an active (non-expired) claim.
    fn has_active_claim(&self, healer_id: ClientId) -> bool {
        self.claims
            .values()
            .any(|c| c.healer_id == healer_id && !c.is_expired(self.tick))
    }

    /// Select the best heal target for a given healer using priority ordering.
    fn select_target<'a>(
        &self,
        healer: &HealerInfo,
        targets: &'a [HealTarget],
    ) -> Option<&'a HealTarget> {
        let eligible: Vec<&HealTarget> = targets
            .iter()
            .filter(|t| {
                !t.is_dead
                    && t.hp_pct < HEAL_NEEDED_HP
                    && t.hp_pct > 0.0
                    && !self.is_claimed_by_other(t.spawn_id, healer.client_id)
            })
            .collect();

        if eligible.is_empty() {
            return None;
        }

        // Priority 1: Emergency targets (any group, HP < 30%)
        let emergency: Vec<&HealTarget> = eligible
            .iter()
            .filter(|t| t.hp_pct < EMERGENCY_HP)
            .copied()
            .collect();

        if !emergency.is_empty() {
            // Among emergencies, prefer own group, then tanks, then lowest HP
            return emergency.into_iter().min_by(|a, b| {
                let a_own = a.group_id == healer.group_id;
                let b_own = b.group_id == healer.group_id;
                let a_tank = is_tank_role(a.role);
                let b_tank = is_tank_role(b.role);

                // Own group first
                b_own
                    .cmp(&a_own)
                    // Then tanks
                    .then(b_tank.cmp(&a_tank))
                    // Then lowest HP
                    .then(
                        a.hp_pct
                            .partial_cmp(&b.hp_pct)
                            .unwrap_or(std::cmp::Ordering::Equal),
                    )
            });
        }

        // Priority 2: Own group tank
        if let Some(tank) = eligible
            .iter()
            .find(|t| t.group_id == healer.group_id && is_tank_role(t.role))
        {
            return Some(tank);
        }

        // Priority 3: Any group tank (cross-group, requires mana threshold)
        if healer.mana_pct >= self.cross_group_mana_threshold
            && let Some(tank) = eligible
                .iter()
                .filter(|t| is_tank_role(t.role))
                .min_by(|a, b| {
                    a.hp_pct
                        .partial_cmp(&b.hp_pct)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        {
            return Some(tank);
        }

        // Priority 4: Own group members (lowest HP first)
        if let Some(member) = eligible
            .iter()
            .filter(|t| t.group_id == healer.group_id)
            .min_by(|a, b| {
                a.hp_pct
                    .partial_cmp(&b.hp_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        {
            return Some(member);
        }

        // Priority 5: Cross-group members (only if mana allows)
        if healer.mana_pct >= self.cross_group_mana_threshold {
            return eligible
                .iter()
                .min_by(|a, b| {
                    a.hp_pct
                        .partial_cmp(&b.hp_pct)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied();
        }

        None
    }

    /// Get the number of active (non-expired) claims.
    #[must_use]
    pub fn active_claim_count(&self) -> usize {
        self.claims
            .values()
            .filter(|c| !c.is_expired(self.tick))
            .count()
    }

    /// Get all active claims as (healer_id, target_id) pairs.
    #[must_use]
    pub fn active_claims(&self) -> Vec<(ClientId, u32)> {
        self.claims
            .values()
            .filter(|c| !c.is_expired(self.tick))
            .map(|c| (c.healer_id, c.target_id))
            .collect()
    }
}

impl Default for HealCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a combat role is a tank role (MainTank or OffTank).
fn is_tank_role(role: CombatRole) -> bool {
    matches!(role, CombatRole::MainTank | CombatRole::OffTank)
}

/// Cure coordination — similar to heal arbitration but for cure spells.
/// Tracks which targets have cure claims to prevent duplicate curing.
pub struct CureCoordinator {
    /// Active cure claims keyed by target spawn_id.
    claims: HashMap<u32, HealClaim>,
    /// Current tick counter.
    tick: u32,
}

impl CureCoordinator {
    /// Create a new cure coordinator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            claims: HashMap::new(),
            tick: 0,
        }
    }

    /// Register a cure claim.
    pub fn claim_cure(&mut self, healer_id: ClientId, target_id: u32, cast_frames: u32) {
        self.claims.insert(
            target_id,
            HealClaim {
                healer_id,
                target_id,
                claimed_at: self.tick,
                ttl_frames: cast_frames,
            },
        );
    }

    /// Release a cure claim.
    pub fn release_cure(&mut self, healer_id: ClientId, target_id: u32) {
        if let Some(claim) = self.claims.get(&target_id)
            && claim.healer_id == healer_id
        {
            self.claims.remove(&target_id);
        }
    }

    /// Advance tick and expire stale claims. Returns cure assignments.
    pub fn tick(
        &mut self,
        healers: &[HealerInfo],
        targets: &[HealTarget],
    ) -> Vec<(ClientId, Command)> {
        self.tick += 1;
        self.claims.retain(|_, c| !c.is_expired(self.tick));

        let mut commands = Vec::new();

        for healer in healers {
            // Skip healers that already have a cure claim
            if self
                .claims
                .values()
                .any(|c| c.healer_id == healer.client_id && !c.is_expired(self.tick))
            {
                continue;
            }

            // Find unclaimed afflicted targets, preferring own group
            let cure_target = targets
                .iter()
                .filter(|t| {
                    t.has_detrimental
                        && !t.is_dead
                        && t.hp_pct > CURE_HP_THRESHOLD
                        && self
                            .claims
                            .get(&t.spawn_id)
                            .is_none_or(|c| c.is_expired(self.tick))
                })
                .min_by(|a, b| {
                    let a_own = a.group_id == healer.group_id;
                    let b_own = b.group_id == healer.group_id;
                    b_own.cmp(&a_own).then(
                        a.hp_pct
                            .partial_cmp(&b.hp_pct)
                            .unwrap_or(std::cmp::Ordering::Equal),
                    )
                });

            if let Some(target) = cure_target {
                self.claim_cure(healer.client_id, target.spawn_id, DEFAULT_CAST_FRAMES);
                // Use SetTarget + CastSpell pattern; the DLL's cleric strategy
                // will select the appropriate cure spell via select_spell().
                commands.push((
                    healer.client_id,
                    Command::SetTarget {
                        spawn_id: target.spawn_id,
                    },
                ));
                tracing::info!(
                    healer = healer.client_id,
                    target = target.spawn_id,
                    "Cure coordinator: assigned cure target"
                );
            }
        }

        commands
    }
}

impl Default for CureCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_healer(client_id: ClientId, group_id: u8, mana_pct: f32) -> HealerInfo {
        HealerInfo {
            client_id,
            group_id,
            is_primary: true,
            mana_pct,
        }
    }

    fn make_target(spawn_id: u32, hp_pct: f32, group_id: u8, role: CombatRole) -> HealTarget {
        HealTarget {
            spawn_id,
            hp_pct,
            role,
            group_id,
            has_detrimental: false,
            is_dead: false,
        }
    }

    #[test]
    fn heal_coordinator_new_is_disabled() {
        let coord = HealCoordinator::new();
        assert!(!coord.is_enabled());
    }

    #[test]
    fn heal_coordinator_enable_disable() {
        let mut coord = HealCoordinator::new();
        coord.set_enabled(true);
        assert!(coord.is_enabled());
        coord.set_enabled(false);
        assert!(!coord.is_enabled());
    }

    #[test]
    fn no_commands_when_disabled() {
        let mut coord = HealCoordinator::new();
        let healers = vec![make_healer(1, 0, 100.0)];
        let targets = vec![make_target(10, 30.0, 0, CombatRole::DpsMelee)];
        let cmds = coord.tick(&healers, &targets);
        assert!(cmds.is_empty());
    }

    #[test]
    fn assigns_heal_to_lowest_hp() {
        let mut coord = HealCoordinator::new();
        coord.set_enabled(true);
        let healers = vec![make_healer(1, 0, 100.0)];
        let targets = vec![
            make_target(10, 70.0, 0, CombatRole::DpsMelee),
            make_target(11, 40.0, 0, CombatRole::DpsMelee),
        ];
        let cmds = coord.tick(&healers, &targets);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].0, 1); // healer 1
        match &cmds[0].1 {
            Command::CombatEmergencyHeal { target_id } => assert_eq!(*target_id, 11),
            _ => panic!("Expected CombatEmergencyHeal"),
        }
    }

    #[test]
    fn claim_prevents_double_heal() {
        let mut coord = HealCoordinator::new();
        coord.set_enabled(true);
        let healers = vec![make_healer(1, 0, 100.0), make_healer(2, 0, 100.0)];
        let targets = vec![
            make_target(10, 40.0, 0, CombatRole::DpsMelee),
            make_target(11, 50.0, 0, CombatRole::DpsMelee),
        ];

        let cmds = coord.tick(&healers, &targets);
        // Two healers, two targets — each should get a different target
        assert_eq!(cmds.len(), 2);
        let target_ids: Vec<u32> = cmds
            .iter()
            .map(|(_, cmd)| match cmd {
                Command::CombatEmergencyHeal { target_id } => *target_id,
                _ => panic!("Expected CombatEmergencyHeal"),
            })
            .collect();
        // Both targets should be assigned, no duplicates
        assert!(target_ids.contains(&10));
        assert!(target_ids.contains(&11));
    }

    #[test]
    fn tank_priority_over_dps() {
        let mut coord = HealCoordinator::new();
        coord.set_enabled(true);
        let healers = vec![make_healer(1, 0, 100.0)];
        let targets = vec![
            make_target(10, 50.0, 0, CombatRole::DpsMelee), // DPS at 50%
            make_target(11, 60.0, 0, CombatRole::MainTank), // Tank at 60% (higher HP)
        ];

        let cmds = coord.tick(&healers, &targets);
        assert_eq!(cmds.len(), 1);
        match &cmds[0].1 {
            Command::CombatEmergencyHeal { target_id } => {
                assert_eq!(*target_id, 11, "Should heal tank over DPS");
            }
            _ => panic!("Expected CombatEmergencyHeal"),
        }
    }

    #[test]
    fn own_group_priority_over_cross_group() {
        let mut coord = HealCoordinator::new();
        coord.set_enabled(true);
        let healers = vec![make_healer(1, 0, 100.0)]; // group 0
        let targets = vec![
            make_target(10, 50.0, 1, CombatRole::DpsMelee), // group 1, lower HP
            make_target(11, 60.0, 0, CombatRole::DpsMelee), // group 0, higher HP
        ];

        let cmds = coord.tick(&healers, &targets);
        assert_eq!(cmds.len(), 1);
        match &cmds[0].1 {
            Command::CombatEmergencyHeal { target_id } => {
                assert_eq!(*target_id, 11, "Should heal own group first");
            }
            _ => panic!("Expected CombatEmergencyHeal"),
        }
    }

    #[test]
    fn emergency_overrides_group_priority() {
        let mut coord = HealCoordinator::new();
        coord.set_enabled(true);
        let healers = vec![make_healer(1, 0, 100.0)]; // group 0
        let targets = vec![
            make_target(10, 15.0, 1, CombatRole::DpsMelee), // group 1, EMERGENCY
            make_target(11, 60.0, 0, CombatRole::DpsMelee), // group 0, moderate
        ];

        let cmds = coord.tick(&healers, &targets);
        assert_eq!(cmds.len(), 1);
        match &cmds[0].1 {
            Command::CombatEmergencyHeal { target_id } => {
                assert_eq!(*target_id, 10, "Emergency should override group priority");
            }
            _ => panic!("Expected CombatEmergencyHeal"),
        }
    }

    #[test]
    fn claim_expires_after_ttl() {
        let mut coord = HealCoordinator::new();
        coord.set_enabled(true);

        // Manually claim target 10 with TTL of 5 frames
        coord.claim_target(1, 10, 5);

        // Should be claimed
        assert!(coord.is_claimed_by_other(10, 2));

        // Advance past TTL
        for _ in 0..6 {
            coord.tick(&[], &[]);
        }

        // Should be expired now
        assert!(!coord.is_claimed_by_other(10, 2));
    }

    #[test]
    fn release_claim_works() {
        let mut coord = HealCoordinator::new();
        coord.claim_target(1, 10, 100);
        assert!(coord.is_claimed_by_other(10, 2));

        coord.release_claim(1, 10);
        assert!(!coord.is_claimed_by_other(10, 2));
    }

    #[test]
    fn release_claim_only_owner() {
        let mut coord = HealCoordinator::new();
        coord.claim_target(1, 10, 100);

        // Healer 2 can't release healer 1's claim
        coord.release_claim(2, 10);
        assert!(coord.is_claimed_by_other(10, 2));
    }

    #[test]
    fn low_mana_skips_cross_group() {
        let mut coord = HealCoordinator::new();
        coord.set_enabled(true);
        coord.set_cross_group_mana_threshold(70.0);

        let healers = vec![make_healer(1, 0, 40.0)]; // low mana
        let targets = vec![
            make_target(10, 50.0, 1, CombatRole::DpsMelee), // different group
        ];

        let cmds = coord.tick(&healers, &targets);
        assert!(cmds.is_empty(), "Low mana healer should skip cross-group");
    }

    #[test]
    fn skip_dead_targets() {
        let mut coord = HealCoordinator::new();
        coord.set_enabled(true);
        let healers = vec![make_healer(1, 0, 100.0)];
        let mut dead_target = make_target(10, 0.0, 0, CombatRole::DpsMelee);
        dead_target.is_dead = true;
        let targets = vec![dead_target];

        let cmds = coord.tick(&healers, &targets);
        assert!(cmds.is_empty());
    }

    #[test]
    fn skip_healthy_targets() {
        let mut coord = HealCoordinator::new();
        coord.set_enabled(true);
        let healers = vec![make_healer(1, 0, 100.0)];
        let targets = vec![make_target(10, 95.0, 0, CombatRole::DpsMelee)];

        let cmds = coord.tick(&healers, &targets);
        assert!(cmds.is_empty(), "95% HP is above heal threshold");
    }

    #[test]
    fn healer_with_active_claim_skipped() {
        let mut coord = HealCoordinator::new();
        coord.set_enabled(true);

        // Manually set a claim for healer 1
        coord.claim_target(1, 99, 100);

        let healers = vec![make_healer(1, 0, 100.0)];
        let targets = vec![make_target(10, 40.0, 0, CombatRole::DpsMelee)];

        let cmds = coord.tick(&healers, &targets);
        assert!(
            cmds.is_empty(),
            "Healer with active claim should not get new assignment"
        );
    }

    #[test]
    fn active_claims_tracking() {
        let mut coord = HealCoordinator::new();
        coord.claim_target(1, 10, 100);
        coord.claim_target(2, 20, 100);

        assert_eq!(coord.active_claim_count(), 2);
        let claims = coord.active_claims();
        assert_eq!(claims.len(), 2);
    }

    // --- Cure coordinator tests ---

    #[test]
    fn cure_coordinator_assigns_afflicted() {
        let mut cure = CureCoordinator::new();
        let healers = vec![make_healer(1, 0, 100.0)];
        let mut target = make_target(10, 70.0, 0, CombatRole::DpsMelee);
        target.has_detrimental = true;
        let targets = vec![target];

        let cmds = cure.tick(&healers, &targets);
        assert_eq!(cmds.len(), 1);
        match &cmds[0].1 {
            Command::SetTarget { spawn_id } => assert_eq!(*spawn_id, 10),
            _ => panic!("Expected SetTarget for cure"),
        }
    }

    #[test]
    fn cure_coordinator_skips_low_hp() {
        let mut cure = CureCoordinator::new();
        let healers = vec![make_healer(1, 0, 100.0)];
        let mut target = make_target(10, 30.0, 0, CombatRole::DpsMelee);
        target.has_detrimental = true; // afflicted but critically low HP
        let targets = vec![target];

        let cmds = cure.tick(&healers, &targets);
        assert!(
            cmds.is_empty(),
            "Should skip cure when target HP is critical (heal first)"
        );
    }

    #[test]
    fn cure_prevents_double_cure() {
        let mut cure = CureCoordinator::new();
        let healers = vec![make_healer(1, 0, 100.0), make_healer(2, 0, 100.0)];
        let mut target = make_target(10, 70.0, 0, CombatRole::DpsMelee);
        target.has_detrimental = true;
        let targets = vec![target];

        let cmds = cure.tick(&healers, &targets);
        // Only one healer should cure the single afflicted target
        assert_eq!(cmds.len(), 1);
    }

    // --- Additional HealCoordinator edge cases ---

    #[test]
    fn disable_clears_active_claims() {
        let mut coord = HealCoordinator::new();
        coord.set_enabled(true);
        coord.claim_target(1, 10, 100);
        coord.claim_target(2, 20, 100);
        assert_eq!(coord.active_claim_count(), 2);

        coord.set_enabled(false);
        assert_eq!(
            coord.active_claim_count(),
            0,
            "disabling should clear claims"
        );
    }

    #[test]
    fn empty_healers_returns_empty() {
        let mut coord = HealCoordinator::new();
        coord.set_enabled(true);
        let targets = vec![make_target(10, 30.0, 0, CombatRole::DpsMelee)];
        let cmds = coord.tick(&[], &targets);
        assert!(cmds.is_empty());
    }

    #[test]
    fn empty_targets_returns_empty() {
        let mut coord = HealCoordinator::new();
        coord.set_enabled(true);
        let healers = vec![make_healer(1, 0, 100.0)];
        let cmds = coord.tick(&healers, &[]);
        assert!(cmds.is_empty());
    }

    #[test]
    fn claim_target_same_owner_not_expired() {
        let mut coord = HealCoordinator::new();
        coord.claim_target(1, 10, 100);
        // Same owner should not be "claimed by other"
        assert!(!coord.is_claimed_by_other(10, 1));
        // Different owner should see it as claimed
        assert!(coord.is_claimed_by_other(10, 2));
    }

    #[test]
    fn unclaimed_target_not_claimed_by_other() {
        let coord = HealCoordinator::new();
        assert!(!coord.is_claimed_by_other(999, 1));
    }

    #[test]
    fn cross_group_mana_threshold_setter() {
        let mut coord = HealCoordinator::new();
        coord.set_cross_group_mana_threshold(80.0);
        coord.set_enabled(true);

        // Healer at 75% mana should skip cross-group at 80% threshold
        let healers = vec![make_healer(1, 0, 75.0)];
        let targets = vec![make_target(10, 50.0, 1, CombatRole::DpsMelee)]; // different group
        let cmds = coord.tick(&healers, &targets);
        assert!(
            cmds.is_empty(),
            "75% mana < 80% threshold should skip cross-group"
        );
    }

    #[test]
    fn zero_hp_target_skipped() {
        let mut coord = HealCoordinator::new();
        coord.set_enabled(true);
        let healers = vec![make_healer(1, 0, 100.0)];
        let targets = vec![make_target(10, 0.0, 0, CombatRole::DpsMelee)];
        let cmds = coord.tick(&healers, &targets);
        assert!(
            cmds.is_empty(),
            "0% HP target should be skipped (likely dead)"
        );
    }

    // --- CureCoordinator edge cases ---

    #[test]
    fn cure_skips_non_afflicted_targets() {
        let mut cure = CureCoordinator::new();
        let healers = vec![make_healer(1, 0, 100.0)];
        // Target without detrimentals
        let targets = vec![make_target(10, 70.0, 0, CombatRole::DpsMelee)];
        let cmds = cure.tick(&healers, &targets);
        assert!(cmds.is_empty());
    }

    #[test]
    fn cure_skips_dead_targets() {
        let mut cure = CureCoordinator::new();
        let healers = vec![make_healer(1, 0, 100.0)];
        let mut target = make_target(10, 0.0, 0, CombatRole::DpsMelee);
        target.has_detrimental = true;
        target.is_dead = true;
        let targets = vec![target];
        let cmds = cure.tick(&healers, &targets);
        assert!(cmds.is_empty());
    }

    #[test]
    fn cure_prefers_own_group() {
        let mut cure = CureCoordinator::new();
        let healers = vec![make_healer(1, 0, 100.0)]; // group 0

        let mut t1 = make_target(10, 70.0, 1, CombatRole::DpsMelee); // group 1
        t1.has_detrimental = true;
        let mut t2 = make_target(11, 70.0, 0, CombatRole::DpsMelee); // group 0 (same as healer)
        t2.has_detrimental = true;

        let cmds = cure.tick(&healers, &[t1, t2]);
        assert_eq!(cmds.len(), 1);
        match &cmds[0].1 {
            Command::SetTarget { spawn_id } => {
                assert_eq!(*spawn_id, 11, "Should cure own group first");
            }
            _ => panic!("Expected SetTarget"),
        }
    }

    #[test]
    fn active_claims_returns_correct_pairs() {
        let mut coord = HealCoordinator::new();
        coord.claim_target(5, 50, 100);
        coord.claim_target(10, 100, 100);

        let claims = coord.active_claims();
        assert_eq!(claims.len(), 2);
        // Check that both healer/target pairs are present
        assert!(claims.iter().any(|(h, t)| *h == 5 && *t == 50));
        assert!(claims.iter().any(|(h, t)| *h == 10 && *t == 100));
    }
}
