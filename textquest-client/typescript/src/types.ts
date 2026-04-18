export interface SessionInfo {
  client_id: number;
  character_name: string;
  zone: string;
  level: number;
  hp_pct: number;
  mana_pct: number;
  endurance_pct: number;
  status: string;
  buff_count: number;
  target_name?: string;
  target_hp_pct?: number;
  pet_name?: string;
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
  group_override: boolean;
  group_name?: string;
}

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

export interface HealthResponse {
  status: string;
  version: string;
}

export interface ErrorResponse {
  error: string;
}

export interface CommandRequest {
  command: string;
  target?: string;
}

export interface CommandResponse {
  success: boolean;
  message: string;
}

export interface GroupAssignment {
  group_id: number;
}

export interface ClientConfig {
  baseUrl: string;
  apiToken?: string;
  timeoutMs?: number;
}

export type SessionEvent = {
  type: 'session_update';
  session: SessionInfo;
} | {
  type: 'command';
  command: string;
  target?: string;
} | {
  type: 'error';
  message: string;
};

export class TextQuestClientError extends Error {
  constructor(
    message: string,
    public statusCode?: number,
    public response?: ErrorResponse
  ) {
    super(message);
    this.name = 'TextQuestClientError';
  }
}