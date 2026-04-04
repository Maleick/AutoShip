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
