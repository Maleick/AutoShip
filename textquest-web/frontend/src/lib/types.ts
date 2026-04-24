// Types mirrored from textquest-web Rust backend.
// Field names stay snake_case to match serde serialization.

export interface Client {
  id: string;
  character: string;
  zone: string;
  connected: boolean;
  hp: number;
  maxHp: number;
  mana: number;
  maxMana: number;
}

export interface Group {
  id: string;
  name: string;
  memberIds: string[];
  active: boolean;
}

export interface ApiResponse<T> {
  ok: boolean;
  data?: T;
  error?: string;
}

// ─── Sessions ──────────────────────────────────────────────────────────────
export type SessionState = "idle" | "active" | "paused" | "error";
export type RoutingScope = "self" | "group" | "broadcast";
export type CommandScope = "self" | "group" | "all";

export interface SessionControlRecord {
  session_id: number;
  group_id: number;
  routing_scope: RoutingScope;
  state: SessionState;
  character_name?: string;
  class?: string;
  // Live-state fields, optional (joined from /api/dashboard SessionCard when available)
  hp_pct?: number;
  mana_pct?: number;
  zone?: string;
  level?: number;
}

export interface SetGroupRequest {
  group_id: number;
}

export interface SlashCommandRequest {
  command: string;
  scope?: CommandScope;
}

export interface SlashCommandResponse {
  session_id: number;
  command: string;
  scope_used: string;
  accepted: boolean;
  message: string;
}

// ─── Auto-Group ────────────────────────────────────────────────────────────
export type GroupRole = "tank" | "healer" | "dps" | "support" | "puller" | "mez" | "slow";

export interface AutoGroupMember {
  name: string;
  role: GroupRole;
}

export interface AutoGroupConfig {
  enabled: boolean;
  members: AutoGroupMember[];
  completion_command?: string | null;
  max_retries: number;
  invite_interval_ticks: number;
  member_wait_ticks: number;
}

export interface AutoGroupStatus {
  phase: string;
  config: AutoGroupConfig;
}

// ─── Characters ────────────────────────────────────────────────────────────
export interface RotationEntry {
  id: string;
  name: string;
  priority: number;
  enabled: boolean;
}

export interface ClassParams {
  ch_chain_timing_ms?: number | null;
  dot_overlap_pct?: number | null;
  burn_at_hp_pct?: number | null;
  slow_at_hp_pct?: number | null;
}

export interface AutoRezConfig {
  enabled: boolean;
  min_xp_pct: number;
  trusted_casters: string[];
  decline_if_untrusted: boolean;
}

export interface TributePreferences {
  auto_activate: boolean;
  warning_threshold_secs: number;
  preferred_tributes: string[];
}

export interface CharacterConfig {
  character_name: string;
  class: string;
  role: string;
  heal_at_pct: number;
  mana_sit_pct: number;
  nuke_at_pct: number;
  rotation: RotationEntry[];
  class_params: ClassParams;
  auto_rez: AutoRezConfig;
  group_override: boolean;
  group_name?: string | null;
  window_title_format: string;
  tribute_preferences: TributePreferences;
}

export type CharacterConfigMap = Record<string, CharacterConfig>;

export const EQ_CLASSES = [
  "WAR",
  "CLR",
  "PAL",
  "RNG",
  "SHD",
  "DRU",
  "MNK",
  "BRD",
  "ROG",
  "SHM",
  "NEC",
  "WIZ",
  "MAG",
  "ENC",
  "BST",
  "BER",
] as const;

export const CHAR_ROLES = [
  "tank",
  "main_tank",
  "off_tank",
  "healer",
  "main_healer",
  "dps",
  "melee_dps",
  "ranged_dps",
  "support",
  "puller",
  "mez",
  "slow",
] as const;

// ─── Economy ───────────────────────────────────────────────────────────────
export interface EconomyLedgerResponse {
  plat_per_hour: number;
  items_distributed: number;
  vendor_sales: number;
}

export interface EconomyQueuesResponse {
  loot_queue_len: number;
  vendor_backlog_len: number;
}

export interface VendorRoute {
  id: string;
  zone: string;
  npc_name: string;
  path_notes: string;
  item_categories: string[];
  enabled: boolean;
}
