export interface Session {
  client_id: number;
  character_name: string;
  zone: string;
  level: number;
  hp_pct: number;
  mana_pct: number;
  endurance_pct?: number;
  status: "active" | "idle" | "dead" | "camping" | "zoning";
  buff_count: number;
  target_name?: string | null;
  target_hp_pct?: number | null;
  pet_name?: string | null;
}

export interface AdminSessionRecord {
  sessionId: string;
  characterName: string;
  profile: string | null;
  groupId: string | null;
  routingScope: string | null;
  lifecycle: string | null;
  status: string | null;
  zone: string | null;
  level: number | null;
  className: string | null;
  lastHeartbeat: string | null;
}

export interface Assault {
  id: string;
  zone: string;
  target_name: string;
  target_hp_pct: number;
  engagement_time: string;
  forces_active: number;
  forces_total: number;
  avg_mana_pct: number;
  casualties: number;
  variant: "magenta" | "cyan";
}

export interface DpsEntry {
  name: string;
  dps: number;
  color: string;
}

export interface CombatLogEntry {
  timestamp: string;
  message: string;
  highlights: CombatHighlight[];
}

export interface CombatHighlight {
  text: string;
  color: string;
  bold?: boolean;
}

export interface Player {
  name: string;
  hp_pct: number;
  zone: string;
  status: "online" | "dead" | "afk";
}

export interface Alert {
  type: "warning" | "info";
  message: string;
  detail: string;
  highlight?: string;
}

export type OperationalAlertSeverity = "critical" | "warning" | "info";

export interface OperationalAlert {
  id: number;
  created_at: string;
  severity: OperationalAlertSeverity;
  kind: string;
  message: string;
  source: string | null;
  actor: string | null;
  zone: string | null;
  metadata_json: string | null;
  acknowledged_at: string | null;
  acknowledged_by: string | null;
}

export interface AlertThresholdConfig {
  death_alert: boolean;
  stuck_alert: boolean;
  memory_warning_mb: number;
  ipc_latency_warning_ms: number;
  error_rate_warning_per_min: number;
  dps_drop_warning_pct: number;
  zone_timeout_secs: number;
}

export interface AlertingConfig {
  enable_discord: boolean;
  discord_webhook_url: string;
  enable_email: boolean;
  smtp_server: string;
  smtp_port: number;
  smtp_username: string;
  smtp_password: string;
  email_from: string;
  email_recipients: string[];
  email_subject_prefix: string;
  warning_batch_window_secs: number;
  thresholds: AlertThresholdConfig;
}

export type NavItem = {
  id: string;
  label: string;
  icon: string;
  active?: boolean;
  pulse?: boolean;
};

// ── Account registry types ───────────────────────────────────────────────────

export type AccountStatus = "active" | "locked" | "banned";

export interface Account {
  id: string;
  name: string;
  server: string;
  character: string;
  class: string;
  group: number;
  status: AccountStatus;
  has_password: boolean;
}

export interface CreateAccountPayload {
  name: string;
  server: string;
  character: string;
  class: string;
  group: number;
  status: AccountStatus;
  password?: string;
}

export interface UpdateAccountPayload {
  server?: string;
  character?: string;
  class?: string;
  group?: number;
  status?: AccountStatus;
  password?: string;
}

// ── Dynamic zone types ───────────────────────────────────────────────────────

export interface DzLockout {
  id: string;
  expedition: string;
  lockout_type: string;
  character: string;
  expires_at: string;
}

export interface RaidInstance {
  id: string;
  expedition: string;
  zone: string;
  group: string;
  entered_at: string;
  elapsed_secs: number;
  members: string[];
}

export interface DzHistoryEntry {
  id: string;
  expedition: string;
  zone: string;
  completed_at: string;
  duration_secs: number;
  participants: string[];
  loot: string[];
}

// ── Economy dashboard types ─────────────────────────────────────────────────

export interface KronoSettings {
  target_rate_per_day: number;
  min_sell_price: number;
  max_buy_price: number;
  restock_threshold: number;
  enabled: boolean;
}

export interface VendorRoute {
  id: string;
  zone: string;
  npc_name: string;
  path_notes: string;
  item_categories: string[];
  enabled: boolean;
}

