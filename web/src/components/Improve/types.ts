// Core domain types for the Improve panel

export interface Session {
  id: string;
  date: string; // ISO 8601
  camp: {
    zone: string;
    mobSet: string[];
    partyComp: string[];
    levelRange: string;
  };
  duration: number; // milliseconds
  xpPerHour: number;
  metrics: {
    dps?: number;
    healing?: number;
    deaths?: number;
  };
  buildSha?: string;
  tlpRuleset?: string;
}

export interface SessionCampFingerprint {
  campId: string; // hash(zone + mobSet + partyComp + levelRange)
  zone: string;
  mobSet: string[];
  partyComp: string[];
  levelRange: string;
}

export interface PersonalBest {
  sessionId: string;
  campId: string;
  metric: "xphour" | "dps" | "deaths";
  value: number;
  recordedAt: string; // ISO 8601
  staleAt?: string; // ISO 8601 (60 days or after patch)
  isPinned: boolean;
}

export type SeverityLevel = "major" | "average" | "minor";

export interface Suggestion {
  id: string;
  severity: SeverityLevel;
  title: string;
  evidence: string;
  deltaValue: number; // +X% or -Y
  deltaType: "dps" | "healing" | "deaths" | "xphour";
  timestamp?: number; // Log offset for "Jump to log"
  dismissedAt?: string; // ISO 8601
}

export interface ChecklistItem {
  id: string;
  character: string;
  description: string;
  passed: boolean; // True = positive performance, False = needs improvement
  progress?: number; // 0-100 for partial credit
}

export interface LogEvent {
  timestamp: number;
  type: string;
  actor: string;
  action: string;
  target?: string;
  details?: Record<string, unknown>;
}

export interface ComparisonPoint {
  label: string;
  current: number;
  personal_best: number;
  delta: number; // percentage
  isWithinNoise: boolean; // Within configurable epsilon (default 1%)
}

export type ViewMode = "tables" | "timeline" | "events";

export interface UIState {
  currentSession: Session | null;
  viewMode: ViewMode;
  showMinorSuggestions: boolean;
  selectedCharacter?: string;
  filterCamp?: SessionCampFingerprint;
  noiseFloor: number; // epsilon, default 0.01 (1%)
}
