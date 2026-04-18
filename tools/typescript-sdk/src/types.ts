/**
 * TextQuest Web API TypeScript Type Definitions
 *
 * This file contains all type definitions for the TextQuest web API responses
 * and request bodies.
 */

// ─── Health & Status ───────────────────────────────────────────────────

export interface HealthResponse {
  status: string;
  version: string;
}

// ─── Sessions ──────────────────────────────────────────────────────────

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

export interface SessionsResponse {
  sessions: Session[];
  last_updated?: string;
}

// ─── Accounts ──────────────────────────────────────────────────────────

export interface Account {
  id: string;
  username: string;
  expansion_level: number;
  status: "active" | "inactive";
  created_at: string;
  last_updated: string;
}

export interface AccountsResponse {
  accounts: Account[];
}

export interface AccountCreateRequest {
  username: string;
  expansion_level?: number;
  password?: string;
}

export interface AccountUpdateRequest {
  expansion_level?: number;
  password?: string;
}

export interface CredentialStoreResponse {
  encrypted: boolean;
}

// ─── Box Chat Settings ─────────────────────────────────────────────────

export interface BoxChatConfig {
  enabled: boolean;
  host: string;
  port: number;
  auto_connect: boolean;
}

// ─── Chat Log Settings ─────────────────────────────────────────────────

export interface ChatLogSettings {
  enabled: boolean;
  path: string;
  rotate_daily: boolean;
}

// ─── Character Configuration ───────────────────────────────────────────

export interface CharacterConfig {
  character_name: string;
  strategy_class: string;
  auto_combat: boolean;
  auto_buff: boolean;
  camp_mode: boolean;
  rotation?: Record<string, unknown>;
  last_updated: string;
}

export interface CharacterConfigsResponse {
  configs: CharacterConfig[];
}

export interface CharacterConfigRequest {
  strategy_class: string;
  auto_combat?: boolean;
  auto_buff?: boolean;
  camp_mode?: boolean;
  rotation?: Record<string, unknown>;
}

// ─── Auto Accept Settings ──────────────────────────────────────────────

export interface AutoAcceptSettings {
  auto_accept_groups: boolean;
  auto_accept_raids: boolean;
  auto_decline_buffs: boolean;
}

export interface AutoAcceptRequest {
  auto_accept_groups?: boolean;
  auto_accept_raids?: boolean;
  auto_decline_buffs?: boolean;
}

// ─── Player Watch Configuration ────────────────────────────────────────

export interface PlayerWatchConfig {
  enabled: boolean;
  watch_list: string[];
  broadcast_on_entry: boolean;
  broadcast_on_exit: boolean;
  exclude_own_group: boolean;
  min_level_threshold: number;
}

export interface PlayerWatchConfigRequest {
  enabled?: boolean;
  watch_list?: string[];
  broadcast_on_entry?: boolean;
  broadcast_on_exit?: boolean;
  exclude_own_group?: boolean;
  min_level_threshold?: number;
}

// ─── Economy Settings ──────────────────────────────────────────────────

export interface EconomySettings {
  vendor_target_stock: number;
  vendor_restock_interval_mins: number;
  krono_target: number;
  tradeskill_priority: string[];
  auto_harvest: boolean;
}

export interface EconomySettingsRequest {
  vendor_target_stock?: number;
  vendor_restock_interval_mins?: number;
  krono_target?: number;
  tradeskill_priority?: string[];
  auto_harvest?: boolean;
}

export interface VendorRoute {
  id: string;
  name: string;
  waypoints: string[];
  priority: number;
  enabled: boolean;
  created_at: string;
  last_updated: string;
}

export interface VendorRoutesResponse {
  routes: VendorRoute[];
}

export interface VendorRouteRequest {
  name: string;
  waypoints: string[];
  priority?: number;
  enabled?: boolean;
}

export interface WealthResponse {
  total_krono: number;
  total_plat: number;
  by_character: Record<string, { krono: number; plat: number }>;
}

// ─── Soul Audit ────────────────────────────────────────────────────────

export interface SoulState {
  character_id: string;
  recovery_status: "healthy" | "degraded" | "critical";
  memory_usage_mb: number;
  last_check: string;
  issues: string[];
}

export interface SoulStatesResponse {
  souls: SoulState[];
}

export interface SoulAuditEntry {
  character_id: string;
  timestamp: string;
  event_type: string;
  details: string;
  severity: "info" | "warning" | "error";
}

export interface SoulAuditResponse {
  entries: SoulAuditEntry[];
  total_count: number;
}

export interface SoulCharacterAuditResponse {
  character_id: string;
  entries: SoulAuditEntry[];
  total_count: number;
}

// ─── Loot Configuration ────────────────────────────────────────────────

export interface LootRule {
  id: string;
  name: string;
  pattern: string;
  priority: number;
  action: "need" | "greed" | "pass" | "ms_only";
  enabled: boolean;
}

export interface LootRulesResponse {
  rules: LootRule[];
}

export interface LootRuleRequest {
  name: string;
  pattern: string;
  priority?: number;
  action?: "need" | "greed" | "pass" | "ms_only";
  enabled?: boolean;
}

export interface LootFilter {
  character: string;
  slots: string[];
  classes: string[];
  custom_rules: string[];
  min_value: number;
}

export interface LootFiltersResponse {
  filters: Record<string, LootFilter>;
}

export interface LootFilterRequest {
  slots?: string[];
  classes?: string[];
  custom_rules?: string[];
  min_value?: number;
}

export interface MasterLooterConfig {
  character_name: string;
  enabled: boolean;
  auto_split: boolean;
  split_method: "even" | "dkp" | "loot_priority";
}

export interface MasterLooterResponse {
  config: MasterLooterConfig;
}

