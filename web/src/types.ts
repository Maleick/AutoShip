export interface Session {
  client_id: number;
  character_name: string;
  zone: string;
  level: number;
  hp_pct: number;
  mana_pct: number;
  status: "active" | "idle" | "dead" | "camping" | "zoning";
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
  tribute_preferences: TributePreferences;
  tribute_status: TributeStatus;
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
  item_type: string;
  quality: string | null;
  assigned_to: string;
  looted_by: string;
  source: string | null;
  policy: LootPolicy;
  estimated_value: number;
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

export type RotationStrategy =
  | { daily: null }
  | { size: number }
  | "none";

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
}