export interface BankingRule {
  id: string;
  item_category: string;
  deposit_threshold: number;
  keep_on_hand: number;
  auto_deposit: boolean;
}

export interface TradeskillSupply {
  id: string;
  skill: string;
  materials: string[];
  restock_quantity: number;
  source_zone: string;
  enabled: boolean;
}

export interface WealthSnapshot {
  timestamp: string;
  plat: number;
  krono: number;
  item_value_estimate: number;
}

export interface WealthHistory {
  current: WealthSnapshot;
  snapshots: WealthSnapshot[];
}

export interface VendorWatchItem {
  item_name: string;
  max_price_copper: number | null;
  enabled: boolean;
}

export interface VendorWatchConfig {
  enabled: boolean;
  items: VendorWatchItem[];
}

export interface VendorWatchAlertEntry {
  id: number;
  vendor_name: string;
  item_name: string;
  expected_max_price_copper: number | null;
  actual_price_copper: number | null;
  price_delta_copper: number | null;
  within_budget: boolean | null;
  quantity: number;
  timestamp: string;
}

export interface VendorWatchAlertPage {
  total: number;
  offset: number;
  limit: number;
  entries: VendorWatchAlertEntry[];
}

export interface VendorWatchStats {
  total_alerts: number;
  watched_items: number;
  budget_hits: number;
}

export interface BoxChatSettings {
  enabled: boolean;
  host: string;
  port: number;
  auto_connect: boolean;
}

// ── Operator config types ────────────────────────────────────────────────────

export type AutoAcceptTrustMode = "anyone" | "trust_list";

export interface AutoAcceptSettings {
  enabled: boolean;
  accept_group_invites: boolean;
  accept_trades: boolean;
  accept_task_adds: boolean;
  accept_dz_adds: boolean;
  accept_translocates: boolean;
  accept_anchors: boolean;
  trust_mode: AutoAcceptTrustMode;
  trusted_players: string[];
}

export type TimestampFormat =
  | "date_time_24"
  | "time_24"
  | "date_time_12"
  | "time_12";

export interface TimestampConfig {
  enabled: boolean;
  format: TimestampFormat;
}

export interface XAssistCharacterConfig {
  character_name: string;
  ma_name: string | null;
  enabled: boolean;
}

export interface XAssistConfigUpdate {
  ma_name: string | null;
  enabled: boolean;
}

export type AutoGroupRole =
  | "none"
  | "main_tank"
  | "main_assist"
  | "puller"
  | "mark_npc"
  | "master_looter";

export interface AutoGroupMember {
  name: string;
  role: AutoGroupRole;
}

export interface AutoGroupProfile {
  name: string;
  leader_name: string;
  enabled: boolean;
  invite_retry_interval_secs: number;
  max_invite_retries: number;
  completion_command: string | null;
  members: AutoGroupMember[];
}

export interface AutoGroupSettings {
  groups: AutoGroupProfile[];
}

// ── GM alert types ───────────────────────────────────────────────────────────

export interface GmAlertConfig {
  enabled: boolean;
  soundEnabled: boolean;
  soundFile: string | null;
  toastEnabled: boolean;
  autoPauseEnabled: boolean;
  discordWebhookUrl: string | null;
  broadcastAllClients: boolean;
}

export interface GmPresenceStatus {
  isGmInZone: boolean;
  gmCount: number;
  gmNames: string[];
}

export interface GmAlertStatus {
  config: GmAlertConfig;
  presence: GmPresenceStatus;
  automationPaused: boolean;
}

// ── Spawn alert types ────────────────────────────────────────────────────────

export interface SpawnAlertEntry {
  id: number;
  spawn_name: string;
  zone: string;
  is_up: boolean;
  timestamp: string;
  time_since_last_pop_ms: number | null;
  match_source: string;
}

export interface WatchPattern {
  pattern: string;
  enabled: boolean;
}

export interface SpawnAlertConfig {
  watch_named_enabled: boolean;
  watch_patterns: WatchPattern[];
  broadcast_to_web: boolean;
  broadcast_to_clients: boolean;
}

export interface SpawnAlertStats {
  total_alerts: number;
  spawns_up: number;
  spawns_down: number;
}

export interface SpawnAlertPage {
  total: number;
  offset: number;
  limit: number;
  entries: SpawnAlertEntry[];
}

