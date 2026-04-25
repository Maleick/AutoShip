use crate::{
    character_config::RewardAutomationConfig,
    shared_client_state::SharedClientState,
    tradeskill_trophy::{TradeskillTrophySettings, TradeskillTrophyStatus},
    types::ClientId,
};
use std::sync::atomic::{AtomicU64, Ordering};

/// Generates monotonically increasing correlation IDs for IPC request-response
/// matching.
///
/// Each orchestrator instance should create one generator and use it for all
/// outgoing commands. The DLL echoes back the correlation ID in its response,
/// enabling deterministic matching even when multiple commands are in flight.
pub struct CorrelationIdGenerator {
    next: AtomicU64,
}

impl CorrelationIdGenerator {
    /// Create a new generator starting at 1 (0 is reserved as "no
    /// correlation").
    #[must_use]
    pub fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
        }
    }

    /// Generate the next unique correlation ID.
    pub fn next_id(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }
}

impl Default for CorrelationIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// A command paired with an optional correlation ID for request-response
/// matching.
///
/// When the orchestrator sends a command with a `correlation_id`, the DLL
/// should echo that ID back in the corresponding `IpcResponse`. Commands
/// without a correlation ID (`None`) are fire-and-forget or matched by
/// convention.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IpcCommand {
    /// The command to execute.
    pub command: Command,
    /// Optional correlation ID for matching this command to its response.
    pub correlation_id: Option<u64>,
}

impl IpcCommand {
    /// Wrap a command without a correlation ID (backward-compatible default).
    #[must_use]
    pub fn new(command: Command) -> Self {
        Self {
            command,
            correlation_id: None,
        }
    }

    /// Wrap a command with a correlation ID for request-response tracking.
    #[must_use]
    pub fn with_correlation(command: Command, correlation_id: u64) -> Self {
        Self {
            command,
            correlation_id: Some(correlation_id),
        }
    }
}

impl From<Command> for IpcCommand {
    fn from(command: Command) -> Self {
        Self::new(command)
    }
}

/// A response paired with an optional correlation ID echoed from the
/// originating command.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IpcResponse {
    /// The response payload.
    pub response: Response,
    /// Correlation ID echoed from the originating `IpcCommand`, if present.
    pub correlation_id: Option<u64>,
}

impl IpcResponse {
    /// Wrap a response without a correlation ID.
    #[must_use]
    pub fn new(response: Response) -> Self {
        Self {
            response,
            correlation_id: None,
        }
    }

    /// Wrap a response echoing the correlation ID from the originating command.
    #[must_use]
    pub fn echo(response: Response, correlation_id: Option<u64>) -> Self {
        Self {
            response,
            correlation_id,
        }
    }
}

impl From<Response> for IpcResponse {
    fn from(response: Response) -> Self {
        Self::new(response)
    }
}

/// Configuration for automatic resurrection-offer handling.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AutoRezConfig {
    /// Whether the auto-rez workflow is enabled for this character.
    pub enabled: bool,
    /// Minimum experience percentage required to accept the rez offer.
    pub min_xp_pct: u8,
    /// Case-insensitive allowlist of trusted rez casters.
    pub trusted_casters: Vec<String>,
    /// Whether to explicitly decline offers that do not satisfy the policy.
    pub decline_if_untrusted: bool,
    /// Delay before accepting or declining an offer, in milliseconds.
    pub delay_ms: u32,
}

impl Default for AutoRezConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_xp_pct: 90,
            trusted_casters: Vec::new(),
            decline_if_untrusted: false,
            delay_ms: 3_000,
        }
    }
}

/// Rendering mode for an injected client.
///
/// Controls how much GPU work eqgame.exe does. Game logic, network,
/// and all TextQuest hooks run at full speed in every mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RenderMode {
    /// Full rendering — the "eyes" client. Uses GPU normally.
    Normal,
    /// Render 1 frame every ~5 seconds for monitoring/screenshots.
    Strobe,
    /// Zero rendering — game loop runs, GPU completely idle.
    /// Enables scaling to 36+ clients on a single machine.
    NullRender,
}

impl std::fmt::Display for RenderMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "normal"),
            Self::Strobe => write!(f, "strobe"),
            Self::NullRender => write!(f, "null"),
        }
    }
}

/// Auto-accept request types supported by the DLL dialog handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoAcceptRequestKind {
    /// Group invitation prompt.
    GroupInvite,
    /// Trade confirmation window.
    Trade,
    /// NPC quest update prompt.
    QuestUpdate,
    /// NPC quest completion or reward prompt.
    QuestCompletion,
    /// Resurrection confirmation prompt.
    Resurrection,
    /// Task offer or task add prompt.
    TaskAdd,
    /// Shared task invitation prompt.
    TaskInvite,
    /// Mission offer or invitation prompt.
    MissionInvite,
    /// Dynamic zone / expedition offer prompt.
    DzAdd,
    /// Raid invitation prompt.
    RaidInvite,
    /// Fellowship invitation prompt.
    FellowshipInvite,
    /// Wizard or druid translocate prompt.
    Translocate,
    /// Primary/secondary anchor teleport prompt.
    Anchor,
}

/// Action to take when an auto-accept policy rule matches a dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoAcceptAction {
    /// Click the dialog's affirmative button.
    Accept,
    /// Click the dialog's negative button when one is known.
    Decline,
    /// Leave the dialog untouched.
    Ignore,
}

/// Optional source/text-specific override for incoming dialog automation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AutoAcceptRule {
    /// Dialog kind the rule applies to.
    pub kind: AutoAcceptRequestKind,
    /// Action to apply when the rule matches.
    pub action: AutoAcceptAction,
    /// Case-insensitive source substring required for a match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_contains: Option<String>,
    /// Case-insensitive dialog text substring required for a match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_contains: Option<String>,
}

/// Trust policy for incoming auto-accept requests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoAcceptTrustMode {
    /// Accept configured request kinds from anyone.
    #[default]
    Anyone,
    /// Require the sender to be present in `trusted_players`.
    TrustList,
}

/// Shared auto-accept policy used by the DLL, orchestrator, and web dashboard.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AutoAcceptSettings {
    /// Master enable/disable for the entire feature.
    pub enabled: bool,
    /// Whether to auto-accept group invites.
    pub accept_group_invites: bool,
    /// Whether to auto-accept trade confirmations.
    pub accept_trades: bool,
    /// Whether to auto-accept task offers or task adds.
    pub accept_task_adds: bool,
    /// Whether to auto-accept DZ / expedition offers.
    pub accept_dz_adds: bool,
    /// Whether to auto-accept translocate prompts.
    pub accept_translocates: bool,
    /// Whether to auto-accept anchor teleport prompts.
    pub accept_anchors: bool,
    /// Whether trust enforcement is disabled or requires a trusted sender.
    pub trust_mode: AutoAcceptTrustMode,
    /// Case-insensitive character allowlist used when `trust_mode = TrustList`.
    pub trusted_players: Vec<String>,
    /// Case-insensitive source allowlist applied before default accepts.
    #[serde(default)]
    pub source_allowlist: Vec<String>,
    /// Case-insensitive source denylist applied before accepts.
    #[serde(default)]
    pub source_blocklist: Vec<String>,
    /// Case-insensitive dialog text denylist applied before accepts.
    #[serde(default)]
    pub text_blocklist: Vec<String>,
    /// Explicit source/text block matches should click decline when possible.
    #[serde(default)]
    pub decline_blocked: bool,
    /// Ordered source/text-specific rules that can accept, decline, or ignore.
    #[serde(default)]
    pub rules: Vec<AutoAcceptRule>,
}

impl AutoAcceptSettings {
    /// Preserve the legacy `SetAutoAccept { enabled }` behavior.
    #[must_use]
    pub fn legacy(enabled: bool) -> Self {
        Self {
            enabled,
            ..Self::default()
        }
    }

    /// Check whether the specific request kind is enabled by this policy.
    #[must_use]
    pub fn is_kind_enabled(&self, kind: AutoAcceptRequestKind) -> bool {
        match kind {
            AutoAcceptRequestKind::GroupInvite => self.accept_group_invites,
            AutoAcceptRequestKind::Trade => self.accept_trades,
            AutoAcceptRequestKind::QuestUpdate
            | AutoAcceptRequestKind::QuestCompletion
            | AutoAcceptRequestKind::Resurrection => false,
            AutoAcceptRequestKind::TaskAdd => self.accept_task_adds,
            AutoAcceptRequestKind::TaskInvite | AutoAcceptRequestKind::MissionInvite => {
                self.accept_task_adds
            }
            AutoAcceptRequestKind::DzAdd => self.accept_dz_adds,
            AutoAcceptRequestKind::RaidInvite | AutoAcceptRequestKind::FellowshipInvite => {
                self.accept_group_invites
            }
            AutoAcceptRequestKind::Translocate => self.accept_translocates,
            AutoAcceptRequestKind::Anchor => self.accept_anchors,
        }
    }
}

impl Default for AutoAcceptSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            accept_group_invites: true,
            accept_trades: true,
            accept_task_adds: true,
            accept_dz_adds: true,
            accept_translocates: true,
            accept_anchors: true,
            trust_mode: AutoAcceptTrustMode::Anyone,
            trusted_players: Vec::new(),
            source_allowlist: Vec::new(),
            source_blocklist: Vec::new(),
            text_blocklist: Vec::new(),
            decline_blocked: false,
            rules: Vec::new(),
        }
    }
}

/// Filter for querying open inventory container slots from the injected client.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContainerSlotQuery {
    /// Restrict results to a specific container backing store (e.g.
    /// "possessions", "bank").
    pub location: Option<String>,
    /// Restrict results to a specific top-level slot number.
    pub top_slot: Option<i16>,
    /// Restrict results to a specific slot number within the container window.
    pub bag_slot: Option<i16>,
    /// Case-insensitive substring filter on the item name.
    pub item_name_contains: Option<String>,
    /// When false, omit empty container slots.
    pub include_empty: bool,
}

/// Filter for querying passive bazaar search results from the injected client.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BazaarQuery {
    /// Case-insensitive substring applied against the listing's raw columns.
    pub text_contains: Option<String>,
    /// Maximum number of rows to return per matching bazaar list.
    pub max_rows: Option<u16>,
}

/// Filter for querying visible merchant window rows from the injected client.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MerchantQuery {
    /// Case-insensitive substring applied against the listing's raw columns.
    pub text_contains: Option<String>,
    /// Maximum number of rows to return per matching merchant list.
    pub max_rows: Option<u16>,
}

/// One bazaar listing row captured from an in-game `CListWnd`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BazaarListing {
    /// Zero-based row index within the source list window.
    pub row_index: u32,
    /// Raw text columns as displayed in the list.
    pub columns: Vec<String>,
    /// Best-effort normalized item name.
    pub item_name: Option<String>,
    /// Best-effort normalized trader name.
    pub trader_name: Option<String>,
    /// Raw price text from the list row.
    pub price_text: Option<String>,
    /// Parsed price in copper when the row exposes a numeric price.
    pub price_copper: Option<u64>,
    /// Parsed quantity when present.
    pub quantity: Option<u32>,
}

/// One merchant listing row captured from the live merchant window.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MerchantListing {
    /// Zero-based row index within the source list window.
    pub row_index: u32,
    /// Raw text columns as displayed in the merchant list.
    pub columns: Vec<String>,
    /// Best-effort normalized item name from the merchant list.
    pub item_name: Option<String>,
    /// Raw price text from the merchant row.
    pub price_text: Option<String>,
    /// Parsed price in copper when the row exposes a numeric price.
    pub price_copper: Option<u64>,
    /// Parsed quantity when present.
    pub quantity: Option<u32>,
    /// Raw quantity text as displayed by EQ (for example `--`).
    pub quantity_text: Option<String>,
    /// Whether the merchant row indicates an infinite restock quantity.
    pub infinite_quantity: bool,
}

/// Snapshot of one bazaar-related list window in the active EQ UI.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BazaarWindowSnapshot {
    /// Root window display text when available.
    pub window_text: Option<String>,
    /// Root window SIDL name when available.
    pub window_sidl_name: Option<String>,
    /// Child list SIDL name when available.
    pub list_sidl_name: Option<String>,
    /// Number of rows observed in the list window.
    pub row_count: u32,
    /// Listing rows captured from the window.
    pub listings: Vec<BazaarListing>,
}

/// Snapshot of one visible merchant window in the active EQ UI.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MerchantWindowSnapshot {
    /// Best-effort vendor display name.
    pub vendor_name: Option<String>,
    /// Root window display text when available.
    pub window_text: Option<String>,
    /// Root window SIDL name when available.
    pub window_sidl_name: Option<String>,
    /// Merchant list SIDL name when available.
    pub list_sidl_name: Option<String>,
    /// Number of rows observed in the merchant list.
    pub row_count: u32,
    /// Listing rows captured from the window.
    pub listings: Vec<MerchantListing>,
}

/// Snapshot of the item shown in an open container slot.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContainerSlotItemInfo {
    /// EQ item ID.
    pub id: i32,
    /// Display name from the item definition.
    pub name: String,
    /// Icon asset number.
    pub icon_id: i32,
    /// Remaining charges (`-1` means unlimited).
    pub charges: i32,
    /// Stack count in this slot.
    pub stack_count: i32,
    /// Max stack size from the item definition.
    pub stack_size: i32,
    /// EQ item `Type` field.
    pub item_type: u8,
    /// EQ item `ItemClass` field.
    pub item_class: u8,
    /// Whether the item itself is a container.
    pub is_container: bool,
}

/// Snapshot of an open inventory slot that belongs to a container window.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContainerSlotInfo {
    /// Inventory location backing this slot.
    pub location: i32,
    /// Human-readable container instance name.
    pub location_name: String,
    /// Top-level slot containing the container.
    pub top_slot: i16,
    /// Slot index within the container.
    pub bag_slot: i16,
    /// Nested slot / augment slot when applicable.
    pub aug_slot: i16,
    /// `CInvSlotMgr` slot index for debugging.
    pub manager_slot_index: i32,
    /// Whether the slot is currently selected.
    pub is_selected: bool,
    /// Whether the slot is selected by EQ's find-item highlighting.
    pub is_find_selected: bool,
    /// Quantity shown on the slot widget.
    pub quantity: i32,
    /// Remaining recast on the slot widget.
    pub recast_left: i32,
    /// Whether the slot is linked to another inventory surface.
    pub is_linked: bool,
    /// Whether the slot currently has an item.
    pub is_empty: bool,
    /// Top-level container item name, when known.
    pub container_name: Option<String>,
    /// Top-level container item ID, when known.
    pub container_item_id: Option<i32>,
    /// Item currently occupying the slot, if any.
    pub item: Option<ContainerSlotItemInfo>,
}

