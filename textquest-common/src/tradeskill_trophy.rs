use serde::{Deserialize, Serialize};

const TAILORING_MAINHAND_TROPHY: &str = "Blessed Akhevan Shadow Shears";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeskillTrophySettings {
    pub enabled: bool,
    pub trophy_item_name: String,
}

impl TradeskillTrophySettings {
    #[must_use]
    pub fn sanitized(&self) -> Self {
        Self {
            enabled: self.enabled,
            trophy_item_name: self.trophy_item_name.trim().to_string(),
        }
    }

    #[must_use]
    pub fn is_configured(&self) -> bool {
        self.enabled && !self.trophy_item_name.trim().is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeskillContainerType {
    Alchemy,
    Baking,
    Brewing,
    Blacksmithing,
    Fletching,
    Fishing,
    Jewelry,
    Poison,
    Pottery,
    Research,
    Tailoring,
    Tinkering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrophyEquipSlot {
    Ammo,
    Mainhand,
}

impl TrophyEquipSlot {
    #[must_use]
    pub fn itemnotify_name(self) -> &'static str {
        match self {
            Self::Ammo => "Ammo",
            Self::Mainhand => "Mainhand",
        }
    }
}

#[must_use]
pub fn detect_tradeskill_container_type(container_name: &str) -> Option<TradeskillContainerType> {
    if container_name.contains("Alchemy Table") {
        return Some(TradeskillContainerType::Alchemy);
    }
    if container_name.contains("Mixing Bowl")
        || container_name.contains("Oven")
        || container_name.contains("Ice Cream")
    {
        return Some(TradeskillContainerType::Baking);
    }
    if container_name.contains("Brewing Barrel") {
        return Some(TradeskillContainerType::Brewing);
    }
    if container_name.contains("Forge") {
        return Some(TradeskillContainerType::Blacksmithing);
    }
    if container_name.contains("Fletching Table") {
        return Some(TradeskillContainerType::Fletching);
    }
    if container_name.contains("Fly Making Bench") {
        return Some(TradeskillContainerType::Fishing);
    }
    if container_name.contains("Jewelry Making Table") {
        return Some(TradeskillContainerType::Jewelry);
    }
    if container_name.contains("Poisoncrafting Table") {
        return Some(TradeskillContainerType::Poison);
    }
    if container_name.contains("Kiln") || container_name.contains("Pottery Wheel") {
        return Some(TradeskillContainerType::Pottery);
    }
    if container_name.contains("Spell Research Table") {
        return Some(TradeskillContainerType::Research);
    }
    if container_name.contains("Loom") {
        return Some(TradeskillContainerType::Tailoring);
    }
    if container_name.contains("Tinkering") {
        return Some(TradeskillContainerType::Tinkering);
    }
    None
}

#[must_use]
pub fn preferred_trophy_slot(
    trophy_item_name: &str,
    container_type: TradeskillContainerType,
) -> TrophyEquipSlot {
    if container_type == TradeskillContainerType::Fishing
        || (container_type == TradeskillContainerType::Tailoring
            && trophy_item_name
                .trim()
                .eq_ignore_ascii_case(TAILORING_MAINHAND_TROPHY))
    {
        return TrophyEquipSlot::Mainhand;
    }
    TrophyEquipSlot::Ammo
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TradeskillTrophyObservation {
    pub open_container_name: Option<String>,
    pub cursor_item_name: Option<String>,
    pub trophy_equipped: bool,
    pub trophy_charges: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TradeskillTrophyStatus {
    pub active: bool,
    pub equipped_by_manager: bool,
    pub open_container_name: Option<String>,
    pub container_type: Option<TradeskillContainerType>,
    pub target_slot: Option<TrophyEquipSlot>,
    pub previous_item_name: Option<String>,
    pub charges_remaining: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingAction {
    EquipPickUp,
    EquipStashPrevious,
    RestorePickUpPrevious,
    RestoreStashTrophy,
    UnequipPickUpTrophy,
    UnequipStashTrophy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveSession {
    trophy_item_name: String,
    open_container_name: Option<String>,
    container_type: TradeskillContainerType,
    target_slot: TrophyEquipSlot,
    previous_item_name: Option<String>,
    equipped_by_manager: bool,
    charges_remaining: Option<i32>,
    pending: Option<PendingAction>,
}

#[derive(Debug, Clone, Default)]
pub struct TradeskillTrophyManager {
    settings: TradeskillTrophySettings,
    session: Option<ActiveSession>,
}

impl TradeskillTrophyManager {
    #[must_use]
    pub fn new(settings: TradeskillTrophySettings) -> Self {
        Self {
            settings: settings.sanitized(),
            session: None,
        }
    }

    pub fn update_settings(&mut self, settings: TradeskillTrophySettings) {
        self.settings = settings.sanitized();
    }

    #[must_use]
    pub fn settings(&self) -> &TradeskillTrophySettings {
        &self.settings
    }

    #[must_use]
    pub fn active_trophy_item_name(&self) -> Option<&str> {
        self.session
            .as_ref()
            .map(|session| session.trophy_item_name.as_str())
    }

    #[must_use]
    pub fn status(&self) -> TradeskillTrophyStatus {
        let Some(session) = &self.session else {
            return TradeskillTrophyStatus::default();
        };

        TradeskillTrophyStatus {
            active: true,
            equipped_by_manager: session.equipped_by_manager,
            open_container_name: session.open_container_name.clone(),
            container_type: Some(session.container_type),
            target_slot: Some(session.target_slot),
            previous_item_name: session.previous_item_name.clone(),
            charges_remaining: session.charges_remaining,
        }
    }

    pub fn tick(&mut self, observation: &TradeskillTrophyObservation) -> Option<String> {
        let settings = self.settings.sanitized();
        let configured = settings.is_configured();
        let container_type = observation
            .open_container_name
            .as_deref()
            .and_then(detect_tradeskill_container_type);
        let requires_slot_change_cleanup = self
            .session
            .as_ref()
            .is_some_and(|session| slot_change_requires_cleanup(session, container_type));

        if requires_slot_change_cleanup
            && self
                .session
                .as_ref()
                .is_some_and(|session| !session.equipped_by_manager)
        {
            self.session = None;
        }

        if let Some(session) = self.session.as_mut() {
            session.open_container_name = observation.open_container_name.clone();
            if let Some(charges) = observation.trophy_charges {
                session.charges_remaining = Some(charges);
            }
            if let Some(container_type) = container_type {
                let preferred_slot =
                    preferred_trophy_slot(session.trophy_item_name.as_str(), container_type);
                if !requires_slot_change_cleanup {
                    session.container_type = container_type;
                    session.target_slot = preferred_slot;
                }
            }
        }

        if self.session.is_none()
            && configured
            && let Some(container_type) = container_type
        {
            let target_slot = preferred_trophy_slot(&settings.trophy_item_name, container_type);
            self.session = Some(ActiveSession {
                trophy_item_name: settings.trophy_item_name.clone(),
                open_container_name: observation.open_container_name.clone(),
                container_type,
                target_slot,
                previous_item_name: None,
                equipped_by_manager: false,
                charges_remaining: observation.trophy_charges,
                pending: if observation.trophy_equipped {
                    None
                } else {
                    Some(PendingAction::EquipPickUp)
                },
            });
        }

        if !configured
            && self
                .session
                .as_ref()
                .is_some_and(|session| !session.equipped_by_manager)
        {
            self.session = None;
            return None;
        }

        let session = self.session.as_mut()?;

        if session.pending.is_none()
            && (container_type.is_none() || !configured || requires_slot_change_cleanup)
        {
            session.pending = cleanup_pending_action(session);
        }

        let command = advance_session(session, observation);

        let should_clear = self.session.as_ref().is_some_and(|session| {
            session.pending.is_none()
                && (container_type.is_none()
                    || !configured
                    || slot_change_requires_cleanup(session, container_type))
                && (!session.equipped_by_manager || !observation.trophy_equipped)
        });
        if should_clear {
            self.session = None;
        }

        command
    }
}

fn advance_session(
    session: &mut ActiveSession,
    observation: &TradeskillTrophyObservation,
) -> Option<String> {
    let trophy_item_name = session.trophy_item_name.as_str();
    let action = session.pending.clone()?;
    match action {
        PendingAction::EquipPickUp => {
            if item_names_match(
                observation.cursor_item_name.as_deref(),
                Some(trophy_item_name),
            ) {
                session.pending = Some(PendingAction::EquipStashPrevious);
                return Some(click_slot_command(session.target_slot));
            }

            Some(pick_up_item_command(trophy_item_name))
        }
        PendingAction::EquipStashPrevious => {
            if let Some(cursor_item_name) = observation.cursor_item_name.as_deref() {
                if cursor_item_name.eq_ignore_ascii_case(trophy_item_name) {
                    return Some(click_slot_command(session.target_slot));
                }

                session.previous_item_name = Some(cursor_item_name.trim().to_string());
                session.equipped_by_manager = true;
                session.pending = None;
                return Some(autoinv_command());
            }

            session.equipped_by_manager = true;
            session.pending = None;
            None
        }
        PendingAction::RestorePickUpPrevious => {
            if item_names_match(
                observation.cursor_item_name.as_deref(),
                session.previous_item_name.as_deref(),
            ) {
                session.pending = Some(PendingAction::RestoreStashTrophy);
                return Some(click_slot_command(session.target_slot));
            }

            session
                .previous_item_name
                .as_deref()
                .map(pick_up_item_command)
        }
        PendingAction::RestoreStashTrophy => {
            if let Some(cursor_item_name) = observation.cursor_item_name.as_deref() {
                if item_names_match(Some(cursor_item_name), Some(trophy_item_name)) {
                    session.equipped_by_manager = false;
                    session.pending = None;
                    return Some(autoinv_command());
                }

                if item_names_match(
                    Some(cursor_item_name),
                    session.previous_item_name.as_deref(),
                ) {
                    return Some(click_slot_command(session.target_slot));
                }
            } else {
                session.equipped_by_manager = false;
                session.pending = None;
            }
            None
        }
        PendingAction::UnequipPickUpTrophy => {
            if item_names_match(
                observation.cursor_item_name.as_deref(),
                Some(trophy_item_name),
            ) {
                session.pending = Some(PendingAction::UnequipStashTrophy);
                return Some(autoinv_command());
            }

            session.pending = Some(PendingAction::UnequipStashTrophy);
            Some(click_slot_command(session.target_slot))
        }
        PendingAction::UnequipStashTrophy => {
            if item_names_match(
                observation.cursor_item_name.as_deref(),
                Some(trophy_item_name),
            ) {
                session.equipped_by_manager = false;
                session.pending = None;
                return Some(autoinv_command());
            }

            if observation.cursor_item_name.is_none() {
                session.equipped_by_manager = false;
                session.pending = None;
            }
            None
        }
    }
}

fn pick_up_item_command(item_name: &str) -> String {
    format!(
        "/squelch /nomodkey /shiftkey /itemnotify \"{}\" leftmouseup",
        sanitize_item_selector(item_name)
    )
}

fn click_slot_command(slot: TrophyEquipSlot) -> String {
    format!(
        "/squelch /nomodkey /shiftkey /itemnotify {} leftmouseup",
        slot.itemnotify_name()
    )
}

fn autoinv_command() -> String {
    "/autoinv".to_string()
}

fn sanitize_item_selector(item_name: &str) -> String {
    item_name
        .chars()
        .filter(|ch| !ch.is_control() && *ch != '"')
        .collect::<String>()
        .trim()
        .to_string()
}

fn item_names_match(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.trim().eq_ignore_ascii_case(right.trim()),
        _ => false,
    }
}

fn cleanup_pending_action(session: &ActiveSession) -> Option<PendingAction> {
    if session.equipped_by_manager {
        if session.previous_item_name.is_some() {
            Some(PendingAction::RestorePickUpPrevious)
        } else {
            Some(PendingAction::UnequipPickUpTrophy)
        }
    } else {
        None
    }
}

fn slot_change_requires_cleanup(
    session: &ActiveSession,
    container_type: Option<TradeskillContainerType>,
) -> bool {
    container_type.is_some_and(|container_type| {
        preferred_trophy_slot(session.trophy_item_name.as_str(), container_type)
            != session.target_slot
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(item_name: &str) -> TradeskillTrophySettings {
        TradeskillTrophySettings {
            enabled: true,
            trophy_item_name: item_name.to_string(),
        }
    }

    fn open_forge() -> TradeskillTrophyObservation {
        TradeskillTrophyObservation {
            open_container_name: Some("Forge".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn detects_container_types() {
        assert_eq!(
            detect_tradeskill_container_type("Forge"),
            Some(TradeskillContainerType::Blacksmithing)
        );
        assert_eq!(
            detect_tradeskill_container_type("Spell Research Table"),
            Some(TradeskillContainerType::Research)
        );
        assert_eq!(
            detect_tradeskill_container_type("Fly Making Bench"),
            Some(TradeskillContainerType::Fishing)
        );
        assert_eq!(detect_tradeskill_container_type("Bank"), None);
    }

    #[test]
    fn chooses_mainhand_for_fishing_and_akhevan_shears() {
        assert_eq!(
            preferred_trophy_slot("Fishing Prize", TradeskillContainerType::Fishing),
            TrophyEquipSlot::Mainhand
        );
        assert_eq!(
            preferred_trophy_slot(
                "Blessed Akhevan Shadow Shears",
                TradeskillContainerType::Tailoring
            ),
            TrophyEquipSlot::Mainhand
        );
        assert_eq!(
            preferred_trophy_slot("Regular Trophy", TradeskillContainerType::Tailoring),
            TrophyEquipSlot::Ammo
        );
    }

    #[test]
    fn equips_and_restores_previous_item_around_session() {
        let mut manager = TradeskillTrophyManager::new(settings("Master Smith Trophy"));

        let command = manager.tick(&open_forge());
        assert_eq!(
            command.as_deref(),
            Some("/squelch /nomodkey /shiftkey /itemnotify \"Master Smith Trophy\" leftmouseup")
        );

        let command = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Forge".to_string()),
            cursor_item_name: Some("Master Smith Trophy".to_string()),
            ..Default::default()
        });
        assert_eq!(
            command.as_deref(),
            Some("/squelch /nomodkey /shiftkey /itemnotify Ammo leftmouseup")
        );

        let command = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Forge".to_string()),
            cursor_item_name: Some("Silver Choker".to_string()),
            trophy_equipped: true,
            trophy_charges: Some(9),
        });
        assert_eq!(command.as_deref(), Some("/autoinv"));

        let status = manager.status();
        assert!(status.active);
        assert!(status.equipped_by_manager);
        assert_eq!(status.previous_item_name.as_deref(), Some("Silver Choker"));
        assert_eq!(status.charges_remaining, Some(9));

        let command = manager.tick(&TradeskillTrophyObservation {
            trophy_equipped: true,
            trophy_charges: Some(8),
            ..Default::default()
        });
        assert_eq!(
            command.as_deref(),
            Some("/squelch /nomodkey /shiftkey /itemnotify \"Silver Choker\" leftmouseup")
        );

        let command = manager.tick(&TradeskillTrophyObservation {
            cursor_item_name: Some("Silver Choker".to_string()),
            trophy_equipped: true,
            trophy_charges: Some(8),
            ..Default::default()
        });
        assert_eq!(
            command.as_deref(),
            Some("/squelch /nomodkey /shiftkey /itemnotify Ammo leftmouseup")
        );

        let command = manager.tick(&TradeskillTrophyObservation {
            cursor_item_name: Some("Master Smith Trophy".to_string()),
            trophy_charges: Some(8),
            ..Default::default()
        });
        assert_eq!(command.as_deref(), Some("/autoinv"));

        let command = manager.tick(&TradeskillTrophyObservation::default());
        assert!(command.is_none());
        assert_eq!(manager.status(), TradeskillTrophyStatus::default());
    }

    #[test]
    fn removes_trophy_when_no_previous_item_exists() {
        let mut manager = TradeskillTrophyManager::new(settings("Master Baker Trophy"));

        let _ = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Oven".to_string()),
            ..Default::default()
        });
        let _ = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Oven".to_string()),
            cursor_item_name: Some("Master Baker Trophy".to_string()),
            ..Default::default()
        });

        let command = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Oven".to_string()),
            trophy_equipped: true,
            trophy_charges: Some(12),
            ..Default::default()
        });
        assert!(command.is_none());
        assert!(manager.status().equipped_by_manager);
        assert_eq!(manager.status().previous_item_name, None);

        let command = manager.tick(&TradeskillTrophyObservation {
            trophy_charges: Some(11),
            ..Default::default()
        });
        assert_eq!(
            command.as_deref(),
            Some("/squelch /nomodkey /shiftkey /itemnotify Ammo leftmouseup")
        );

        let command = manager.tick(&TradeskillTrophyObservation {
            cursor_item_name: Some("Master Baker Trophy".to_string()),
            trophy_charges: Some(11),
            ..Default::default()
        });
        assert_eq!(command.as_deref(), Some("/autoinv"));

        let command = manager.tick(&TradeskillTrophyObservation::default());
        assert!(command.is_none());
        assert_eq!(manager.status(), TradeskillTrophyStatus::default());
    }