export interface MasterLooterRequest {
  enabled?: boolean;
  auto_split?: boolean;
  split_method?: "even" | "dkp" | "loot_priority";
}

export interface LootDistributionEntry {
  item_id: string;
  item_name: string;
  recipient: string;
  timestamp: string;
  method: string;
}

export interface LootDistributionResponse {
  entries: LootDistributionEntry[];
  total_value: number;
}

export interface LootDistributionRequest {
  item_id: string;
  item_name: string;
  recipient: string;
  method?: string;
}

export interface LootHistoryResponse {
  entries: LootDistributionEntry[];
  last_20_total: number;
}

// ─── Kill Tracker ──────────────────────────────────────────────────────

export interface KillTrackerSettings {
  enabled: boolean;
  auto_report_interval_minutes: number;
  auto_report_channel: string;
  auto_report_include_mobs: boolean;
  auto_report_include_kph: boolean;
  track_per_character: boolean;
  max_session_history: number;
}

export interface KillEntry {
  mob_id: string;
  mob_name: string;
  zone: string;
  timestamp: string;
  killers: string[];
  loot_value: number;
}

export interface KillTrackerResponse {
  kills: KillEntry[];
  session_total: number;
}

export interface KillTrackerStatsResponse {
  total_kills: number;
  kills_by_zone: Record<string, number>;
  total_loot_value: number;
  average_kill_value: number;
}

// ─── Spawn Alerts ──────────────────────────────────────────────────────

export interface SpawnAlert {
  spawn_id: string;
  spawn_name: string;
  zone: string;
  timestamp: string;
  status: "spawned" | "despawned" | "roaming";
  last_seen_location?: string;
}

export interface SpawnAlertsResponse {
  alerts: SpawnAlert[];
}

export interface SpawnAlertConfig {
  enabled: boolean;
  watch_patterns: string[];
  alert_discord: boolean;
  alert_in_game: boolean;
  min_difficulty_tier: number;
}

export interface SpawnAlertConfigRequest {
  enabled?: boolean;
  watch_patterns?: string[];
  alert_discord?: boolean;
  alert_in_game?: boolean;
  min_difficulty_tier?: number;
}

export interface SpawnAlertStatsResponse {
  total_alerts: number;
  alerts_today: number;
  unique_spawns: number;
  active_watches: number;
}

export interface SpawnAlertWatchListResponse {
  patterns: string[];
}

export interface SpawnAlertWatchPatternRequest {
  pattern: string;
}

// ─── Timestamp Configuration ────────────────────────────────────────────

export interface TimestampConfig {
  character: string;
  enabled: boolean;
  log_level: "verbose" | "normal" | "minimal";
  include_timestamps: boolean;
  format: "12h" | "24h";
  last_updated: string;
}

export interface TimestampConfigsResponse {
  configs: Record<string, TimestampConfig>;
}

export interface TimestampConfigRequest {
  enabled?: boolean;
  log_level?: "verbose" | "normal" | "minimal";
  include_timestamps?: boolean;
  format?: "12h" | "24h";
}

// ─── GM Alerts ────────────────────────────────────────────────────────

export interface GmAlert {
  gm_name: string;
  zone: string;
  timestamp: string;
  action_type: string;
  affected_characters: string[];
}

export interface GmAlertsResponse {
  alerts: GmAlert[];
  zone_gm_status: Record<string, boolean>;
}

// ─── Operational Alerts ────────────────────────────────────────────────

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

export interface AlertingConfigRequest {
  enable_discord?: boolean;
  discord_webhook_url?: string;
  enable_email?: boolean;
  smtp_server?: string;
  smtp_port?: number;
  smtp_username?: string;
  smtp_password?: string;
  email_from?: string;
  email_recipients?: string[];
  email_subject_prefix?: string;
  warning_batch_window_secs?: number;
  thresholds?: Partial<AlertThresholdConfig>;
}

export interface OperationalAlertsResponse {
  alerts: OperationalAlert[];
  total_count: number;
}

// ─── XAssist Configuration ──────────────────────────────────────────────

export interface XAssistConfig {
  character: string;
  enabled: boolean;
  auto_assist_on_follow: boolean;
  assist_target: string;
  keybind: string;
  last_updated: string;
}

export interface XAssistConfigsResponse {
  configs: Record<string, XAssistConfig>;
}

export interface XAssistConfigRequest {
  enabled?: boolean;
  auto_assist_on_follow?: boolean;
  assist_target?: string;
  keybind?: string;
}

// ─── Chat Pattern Rules ────────────────────────────────────────────────

export interface ChatPatternRule {
  id: string;
  pattern: string;
  response: string;
  enabled: boolean;
  priority: number;
  cooldown_secs: number;
  last_triggered?: string;
  trigger_count: number;
}

export interface ChatPatternRulesResponse {
  rules: ChatPatternRule[];
}

export interface ChatPatternRuleRequest {
  pattern: string;
  response: string;
  enabled?: boolean;
  priority?: number;
  cooldown_secs?: number;
}

export interface ChatPatternRuleStatsResponse {
  total_rules: number;
  enabled_count: number;
  total_triggers: number;
  rules_on_cooldown: number;
}

export interface ChatPatternRuleImportRequest {
  rules: ChatPatternRuleRequest[];
}

// ─── Say Detection ──────────────────────────────────────────────────────

export interface SayDetectionConfig {
  enabled: boolean;
  log_all_says: boolean;
  alert_on_patterns: string[];
}

export interface SayDetectionResponse {
  config: SayDetectionConfig;
}

// ─── Error Response ────────────────────────────────────────────────────

export interface ErrorResponse {
  error: string;
}

// ─── Generic Response Types ────────────────────────────────────────────

export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  per_page: number;
}