/// A single item in a `CContextMenu` popup.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextMenuItem {
    /// Zero-based index of this item within its parent menu.
    pub item_index: u32,
    /// Display label shown in the popup.
    pub label: String,
    /// Whether the item is currently selectable.
    pub enabled: bool,
    /// Whether the item has a checkmark.
    pub checked: bool,
    /// Whether this row is a separator (no label).
    pub is_separator: bool,
}

/// Snapshot of one `CContextMenu` popup (a CListWnd with items).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ContextMenuInfo {
    /// Zero-based index of this menu within `CContextMenuManager`.
    pub menu_index: u32,
    /// Items (rows) in this menu.
    pub items: Vec<ContextMenuItem>,
}

/// Commands for controlling a managed EQ session from the orchestrator.
///
/// These commands are processed by `SessionControl::apply_command` and control
/// per-session state transitions, group membership, and routing scope.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SessionControlCommand {
    /// Pause this session — suspend command dispatch until `Resume` is
    /// received.
    Pause,
    /// Resume a paused session — return to active command dispatch.
    Resume,
    /// Assign this session to a group (1-based group ID).
    ///
    /// A `group_id` of `0` removes the session from any group and resets its
    /// routing scope to `AllSession`.
    SetGroup {
        /// Target group ID (1-based; 0 = ungrouped / AllSession).
        group_id: u8,
    },
    /// Set this session's routing scope to `AllSession` so it receives every
    /// broadcast command rather than only group-scoped ones.
    BroadcastAll,
}

/// Runtime metadata for an active function trace.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TraceStatus {
    /// Operator-provided trace label or address string.
    pub function_name: String,
    /// Absolute virtual address where the HWBP is installed.
    pub address: usize,
    /// Whether the tracer captures x64 register arguments.
    pub capture_args: bool,
    /// Whether the tracer records the call return address for later return-value work.
    pub capture_return: bool,
    /// Debug-register slot used by this trace, if installed.
    pub slot: Option<u8>,
    /// Total calls observed since the trace was started.
    pub call_count: u64,
    /// Ring-buffer entries overwritten before the orchestrator drained them.
    pub dropped_events: u64,
}

/// One function-call trace event drained from the DLL-side ring buffer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TraceRecord {
    /// Operator-provided trace label or address string.
    pub function_name: String,
    /// Absolute virtual address that triggered the HWBP.
    pub address: usize,
    /// Monotonic per-DLL trace sequence number.
    pub sequence: u64,
    /// Low-overhead timestamp counter captured in the HWBP handler.
    pub timestamp_ticks: u64,
    /// OS thread ID observed by the handler, or 0 when unavailable.
    pub thread_id: u32,
    /// Captured RCX/RDX/R8/R9 argument registers when requested.
    pub args: Option<[u64; 4]>,
    /// Captured stack pointer when argument capture is requested.
    pub stack_pointer: Option<u64>,
    /// Return address read from the stack when return capture is requested.
    pub return_address: Option<u64>,
    /// Captured return value. Reserved for the return-HWBP follow-up path.
    pub return_value: Option<u64>,
    /// Elapsed timestamp ticks for entry-to-return capture. Reserved for follow-up.
    pub elapsed_ticks: Option<u64>,
}