// ── Say detection types ──────────────────────────────────────────────────────

export type SayPatternType = "substring" | "exact" | "regex";
export type SayRuleAction = "alert" | "broadcast" | "command";

export interface SayDetectionRule {
  name: string;
  pattern: string;
  patternType: SayPatternType;
  actionType: SayRuleAction;
  actionValue: string | null;
  enabled: boolean;
}

export interface SayDetectionConfig {
  enabled: boolean;
  soundEnabled: boolean;
  soundFile: string | null;
  toastEnabled: boolean;
  discordWebhookUrl: string | null;
  broadcastAllClients: boolean;
  rules: SayDetectionRule[];
}

export interface SayDetectionMatchSummary {
  ruleName: string;
  sender: string;
  message: string;
  timestamp: number;
}

export interface SayDetectionStatus {
  config: SayDetectionConfig;
  totalMatches: number;
  lastMatch: SayDetectionMatchSummary | null;
}

// ── Group Builder types ────────────────────────────────────────────────────

export type EQClass =
  | "Warrior"
  | "Paladin"
  | "Shadow Knight"
  | "Ranger"
  | "Monk"
  | "Bard"
  | "Rogue"
  | "Berserker"
  | "Cleric"
  | "Druid"
  | "Shaman"
  | "Necromancer"
  | "Wizard"
  | "Magician"
  | "Enchanter"
  | "Beastlord";

export type Role = "Tank" | "Healer" | "DPS" | "Support" | "Puller" | "CC";

export interface Character {
  id: string;
  name: string;
  eqClass: EQClass;
  level: number;
  zone?: string;
  status: "online" | "idle" | "offline";
}

export interface GroupSlot {
  role: Role;
  characterId: string | null;
  locked: boolean;
  classPreference?: EQClass;
}

export interface GroupTemplate {
  id: string;
  name: string;
  description: string;
  slots: GroupSlot[];
}

export interface GroupBuilderState {
  templates: GroupTemplate[];
  activeTemplateId: string;
  characters: Character[];
}

// ── Raid configuration types ────────────────────────────────────────────────

export type RaidRole =
  | "none"
  | "main_tank"
  | "main_assist"
  | "ch_chain"
  | "puller"
  | "healer"
  | "dps";

export type BehaviorMode = "camp" | "hunt" | "follow";

export interface CampPosition {
  x: number;
  y: number;
  z: number;
}

export interface RaidMember {
  name: string;
  class: string;
  role: RaidRole;
  is_online: boolean;
  hp_pct: number;
  mana_pct?: number;
}

export interface RaidConfig {
  main_assist: string | null;
  main_tank: string | null;
  ch_chain: string[];
  pull_target: string | null;
  camp_position: CampPosition | null;
  behavior_mode: BehaviorMode;
  members: RaidMember[];
}

// ── Discord configuration types ─────────────────────────────────────────────

export type DiscordSeverity = "INFO" | "WARNING" | "ERROR" | "CRITICAL";

export type DiscordMessageMode = "plain_text" | "rich_embed";

export type DiscordMentionPolicy = "none" | "everyone";

export interface DiscordRouteConfig {
  enabled: boolean;
  webhook_url: string;
  level: DiscordSeverity;
  message_mode: DiscordMessageMode;
  mention_policy: DiscordMentionPolicy;
}

export interface DiscordSettings {
  webhook_url: string;
  channels: Record<string, string>;
  notification_routes: Record<string, DiscordRouteConfig>;
}

// ── Strategy tuning types ───────────────────────────────────────────────────

export type CharacterRole = "Tank" | "Healer" | "Support" | "DPS";

export interface RotationEntry {
  id: string;
  name: string;
  priority: number;
  enabled: boolean;
}

export interface ClassParams {
  ch_chain_timing_ms?: number;
  cross_client_heal_enabled?: boolean;
  cross_client_heal_threshold_pct?: number;
  cross_client_heal_priority?: number;
  cross_client_claim_timeout_ms?: number;
  dot_overlap_pct?: number;
  burn_at_hp_pct?: number;
  slow_at_hp_pct?: number;
}

export type RewardPreference =
  | { kind: "by_name"; reward_name: string }
  | { kind: "by_position"; reward_position: number };

export interface TaskRewardPreference {
  task_matcher: string;
  preference: RewardPreference;
}