    #[test]
    fn does_not_restore_player_equipped_trophy() {
        let mut manager = TradeskillTrophyManager::new(settings("Master Baker Trophy"));

        let command = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Oven".to_string()),
            trophy_equipped: true,
            trophy_charges: Some(50),
            ..Default::default()
        });
        assert!(command.is_none());
        assert!(!manager.status().equipped_by_manager);

        let command = manager.tick(&TradeskillTrophyObservation::default());
        assert!(command.is_none());
        assert_eq!(manager.status(), TradeskillTrophyStatus::default());
    }

    #[test]
    fn disabled_or_blank_configuration_stays_idle() {
        let mut disabled = TradeskillTrophyManager::new(TradeskillTrophySettings::default());
        assert!(disabled.tick(&open_forge()).is_none());

        let mut blank_name = TradeskillTrophyManager::new(TradeskillTrophySettings {
            enabled: true,
            trophy_item_name: "   ".to_string(),
        });
        assert!(blank_name.tick(&open_forge()).is_none());
    }

    #[test]
    fn disabling_before_equip_completes_cancels_session() {
        let mut manager = TradeskillTrophyManager::new(settings("Master Smith Trophy"));

        let command = manager.tick(&open_forge());
        assert_eq!(
            command.as_deref(),
            Some("/squelch /nomodkey /shiftkey /itemnotify \"Master Smith Trophy\" leftmouseup")
        );

        manager.update_settings(TradeskillTrophySettings::default());

        let command = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Forge".to_string()),
            cursor_item_name: Some("Master Smith Trophy".to_string()),
            ..Default::default()
        });
        assert!(command.is_none());
        assert_eq!(manager.status(), TradeskillTrophyStatus::default());
    }

    #[test]
    fn disabling_after_manager_equip_still_restores_original_trophy() {
        let mut manager = TradeskillTrophyManager::new(settings("Master Smith Trophy"));

        let _ = manager.tick(&open_forge());
        let _ = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Forge".to_string()),
            cursor_item_name: Some("Master Smith Trophy".to_string()),
            ..Default::default()
        });

        let command = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Forge".to_string()),
            cursor_item_name: Some("Silver Choker".to_string()),
            trophy_equipped: true,
            trophy_charges: Some(9),
        });
        assert_eq!(command.as_deref(), Some("/autoinv"));
        assert!(manager.status().equipped_by_manager);

        manager.update_settings(TradeskillTrophySettings::default());

        let command = manager.tick(&TradeskillTrophyObservation {
            trophy_equipped: true,
            trophy_charges: Some(8),
            ..Default::default()
        });
        assert_eq!(
            command.as_deref(),
            Some("/squelch /nomodkey /shiftkey /itemnotify \"Silver Choker\" leftmouseup")
        );

        let command = manager.tick(&TradeskillTrophyObservation {
            cursor_item_name: Some("Silver Choker".to_string()),
            trophy_equipped: true,
            trophy_charges: Some(8),
            ..Default::default()
        });
        assert_eq!(
            command.as_deref(),
            Some("/squelch /nomodkey /shiftkey /itemnotify Ammo leftmouseup")
        );

        let command = manager.tick(&TradeskillTrophyObservation {
            cursor_item_name: Some("Master Smith Trophy".to_string()),
            trophy_charges: Some(8),
            ..Default::default()
        });
        assert_eq!(command.as_deref(), Some("/autoinv"));
    }

    #[test]
    fn restore_pick_up_previous_retries_until_item_reaches_cursor() {
        let mut manager = TradeskillTrophyManager::new(settings("Master Smith Trophy"));

        let _ = manager.tick(&open_forge());
        let _ = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Forge".to_string()),
            cursor_item_name: Some("Master Smith Trophy".to_string()),
            ..Default::default()
        });
        let _ = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Forge".to_string()),
            cursor_item_name: Some("Silver Choker".to_string()),
            trophy_equipped: true,
            trophy_charges: Some(9),
        });

        let command = manager.tick(&TradeskillTrophyObservation {
            trophy_equipped: true,
            trophy_charges: Some(8),
            ..Default::default()
        });
        assert_eq!(
            command.as_deref(),
            Some("/squelch /nomodkey /shiftkey /itemnotify \"Silver Choker\" leftmouseup")
        );

        let command = manager.tick(&TradeskillTrophyObservation {
            trophy_equipped: true,
            trophy_charges: Some(8),
            ..Default::default()
        });
        assert_eq!(
            command.as_deref(),
            Some("/squelch /nomodkey /shiftkey /itemnotify \"Silver Choker\" leftmouseup")
        );
    }

    #[test]
    fn slot_change_cleans_up_old_session_before_restarting() {
        let mut manager = TradeskillTrophyManager::new(settings("Blessed Akhevan Shadow Shears"));

        let _ = manager.tick(&open_forge());
        let _ = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Forge".to_string()),
            cursor_item_name: Some("Blessed Akhevan Shadow Shears".to_string()),
            ..Default::default()
        });
        let command = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Forge".to_string()),
            cursor_item_name: Some("Silver Choker".to_string()),
            trophy_equipped: true,
            trophy_charges: Some(9),
        });
        assert_eq!(command.as_deref(), Some("/autoinv"));
        assert_eq!(manager.status().target_slot, Some(TrophyEquipSlot::Ammo));

        let command = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Loom".to_string()),
            trophy_equipped: false,
            trophy_charges: Some(8),
            ..Default::default()
        });
        assert_eq!(
            command.as_deref(),
            Some("/squelch /nomodkey /shiftkey /itemnotify \"Silver Choker\" leftmouseup")
        );

        let command = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Loom".to_string()),
            cursor_item_name: Some("Silver Choker".to_string()),
            trophy_equipped: false,
            trophy_charges: Some(8),
        });
        assert_eq!(
            command.as_deref(),
            Some("/squelch /nomodkey /shiftkey /itemnotify Ammo leftmouseup")
        );

        let command = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Loom".to_string()),
            cursor_item_name: Some("Blessed Akhevan Shadow Shears".to_string()),
            trophy_equipped: false,
            trophy_charges: Some(8),
        });
        assert_eq!(command.as_deref(), Some("/autoinv"));

        let command = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Loom".to_string()),
            trophy_equipped: false,
            trophy_charges: Some(8),
            ..Default::default()
        });
        assert_eq!(
            command.as_deref(),
            Some(
                "/squelch /nomodkey /shiftkey /itemnotify \"Blessed Akhevan Shadow Shears\" leftmouseup"
            )
        );

        let command = manager.tick(&TradeskillTrophyObservation {
            open_container_name: Some("Loom".to_string()),
            cursor_item_name: Some("Blessed Akhevan Shadow Shears".to_string()),
            trophy_equipped: false,
            trophy_charges: Some(8),
        });
        assert_eq!(
            command.as_deref(),
            Some("/squelch /nomodkey /shiftkey /itemnotify Mainhand leftmouseup")
        );
        assert_eq!(
            manager.status().target_slot,
            Some(TrophyEquipSlot::Mainhand)
        );
    }

    #[test]
    fn equip_pick_up_retries_until_trophy_reaches_cursor() {
        let mut manager = TradeskillTrophyManager::new(settings("Master Smith Trophy"));

        let command = manager.tick(&open_forge());
        assert_eq!(
            command.as_deref(),
            Some("/squelch /nomodkey /shiftkey /itemnotify \"Master Smith Trophy\" leftmouseup")
        );

        let command = manager.tick(&open_forge());
        assert_eq!(
            command.as_deref(),
            Some("/squelch /nomodkey /shiftkey /itemnotify \"Master Smith Trophy\" leftmouseup")
        );
    }

    #[test]
    fn settings_is_configured_requires_enabled_and_nonempty_name() {
        let mut s = TradeskillTrophySettings {
            enabled: false,
            trophy_item_name: "My Trophy".into(),
        };
        assert!(!s.is_configured(), "disabled should not be configured");

        s.enabled = true;
        assert!(
            s.is_configured(),
            "enabled with a name should be configured"
        );

        s.trophy_item_name = "   ".into();
        assert!(
            !s.is_configured(),
            "enabled with whitespace-only name should not be configured"
        );

        s.trophy_item_name = String::new();
        assert!(
            !s.is_configured(),
            "enabled with empty name should not be configured"
        );
    }

    #[test]
    fn settings_sanitized_trims_whitespace_from_name() {
        let s = TradeskillTrophySettings {
            enabled: true,
            trophy_item_name: "  My Trophy  ".into(),
        };
        let sanitized = s.sanitized();
        assert_eq!(sanitized.trophy_item_name, "My Trophy");
        assert!(sanitized.enabled);
    }

    #[test]
    fn settings_sanitized_preserves_disabled_flag() {
        let s = TradeskillTrophySettings {
            enabled: false,
            trophy_item_name: " Trophy ".into(),
        };
        let sanitized = s.sanitized();
        assert!(!sanitized.enabled);
        assert_eq!(sanitized.trophy_item_name, "Trophy");
    }

    #[test]
    fn trophy_equip_slot_itemnotify_names() {
        assert_eq!(TrophyEquipSlot::Ammo.itemnotify_name(), "Ammo");
        assert_eq!(TrophyEquipSlot::Mainhand.itemnotify_name(), "Mainhand");
    }

    #[test]
    fn preferred_trophy_slot_uses_ammo_for_non_fishing_containers() {
        for ct in [
            TradeskillContainerType::Baking,
            TradeskillContainerType::Brewing,
            TradeskillContainerType::Blacksmithing,
            TradeskillContainerType::Alchemy,
            TradeskillContainerType::Jewelry,
            TradeskillContainerType::Research,
            TradeskillContainerType::Tinkering,
        ] {
            assert_eq!(
                preferred_trophy_slot("Any Trophy", ct),
                TrophyEquipSlot::Ammo,
                "expected Ammo slot for {ct:?}"
            );
        }
    }

    #[test]
    fn detect_tradeskill_container_type_all_variants() {
        let cases = [
            ("Alchemy Table", TradeskillContainerType::Alchemy),
            ("Mixing Bowl", TradeskillContainerType::Baking),
            ("Oven", TradeskillContainerType::Baking),
            ("Ice Cream", TradeskillContainerType::Baking),
            ("Brewing Barrel", TradeskillContainerType::Brewing),
            ("Forge", TradeskillContainerType::Blacksmithing),
            ("Fletching Table", TradeskillContainerType::Fletching),
            ("Fly Making Bench", TradeskillContainerType::Fishing),
            ("Jewelry Making Table", TradeskillContainerType::Jewelry),
            ("Poisoncrafting Table", TradeskillContainerType::Poison),
            ("Kiln", TradeskillContainerType::Pottery),
            ("Pottery Wheel", TradeskillContainerType::Pottery),
            ("Spell Research Table", TradeskillContainerType::Research),
            ("Loom", TradeskillContainerType::Tailoring),
            ("Tinkering", TradeskillContainerType::Tinkering),
        ];

        for (name, expected) in cases {
            assert_eq!(
                detect_tradeskill_container_type(name),
                Some(expected),
                "failed for container '{name}'"
            );
        }

        assert_eq!(detect_tradeskill_container_type("Merchant"), None);
        assert_eq!(detect_tradeskill_container_type(""), None);
    }
}