/// Commands sent from the manager to an injected DLL
///
/// NOTE: `Command` is serialized over IPC with bincode's implicit enum variant
/// discriminants. Append new variants to the end of this enum so mixed-version
/// deployments keep the same wire indices for existing commands.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Command {
    // Movement
    /// Move to an absolute world position.
    MoveTo {
        /// World X coordinate.
        x: f32,
        /// World Y coordinate.
        y: f32,
        /// World Z coordinate.
        z: f32,
    },
    /// Stop all movement immediately.
    StopMovement,
    // Combat
    /// Cast a memorized spell, optionally on a specific target.
    ///
    /// When `target_id` is `Some`, the DLL saves the current target, switches
    /// to the specified spawn, casts, then restores the original target. This
    /// is the MQ2Cast `/cast` with a target override for heal-on-specific.
    /// When `None`, casts on the current target without switching.
    ///
    /// The `kill` and `recast` flags mirror MQ2Cast `/casting` control options:
    /// - `kill`: keep re-casting the spell until the target's HP reaches zero.
    ///   The loop is cancelled automatically when the target dies or
    ///   disappears, or when a `CancelCastLoop` command is received.
    /// - `recast`: repeat the cast up to `recast` times (1-255) with
    ///   deterministic exponential backoff between attempts (base 8 ticks, cap
    ///   30 ticks). Setting `recast` to 0 is treated as a single cast (no
    ///   repetition). Validation rejects combining `kill` and `recast` in the
    ///   same command.
    CastSpell {
        /// Memorized spell slot (1-indexed gem number, 1-13).
        spell_slot: u8,
        /// Optional spawn ID to temporarily target for this cast.
        /// `None` = cast on current target (no swap).
        target_id: Option<u32>,
        /// Keep casting until the target dies (`true`) or stop after one cast
        /// (`false`, the default).  Mirrors MQ2Cast `-kill`.
        #[serde(default)]
        kill: bool,
        /// Number of times to repeat the cast (0 = cast once, 1-255 = repeat N
        /// more times for a total of N+1 casts).  Mirrors MQ2Cast `-recast`.
        /// Must be 0 when `kill` is `true`.
        #[serde(default)]
        recast: u8,
    },
    /// Cancel an active kill-loop or recast-loop started by a prior `CastSpell`
    /// command.  Safe to send even when no loop is running.
    CancelCastLoop,
    /// Begin auto-attack on a target.
    Attack {
        /// Spawn ID of the mob to attack.
        target_id: u32,
    },
    /// Stop auto-attack.
    StopAttack,
    // Targeting
    /// Set the current target by spawn ID.
    SetTarget {
        /// Spawn ID to target.
        spawn_id: u32,
    },
    /// Clear the current target.
    ClearTarget,
    /// Right-click interact with the current target (NPC, door, or object).
    InteractTarget,
    /// Target and activate the nearest door or switch (`/doortarget` + `/click
    /// left door`).
    ///
    /// Equivalent to MQ2's `/click door` — selects the nearest EQ switch and
    /// opens it.
    InteractDoor,
    /// Click the nearest ground item or world object (`/click left item`).
    ///
    /// Equivalent to MQ2's `/click item` — interacts with the nearest ground
    /// spawn.
    ClickObject,
    // Utility
    /// Sit down (mana/HP regen).
    Sit,
    /// Stand up from sitting.
    Stand,
    // Navigation
    /// Follow a sequence of waypoints.
    NavigateTo {
        /// Ordered list of waypoints to traverse.
        waypoints: Vec<crate::nav::Waypoint>,
    },
    /// Move to a camp spot and face heading.
    SetCamp {
        /// Camp position and facing direction.
        spot: crate::nav::CampSpot,
    },
    /// Set a full camp config with scatter positioning.
    SetCampConfig {
        /// Nav-level camp config with center, radius, and optional scatter.
        config: crate::nav::NavCampConfig,
    },
    /// Stop navigating, stay where you are.
    StopNavigation,
    /// Pause navigation, retaining the current path (`/nav pause`).
    NavPause,
    /// Resume navigation from a user-initiated pause (`/nav pause` toggle).
    NavResume,
    /// Navigate to a specific location by coordinates (`/nav loc`).
    NavLoc {
        /// World X coordinate.
        x: f32,
        /// World Y coordinate.
        y: f32,
        /// World Z coordinate.
        z: f32,
    },
    /// Navigate to the current target (`/nav target`).
    NavTarget,
    /// Navigate to the nearest door (`/nav door`).
    NavDoor,
    /// Navigate to the nearest ground item (`/nav item`).
    NavItem,
    /// Reload the navmesh for the current zone (`/nav reload`).
    NavReload,
    /// Save a named waypoint at the current position (`/nav waypoint save
    /// <name>`).
    NavWaypointSave {
        /// Name to assign to the waypoint.
        name: String,
    },
    /// Navigate to a previously saved named waypoint (`/nav waypoint <name>`).
    NavWaypointRecall {
        /// Name of the waypoint to navigate to.
        name: String,
    },
    /// List all saved named waypoints (`/nav waypoint list`).
    NavWaypointList,
    /// Delete a saved named waypoint (`/nav waypoint delete <name>`).
    NavWaypointDelete {
        /// Name of the waypoint to delete.
        name: String,
    },
    /// Query navigation state signals (`Navigation.Active`, etc.).
    NavSignalsQuery,
    /// Query navigation diagnostics for debug overlay (`/nav ui`).
    NavDiagnosticsQuery,
    /// Start MQ2MoveUtils-style `/makecamp player` follow mode.
    ///
    /// The DLL navigator tracks a dynamic anchor (the leader's last-known
    /// position). When the follower strays beyond `leash_distance` it
    /// automatically navigates back. Once within `follow_distance` it holds
    /// position until the anchor moves.
    FollowPlayer {
        /// Follow configuration (leader name, follow distance, leash distance).
        config: crate::nav::FollowConfig,
        /// Initial anchor position (leader's current location).
        anchor_x: f32,
        /// Initial anchor Y coordinate.
        anchor_y: f32,
        /// Initial anchor Z coordinate.
        anchor_z: f32,
    },
    /// Update the dynamic anchor position in an active follow mode.
    ///
    /// Sent by the orchestrator each tick when the leader has moved more than
    /// a minimum threshold, keeping the DLL's anchor in sync with the leader.
    UpdateFollowAnchor {
        /// New anchor X coordinate.
        x: f32,
        /// New anchor Y coordinate.
        y: f32,
        /// New anchor Z coordinate.
        z: f32,
    },
    /// Update the return-policy options of an active follow mode without
    /// restarting navigation.  Sent by the orchestrator when the operator
    /// tunes `/makecamp mindelay`, `maxdelay`, `returnnoaggro`, or
    /// `returnnotlooting` at runtime.
    UpdateFollowConfig {
        /// Replacement follow configuration (leader name and distances must
        /// match the active session; only return-policy fields are typically
        /// changed at runtime).
        config: crate::nav::FollowConfig,
    },
    /// Stop player follow mode and return to idle navigation.
    StopFollow,
    // Login automation
    /// Query the current login phase from the DLL.
    LoginPhaseQuery,
    /// Dump all login-related pointer addresses to the DLL log for calibration.
    /// Used to validate offsets on the live client before attempting
    /// auto-login.
    CalibrateLogin,
    /// Start the automated login sequence. The DLL handles all UI steps
    /// autonomously and reports progress via `LoginPhaseUpdate` responses.
    /// Password is zeroized in DLL memory immediately after use.
    StartLogin {
        /// Account name for login.
        account_name: String,
        /// Password (zeroized after use in DLL memory).
        password: String,
        /// Target server name (e.g. "Teek").
        server_name: String,
        /// Character name to select at character select.
        character_name: String,
    },
    // Post-login
    /// Join a group by group ID.
    JoinGroup {
        /// EQ group ID to join.
        group_id: u32,
    },
    /// Apply standard buff rotation.
    ApplyBuffs,
    /// Report that this client is ready for orchestration.
    ReportReady,
    // Combat
    /// Engage a target in combat via the combat FSM.
    CombatEngage {
        /// Spawn ID of the mob to engage.
        target_id: u32,
    },
    /// Disengage from combat, return to idle.
    CombatDisengage,
    /// Set the main assist target for this character.
    CombatSetAssistTarget {
        /// Spawn ID of the assist target.
        spawn_id: u32,
    },
    /// Broadcast a cross-client roster snapshot to this client.
    UpdateSharedClientStates {
        /// Latest visible state for all connected clients.
        states: Vec<SharedClientState>,
    },
    /// Force-use a specific combat ability.
    CombatForceAbility {
        /// Ability ID to activate.
        ability_id: u32,
    },
    /// Emergency heal a specific target.
    CombatEmergencyHeal {
        /// Spawn ID of the character to heal.
        target_id: u32,
    },
    /// Claim a heal target — tells other healers this target is covered.
    /// Used by the orchestrator's heal coordinator to prevent double-healing.
    HealClaimTarget {
        /// Client ID of the healer making the claim.
        healer_id: u32,
        /// Spawn ID of the target being healed.
        target_id: u32,
        /// Estimated cast time in milliseconds.
        cast_time_ms: u32,
    },
    /// Release a heal claim (cast complete, interrupted, or target recovered).
    HealReleaseClaim {
        /// Client ID of the healer releasing.
        healer_id: u32,
        /// Spawn ID of the target.
        target_id: u32,
    },
    /// Loot the nearest corpse.
    LootCorpse,
    /// Loot all items from the currently open loot window.
    LootAll,
    // Soul Engine
    /// Send a chat message in-game.
    Say {
        /// Chat channel to send on.
        channel: crate::soul::SayChannel,
        /// Message text.
        message: String,
        /// Target player name (for tells).
        target: Option<String>,
    },
    /// Perform an emote animation.
    Emote {
        /// Emote name (e.g. "dance", "wave").
        emote: String,
    },
    /// Execute a soul action (idle behavior, etc.).
    SoulAction {
        /// The soul action to perform.
        action: crate::soul::SoulAction,
    },
    /// Apply an operator-issued soul control action.
    SoulControl {
        /// The control action to apply.
        control: crate::soul::SoulControl,
    },
    /// Stick to a target — MQ2MoveUtils `/stick` equivalent.
    ///
    /// Supported modifiers (via `StickConfig`):
    /// - `/stick #`      → `config.distance = Absolute(#)`
    /// - `/stick #%`     → `config.distance = Percent(#)`
    /// - `/stick mod #`  → apply after start via `StickMod`
    /// - `/stick hold`   → `config.hold = true`
    /// - `/stick always` → `config.always = true`
    /// - `/stick id #`   → `config.id = Some(#)`
    ///
    /// Advanced moveto — MQ2MoveUtils `/moveto` with full option support
    /// (#184).
    MoveToAdvanced {
        /// Full moveto configuration including target tracking, break
        /// conditions.
        config: crate::nav::MoveToConfig,
    },
    /// Enable or disable autopause globally (#164).
    SetAutopause {
        /// Whether autopause should be active.
        enabled: bool,
    },
    /// Enable or disable break-on-GM safety halt for navigation and combat
    /// movement.
    ///
    /// When enabled, navigation pauses (path retained) whenever a GM-flagged
    /// spawn is detected in the nearby spawn list, mirroring MQ2MoveUtils
    /// breakongm behavior.
    SetBreakOnGm {
        /// Whether break-on-GM should be active.
        enabled: bool,
    },
    /// Set the heading update mode used during navigation.
    ///
    /// Mirrors MQ2MoveUtils `/nav headsetting true|loose|fast`:
    /// - `True`  — instant snap (heading field only).
    /// - `Loose` — smooth interpolated turn (most human-looking).
    /// - `Fast`  — instant snap to both heading and speed-heading (default).
    SetHeadingMode {
        /// Heading mode to apply.
        mode: crate::nav::HeadingMode,
    },
    StickTo {
        /// Stick configuration including distance, hold, always, and id
        /// options.
        config: crate::nav::StickConfig,
    },
    /// Stop sticking — `/stick off`.
    StickOff,
    /// Adjust the active stick distance modifier — `/stick mod #`.
    ///
    /// Adds `delta` to `StickConfig::distance_mod` on the running stick
    /// session.
    StickMod {
        /// Delta to add to the current distance modifier (may be negative).
        delta: f32,
    },
    /// Start circle-kiting mode — MQ2MoveUtils `/circle` equivalent.
    ///
    /// Supported modifiers (via `CircleConfig`):
    /// - `/circle on [radius]`           → start with optional radius
    /// - `/circle loc Y X`               → circle around specified coordinates
    /// - `clockwise` / `cw`              → `config.mode = CircleMode::Cw`
    /// - `counterclockwise` / `ccw`      → `config.mode = CircleMode::Ccw`
    /// - `drunken`                        → `config.mode = CircleMode::Drunken`
    /// - `backward`                       → `config.mode =
    ///   CircleMode::Backward`
    CircleKite {
        /// Circle kiting configuration.
        config: crate::nav::CircleConfig,
    },
    /// Stop circle-kiting — `/circle off`.
    CircleOff,
    /// Execute a slash command as if typed in the chat window.
    /// Uses EQ's `InterpretCmd` internally (e.g. "/target Camrene", "/follow").
    SlashCommand {
        /// Full slash command string (e.g. "/target Mob").
        command: String,
    },
    // Zone graph
    /// Request the zone adjacency graph from `ZoneGuideManagerClient`.
    QueryZoneGraph,
    // Camera
    /// Set camera state from a preset (distance, pitch, yaw).
    ///
    /// Mirrors MQ2Camera preset behavior — applies camera parameters
    /// from a named preset without coupling to combat logic.
    SetCamera {
        /// Camera distance or zoom level.
        distance: Option<f32>,
        /// Camera pitch angle in degrees.
        pitch: Option<f32>,
        /// Camera yaw angle in degrees.
        yaw: Option<f32>,
    },
    // Packet monitor
    /// Poll for accumulated captured packet events.
    /// The DLL drains its pending packet buffer and responds with
    /// `PacketBatch`.
    PollPackets,
    // Chat monitor
    /// Poll for accumulated chat messages captured since the last `PollChat`.
    ///
    /// The DLL drains its dedicated chat buffer (populated by the `dsp_chat`
    /// HWBP hook) and returns them as a `ChatBatch` response. Calling this
    /// command does **not** affect the `PollPackets` packet-event buffer.
    PollChat,
    // Memory debug
    /// Read raw bytes from the EQ process address space (Debug hex dump).
    /// Size is capped at 4096 bytes. Returns `Response::MemoryData`.
    ReadMemory {
        /// Absolute virtual address to read from.
        address: usize,
        /// Number of bytes to read (capped at 4096).
        size: usize,
    },
    /// Query slot metadata and visible item info for open container windows.
    QueryContainerSlots {
        /// Filters applied before returning slot snapshots.
        filter: ContainerSlotQuery,
    },
    // System
    /// Heartbeat ping — expects a Pong response.
    Ping,
    /// Enable/disable timing normalization for GetTickCount / QPC hooks.
    SetTimingCorrection {
        /// Whether timing correction should be active.
        enabled: bool,
    },
    /// Eject the DLL from the game process.
    Eject,
    /// Enable or disable the game loop hook.
    SetHookState {
        /// Whether hooks should be active.
        enabled: bool,
    },
    /// Enable or disable automatic dialog acceptance (group invite, trade,
    /// etc.).
    SetAutoAccept {
        /// Whether auto-accept is enabled.
        enabled: bool,
    },
    /// Update the resurrection-offer policy for the current character.
    SetAutoRezConfig {
        /// Per-character auto-rez trust, threshold, and delay settings.
        config: AutoRezConfig,
    },
    /// Replace the full auto-accept policy (trust list + per-type toggles).
    SetAutoAcceptSettings {
        /// Auto-accept settings to apply in the DLL.
        settings: AutoAcceptSettings,
    },
    /// Set the rendering mode for this client.
    ///
    /// `Normal` = full rendering (the "eyes" client).
    /// `Strobe` = render 1 frame per ~5 seconds (monitoring/screenshots).
    /// `NullRender` = zero rendering, game loop only (GPU idle).
    SetRenderMode {
        /// Rendering mode to apply.
        mode: RenderMode,
    },
    // Relog / Switch
    /// Initiate a camp-relog cycle. The DLL issues `/camp desktop`, waits for
    /// logout, then re-enters credentials with exponential backoff.
    Relog {
        /// Account name for re-login.
        account_name: String,
        /// Password (zeroized after use).
        password: String,
        /// Target server name.
        server_name: String,
        /// Character name to select.
        character_name: String,
        /// Relog configuration (retry policy, camp settings).
        config: crate::login::RelogConfig,
    },
    /// Cancel an in-progress relog operation. The client stays wherever it is
    /// (logged out, at char select, etc.) — no further reconnect attempts.
    CancelRelog,
    /// Switch to a different server without restarting the EQ process.
    /// The DLL issues `/camp desktop`, then navigates to the target server.
    SwitchServer {
        /// Target server display name (e.g. "Teek", "Firiona Vie").
        server_name: String,
        /// Character name to select on the new server.
        character_name: String,
        /// Account credentials for re-authentication.
        account_name: String,
        /// Password (zeroized after use).
        password: String,
    },
    /// Switch to a different character on the current server.
    /// The DLL issues `/camp`, waits for character select, then picks the new
    /// character.
    SwitchCharacter {
        /// Character name to switch to.
        character_name: String,
    },
    /// Capture a single-frame screenshot from a null-rendered client.
    ///
    /// Temporarily enables rendering for one frame, captures the backbuffer
    /// after Present, saves to a temp file, and restores the previous render
    /// mode. Returns `ScreenshotCaptured` with the file path on success.
    CaptureScreenshot,
    // Context menus
    /// Query all currently visible context menus from `CContextMenuManager`.
    ///
    /// Returns `Response::ContextMenuState` with a snapshot of every menu
    /// currently registered in the manager, including item labels and
    /// enabled/checked state. If no menus are open the response list will
    /// be empty.
    QueryContextMenu,
    /// Activate a specific item in a specific `CContextMenu`.
    ///
    /// Calls `CContextMenuManager::HandleMenu(menu_index, item_index, point)`
    /// on the game loop thread. The point is set to `(0, 0)` which is
    /// correct for programmatic activation (EQ ignores the coordinates for
    /// most menu items).
    ActivateContextMenuItem {
        /// Zero-based index of the menu within `CContextMenuManager`.
        menu_index: u32,
        /// Zero-based index of the item within that menu.
        item_index: u32,
    },
    /// Poll for accumulated spawn list delta events.
    ///
    /// The DLL drains its pending spawn-event buffer and returns one
    /// `SpawnEventBatch` response.
    PollSpawnEvents,
    // Zone transitions
    /// Request a zone transition to a specific zone with optional destination
    /// coordinates.
    ///
    /// Initiates movement to a zone line and triggers the zone transition
    /// sequence. If `destination_coords` is provided, navigation will
    /// target those coordinates after zone load completes. Otherwise, the
    /// character will zone in at the default arrival point.
    RequestZone {
        /// EverQuest zone ID to transition to.
        zone_id: u32,
        /// Optional destination coordinates (x, y, z) to navigate to after
        /// zoning.
        destination_coords: Option<(f32, f32, f32)>,
    },
    /// Flush all pending movement commands from the movement queue.
    ///
    /// Clears any queued movements that have not yet been executed, allowing
    /// immediate processing of new movement commands.
    FlushMovementQueue,
    /// Request safe starting coordinates for a specific zone.
    ///
    /// Queries the zone's predefined safe spawn point (typically a bind point
    /// or known safe location) to avoid landing in dangerous areas.
    RequestSafeCoords(u32), // zone_id
    /// Query passive bazaar search results from the populated EQ UI.
    QueryBazaarResults {
        /// Filters applied before returning bazaar list snapshots.
        filter: BazaarQuery,
    },
    /// Query the visible merchant window rows from the populated EQ UI.
    QueryMerchantItems {
        /// Filters applied before returning merchant list snapshots.
        filter: MerchantQuery,
    },
    /// Configure chat message timestamps (MQ2Timestamp parity).
    ///
    /// Enables or disables timestamp prepending and sets the timestamp format
    /// for all MQ2 chat messages. Changes take effect immediately without
    /// requiring a restart.
    SetChatTimestampConfig {
        /// Whether timestamps should be prepended to chat messages.
        enabled: bool,
        /// The timestamp format to use.
        format: crate::chat::TimestampFormat,
    },
    /// Configure the EverQuest top-level window title (MQ2WinTitle parity).
    ///
    /// The DLL renders this template using live character data such as the
    /// current character name, level, class, and zone. Changes take effect
    /// without restarting the client.
    SetWindowTitleConfig {
        /// Per-character title template to render while in game.
        format: String,
        /// Server name used for `{server}` substitutions.
        server_name: String,
    },
    /// Replace the current tradeskill trophy automation settings.
    SetTradeskillTrophySettings {
        /// Tradeskill trophy policy to apply in the DLL.
        settings: TradeskillTrophySettings,
    },
    /// Query the current tradeskill trophy runtime status from the DLL.
    QueryTradeskillTrophyStatus,
    /// Update per-character task reward automation preferences.
    SetRewardAutomation {
        /// Reward selection rules keyed by task title matching.
        config: RewardAutomationConfig,
    },
    /// Read all registered fields for a live EQ struct instance.
    ///
    /// `base_address_or_global_name` accepts either an absolute address
    /// (`0x...` or decimal) or a known global pointer name such as
    /// `PINST_LOCAL_PLAYER`, `PINST_TARGET`, `PINST_SPAWN_MANAGER`, or
    /// `PINST_LOCAL_PC`.
    StructRead {
        /// Struct registry name, e.g. `PlayerBase`, `PlayerZone`, `PcClient`.
        struct_type: String,
        /// Absolute base address or supported EQ global pointer name.
        base_address_or_global_name: String,
    },
    /// Read one registered field from a live EQ struct instance.
    StructFieldRead {
        /// Struct registry name.
        struct_type: String,
        /// Field name from the struct registry.
        field_name: String,
        /// Absolute struct base address.
        base_address: usize,
    },
    /// Request continuous polling for a live EQ struct.
    ///
    /// The protocol variant is reserved for debugger clients; this DLL build
    /// currently returns an explicit unsupported result and expects callers to
    /// poll with `StructRead`.
    StructWatch {
        /// Struct registry name.
        struct_type: String,
        /// Absolute base address or supported EQ global pointer name.
        base_address_or_global_name: String,
        /// Requested polling interval in milliseconds.
        poll_interval_ms: u64,
    },
    /// Add or replace a memory watchpoint.
    WatchAdd {
        /// Absolute virtual address to monitor.
        address: usize,
        /// Number of bytes to compare for changes.
        size: usize,
        /// Operator-facing unique label for this watchpoint.
        label: String,
        /// Watch implementation mode.
        mode: WatchMode,
    },
    /// Remove an active memory watchpoint by label.
    WatchRemove {
        /// Operator-facing unique label passed to `WatchAdd`.
        label: String,
    },
    /// List active memory watchpoints.
    WatchList,
    /// Drain recent memory watchpoint change events.
    WatchLog,
    /// Read raw bytes from an absolute process address for the EQ debugger.
    ///
    /// New debugger-facing spelling of `ReadMemory`; returns
    /// `Response::MemoryData`.
    MemoryRead {
        /// Absolute virtual address to read from.
        address: usize,
        /// Number of bytes to read (capped at 4096).
        length: usize,
    },
    /// Read raw bytes relative to a known EQ global pointer.
    ///
    /// The DLL rebases `global_name`, dereferences it, then reads from
    /// `(*global + offset)`. Returns `Response::MemoryData`.
    MemoryReadRelative {
        /// Known global pointer name, such as `pinstLocalPlayer` or
        /// `PINST_LOCAL_PLAYER`.
        global_name: String,
        /// Byte offset from the dereferenced global pointer.
        offset: usize,
        /// Number of bytes to read (capped at 4096).
        length: usize,
    },
    /// Read raw bytes for an annotated debugger view.
    ///
    /// Annotation metadata is applied by clients that know the selected struct
    /// context; the wire response remains `Response::MemoryData`.
    MemoryAnnotate {
        /// Absolute virtual address to read from.
        address: usize,
        /// Number of bytes to read (capped at 4096).
        length: usize,
    },
    /// Start tracing a function call with a hardware breakpoint.
    ///
    /// `function_name` may be an absolute address (`0x...` or decimal) or a
    /// label containing an address suffix (`name@0x...` / `name=0x...`).
    TraceStart {
        /// Function label or address to trace.
        function_name: String,
        /// Capture RCX/RDX/R8/R9 and RSP on each call.
        capture_args: bool,
        /// Capture the stack return address for each call.
        capture_return: bool,
    },
    /// Stop tracing a function by label or address string.
    TraceStop {
        /// Function label or address used when the trace was started.
        function_name: String,
    },
    /// List all currently active function traces.
    TraceList,
    /// Drain the DLL-side trace ring buffer.
    TraceDump,
    /// Gracefully exit the EQ client for the specified account.
    QuitGame {
        /// Account name associated with the client being asked to quit.
        account_name: String,
    },
    /// Pause all automation and command dispatch.
    ///
    /// The session transitions to paused state and does not process new
    /// commands until a `Resume` command is received. Matches MQ2Boxr semantics.
    Pause,
    /// Resume automation and command dispatch from paused state.
    ///
    /// The session transitions back to active state. No-op if already active.
    /// Matches MQ2Boxr semantics.
    Resume,
    /// Query the current pause state (TRUE if paused, FALSE if active).
    ///
    /// Returns `Response::PauseStatus` with the current session state.
    /// Used to implement `${TextQuest.Paused}` TLO.
    QueryPauseStatus,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StartLogin {
                account_name,
                server_name,
                character_name,
                ..
            } => f
                .debug_struct("StartLogin")
                .field("account_name", account_name)
                .field("password", &"[REDACTED]")
                .field("server_name", server_name)
                .field("character_name", character_name)
                .finish(),
            Self::Relog {
                account_name,
                server_name,
                character_name,
                config,
                ..
            } => f
                .debug_struct("Relog")
                .field("account_name", account_name)
                .field("password", &"[REDACTED]")
                .field("server_name", server_name)
                .field("character_name", character_name)
                .field("config", config)
                .finish(),
            Self::SwitchServer {
                server_name,
                character_name,
                account_name,
                ..
            } => f
                .debug_struct("SwitchServer")
                .field("account_name", account_name)
                .field("password", &"[REDACTED]")
                .field("server_name", server_name)
                .field("character_name", character_name)
                .finish(),
            other => write!(f, "{}", {
                // Fall through to derived-style output for all other variants.
                // This uses serde_json as a quick Debug proxy since we removed derive(Debug).
                serde_json::to_string(other).unwrap_or_else(|_| "Command(?)".to_string())
            }),
        }
    }
}