export interface RewardAutomationConfig {
  rules: TaskRewardPreference[];
}

export type TributeAlertState = "ok" | "expiring" | "expired";

export interface TributePreferences {
  auto_activate: boolean;
  warning_threshold_secs: number;
  preferred_tributes: string[];
}

export interface TributeStatus {
  active: boolean;
  remaining_secs: number;
  point_balance: number;
  active_tributes: string[];
  alert_state: TributeAlertState;
}

export interface AutoRezConfig {
  enabled: boolean;
  min_xp_pct: number;
  trusted_casters: string[];
  decline_if_untrusted: boolean;
  delay_ms: number;
}

export interface AutoCampOnDeathConfig {
  enabled: boolean;
  camp_delay_secs: number;
  relog_wait_secs: number;
}

export interface CharacterConfig {
  character_name: string;
  class: string;
  role: CharacterRole;
  heal_at_pct: number;
  mana_sit_pct: number;
  nuke_at_pct: number;
  rotation: RotationEntry[];
  class_params: ClassParams;
  auto_rez?: AutoRezConfig;
  auto_camp_on_death?: AutoCampOnDeathConfig;
  group_override: boolean;
  group_name?: string;
  reward_automation?: RewardAutomationConfig;
  window_title_format?: string;
  tribute_preferences?: TributePreferences;
  tribute_status?: TributeStatus;
}

// ── Bard configuration types ─────────────────────────────────────────────────

export type InstrumentType =
  | "None"
  | "String"
  | "Brass"
  | "Wind"
  | "Percussion";

export type SongCategory =
  | "Haste"
  | "SpellFocus"
  | "MeleeProc"
  | "Crescendo"
  | "Insult"
  | "RunSpeed"
  | "Regen"
  | "Tank"
  | "Slow"
  | "Accelerando"
  | "Mez"
  | "Dot"
  | "Arcane"
  | "Other";

export type InstrumentSlot = "Primary" | "Secondary";

export interface SongSlotConfig {
  id: string;
  gem: number;
  name: string;
  priority: number;
  enabled: boolean;
  min_recast_ticks: number;
  buff_duration_ticks: number | null;
  category: SongCategory;
  instrument_type: InstrumentType;
  instrument_slot: InstrumentSlot;
}

export interface InstrumentSet {
  string_item_id: number | null;
  brass_item_id: number | null;
  wind_item_id: number | null;
  percussion_item_id: number | null;
}

export interface BardConfig {
  character_name: string;
  twist_enabled: boolean;
  full_rotation_enabled: boolean;
  instrument_swap_enabled: boolean;
  songs: SongSlotConfig[];
  instruments: InstrumentSet[];
}

// ── Loot configuration types ─────────────────────────────────────────────────

export interface LootRules {
  keep_items: string[];
  sell_items: string[];
  destroy_items: string[];
  loot_all: boolean;
  auto_split: boolean;
}

export type FilterAction = "keep" | "sell" | "destroy" | "bank";
export type LootFilterAction = FilterAction;

export type LootPolicy =
  | "need-before-greed"
  | "round-robin"
  | "master-looter"
  | "greed-only";

export interface LootRule {
  id: string;
  item_scope: string;
  quality: string;
  policy: LootPolicy;
  master_looter: string;
  enabled: boolean;
}

export interface AutoLootFilter {
  id: string;
  character_name: string;
  matcher: string;
  action: LootFilterAction;
  notes?: string;
}

export interface ItemFilterEntry {
  item_name: string;
  action: FilterAction;
}

export interface CharacterLootFilter {
  character: string;
  filters: ItemFilterEntry[];
}

export type DistributionMethod =
  | "need_before_greed"
  | "greed"
  | "round_robin"
  | "master_looter"
  | "free_for_all";

export interface DistributionRule {
  id: string;
  item_type: string;
  quality: string | null;
  method: DistributionMethod;
}

export interface DistributionConfig {
  rules: DistributionRule[];
}

export interface MasterLooter {
  character: string | null;
}

// ── Soul Engine types ─────────────────────────────────────────────────────────

export type SoulMood =
  | "content"
  | "anxious"
  | "focused"
  | "bored"
  | "excited"
  | "melancholic";

export interface SoulState {
  character_id: string;
  mood: SoulMood;
  personality_traits: string[];
  memory_count: number;
  last_event: string | null;
}

