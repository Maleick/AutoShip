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