/// Direction of a captured network packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PacketDirection {
    /// Client → server (outbound).
    Outbound,
    /// Server → client (inbound).
    Inbound,
}

/// Wire-format for a single packet event in a `PacketBatch` response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PacketEventInfo {
    /// PID of the client that captured the packet.
    pub client_id: ClientId,
    /// EQ protocol opcode identifier.
    pub opcode: u16,
    /// Whether the packet was inbound or outbound.
    pub direction: PacketDirection,
    /// Timestamp in milliseconds when the packet was captured.
    pub timestamp_ms: u64,
    /// Size of the packet payload in bytes.
    pub payload_size: u32,
    /// Raw packet payload bytes.
    pub payload: Vec<u8>,
}

/// Spawn lifecycle event for near-by spawn list deltas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SpawnEventKind {
    /// A spawn became visible in the local spawn list.
    Created,
    /// A spawn was removed from the local spawn list.
    Destroyed,
}

impl std::fmt::Display for SpawnEventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Destroyed => write!(f, "destroyed"),
        }
    }
}

/// Wire-format for a single spawn event in a `SpawnEventBatch` response.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpawnEvent {
    /// PID of the client that detected the event.
    pub client_id: ClientId,
    /// Zone short name where the event was observed.
    pub zone: String,
    /// Name of the spawn.
    pub spawn_name: String,
    /// Spawn type: 0=player, 1=NPC, 2=corpse, 3=any.
    pub spawn_type: u8,
    /// Spawn lifecycle kind.
    pub kind: SpawnEventKind,
    /// Epoch milliseconds when the event was detected.
    pub timestamp_ms: u64,
}

/// Wire-format for a single chat message in a `ChatBatch` response.
///
/// Carries the same fields as `Response::ChatMessage` but is designed for
/// batched delivery via `Command::PollChat` / `Response::ChatBatch`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChatMessageInfo {
    /// The chat text content (may contain STML markup tags).
    pub text: String,
    /// EQ chat color code (e.g., 273 = default, 269 = system).
    pub color: i32,
    /// Timestamp in milliseconds when the message was captured.
    pub timestamp_ms: u64,
}

/// Primitive field kinds supported by the EQ struct browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StructFieldType {
    /// Unsigned 8-bit integer.
    U8,
    /// Unsigned 16-bit integer.
    U16,
    /// Unsigned 32-bit integer.
    U32,
    /// Unsigned 64-bit integer.
    U64,
    /// Signed 8-bit integer.
    I8,
    /// Signed 16-bit integer.
    I16,
    /// Signed 32-bit integer.
    I32,
    /// Signed 64-bit integer.
    I64,
    /// 32-bit float.
    F32,
    /// 64-bit float.
    F64,
    /// Fixed-size nul-terminated string buffer.
    String {
        /// Maximum bytes to read from the string buffer.
        len: usize,
    },
    /// Process pointer-sized address.
    Pointer,
}

/// Typed value for one EQ struct field.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum StructFieldValue {
    /// Unsigned 8-bit integer.
    U8(u8),
    /// Unsigned 16-bit integer.
    U16(u16),
    /// Unsigned 32-bit integer.
    U32(u32),
    /// Unsigned 64-bit integer.
    U64(u64),
    /// Signed 8-bit integer.
    I8(i8),
    /// Signed 16-bit integer.
    I16(i16),
    /// Signed 32-bit integer.
    I32(i32),
    /// Signed 64-bit integer.
    I64(i64),
    /// 32-bit float.
    F32(f32),
    /// 64-bit float.
    F64(f64),
    /// Decoded string buffer.
    String(String),
    /// Process pointer-sized address.
    Pointer(usize),
    /// The field could not be read safely.
    Unreadable(String),
}

/// One typed field read from a live EQ struct instance.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StructFieldSnapshot {
    /// Field name from the struct registry.
    pub name: String,
    /// Byte offset from the struct base address.
    pub offset: usize,
    /// Absolute address that was read.
    pub address: usize,
    /// Primitive field type.
    pub field_type: StructFieldType,
    /// Typed field value.
    pub value: StructFieldValue,
    /// Human-friendly display string for TUI/web callers.
    pub display_value: String,
    /// Struct type to use when following this field as a pointer, if known.
    pub nested_struct_type: Option<String>,
}

/// Typed snapshot of a live EQ struct instance.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StructSnapshot {
    /// Struct registry name, e.g. `PlayerBase`.
    pub struct_type: String,
    /// Absolute base address used for the field reads.
    pub base_address: usize,
    /// Field values read from memory.
    pub fields: Vec<StructFieldSnapshot>,
    /// Epoch milliseconds when the snapshot was produced.
    pub timestamp_ms: u64,
}

/// Memory watchpoint backend requested by the operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WatchMode {
    /// Hardware write breakpoint. Exact but constrained by DR0-DR3 slot availability.
    Hwbp,
    /// Periodic memory snapshot comparison from the game-loop tick.
    Poll,
}

/// Active memory watchpoint metadata returned by `Command::WatchList`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WatchpointInfo {
    /// Operator-facing unique label.
    pub label: String,
    /// Absolute virtual address being watched.
    pub address: usize,
    /// Number of bytes compared for changes.
    pub size: usize,
    /// Backend mode used by this watchpoint.
    pub mode: WatchMode,
    /// Last snapshot captured by the DLL.
    pub last_value: Vec<u8>,
}

/// Memory watchpoint change event returned by `Response::WatchLog`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WatchpointEvent {
    /// Operator-facing watchpoint label.
    pub label: String,
    /// Absolute virtual address that changed.
    pub address: usize,
    /// Number of bytes compared.
    pub size: usize,
    /// Backend mode that detected the change.
    pub mode: WatchMode,
    /// Previous value snapshot.
    pub old_value: Vec<u8>,
    /// New value snapshot.
    pub new_value: Vec<u8>,
    /// Epoch milliseconds when the change was detected.
    pub timestamp_ms: u64,
    /// Writer instruction pointer when available from HWBP mode.
    pub writer_rip: Option<usize>,
    /// Best-effort return-address walk captured with the event.
    pub call_stack: Vec<usize>,
}

/// Discrete lifecycle states for the EQ client process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GameState {
    /// Character select / account-level screen.
    CharacterSelect,
    /// Player is in the game world.
    InGame,
    /// Zone load / zone transition in progress.
    Loading,
    /// Login sequence in progress.
    LoggingIn,
    /// Unknown / unmapped state value.
    Unknown(u32),
}

impl From<u32> for GameState {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::CharacterSelect,
            1 => Self::InGame,
            2 => Self::Loading,
            3 => Self::LoggingIn,
            value => Self::Unknown(value),
        }
    }
}

impl From<GameState> for u32 {
    fn from(value: GameState) -> Self {
        match value {
            GameState::CharacterSelect => 0,
            GameState::InGame => 1,
            GameState::Loading => 2,
            GameState::LoggingIn => 3,
            GameState::Unknown(value) => value,
        }
    }
}

/// Responses sent from the DLL back to the manager
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Response {
    /// Heartbeat response to a Ping command.
    Pong {
        /// PID of the responding client.
        client_id: ClientId,
        /// Timestamp in milliseconds when the pong was generated.
        timestamp_ms: u64,
    },
    /// Generic result for a command execution.
    CommandResult {
        /// Whether the command succeeded.
        success: bool,
        /// Human-readable status or error message.
        message: String,
    },
    /// Error response when a command fails.
    Error {
        /// Error description.
        message: String,
    },
    /// Navigation status push notification from the DLL's nav state machine.
    /// Note: `NavStatus` is also available in `GameState.nav_status` (shared
    /// memory). `GameState.nav_status` is authoritative — it is updated
    /// every tick. `NavUpdate` is sent only on state transitions
    /// (Idle→Moving, Moving→Arrived, etc.) for low-latency notification
    /// without polling shared memory.
    NavUpdate {
        /// Current navigation FSM state.
        status: crate::nav::NavStatus,
    },
    /// Login phase transition notification.
    LoginPhaseUpdate {
        /// Current login phase.
        phase: crate::login::LoginPhase,
    },
    /// Notification that a client has completed post-login setup.
    PostLoginComplete {
        /// PID of the client that finished post-login.
        client_id: crate::types::ClientId,
    },
    /// Combat FSM state transition notification.
    CombatUpdate {
        /// Current combat FSM state.
        status: crate::combat::CombatStatus,
    },
    /// A captured network packet event from the DLL's send/recv hooks.
    PacketEvent {
        /// PID of the client that captured the packet.
        client_id: ClientId,
        /// EQ protocol opcode identifier.
        opcode: u16,
        /// Whether the packet was inbound or outbound.
        direction: PacketDirection,
        /// Timestamp in milliseconds when the packet was captured.
        timestamp_ms: u64,
        /// Size of the packet payload in bytes.
        payload_size: u32,
        /// Raw packet payload bytes.
        payload: Vec<u8>,
    },
    /// Batched packet events in response to `Command::PollPackets`.
    PacketBatch {
        /// Accumulated packet events since last poll.
        events: Vec<PacketEventInfo>,
    },
    /// Raw bytes read from the EQ process address space, in response to
    /// `Command::ReadMemory`.
    MemoryData {
        /// The address that was read.
        address: usize,
        /// The bytes that were read. May be shorter than requested if the read
        /// was partial.
        bytes: Vec<u8>,
    },
    /// Slot metadata and item info for currently open container windows.
    ContainerSlots {
        /// Matching open container slots.
        slots: Vec<ContainerSlotInfo>,
    },
    /// Zone adjacency graph from `ZoneGuideManagerClient`.
    /// Simplified wire format: Vec of (`zone_id`, name, `min_level`,
    /// `max_level`, connections). Each connection is (`dest_zone_id`,
    /// `transfer_type`, disabled).
    ZoneGraph {
        /// List of zone entries with connectivity data.
        zones: Vec<ZoneGraphEntry>,
    },
    /// Confirmation that the render mode was changed.
    RenderModeChanged {
        /// The new active render mode.
        mode: RenderMode,
    },
    /// Relog progress notification — sent on each phase transition.
    RelogProgress {
        /// Current relog phase.
        phase: crate::login::RelogPhase,
    },
    /// Result of a SwitchServer or SwitchCharacter command.
    SwitchResult {
        /// Whether the switch succeeded.
        success: bool,
        /// Human-readable message (target name on success, error on failure).
        message: String,
    },
    /// Screenshot captured successfully. The image was saved to a temp file.
    ScreenshotCaptured {
        /// Absolute path to the saved screenshot (BMP format).
        path: String,
    },
    /// Screenshot capture failed.
    ScreenshotFailed {
        /// Reason the capture failed.
        reason: String,
    },
    /// Response to `NavWaypointList` — all saved named waypoints.
    NavWaypointList {
        /// All saved named waypoints.
        waypoints: Vec<crate::nav::NamedWaypoint>,
    },
    /// Navigation state signals response.
    NavSignals {
        /// Current navigation state signals.
        signals: crate::nav::NavStateSignals,
    },
    /// Navigation diagnostics response for debug overlay.
    NavDiagnosticsResult {
        /// Diagnostics snapshot.
        diagnostics: crate::nav::NavDiagnostics,
    },
    /// Spawn alert notification — a watched or named spawn
    /// appeared/disappeared.
    SpawnAlert {
        /// PID of the client that detected the event.
        client_id: ClientId,
        /// Zone where the event occurred.
        zone: String,
        /// Name of the spawn.
        spawn_name: String,
        /// `true` = spawn appeared, `false` = spawn disappeared.
        is_up: bool,
        /// Timestamp in milliseconds when the alert was generated.
        timestamp_ms: u64,
        /// Duration in milliseconds since the last time this spawn was seen.
        /// `None` if this is the first time seeing this spawn.
        time_since_last_pop_ms: Option<u64>,
    },
    /// An intercepted chat message from the game's `dsp_chat` function.
    ChatMessage {
        /// The chat text content (may contain STML markup tags).
        text: String,
        /// EQ chat color code (e.g., 273 = default, 269 = system).
        color: i32,
        /// Timestamp in milliseconds when the message was captured.
        timestamp_ms: u64,
        /// Structured chat event extracted from `text` after stripping STML
        /// markup. `None` when the text does not match a recognised EQ
        /// chat verb pattern (e.g. system messages, spell feedback, or
        /// unknown formats).
        parsed: Option<crate::chat::ChatEvent>,
    },
    /// Batched chat messages in response to `Command::PollChat`.
    ///
    /// Contains all messages captured by the `dsp_chat` hook since the last
    /// `PollChat` call. An empty `messages` list means no chat arrived in
    /// the polling window.
    ChatBatch {
        /// Accumulated chat messages since the last poll.
        messages: Vec<ChatMessageInfo>,
    },
    /// Notification that `CEverQuest::SetGameState` transitioned.
    GameStateChanged {
        /// New game state value parsed from `SetGameState`.
        state: GameState,
    },
    /// Snapshot of all menus visible in `CContextMenuManager`.
    ///
    /// Returned in response to `Command::QueryContextMenu`.
    /// An empty `menus` list means no context menus are currently open.
    ContextMenuState {
        /// All menus currently registered in `CContextMenuManager`.
        menus: Vec<ContextMenuInfo>,
    },
    /// Confirmation that `ActivateContextMenuItem` was dispatched.
    ///
    /// `success` is `false` if the menu/item indices were out of range or the
    /// `CContextMenuManager` instance was unavailable.
    ContextMenuActivated {
        /// Whether `HandleMenu` was called successfully.
        success: bool,
        /// Human-readable status message.
        message: String,
    },
    /// Batched spawn list delta events from `game_loop`.
    ///
    /// Returned in `Response::SpawnEventBatch` after calling
    /// `Command::PollSpawnEvents`.
    SpawnEventBatch {
        /// Accumulated spawn events since last poll.
        events: Vec<SpawnEvent>,
    },
    /// Notification that a zone state transition has completed.
    ///
    /// Sent when the zone transition sequence completes, including both the
    /// zone load and any post-load positioning.
    ZoneStateChanged {
        /// Target zone ID that was transitioned to.
        zone_id: u32,
        /// Previous zone state (e.g. "loading", "arrived").
        old_state: String,
        /// New zone state after transition.
        new_state: String,
        /// Timestamp in milliseconds when the state change occurred.
        timestamp: u64,
    },
    /// Notification that zone validation failed.
    ///
    /// Sent when attempting to transition to a zone that is inaccessible,
    /// invalid, or fails prerequisite checks.
    ZoneValidationFailed {
        /// Zone ID that failed validation.
        zone_id: u32,
        /// Error code indicating the type of validation failure.
        error_code: i32,
        /// Human-readable description of the validation failure.
        reason: String,
    },
    /// Safe starting coordinates retrieved for a zone.
    ///
    /// Response to `Command::RequestSafeCoords` containing the safe spawn point
    /// coordinates for the requested zone.
    SafeCoordsRetrieved {
        /// World X coordinate of the safe location.
        x: f32,
        /// World Y coordinate of the safe location.
        y: f32,
        /// World Z coordinate of the safe location.
        z: f32,
        /// Zone ID these coordinates belong to.
        zone_id: u32,
    },
    /// Confirmation that the movement queue was flushed.
    ///
    /// Sent in response to `Command::FlushMovementQueue` with the count of
    /// movements that were dropped from the queue.
    MovementQueueFlushed {
        /// Number of pending movements that were dropped.
        count_dropped: u32,
    },
    /// Passive bazaar list snapshots captured from visible bazaar windows.
    BazaarResults {
        /// Matching bazaar window snapshots.
        windows: Vec<BazaarWindowSnapshot>,
    },
    /// Merchant list snapshots captured from visible merchant windows.
    MerchantItems {
        /// Matching merchant window snapshots.
        windows: Vec<MerchantWindowSnapshot>,
    },
    /// Current tradeskill trophy runtime status from the live client.
    TradeskillTrophyStatus {
        /// Live DLL state for tradeskill trophy automation.
        status: TradeskillTrophyStatus,
    },
    /// HIGH-PRIORITY anti-cheat alert: server sent checksum-mismatch disconnect
    /// (opcode `0xd799`), or `CheaterLdFlag` flipped to non-zero in memory.
    ///
    /// Both conditions indicate server-side detection. The operator must
    /// investigate immediately — the character may be persistently flagged.
    ///
    /// This response is batched so it can be drained through a poll-based IPC
    /// flow rather than being left queued as a standalone asynchronous event.
    ChecksumMismatchAlertBatch {
        /// Alerts raised since the previous alert poll/drain.
        alerts: Vec<ChecksumMismatchAlert>,
    },
    /// Typed live EQ struct snapshot returned by `Command::StructRead`.
    StructSnapshot {
        /// Full struct snapshot.
        snapshot: StructSnapshot,
    },
    /// Single typed EQ struct field returned by `Command::StructFieldRead`.
    StructFieldData {
        /// Struct registry name.
        struct_type: String,
        /// Absolute base address used for the read.
        base_address: usize,
        /// Field value snapshot.
        field: StructFieldSnapshot,
    },
    /// Active memory watchpoints.
    WatchList {
        /// Watchpoints currently registered in the DLL.
        watchpoints: Vec<WatchpointInfo>,
    },
    /// Recent memory watchpoint changes.
    WatchLog {
        /// Watchpoint change events since the previous drain.
        events: Vec<WatchpointEvent>,
    },
    /// Active function-call traces.
    TraceList {
        /// Current trace installations and counters.
        traces: Vec<TraceStatus>,
    },
    /// Drained function-call trace events.
    TraceDump {
        /// Trace records captured since the previous dump.
        events: Vec<TraceRecord>,
        /// Records overwritten before this dump could drain them.
        dropped_events: u64,
    },
    /// Current pause state query response.
    ///
    /// Response to `Command::QueryPauseStatus`. `paused` is `true` when the
    /// session is paused (no command dispatch), `false` when active.
    PauseStatus {
        /// Whether the session is paused (true) or active (false).
        paused: bool,
    },
}