export interface LootHistoryEntry {
  id: number;
  timestamp: string;
  item_name: string;
  recipient: string;
  source_mob: string | null;
  zone: string | null;
  quantity: number;
  assigned_by: string | null;
  item_type?: string;
  quality?: string | null;
  assigned_to?: string;
  looted_by?: string;
  source?: string | null;
  policy?: LootPolicy;
  estimated_value?: number;
}

// ── Kill Tracker types ────────────────────────────────────────────────────────

export interface KillTrackerSettings {
  enabled: boolean;
  autoReportIntervalMinutes: number;
  autoReportChannel: string;
  autoReportIncludeMobs: boolean;
  autoReportIncludeKph: boolean;
  trackPerCharacter: boolean;
  maxSessionHistory: number;
}

export interface KillRecord {
  mobName: string;
  mobLevel: number;
  zone: string;
  timestamp: string;
  killTimeMs: number;
  totalDamage: number;
}

export interface MobStats {
  mobName: string;
  killCount: number;
  bestTimeMs: number;
  avgTimeMs: number;
  avgDps: number;
}

export interface EfficiencyScore {
  killsPerHour: number;
  avgKillTimeSecs: number;
  deathRatio: number;
  score: number;
}

export interface SessionStats {
  character: string;
  sessionStart: string;
  totalKills: number;
  totalDeaths: number;
  killsPerHour: number;
  efficiency: EfficiencyScore;
  mobStats: MobStats[];
  topMobs: [string, number][];
  zone: string;
}

export interface CharacterHistory {
  character: string;
  sessions: SessionStats[];
}

export interface KillTrackerDashboard {
  currentSession: SessionStats | null;
  characterHistory: CharacterHistory[];
  settings: KillTrackerSettings;
}

// ── Chat Log types ───────────────────────────────────────────────────────────

export type ChatChannel =
  | "say"
  | "tell"
  | "tell_out"
  | "group"
  | "raid"
  | "guild"
  | "ooc"
  | "shout"
  | "auction"
  | "shout2"
  | "pet"
  | "spontaneous"
  | "mpets"
  | "mq2";

export type LogLevel = "trace" | "debug" | "info" | "warn" | "error";

export type RotationStrategy = { daily: null } | { size: number } | "none";

export interface ChatLogConfig {
  enabled: boolean;
  channels: ChatChannel[];
  rotation_strategy: RotationStrategy;
  max_file_size_bytes: number;
  min_level: LogLevel;
  log_eq_chat: boolean;
}

export interface PerCharacterChatLogConfig {
  character_name: string;
  enabled: boolean;
  channels: ChatChannel[];
}

export type LogRotation =
  | { type: "none" }
  | { type: "daily" }
  | { type: "by_size"; max_bytes: number };

export interface ChatLogSettings {
  enabled: boolean;
  rotation: LogRotation;
  level: LogLevel;
  channels: ChatChannel[];
}

// ── Auto-accept types (deduplicated — defined earlier in file) ───────────────

export interface TradeskillTrophySettings {
  enabled: boolean;
  trophy_item_name: string;
}

export type TradeskillContainerType =
  | "alchemy"
  | "baking"
  | "brewing"
  | "blacksmithing"
  | "fletching"
  | "fishing"
  | "jewelry"
  | "poison"
  | "pottery"
  | "research"
  | "tailoring"
  | "tinkering";

export type TrophyEquipSlot = "ammo" | "mainhand";

export interface TradeskillTrophyStatus {
  active: boolean;
  equipped_by_manager: boolean;
  open_container_name: string | null;
  container_type: TradeskillContainerType | null;
  target_slot: TrophyEquipSlot | null;
  previous_item_name: string | null;
  charges_remaining: number | null;
}

export interface LiveTradeskillTrophyStatus {
  pid: number;
  status: TradeskillTrophyStatus;
}

export type PlayerFilterMode = "all" | "strangers_only" | "friends_only";

export interface PlayerWatchConfig {
  filter_mode: PlayerFilterMode;
  sound_on_zone_in: boolean;
  friends: string[];
}

// ── GM alerts types ──────────────────────────────────────────────────────────

