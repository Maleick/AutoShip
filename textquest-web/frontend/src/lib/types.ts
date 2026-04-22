/**
 * Shared TypeScript types for the TextQuest web frontend.
 * These mirror the Rust types from textquest-common/src/protocol.rs
 */

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