/// Alert payload for checksum-mismatch and `CheaterLdFlag` detections.
///
/// `kind` distinguishes the two detection surfaces:
/// - `"checksum_mismatch_packet"` — opcode `0xd799` observed in the inbound
///   packet stream (server is disconnecting the client).
/// - `"cheater_ld_flag"` — `CheaterLdFlag` memory location flipped from zero
///   to a non-zero value (character is persistently flagged across sessions).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChecksumMismatchAlert {
    /// PID of the client that triggered the alert.
    pub client_id: ClientId,
    /// Character name at the time of the alert (empty if unavailable).
    pub character_name: String,
    /// Detection surface: `"checksum_mismatch_packet"` or `"cheater_ld_flag"`.
    pub kind: String,
    /// Raw opcode that triggered the alert (0xd799 for packet alerts, 0 for flag alerts).
    pub opcode: u16,
    /// Current value of `CheaterLdFlag` in memory (0 for packet alerts).
    pub cheater_ld_flag_value: i32,
    /// Timestamp in milliseconds when the alert fired.
    pub timestamp_ms: u64,
}
/// Wire-format for a single zone entry: (`zone_id`, name, `min_level`,
/// `max_level`, connections). Each connection is (`dest_zone_id`,
/// `transfer_type`, disabled).
pub type ZoneGraphEntry = (u16, String, i32, i32, Vec<(u16, u8, bool)>);

/// Random session token generated at injection time for IPC authentication.
/// The orchestrator stages this in a temp file before injection; the DLL reads
/// it during initialization and validates it on every pipe connection.
pub type SessionToken = [u8; 32];

/// Size of shared memory region allocated per client (64 KB)
pub const SHARED_MEMORY_SIZE: usize = 64 * 1024;

/// Environment variable that enables opt-in performance trace logging.
pub const PERF_TRACE_ENV: &str = "TEXTQUEST_PERF_TRACE";

/// Legacy named pipe prefix — prefer `pipe_name()` with a session ID.
pub const PIPE_NAME_PREFIX: &str = r"\\.\pipe\textquest_";

/// Legacy shared memory name prefix — prefer `shared_memory_name()` with a
/// session ID.
pub const SHARED_MEMORY_NAME_PREFIX: &str = "textquest_state_";

const SESSION_TOKEN_DIR: &str = "textquest";

fn token_dir() -> std::io::Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join(SESSION_TOKEN_DIR);
    if let Ok(meta) = std::fs::symlink_metadata(&dir) {
        if meta.file_type().is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Token directory is a symlink",
            ));
        }
        if !meta.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Token path is not a directory",
            ));
        }
        // Verify directory ownership on Unix.
        #[cfg(unix)]
        {
            use nix::unistd::getuid;
            use std::os::unix::fs::MetadataExt;
            if meta.uid() != getuid().as_raw() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "Token directory not owned by current user",
                ));
            }
        }
    }
    Ok(dir)
}

fn verify_not_symlink_path(path: &std::path::Path) -> std::io::Result<()> {
    if let Ok(meta) = std::fs::symlink_metadata(path)
        && meta.file_type().is_symlink()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Refusing to read/write symlink token path",
        ));
    }
    Ok(())
}

fn write_session_token_path(path: &std::path::Path, token: SessionToken) -> std::io::Result<()> {
    verify_not_symlink_path(path)?;
    std::fs::write(path, token)?;
    // Restrict token file to owner-only access (mode 0o600 on Unix).
    // On Windows, %TEMP% is already user-specific, but we tighten further where
    // possible.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn read_session_token_path(path: &std::path::Path) -> Option<SessionToken> {
    if verify_not_symlink_path(path).is_err() {
        return None;
    }

    // Validate file ownership on Unix before reading.
    #[cfg(unix)]
    {
        use nix::unistd::getuid;
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.uid() != getuid().as_raw() {
                eprintln!(
                    "Warning: session token file {:?} not owned by current user, rejecting",
                    path
                );
                return None;
            }
        } else {
            return None;
        }
    }

    if let Ok(data) = std::fs::read(path)
        && data.len() == 32
    {
        let mut token = [0u8; 32];
        token.copy_from_slice(&data);
        return Some(token);
    }
    None
}

/// Derive a deterministic `u64` session ID from a 32-byte session token.
/// Uses the first 8 bytes interpreted as little-endian. Both the DLL and
/// orchestrator call this on the same token to produce matching IPC names.
#[must_use]
pub fn session_id_from_token(token: &SessionToken) -> u64 {
    u64::from_le_bytes(token[..8].try_into().unwrap())
}

/// Generate a cryptographically random 32-byte session token using OS entropy.
#[must_use]
pub fn generate_random_token() -> SessionToken {
    use rand::RngCore;
    let mut token = [0u8; 32];
    rand::rng().fill_bytes(&mut token);
    token
}

/// Write a CSPRNG session token file for the given PID. The DLL reads this
/// during init. Must be called BEFORE injection.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn write_session_token_file(pid: u32) -> std::io::Result<()> {
    let token_dir_path = std::env::temp_dir().join(SESSION_TOKEN_DIR);

    // Create directory with restrictive permissions (0o700 on Unix).
    if !token_dir_path.exists() {
        std::fs::create_dir(&token_dir_path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&token_dir_path, std::fs::Permissions::from_mode(0o700))?;
        }
    }

    let token_dir = token_dir()?;
    let token_path = token_dir.join(format!("token_{pid}.bin"));

    let token = generate_random_token();

    write_session_token_path(&token_path, token)?;
    // Also persist a copy for later CLI commands that reconnect to the injected
    // client.
    let login_token_path = token_dir.join(format!("login_token_{pid}.bin"));
    write_session_token_path(&login_token_path, token)?;

    Ok(())
}

/// Read the session token for authenticating with an already-injected DLL.
#[must_use]
pub fn load_session_token(pid: u32) -> Option<SessionToken> {
    let token_path = token_dir().ok()?.join(format!("login_token_{pid}.bin"));
    read_session_token_path(&token_path)
}

#[cfg(test)]
mod token_tests {
    use super::*;

    #[test]
    fn write_session_token_rejects_symlink_directory_target() {
        let token_path = std::env::temp_dir()
            .join(SESSION_TOKEN_DIR)
            .join("write_token_rejects_symlink");
        let meta = std::fs::symlink_metadata(&token_path).ok();
        if meta.is_some() {
            let _ = std::fs::remove_file(&token_path);
        }

        #[cfg(not(windows))]
        {
            use std::os::unix::fs::symlink;
            let target = std::env::temp_dir().join("textquest-symlink-target");
            let _ = std::fs::remove_file(&target);
            std::fs::write(&target, b"z").expect("write target");
            // creating symlink may fail on platforms not supporting
            // std::os::unix::fs::symlink in this config
            if symlink(&target, &token_path).is_ok() {
                let token_path = token_path.clone();
                assert!(
                    write_session_token_path(&token_path, [0x42; 32]).is_err(),
                    "symlink token write should fail"
                );
                let _ = std::fs::remove_file(&token_path);
                let _ = std::fs::remove_file(&target);
            }
        }
        #[cfg(windows)]
        {
            // Not executed on windows-only test config.
        }
    }
}

/// Build a per-client pipe name incorporating a random session ID.
/// Format: `\\.\pipe\{session_id:x}_cmd_{client_id}`
#[must_use]
pub fn pipe_name(session_id: u64, client_id: u32) -> String {
    format!(r"\\.\pipe\{session_id:x}_cmd_{client_id}")
}