export interface GmAlertConfig {
  enabled: boolean;
  soundEnabled: boolean;
  soundFile: string | null;
  toastEnabled: boolean;
  autoPauseEnabled: boolean;
  discordWebhookUrl: string | null;
  broadcastAllClients: boolean;
}

export interface GmPresenceStatus {
  isGmInZone: boolean;
  gmCount: number;
  gmNames: string[];
}

export interface GmAlertStatus {
  config: GmAlertConfig;
  presence: GmPresenceStatus;
  automationPaused: boolean;
}

export type StatWeights = Record<string, number>;

export interface ItemScoreConfig {
  min_upgrade_delta: number;
  class_weights: Record<string, StatWeights>;
}

export type PluginCoverageStatus = "native" | "adapted" | "deferred";

export interface PluginMapping {
  plugin: string;
  owner: string;
  status: PluginCoverageStatus;
  config_surface: string;
  notes: string;
}

export interface LegacyAdapterWarning {
  plugin: string;
  source_reference: string;
  adapted_into: string;
  unsupported_fields: string[];
}

export interface ItemKnowledgeConfig {
  link_sources: string[];
  show_provenance: boolean;
  show_unsupported_fields: boolean;
}

export type CursorAction = "keep" | "sell" | "destroy" | "consume";

export interface CursorRule {
  item_matcher: string;
  action: CursorAction;
  keep_at_or_below: number | null;
  overflow_action: CursorAction | null;
}

export type CollectionRoute = "keep" | "bank" | "tribute" | "sell";

export interface CollectionRoutingRule {
  set_matcher: string;
  incomplete_route: CollectionRoute;
  completed_route: CollectionRoute;
  duplicate_route: CollectionRoute;
}

export interface RewardRoutingRule {
  task_matcher: string;
  preference: RewardPreference;
  auto_claim: boolean;
}

export interface ConsumablePreferences {
  enabled: boolean;
  preferred_food: string[];
  preferred_drink: string[];
  ignored_items: string[];
}

export interface VendorWatchRule {
  item_name: string;
  max_price_pp: number | null;
  notify: boolean;
}

export interface RelocationRule {
  destination: string;
  required_option_id: string | null;
  keep_on_hand: number;
  notify_if_unavailable: boolean;
}

export interface TrophyPreferences {
  enabled: boolean;
  auto_equip: boolean;
  restore_after_craft: boolean;
  trophy_items: string[];
}

export interface AutoClaimPreferences {
  enabled: boolean;
  claim_membership_grants: boolean;
  claim_task_windows: boolean;
  once_per_session: boolean;
}

export interface InventoryUtilityConfig {
  plugin_mappings: PluginMapping[];
  legacy_adapters: LegacyAdapterWarning[];
  item_knowledge: ItemKnowledgeConfig;
  cursor_rules: CursorRule[];
  collection_routing: CollectionRoutingRule[];
  reward_routing: RewardRoutingRule[];
  consumption: ConsumablePreferences;
  vendor_watch: VendorWatchRule[];
  relocation_rules: RelocationRule[];
  trophy_preferences: TrophyPreferences;
  auto_claim: AutoClaimPreferences;
}

// ── XAssist types ────────────────────────────────────────────────────────────

export interface XAssistCharacterConfig {
  character_name: string;
  ma_name: string | null;
  enabled: boolean;
}

export interface XAssistConfigUpdate {
  ma_name: string | null;
  enabled: boolean;
}

// ── Inventory utility parity types ───────────────────────────────────────────

export interface LegacyAdapterProvenance {
  plugin: string;
  source_reference: string;
  adapted_into: string;
  unsupported_fields: string[];
}

export interface FoodRule {
  item_name: string;
  consume_below_pct: number;
  hydrate: boolean;
}

export interface AutoClaimRule {
  claim_name: string;
  enabled: boolean;
}

export interface InventoryUtilityParityConfig {
  plugin_mappings: PluginMapping[];
  provenance: LegacyAdapterProvenance[];
  reward_routing_rules: RewardRoutingRule[];
  collection_routing_rules: CollectionRoutingRule[];
  cursor_rules: CursorRule[];
  food_rules: FoodRule[];
  relocation_rules: RelocationRule[];
  auto_claim_rules: AutoClaimRule[];
}

// ── Groups & Camp Configuration types ────────────────────────────────────────

