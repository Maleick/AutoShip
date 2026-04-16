export interface Session {
  client_id: number;
  character_name: string;
  zone: string;
  level: number;
  hp_pct: number;
  mana_pct: number;
  endurance_pct: number;
  status: "active" | "idle" | "dead" | "camping" | "zoning";
  buff_count: number;
  target_name?: string | null;
  target_hp_pct?: number | null;
  pet_name?: string | null;
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

export interface BoxChatSettings {
  enabled: boolean;
  host: string;
  port: number;
  auto_connect: boolean;
}

// ── GM Alert types ───────────────────────────────────────────────────────────

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

// ── XAssist (Cross-Group Outside-Group Assist) types ─────────────────────────

export interface XAssistCharacterConfig {
  character_name: string;
  ma_name: string | null;
  enabled: boolean;
}

export interface XAssistConfigUpdate {
  ma_name: string | null;
  enabled: boolean;
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
  auto_rez: AutoRezConfig;
  group_override: boolean;
  group_name?: string;
  auto_camp_on_death: AutoCampOnDeathConfig;
  tribute_preferences: TributePreferences;
  tribute_status: TributeStatus;
  bard?: BardConfig;
}

// ── Auto-accept configuration types ─────────────────────────────────────────

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

// ── Bard song configuration types ────────────────────────────────────────────

export type InstrumentType = "None" | "String" | "Brass" | "Wind" | "Percussion";

export type InstrumentSlot = "Primary" | "Secondary";

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

export interface BardStatus {
  character_name: string;
  active_songs: string[];
  current_twist_index: number;
  twist_active: boolean;
  equipped_instrument: InstrumentType;
  next_cast_gem: number | null;
  mez_queue_size: number;
}

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
  item_type: string;
  quality: string | null;
  assigned_to: string;
  looted_by: string;
  source: string | null;
  policy: LootPolicy;
  estimated_value: number;
}

// ── Player Watch types ─────────────────────────────────────────────────────────

export type PlayerFilterMode = "all" | "strangers_only" | "friends_only";

export interface PlayerWatchConfig {
  filter_mode: PlayerFilterMode;
  sound_on_zone_in: boolean;
  friends: string[];
}