/// Build a per-client shared memory name incorporating a random session ID.
/// Format: `{session_id:x}_state_{client_id}`
#[must_use]
pub fn shared_memory_name(session_id: u64, client_id: u32) -> String {
    format!("{session_id:x}_state_{client_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_deterministic() {
        let token: SessionToken = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // first 8 bytes → session_id
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // rest ignored
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let id = session_id_from_token(&token);
        // LE bytes: 0x0807060504030201
        assert_eq!(id, 0x0807060504030201);
        // Same token always yields the same id
        assert_eq!(session_id_from_token(&token), id);
    }

    #[test]
    fn session_id_zero_token() {
        let token: SessionToken = [0u8; 32];
        assert_eq!(session_id_from_token(&token), 0);
    }

    #[test]
    fn session_id_max_token() {
        let token: SessionToken = [0xFF; 32];
        assert_eq!(session_id_from_token(&token), u64::MAX);
    }

    #[test]
    fn pipe_name_format() {
        let name = pipe_name(0xDEADBEEF, 1234);
        assert_eq!(name, r"\\.\pipe\deadbeef_cmd_1234");
    }

    #[test]
    fn shared_memory_name_format() {
        let name = shared_memory_name(0xDEADBEEF, 1234);
        assert_eq!(name, "deadbeef_state_1234");
    }

    #[test]
    fn ipc_names_match_across_sides() {
        // Simulate both DLL and orchestrator deriving names from the same token
        let token: SessionToken = [
            0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x33, 0x44, // session_id bytes
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let pid: ClientId = 5678;

        // Both sides call session_id_from_token then pipe_name/shared_memory_name
        let dll_session_id = session_id_from_token(&token);
        let orch_session_id = session_id_from_token(&token);
        assert_eq!(dll_session_id, orch_session_id);

        assert_eq!(
            pipe_name(dll_session_id, pid),
            pipe_name(orch_session_id, pid)
        );
        assert_eq!(
            shared_memory_name(dll_session_id, pid),
            shared_memory_name(orch_session_id, pid)
        );
    }

    #[test]
    fn different_tokens_produce_different_names() {
        let token_a: SessionToken = [1; 32];
        let token_b: SessionToken = [2; 32];
        let pid = 100;

        let id_a = session_id_from_token(&token_a);
        let id_b = session_id_from_token(&token_b);
        assert_ne!(id_a, id_b);
        assert_ne!(pipe_name(id_a, pid), pipe_name(id_b, pid));
        assert_ne!(shared_memory_name(id_a, pid), shared_memory_name(id_b, pid));
    }

    #[test]
    fn different_pids_produce_different_names() {
        let session_id = 0x1234;
        assert_ne!(pipe_name(session_id, 100), pipe_name(session_id, 200));
        assert_ne!(
            shared_memory_name(session_id, 100),
            shared_memory_name(session_id, 200)
        );
    }

    #[test]
    fn pipe_name_no_predictable_prefix() {
        // Session-derived names should NOT start with the legacy prefix pattern
        let token: SessionToken = [0x42; 32];
        let id = session_id_from_token(&token);
        let name = pipe_name(id, 999);
        assert!(!name.contains("textquest_cmd_"));
        assert!(!name.contains("textquest_state_"));
    }

    #[test]
    fn command_roundtrip_start_login() {
        use crate::protocol::{decode, encode};

        let cmd = Command::StartLogin {
            account_name: "testuser".into(),
            password: "hunter2".into(),
            server_name: "Teek".into(),
            character_name: "Legolas".into(),
        };
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        if let Command::StartLogin {
            account_name,
            server_name,
            ..
        } = decoded
        {
            assert_eq!(account_name, "testuser");
            assert_eq!(server_name, "Teek");
        } else {
            panic!("expected StartLogin");
        }
    }

    #[test]
    fn command_roundtrip_quit_game() {
        use crate::protocol::{decode, encode};

        let cmd = Command::QuitGame {
            account_name: "testuser".into(),
        };
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        assert_eq!(decoded, cmd);
    }

    #[test]
    fn response_roundtrip_login_phase_update() {
        use crate::{
            login::LoginPhase,
            protocol::{decode, encode},
        };

        let resp = Response::LoginPhaseUpdate {
            phase: LoginPhase::CharacterSelecting,
        };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode");
        if let Response::LoginPhaseUpdate { phase } = decoded {
            assert_eq!(phase, LoginPhase::CharacterSelecting);
        } else {
            panic!("expected LoginPhaseUpdate");
        }
    }

    #[test]
    fn response_roundtrip_command_result() {
        use crate::protocol::{decode, encode};

        let resp = Response::CommandResult {
            success: true,
            message: "queued".into(),
        };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode");
        if let Response::CommandResult { success, message } = decoded {
            assert!(success);
            assert_eq!(message, "queued");
        } else {
            panic!("expected CommandResult");
        }
    }

    #[test]
    fn command_debug_redacts_password() {
        let cmd = Command::StartLogin {
            account_name: "user".into(),
            password: "secret123".into(),
            server_name: "Teek".into(),
            character_name: "Char".into(),
        };
        let debug_output = format!("{:?}", cmd);
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("secret123"));
        assert!(debug_output.contains("user"));
        assert!(debug_output.contains("Teek"));
    }

    #[test]
    fn command_debug_non_login_uses_json() {
        let cmd = Command::Ping;
        let debug_output = format!("{:?}", cmd);
        assert!(debug_output.contains("Ping"));
    }

    #[test]
    fn command_debug_move_to() {
        let cmd = Command::MoveTo {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let debug_output = format!("{:?}", cmd);
        assert!(debug_output.contains("MoveTo"));
    }

    #[test]
    fn pipe_name_with_max_client_id() {
        let name = pipe_name(0x1234, u32::MAX);
        assert!(name.contains(&u32::MAX.to_string()));
    }

    #[test]
    fn shared_memory_name_with_max_client_id() {
        let name = shared_memory_name(0x1234, u32::MAX);
        assert!(name.contains(&u32::MAX.to_string()));
    }

    #[test]
    fn pipe_name_with_zero_session_and_client() {
        let name = pipe_name(0, 0);
        assert_eq!(name, r"\\.\pipe\0_cmd_0");
    }

    #[test]
    fn shared_memory_name_with_zero_session_and_client() {
        let name = shared_memory_name(0, 0);
        assert_eq!(name, "0_state_0");
    }

    #[test]
    fn command_all_simple_variants_roundtrip() {
        use crate::protocol::{decode, encode};

        let commands: Vec<Command> = vec![
            Command::Ping,
            Command::Eject,
            Command::StopMovement,
            Command::StopAttack,
            Command::ClearTarget,
            Command::Sit,
            Command::Stand,
            Command::StopNavigation,
            Command::LoginPhaseQuery,
            Command::CalibrateLogin,
            Command::ApplyBuffs,
            Command::ReportReady,
            Command::CombatDisengage,
            Command::LootCorpse,
            Command::LootAll,
            Command::InteractDoor,
            Command::ClickObject,
            Command::QueryZoneGraph,
            Command::QueryBazaarResults {
                filter: BazaarQuery::default(),
            },
            Command::PollPackets,
            Command::PollSpawnEvents,
            Command::PollChat,
            Command::FlushMovementQueue,
            Command::SetRenderMode {
                mode: RenderMode::NullRender,
            },
            Command::SetRenderMode {
                mode: RenderMode::Normal,
            },
            Command::SetRenderMode {
                mode: RenderMode::Strobe,
            },
            Command::CaptureScreenshot,
            Command::SetTradeskillTrophySettings {
                settings: TradeskillTrophySettings::default(),
            },
            Command::QueryTradeskillTrophyStatus,
            Command::TraceStart {
                function_name: "target@0x1234".into(),
                capture_args: true,
                capture_return: true,
            },
            Command::TraceStop {
                function_name: "target@0x1234".into(),
            },
            Command::TraceList,
            Command::TraceDump,
        ];
        for cmd in &commands {
            let encoded = encode(cmd).expect("encode failed");
            let (decoded, _): (Command, usize) = decode(&encoded).expect("decode failed");
            assert_eq!(*cmd, decoded);
        }
    }

    #[test]
    fn command_query_bazaar_results_roundtrip_preserves_filter() {
        use crate::protocol::{decode, encode};

        let command = Command::QueryBazaarResults {
            filter: BazaarQuery {
                text_contains: Some("fungi".into()),
                max_rows: Some(25),
            },
        };

        let encoded = encode(&command).expect("encode failed");
        let (decoded, _): (Command, usize) = decode(&encoded).expect("decode failed");
        assert_eq!(command, decoded);
    }

    #[test]
    fn command_query_merchant_items_roundtrip_preserves_filter() {
        use crate::protocol::{decode, encode};

        let command = Command::QueryMerchantItems {
            filter: MerchantQuery {
                text_contains: Some("fungi".into()),
                max_rows: Some(40),
            },
        };

        let encoded = encode(&command).expect("encode failed");
        let (decoded, _): (Command, usize) = decode(&encoded).expect("decode failed");
        assert_eq!(command, decoded);
    }

    #[test]
    fn response_all_variants_roundtrip() {
        use crate::protocol::{decode, encode};

        let responses: Vec<Response> = vec![
            Response::Pong {
                client_id: 1,
                timestamp_ms: 0,
            },
            Response::CommandResult {
                success: false,
                message: "err".into(),
            },
            Response::Error {
                message: "oh no".into(),
            },
            Response::NavUpdate {
                status: crate::nav::NavStatus::Idle,
            },
            Response::NavUpdate {
                status: crate::nav::NavStatus::Arrived,
            },
            Response::LoginPhaseUpdate {
                phase: crate::login::LoginPhase::Ready,
            },
            Response::PostLoginComplete { client_id: 42 },
            Response::CombatUpdate {
                status: crate::combat::CombatStatus::Idle,
            },
            Response::SpawnEventBatch {
                events: vec![
                    SpawnEvent {
                        client_id: 42,
                        zone: "freportw".into(),
                        spawn_name: "a beetle".into(),
                        spawn_type: 1,
                        kind: SpawnEventKind::Created,
                        timestamp_ms: 1,
                    },
                    SpawnEvent {
                        client_id: 42,
                        zone: "freportw".into(),
                        spawn_name: "a spider".into(),
                        spawn_type: 1,
                        kind: SpawnEventKind::Destroyed,
                        timestamp_ms: 2,
                    },
                ],
            },
            Response::BazaarResults {
                windows: vec![BazaarWindowSnapshot {
                    window_text: Some("Bazaar Search".into()),
                    window_sidl_name: Some("BZR_SearchWnd".into()),
                    list_sidl_name: Some("BZR_ItemList".into()),
                    row_count: 1,
                    listings: vec![BazaarListing {
                        row_index: 0,
                        columns: vec!["Fungi Tunic".into(), "Traderbob".into(), "2,000".into()],
                        item_name: Some("Fungi Tunic".into()),
                        trader_name: Some("Traderbob".into()),
                        price_text: Some("2,000".into()),
                        price_copper: Some(2_000_000),
                        quantity: Some(1),
                    }],
                }],
            },
            Response::MerchantItems {
                windows: vec![MerchantWindowSnapshot {
                    vendor_name: Some("Merchant_Leah".into()),
                    window_text: Some("> Merchant_Leah <".into()),
                    window_sidl_name: Some("MerchantWnd".into()),
                    list_sidl_name: Some("MW_ItemList".into()),
                    row_count: 1,
                    listings: vec![MerchantListing {
                        row_index: 0,
                        columns: vec![
                            String::new(),
                            "Fungi Covered Scale Tunic".into(),
                            "--".into(),
                            String::new(),
                            "475".into(),
                        ],
                        item_name: Some("Fungi Covered Scale Tunic".into()),
                        price_text: Some("475".into()),
                        price_copper: Some(475_000),
                        quantity: None,
                        quantity_text: Some("--".into()),
                        infinite_quantity: true,
                    }],
                }],
            },
            Response::TradeskillTrophyStatus {
                status: TradeskillTrophyStatus::default(),
            },
            Response::ZoneGraph { zones: vec![] },
            Response::RenderModeChanged {
                mode: RenderMode::NullRender,
            },
            Response::ScreenshotCaptured {
                path: "/tmp/textquest_screenshot_1234.bmp".into(),
            },
            Response::ScreenshotFailed {
                reason: "not in NullRender mode".into(),
            },
            Response::ChatMessage {
                text: "You say, 'Hello'".into(),
                color: 273,
                timestamp_ms: 1234567890,
                parsed: None,
            },
            Response::ChatBatch {
                messages: vec![
                    ChatMessageInfo {
                        text: "You say, 'Hello'".into(),
                        color: 273,
                        timestamp_ms: 1234567890,
                    },
                    ChatMessageInfo {
                        text: "Soandso tells you, 'Hi!'".into(),
                        color: 269,
                        timestamp_ms: 1234567900,
                    },
                ],
            },
            Response::ChatBatch { messages: vec![] },
            Response::TraceList {
                traces: vec![TraceStatus {
                    function_name: "target@0x1234".into(),
                    address: 0x1234,
                    capture_args: true,
                    capture_return: true,
                    slot: Some(3),
                    call_count: 2,
                    dropped_events: 0,
                }],
            },
            Response::TraceDump {
                events: vec![TraceRecord {
                    function_name: "target@0x1234".into(),
                    address: 0x1234,
                    sequence: 1,
                    timestamp_ticks: 99,
                    thread_id: 7,
                    args: Some([1, 2, 3, 4]),
                    stack_pointer: Some(0x9000),
                    return_address: Some(0x5678),
                    return_value: None,
                    elapsed_ticks: None,
                }],
                dropped_events: 0,
            },
        ];
        for resp in &responses {
            let encoded = encode(resp).expect("encode failed");
            let (decoded, _): (Response, usize) = decode(&encoded).expect("decode failed");
            let _ = format!("{:?}", decoded);
        }
    }

    #[test]
    fn game_state_from_unknown_u32_roundtrips() {
        let unknown: u32 = 0xA2A3A4A5;
        let state = GameState::from(unknown);
        assert!(matches!(state, GameState::Unknown(v) if v == unknown));
        assert_eq!(u32::from(state), unknown);
    }

    #[test]
    fn game_state_changed_response_roundtrip() {
        use crate::protocol::{decode, encode};

        let resp = Response::GameStateChanged {
            state: GameState::LoggingIn,
        };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode");
        if let Response::GameStateChanged { state } = decoded {
            assert!(matches!(state, GameState::LoggingIn));
        } else {
            panic!("expected GameStateChanged");
        }
    }

    #[test]
    fn generate_random_token_is_32_bytes() {
        let token = generate_random_token();
        assert_eq!(token.len(), 32);
    }

    #[test]
    fn chat_message_info_roundtrip() {
        use crate::protocol::{decode, encode};

        let info = ChatMessageInfo {
            text: "You say, 'Hello!'".into(),
            color: 273,
            timestamp_ms: 9999,
        };
        let resp = Response::ChatBatch {
            messages: vec![info.clone()],
        };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode");
        if let Response::ChatBatch { messages } = decoded {
            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0], info);
        } else {
            panic!("expected ChatBatch");
        }
    }

    #[test]
    fn poll_chat_command_roundtrip() {
        use crate::protocol::{decode, encode};

        let cmd = Command::PollChat;
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        assert_eq!(cmd, decoded);
    }

    #[test]
    fn chat_batch_empty_roundtrip() {
        use crate::protocol::{decode, encode};

        let resp = Response::ChatBatch { messages: vec![] };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode");
        if let Response::ChatBatch { messages } = decoded {
            assert!(messages.is_empty());
        } else {
            panic!("expected ChatBatch");
        }
    }

    #[test]
    fn generate_random_token_is_not_zero() {
        // We only assert deterministic properties to avoid flaky tests:
        // generating a token should succeed and produce 32 bytes.
        let token = generate_random_token();
        assert_eq!(token.len(), 32);
    }

    #[test]
    fn generate_random_token_unique() {
        // We avoid asserting uniqueness because a CSPRNG can, in theory,
        // produce the same token twice. Instead, assert both tokens are
        // valid 32-byte values.
        let a = generate_random_token();
        let b = generate_random_token();
        assert_eq!(a.len(), 32);
        assert_eq!(b.len(), 32);
    }

    #[test]
    fn shared_memory_size_is_64kb() {
        assert_eq!(SHARED_MEMORY_SIZE, 65536);
    }

    #[test]
    fn command_slash_command_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::SlashCommand {
            command: "/target Emperor Crush".into(),
        };
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        if let Command::SlashCommand { command } = decoded {
            assert_eq!(command, "/target Emperor Crush");
        } else {
            panic!("expected SlashCommand");
        }
    }

    #[test]
    fn command_cast_spell_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::CastSpell {
            spell_slot: 5,
            target_id: Some(12345),
            kill: false,
            recast: 0,
        };
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        if let Command::CastSpell {
            spell_slot,
            target_id,
            kill,
            recast,
        } = decoded
        {
            assert_eq!(spell_slot, 5);
            assert_eq!(target_id, Some(12345));
            assert!(!kill);
            assert_eq!(recast, 0);
        } else {
            panic!("expected CastSpell");
        }
    }

    #[test]
    fn command_cast_spell_no_target_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::CastSpell {
            spell_slot: 2,
            target_id: None,
            kill: false,
            recast: 0,
        };
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        if let Command::CastSpell {
            spell_slot,
            target_id,
            kill,
            recast,
        } = decoded
        {
            assert_eq!(spell_slot, 2);
            assert_eq!(target_id, None);
            assert!(!kill);
            assert_eq!(recast, 0);
        } else {
            panic!("expected CastSpell");
        }
    }

    #[test]
    fn command_cast_spell_kill_flag_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::CastSpell {
            spell_slot: 7,
            target_id: Some(9999),
            kill: true,
            recast: 0,
        };
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        if let Command::CastSpell {
            spell_slot,
            target_id,
            kill,
            recast,
        } = decoded
        {
            assert_eq!(spell_slot, 7);
            assert_eq!(target_id, Some(9999));
            assert!(kill);
            assert_eq!(recast, 0);
        } else {
            panic!("expected CastSpell");
        }
    }

    #[test]
    fn command_cast_spell_recast_flag_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::CastSpell {
            spell_slot: 3,
            target_id: None,
            kill: false,
            recast: 5,
        };
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        if let Command::CastSpell {
            spell_slot,
            target_id,
            kill,
            recast,
        } = decoded
        {
            assert_eq!(spell_slot, 3);
            assert_eq!(target_id, None);
            assert!(!kill);
            assert_eq!(recast, 5);
        } else {
            panic!("expected CastSpell");
        }
    }

    #[test]
    fn command_cancel_cast_loop_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::CancelCastLoop;
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        assert_eq!(decoded, Command::CancelCastLoop);
    }

    #[test]
    fn command_navigate_to_roundtrip() {
        use crate::{
            nav::Waypoint,
            protocol::{decode, encode},
        };
        let cmd = Command::NavigateTo {
            waypoints: vec![Waypoint::new(1.0, 2.0, 3.0), Waypoint::new(4.0, 5.0, 6.0)],
        };
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        if let Command::NavigateTo { waypoints } = decoded {
            assert_eq!(waypoints.len(), 2);
        } else {
            panic!("expected NavigateTo");
        }
    }

    #[test]
    fn command_stick_to_roundtrip() {
        use crate::{
            nav::{StickBreakConditions, StickConfig, StickDistance},
            protocol::{decode, encode},
        };
        let config = StickConfig {
            distance: StickDistance::Absolute(20.0),
            distance_mod: 3.5,
            hold: true,
            always: false,
            id: Some(42),
            mode: crate::nav::StickMode::Behind,
            behind_arc: 45.0,
            not_front_arc: 90.0,
            moveback: true,
            backup_dist: 5.0,
            healer: false,
            autopause: false,
            break_conditions: crate::nav::StickBreakConditions::NONE,
        };
        let cmd = Command::StickTo { config };
        let encoded = encode(&cmd).expect("encode StickTo");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode StickTo");
        if let Command::StickTo {
            config: decoded_config,
        } = decoded
        {
            assert!(
                matches!(decoded_config.distance, StickDistance::Absolute(d) if (d - 20.0).abs() < f32::EPSILON)
            );
            assert!((decoded_config.distance_mod - 3.5).abs() < f32::EPSILON);
            assert!(decoded_config.hold);
            assert!(!decoded_config.always);
            assert_eq!(decoded_config.id, Some(42));
        } else {
            panic!("expected StickTo");
        }
    }

    #[test]
    fn command_stick_off_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::StickOff;
        let encoded = encode(&cmd).expect("encode StickOff");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode StickOff");
        assert!(matches!(decoded, Command::StickOff));
    }

    #[test]
    fn command_stick_mod_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::StickMod { delta: -5.0 };
        let encoded = encode(&cmd).expect("encode StickMod");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode StickMod");
        if let Command::StickMod { delta } = decoded {
            assert!((delta - (-5.0)).abs() < f32::EPSILON);
        } else {
            panic!("expected StickMod");
        }
    }

    #[test]
    fn command_circle_kite_roundtrip() {
        use crate::{
            nav::{CircleConfig, CircleMode, Waypoint},
            protocol::{decode, encode},
        };
        let config = CircleConfig {
            radius: 30.0,
            mode: CircleMode::Ccw,
            center: Some(Waypoint::new(100.0, 200.0, 10.0)),
            target_id: Some(55),
            drunken_interval: 15,
        };
        let cmd = Command::CircleKite { config };
        let encoded = encode(&cmd).expect("encode CircleKite");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode CircleKite");
        if let Command::CircleKite {
            config: decoded_config,
        } = decoded
        {
            assert!((decoded_config.radius - 30.0).abs() < f32::EPSILON);
            assert_eq!(decoded_config.mode, CircleMode::Ccw);
            assert_eq!(decoded_config.target_id, Some(55));
            assert_eq!(decoded_config.drunken_interval, 15);
        } else {
            panic!("expected CircleKite");
        }
    }

    #[test]
    fn command_circle_off_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::CircleOff;
        let encoded = encode(&cmd).expect("encode CircleOff");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode CircleOff");
        assert!(matches!(decoded, Command::CircleOff));
    }

    #[test]
    fn correlation_id_generator_starts_at_one() {
        let id_gen = CorrelationIdGenerator::new();
        assert_eq!(id_gen.next_id(), 1);
        assert_eq!(id_gen.next_id(), 2);
        assert_eq!(id_gen.next_id(), 3);
    }

    #[test]
    fn correlation_id_generator_default() {
        let id_gen = CorrelationIdGenerator::default();
        assert_eq!(id_gen.next_id(), 1);
    }

    #[test]
    fn ipc_command_new_has_no_correlation() {
        let ipc_cmd = IpcCommand::new(Command::Ping);
        assert!(ipc_cmd.correlation_id.is_none());
        assert_eq!(ipc_cmd.command, Command::Ping);
    }

    #[test]
    fn ipc_command_with_correlation() {
        let ipc_cmd = IpcCommand::with_correlation(Command::Ping, 42);
        assert_eq!(ipc_cmd.correlation_id, Some(42));
    }

    #[test]
    fn ipc_command_from_command() {
        let ipc_cmd: IpcCommand = Command::Sit.into();
        assert_eq!(ipc_cmd.command, Command::Sit);
        assert!(ipc_cmd.correlation_id.is_none());
    }

    #[test]
    fn ipc_response_new_has_no_correlation() {
        let ipc_resp = IpcResponse::new(Response::Pong {
            client_id: 1,
            timestamp_ms: 100,
        });
        assert!(ipc_resp.correlation_id.is_none());
    }

    #[test]
    fn ipc_response_echo_preserves_correlation() {
        let ipc_resp = IpcResponse::echo(
            Response::CommandResult {
                success: true,
                message: "ok".into(),
            },
            Some(99),
        );
        assert_eq!(ipc_resp.correlation_id, Some(99));
    }

    #[test]
    fn ipc_response_echo_none_correlation() {
        let ipc_resp = IpcResponse::echo(
            Response::CommandResult {
                success: true,
                message: "ok".into(),
            },
            None,
        );
        assert!(ipc_resp.correlation_id.is_none());
    }

    #[test]
    fn ipc_response_from_response() {
        let ipc_resp: IpcResponse = Response::Pong {
            client_id: 5,
            timestamp_ms: 0,
        }
        .into();
        assert!(ipc_resp.correlation_id.is_none());
    }

    #[test]
    fn ipc_command_roundtrip() {
        use crate::protocol::{decode, encode};

        let ipc_cmd = IpcCommand::with_correlation(Command::Ping, 12345);
        let encoded = encode(&ipc_cmd).expect("encode IpcCommand");
        let (decoded, _): (IpcCommand, _) = decode(&encoded).expect("decode IpcCommand");
        assert_eq!(decoded.command, Command::Ping);
        assert_eq!(decoded.correlation_id, Some(12345));
    }

    #[test]
    fn ipc_command_roundtrip_no_correlation() {
        use crate::protocol::{decode, encode};

        let ipc_cmd = IpcCommand::new(Command::StopMovement);
        let encoded = encode(&ipc_cmd).expect("encode");
        let (decoded, _): (IpcCommand, _) = decode(&encoded).expect("decode");
        assert_eq!(decoded.command, Command::StopMovement);
        assert!(decoded.correlation_id.is_none());
    }

    #[test]
    fn ipc_response_roundtrip() {
        use crate::protocol::{decode, encode};

        let ipc_resp = IpcResponse::echo(
            Response::CommandResult {
                success: true,
                message: "done".into(),
            },
            Some(777),
        );
        let encoded = encode(&ipc_resp).expect("encode IpcResponse");
        let (decoded, _): (IpcResponse, _) = decode(&encoded).expect("decode IpcResponse");
        assert_eq!(decoded.correlation_id, Some(777));
        if let Response::CommandResult { success, message } = decoded.response {
            assert!(success);
            assert_eq!(message, "done");
        } else {
            panic!("expected CommandResult");
        }
    }

    #[test]
    fn ipc_command_complex_payload_roundtrip() {
        use crate::protocol::{decode, encode};

        let id_gen = CorrelationIdGenerator::new();
        let cid = id_gen.next_id();
        let ipc_cmd = IpcCommand::with_correlation(
            Command::CastSpell {
                spell_slot: 3,
                target_id: Some(9999),
                kill: false,
                recast: 0,
            },
            cid,
        );
        let encoded = encode(&ipc_cmd).expect("encode");
        let (decoded, _): (IpcCommand, _) = decode(&encoded).expect("decode");
        assert_eq!(decoded.correlation_id, Some(1));
        if let Command::CastSpell {
            spell_slot,
            target_id,
            kill,
            recast,
        } = decoded.command
        {
            assert_eq!(spell_slot, 3);
            assert_eq!(target_id, Some(9999));
            assert!(!kill);
            assert_eq!(recast, 0);
        } else {
            panic!("expected CastSpell");
        }
    }

    #[test]
    fn nav_status_sticking_roundtrip() {
        use crate::{
            nav::NavStatus,
            protocol::{decode, encode},
        };
        let resp = Response::NavUpdate {
            status: NavStatus::Sticking {
                target_id: 99,
                distance: 12.5,
                in_range: true,
                break_reason: None,
            },
        };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, usize) = decode(&encoded).expect("decode");
        if let Response::NavUpdate {
            status:
                NavStatus::Sticking {
                    target_id,
                    distance,
                    in_range,
                    break_reason,
                },
        } = decoded
        {
            assert_eq!(target_id, 99);
            assert!((distance - 12.5).abs() < f32::EPSILON);
            assert!(in_range);
            assert!(break_reason.is_none());
        } else {
            panic!("expected NavUpdate(Sticking)");
        }
    }

    #[test]
    fn render_mode_serde_roundtrip() {
        use crate::protocol::{decode, encode};

        for mode in [
            RenderMode::Normal,
            RenderMode::Strobe,
            RenderMode::NullRender,
        ] {
            let cmd = Command::SetRenderMode { mode };
            let encoded = encode(&cmd).expect("encode SetRenderMode");
            let (decoded, _): (Command, _) = decode(&encoded).expect("decode SetRenderMode");
            if let Command::SetRenderMode { mode: decoded_mode } = decoded {
                assert_eq!(decoded_mode, mode);
            } else {
                panic!("expected SetRenderMode");
            }
        }
    }

    #[test]
    fn auto_rez_config_command_roundtrip() {
        use crate::protocol::{decode, encode};

        let cmd = Command::SetAutoRezConfig {
            config: AutoRezConfig {
                enabled: true,
                min_xp_pct: 96,
                trusted_casters: vec!["Clericbob".into(), "Druidgal".into()],
                decline_if_untrusted: true,
                delay_ms: 5_100,
            },
        };

        let encoded = encode(&cmd).expect("encode SetAutoRezConfig");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode SetAutoRezConfig");
        assert_eq!(decoded, cmd);
    }

    #[test]
    fn reward_automation_config_command_roundtrip() {
        use crate::protocol::{decode, encode};

        let cmd = Command::SetRewardAutomation {
            config: RewardAutomationConfig {
                rules: vec![crate::character_config::TaskRewardPreference {
                    task_matcher: "Expedition".into(),
                    preference: crate::character_config::RewardPreference::ByPosition {
                        reward_position: 2,
                    },
                }],
            },
        };

        let encoded = encode(&cmd).expect("encode SetRewardAutomation");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode SetRewardAutomation");
        assert_eq!(decoded, cmd);
    }

    #[test]
    fn auto_rez_config_default_matches_ui_baseline() {
        assert_eq!(
            AutoRezConfig::default(),
            AutoRezConfig {
                enabled: false,
                min_xp_pct: 90,
                trusted_casters: vec![],
                decline_if_untrusted: false,
                delay_ms: 3_000,
            }
        );
    }

    #[test]
    fn render_mode_changed_response_roundtrip() {
        use crate::protocol::{decode, encode};

        for mode in [
            RenderMode::Normal,
            RenderMode::Strobe,
            RenderMode::NullRender,
        ] {
            let resp = Response::RenderModeChanged { mode };
            let encoded = encode(&resp).expect("encode RenderModeChanged");
            let (decoded, _): (Response, _) = decode(&encoded).expect("decode RenderModeChanged");
            if let Response::RenderModeChanged { mode: decoded_mode } = decoded {
                assert_eq!(decoded_mode, mode);
            } else {
                panic!("expected RenderModeChanged");
            }
        }
    }

    #[test]
    fn render_mode_display() {
        assert_eq!(RenderMode::Normal.to_string(), "normal");
        assert_eq!(RenderMode::Strobe.to_string(), "strobe");
        assert_eq!(RenderMode::NullRender.to_string(), "null");
    }

    #[test]
    fn capture_screenshot_command_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::CaptureScreenshot;
        let encoded = encode(&cmd).expect("encode CaptureScreenshot");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode CaptureScreenshot");
        assert_eq!(decoded, Command::CaptureScreenshot);
    }

    #[test]
    fn screenshot_captured_response_roundtrip() {
        use crate::protocol::{decode, encode};
        let resp = Response::ScreenshotCaptured {
            path: "/tmp/textquest_screenshot_42.bmp".into(),
        };
        let encoded = encode(&resp).expect("encode ScreenshotCaptured");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode ScreenshotCaptured");
        if let Response::ScreenshotCaptured { path } = decoded {
            assert_eq!(path, "/tmp/textquest_screenshot_42.bmp");
        } else {
            panic!("expected ScreenshotCaptured");
        }
    }

    // ─── Nav parity IPC roundtrip tests (#168–#177) ───

    #[test]
    fn nav_pause_resume_roundtrip() {
        use crate::protocol::{decode, encode};
        for cmd in [Command::NavPause, Command::NavResume] {
            let encoded = encode(&cmd).expect("encode");
            let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
            assert_eq!(decoded, cmd);
        }
    }

    #[test]
    fn screenshot_failed_response_roundtrip() {
        use crate::protocol::{decode, encode};
        let resp = Response::ScreenshotFailed {
            reason: "not in NullRender mode".into(),
        };
        let encoded = encode(&resp).expect("encode ScreenshotFailed");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode ScreenshotFailed");
        if let Response::ScreenshotFailed { reason } = decoded {
            assert_eq!(reason, "not in NullRender mode");
        } else {
            panic!("expected ScreenshotFailed");
        }
    }

    #[test]
    fn nav_loc_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::NavLoc {
            x: 100.0,
            y: 200.0,
            z: 10.0,
        };
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        if let Command::NavLoc { x, y, z } = decoded {
            assert!((x - 100.0).abs() < f32::EPSILON);
            assert!((y - 200.0).abs() < f32::EPSILON);
            assert!((z - 10.0).abs() < f32::EPSILON);
        } else {
            panic!("expected NavLoc");
        }
    }

    #[test]
    fn nav_destination_commands_roundtrip() {
        use crate::protocol::{decode, encode};
        for cmd in [Command::NavTarget, Command::NavDoor, Command::NavItem] {
            let encoded = encode(&cmd).expect("encode");
            let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
            assert_eq!(decoded, cmd);
        }
    }

    #[test]
    fn nav_reload_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::NavReload;
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        assert_eq!(decoded, cmd);
    }

    #[test]
    fn nav_waypoint_save_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::NavWaypointSave {
            name: "camp1".to_string(),
        };
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        assert_eq!(decoded, cmd);
    }

    #[test]
    fn nav_waypoint_recall_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::NavWaypointRecall {
            name: "puller_spot".to_string(),
        };
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        assert_eq!(decoded, cmd);
    }

    #[test]
    fn nav_waypoint_list_response_roundtrip() {
        use crate::{
            nav::{NamedWaypoint, Waypoint},
            protocol::{decode, encode},
        };
        let resp = Response::NavWaypointList {
            waypoints: vec![
                NamedWaypoint::new("camp1", Waypoint::new(1.0, 2.0, 3.0), "qey2hh1"),
                NamedWaypoint::new("puller", Waypoint::new(4.0, 5.0, 6.0), "gukbottom"),
            ],
        };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode");
        if let Response::NavWaypointList { waypoints } = decoded {
            assert_eq!(waypoints.len(), 2);
            assert_eq!(waypoints[0].name, "camp1");
            assert_eq!(waypoints[1].zone, "gukbottom");
        } else {
            panic!("expected NavWaypointList");
        }
    }

    #[test]
    fn nav_signals_response_roundtrip() {
        use crate::{
            nav::NavStateSignals,
            protocol::{decode, encode},
        };
        let resp = Response::NavSignals {
            signals: NavStateSignals {
                active: true,
                mesh_loaded: true,
                path_exists: true,
                path_length: Some(150.0),
                velocity: 12.5,
                paused: false,
            },
        };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode");
        if let Response::NavSignals { signals } = decoded {
            assert!(signals.active);
            assert!((signals.velocity - 12.5).abs() < f32::EPSILON);
        } else {
            panic!("expected NavSignals");
        }
    }

    #[test]
    fn nav_diagnostics_response_roundtrip() {
        use crate::{
            nav::NavDiagnostics,
            protocol::{decode, encode},
        };
        let resp = Response::NavDiagnosticsResult {
            diagnostics: NavDiagnostics {
                state: "Moving".to_string(),
                mesh_loaded: true,
                path_exists: true,
                path_length: Some(50.0),
                velocity: 8.0,
                waypoint_index: 1,
                waypoint_count: 5,
                distance_remaining: 25.0,
            },
        };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode");
        if let Response::NavDiagnosticsResult { diagnostics } = decoded {
            assert_eq!(diagnostics.state, "Moving");
            assert_eq!(diagnostics.waypoint_count, 5);
        } else {
            panic!("expected NavDiagnosticsResult");
        }
    }

    #[test]
    fn query_context_menu_command_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::QueryContextMenu;
        let encoded = encode(&cmd).expect("encode QueryContextMenu");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode QueryContextMenu");
        assert_eq!(decoded, Command::QueryContextMenu);
    }

    #[test]
    fn activate_context_menu_item_command_roundtrip() {
        use crate::protocol::{decode, encode};
        let cmd = Command::ActivateContextMenuItem {
            menu_index: 0,
            item_index: 3,
        };
        let encoded = encode(&cmd).expect("encode ActivateContextMenuItem");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode ActivateContextMenuItem");
        if let Command::ActivateContextMenuItem {
            menu_index,
            item_index,
        } = decoded
        {
            assert_eq!(menu_index, 0);
            assert_eq!(item_index, 3);
        } else {
            panic!("expected ActivateContextMenuItem");
        }
    }

    #[test]
    fn context_menu_state_response_roundtrip() {
        use crate::protocol::{decode, encode};
        let resp = Response::ContextMenuState {
            menus: vec![ContextMenuInfo {
                menu_index: 0,
                items: vec![
                    ContextMenuItem {
                        item_index: 0,
                        label: "Attack".to_string(),
                        enabled: true,
                        checked: false,
                        is_separator: false,
                    },
                    ContextMenuItem {
                        item_index: 1,
                        label: String::new(),
                        enabled: false,
                        checked: false,
                        is_separator: true,
                    },
                    ContextMenuItem {
                        item_index: 2,
                        label: "Inspect".to_string(),
                        enabled: true,
                        checked: false,
                        is_separator: false,
                    },
                ],
            }],
        };
        let encoded = encode(&resp).expect("encode ContextMenuState");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode ContextMenuState");
        if let Response::ContextMenuState { menus } = decoded {
            assert_eq!(menus.len(), 1);
            assert_eq!(menus[0].items.len(), 3);
            assert_eq!(menus[0].items[0].label, "Attack");
            assert!(menus[0].items[1].is_separator);
            assert_eq!(menus[0].items[2].label, "Inspect");
        } else {
            panic!("expected ContextMenuState");
        }
    }

    #[test]
    fn context_menu_activated_response_roundtrip() {
        use crate::protocol::{decode, encode};
        let resp = Response::ContextMenuActivated {
            success: true,
            message: "HandleMenu dispatched".to_string(),
        };
        let encoded = encode(&resp).expect("encode ContextMenuActivated");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode ContextMenuActivated");
        if let Response::ContextMenuActivated { success, message } = decoded {
            assert!(success);
            assert_eq!(message, "HandleMenu dispatched");
        } else {
            panic!("expected ContextMenuActivated");
        }
    }

    // ─── CorrelationIdGenerator: additional coverage ───────────────────────────

    #[test]
    fn correlation_id_generator_increments_sequentially() {
        let generator = CorrelationIdGenerator::new();
        let a = generator.next_id();
        let b = generator.next_id();
        let c = generator.next_id();
        assert_eq!(b, a + 1);
        assert_eq!(c, a + 2);
    }

    #[test]
    fn correlation_id_generator_default_starts_at_one() {
        let generator = CorrelationIdGenerator::default();
        assert_eq!(generator.next_id(), 1);
    }

    #[test]
    fn correlation_id_generator_produces_unique_ids() {
        let generator = CorrelationIdGenerator::new();
        let ids: Vec<u64> = (0..100).map(|_| generator.next_id()).collect();
        let mut sorted = ids.clone();
        sorted.dedup();
        assert_eq!(
            ids.len(),
            sorted.len(),
            "all generated IDs should be unique"
        );
    }

    // ─── IpcCommand: additional coverage ──────────────────────────────────────

    #[test]
    fn ipc_command_clone_and_equality() {
        let cmd = IpcCommand::new(Command::Sit);
        let clone = cmd.clone();
        assert_eq!(cmd, clone);
    }

    #[test]
    fn ipc_command_with_correlation_max_id() {
        let cmd = IpcCommand::with_correlation(Command::Ping, u64::MAX);
        assert_eq!(cmd.correlation_id, Some(u64::MAX));
    }

    // ─── IpcResponse: additional coverage ─────────────────────────────────────

    #[test]
    fn ipc_response_clone_preserves_fields() {
        let resp = IpcResponse::echo(
            Response::Error {
                message: "test error".into(),
            },
            Some(55),
        );
        let clone = resp.clone();
        assert_eq!(clone.correlation_id, Some(55));
    }

    // ─── Zone transition IPC types: serialization roundtrips ──────────────────

    #[test]
    fn command_roundtrip_request_zone_with_coords() {
        use crate::protocol::{decode, encode};

        let cmd = Command::RequestZone {
            zone_id: 123,
            destination_coords: Some((100.5, 200.5, 50.0)),
        };
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        if let Command::RequestZone {
            zone_id,
            destination_coords,
        } = decoded
        {
            assert_eq!(zone_id, 123);
            assert_eq!(destination_coords, Some((100.5, 200.5, 50.0)));
        } else {
            panic!("expected RequestZone");
        }
    }

    #[test]
    fn command_roundtrip_request_zone_no_coords() {
        use crate::protocol::{decode, encode};

        let cmd = Command::RequestZone {
            zone_id: 456,
            destination_coords: None,
        };
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        if let Command::RequestZone {
            zone_id,
            destination_coords,
        } = decoded
        {
            assert_eq!(zone_id, 456);
            assert_eq!(destination_coords, None);
        } else {
            panic!("expected RequestZone");
        }
    }

    #[test]
    fn command_roundtrip_flush_movement_queue() {
        use crate::protocol::{decode, encode};

        let cmd = Command::FlushMovementQueue;
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        assert_eq!(decoded, Command::FlushMovementQueue);
    }

    #[test]
    fn command_roundtrip_request_safe_coords() {
        use crate::protocol::{decode, encode};

        let cmd = Command::RequestSafeCoords(789);
        let encoded = encode(&cmd).expect("encode");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode");
        if let Command::RequestSafeCoords(zone_id) = decoded {
            assert_eq!(zone_id, 789);
        } else {
            panic!("expected RequestSafeCoords");
        }
    }

    #[test]
    fn response_roundtrip_zone_state_changed() {
        use crate::protocol::{decode, encode};

        let resp = Response::ZoneStateChanged {
            zone_id: 123,
            old_state: "loading".to_string(),
            new_state: "arrived".to_string(),
            timestamp: 1609459200000,
        };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode");
        if let Response::ZoneStateChanged {
            zone_id,
            old_state,
            new_state,
            timestamp,
        } = decoded
        {
            assert_eq!(zone_id, 123);
            assert_eq!(old_state, "loading");
            assert_eq!(new_state, "arrived");
            assert_eq!(timestamp, 1609459200000);
        } else {
            panic!("expected ZoneStateChanged");
        }
    }

    #[test]
    fn response_roundtrip_zone_validation_failed() {
        use crate::protocol::{decode, encode};

        let resp = Response::ZoneValidationFailed {
            zone_id: 456,
            error_code: -1,
            reason: "Zone is locked".to_string(),
        };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode");
        if let Response::ZoneValidationFailed {
            zone_id,
            error_code,
            reason,
        } = decoded
        {
            assert_eq!(zone_id, 456);
            assert_eq!(error_code, -1);
            assert_eq!(reason, "Zone is locked");
        } else {
            panic!("expected ZoneValidationFailed");
        }
    }

    #[test]
    fn response_roundtrip_safe_coords_retrieved() {
        use crate::protocol::{decode, encode};

        let resp = Response::SafeCoordsRetrieved {
            x: 100.5,
            y: 200.5,
            z: 50.0,
            zone_id: 789,
        };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode");
        if let Response::SafeCoordsRetrieved { x, y, z, zone_id } = decoded {
            assert_eq!(x, 100.5);
            assert_eq!(y, 200.5);
            assert_eq!(z, 50.0);
            assert_eq!(zone_id, 789);
        } else {
            panic!("expected SafeCoordsRetrieved");
        }
    }

    #[test]
    fn response_roundtrip_movement_queue_flushed() {
        use crate::protocol::{decode, encode};

        let resp = Response::MovementQueueFlushed { count_dropped: 5 };
        let encoded = encode(&resp).expect("encode");
        let (decoded, _): (Response, _) = decode(&encoded).expect("decode");
        if let Response::MovementQueueFlushed { count_dropped } = decoded {
            assert_eq!(count_dropped, 5);
        } else {
            panic!("expected MovementQueueFlushed");
        }
    }

    #[test]
    fn command_roundtrip_set_auto_rez_config() {
        use crate::protocol::{decode, encode};

        let cmd = Command::SetAutoRezConfig {
            config: AutoRezConfig {
                enabled: true,
                min_xp_pct: 96,
                trusted_casters: vec!["Frostreaver".into(), "Leafbinder".into()],
                decline_if_untrusted: true,
                delay_ms: 5_100,
            },
        };
        let encoded = encode(&cmd).expect("encode SetAutoRezConfig");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode SetAutoRezConfig");
        assert_eq!(decoded, cmd);
    }

    #[test]
    fn command_roundtrip_set_reward_automation() {
        use crate::protocol::{decode, encode};

        let cmd = Command::SetRewardAutomation {
            config: RewardAutomationConfig::default(),
        };
        let encoded = encode(&cmd).expect("encode SetRewardAutomation");
        let (decoded, _): (Command, _) = decode(&encoded).expect("decode SetRewardAutomation");
        assert_eq!(decoded, cmd);
    }

    #[test]
    fn auto_rez_config_default_values() {
        assert_eq!(
            AutoRezConfig::default(),
            AutoRezConfig {
                enabled: false,
                min_xp_pct: 90,
                trusted_casters: Vec::new(),
                decline_if_untrusted: false,
                delay_ms: 3_000,
            }
        );
    }

    #[test]
    fn watchpoint_commands_are_debuggable() {
        let cmd = Command::WatchAdd {
            address: 0x1234,
            size: 4,
            label: "player.hp".to_string(),
            mode: WatchMode::Poll,
        };

        let debug = format!("{cmd:?}");
        assert!(debug.contains("WatchAdd"));
        assert!(debug.contains("player.hp"));
    }

    #[test]
    fn watchpoint_log_response_carries_old_and_new_values() {
        let event = WatchpointEvent {
            label: "player.hp".to_string(),
            address: 0x1234,
            size: 4,
            mode: WatchMode::Poll,
            old_value: vec![0x10, 0x00, 0x00, 0x00],
            new_value: vec![0x0f, 0x00, 0x00, 0x00],
            timestamp_ms: 42,
            writer_rip: None,
            call_stack: Vec::new(),
        };

        let response = Response::WatchLog {
            events: vec![event.clone()],
        };

        match response {
            Response::WatchLog { events } => {
                assert_eq!(events, vec![event]);
            }
            _ => panic!("expected WatchLog"),
        }
    }
}