export type GroupMemberRole =
  | "main_tank"
  | "main_assist"
  | "puller"
  | "healer"
  | "dps"
  | "support"
  | "cc";

export interface GroupMember {
  character_name: string;
  class: string;
  role: GroupMemberRole;
  order: number;
  level?: number;
  class_settings?: ClassSpecificSettings;
}

export interface ClassSpecificSettings {
  pet_management_enabled: boolean;
  spell_priority: string;
  cc_assignment: string;
}

export interface Group {
  id: string;
  name: string;
  zone: string | null;
  members: GroupMember[];
  created_at: string;
  updated_at: string;
}

export interface CreateGroupPayload {
  name: string;
  zone?: string | null;
  members?: GroupMember[];
}

export interface UpdateGroupPayload {
  name?: string;
  zone?: string | null;
  members?: GroupMember[];
}

export interface CampCoordinate {
  x: number;
  y: number;
  z: number;
}

export interface PullTarget {
  name: string;
  enabled: boolean;
}

export interface PullPoint {
  label: string;
  location: CampCoordinate;
  enabled: boolean;
}

export interface SafeZoneMarker {
  name: string;
  center: CampCoordinate;
  radius: number;
}

export type PullStrategy = "melee" | "caster" | "balanced";

export interface CombatSettings {
  hp_buff_threshold_pct: number;
  mana_buff_threshold_pct: number;
  pull_strategy: PullStrategy;
}

export interface CampConfiguration {
  id: string;
  group_id: string;
  template_name: string | null;
  camp_zone: string;
  camp_center: CampCoordinate;
  pull_radius: number;
  pull_points: PullPoint[];
  pull_targets: PullTarget[];
  safe_zone_markers: SafeZoneMarker[];
  combat_settings: CombatSettings;
  recommended_level_min?: number;
  recommended_level_max?: number;
  requires_fear_class?: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateCampConfigPayload {
  group_id: string;
  template_name?: string | null;
  camp_zone: string;
  camp_center: CampCoordinate;
  pull_radius: number;
  pull_points?: PullPoint[];
  pull_targets?: PullTarget[];
  safe_zone_markers?: SafeZoneMarker[];
  combat_settings?: CombatSettings;
}

export interface UpdateCampConfigPayload {
  template_name?: string | null;
  camp_zone?: string;
  camp_center?: CampCoordinate;
  pull_radius?: number;
  pull_points?: PullPoint[];
  pull_targets?: PullTarget[];
  safe_zone_markers?: SafeZoneMarker[];
  combat_settings?: CombatSettings;
}

export type CampId = string;
export type ZoneId = string;

export interface Estimate {
  mu: number;
  sigma: number;
  n: number;
}

export type PerCharacter<T> = Record<string, T>;

export interface ObjectiveWeights {
  xp: number;
  plat: number;
  upgrades: number;
  safety: number;
}

export interface PartyMember {
  name: string;
  class: string;
  role: GroupMemberRole;
  level?: number;
}

export interface Party {
  members: PartyMember[];
}

export type Goal =
  | { kind: "xp" }
  | { kind: "plat" }
  | { kind: "item"; slot: string; character: string }
  | { kind: "faction"; faction: string };

export interface Route {
  from_zone: ZoneId;
  to_zone: ZoneId;
  eta_min: number;
  path: string[];
}

export interface RecommendRequest {
  party: Party;
  current_zone: ZoneId;
  goal: Goal;
  time_budget_min?: number;
  weights: ObjectiveWeights;
  min_confidence: number;
}

export interface CampRecommendation {
  camp_id: CampId;
  xp_per_hr: Estimate;
  pp_per_hr: Estimate;
  upgrade_probability: PerCharacter<number>;
  risk: number;
  eta_min: number;
  route: Route;
  rationale: string;
  confidence_band: [number, number];
  exploratory: boolean;
}

// ── Admin diagnostics & operations types ────────────────────────────────────

export interface MetricsData {
  response_time_ms: number;
  uptime_secs: number;
  memory_usage_mb: number;
  timestamp: string;
}

export interface LogEntry {
  timestamp: string;
  level: string;
  message: string;
  source?: string;
}

export interface BackupEntry {
  id: string;
  created_at: string;
  size_bytes: number;
  status: "pending" | "completed" | "failed";
  description?: string;
}
