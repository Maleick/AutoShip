//! Relocation option selection for teleport clickies and AAs.

use std::collections::HashMap;

use textquest_common::{
    nav::{
        RelocationDestinationStatus, RelocationOptionState, build_relocation_destination_statuses,
        select_best_relocation_option,
    },
    types::ClientId,
};

/// Per-client relocation inventory and AA state.
#[derive(Debug, Clone, Default)]
pub struct RelocationLoadout {
    options: Vec<RelocationOptionState>,
}

impl RelocationLoadout {
    /// Create a loadout from relocation option states.
    #[must_use]
    pub fn new(options: Vec<RelocationOptionState>) -> Self {
        Self { options }
    }

    /// Borrow the raw option list.
    #[must_use]
    pub fn options(&self) -> &[RelocationOptionState] {
        &self.options
    }

    /// Group the loadout for operator display.
    #[must_use]
    pub fn destination_statuses(&self) -> Vec<RelocationDestinationStatus> {
        build_relocation_destination_statuses(&self.options)
    }

    /// Choose the best ready relocation option for the requested destination.
    #[must_use]
    pub fn best_ready_option_for_zone(&self, zone_name: &str) -> Option<RelocationOptionState> {
        let zone_name = canonicalize_zone_name(zone_name);
        let matching = self
            .options
            .iter()
            .filter(|option| option.option.zone_name == zone_name)
            .cloned()
            .collect::<Vec<_>>();
        select_best_relocation_option(&matching)
            .filter(|option| option.ready)
            .cloned()
    }
}

/// Normalize a destination key for catalog lookups.
#[must_use]
pub fn canonicalize_zone_name(zone_name: &str) -> String {
    zone_name.trim().to_ascii_lowercase()
}

/// Resolve the best ready relocation option for every requested client.
///
/// Returns `None` unless every client in the group has a ready relocation for
/// the destination, which keeps the travel router from splitting the group
/// across mixed travel modes.
#[must_use]
pub fn group_ready_relocations(
    client_ids: &[ClientId],
    loadouts: &HashMap<ClientId, RelocationLoadout>,
    zone_name: &str,
) -> Option<HashMap<ClientId, RelocationOptionState>> {
    let zone_name = canonicalize_zone_name(zone_name);
    client_ids
        .iter()
        .map(|&client_id| {
            loadouts
                .get(&client_id)
                .and_then(|loadout| loadout.best_ready_option_for_zone(&zone_name))
                .map(|option| (client_id, option))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::nav::{RelocationOptionState, relocation_catalog};

    fn option_state(id: &str, cooldown_remaining_secs: Option<u32>) -> RelocationOptionState {
        let option = relocation_catalog()
            .into_iter()
            .find(|option| option.id == id)
            .expect("catalog option");
        RelocationOptionState::new(option, true, cooldown_remaining_secs)
    }

    #[test]
    fn loadout_returns_ready_option_for_destination() {
        let loadout = RelocationLoadout::new(vec![
            option_state("throne_of_heroes", None),
            option_state("secondary_anchor", Some(300)),
        ]);

        let selected = loadout
            .best_ready_option_for_zone("guildlobby")
            .expect("ready guildlobby option");

        assert_eq!(selected.option.id, "throne_of_heroes");
    }

    #[test]
    fn group_ready_relocations_requires_every_client_to_be_ready() {
        let mut loadouts = HashMap::new();
        loadouts.insert(
            1,
            RelocationLoadout::new(vec![option_state("throne_of_heroes", None)]),
        );
        loadouts.insert(
            2,
            RelocationLoadout::new(vec![option_state("throne_of_heroes", Some(180))]),
        );

        assert!(group_ready_relocations(&[1, 2], &loadouts, "guildlobby").is_none());
    }

    #[test]
    fn group_ready_relocations_returns_plan_for_all_clients() {
        let mut loadouts = HashMap::new();
        loadouts.insert(
            1,
            RelocationLoadout::new(vec![option_state("throne_of_heroes", None)]),
        );
        loadouts.insert(
            2,
            RelocationLoadout::new(vec![option_state("throne_of_heroes", None)]),
        );

        let planned =
            group_ready_relocations(&[1, 2], &loadouts, "guildlobby").expect("group relocation");

        assert_eq!(planned.len(), 2);
        assert!(planned.values().all(|option| option.ready));
    }
}
