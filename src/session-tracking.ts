/**
 * Session tracking pack — MQ2XPTracker, MQ2KillTracker, MQ2PlatTracker
 *
 * Provides per-session and cumulative tracking of:
 *   • XP gained (per hour, per session, per level)
 *   • Kills (per hour, by NPC type, named kills)
 *   • Platinum earned (per hour, per session, loot value)
 *
 * Data is persisted across sessions for trend analysis and exported
 * as structured summaries for the self-improvement loop (#2758).
 *
 * @module autoship-session-tracking
 */

/** A single tracked kill event. */
export interface KillEvent {
  /** ISO-8601 timestamp of the kill. */
  timestamp: string;
  /** Name of the NPC killed. */
  npc_name: string;
  /** Whether the NPC was a named / rare spawn. */
  named: boolean;
  /** Optional NPC type / category (e.g. "undead", "animal"). */
  npc_type?: string;
}

/** A single tracked loot / platinum event. */
export interface PlatEvent {
  /** ISO-8601 timestamp of the event. */
  timestamp: string;
  /** Platinum earned (can be negative for trades / purchases). */
  amount: number;
  /** Source description (e.g. "loot", "vendor", "trade"). */
  source: string;
}

/** A single tracked XP event. */
export interface XPEvent {
  /** ISO-8601 timestamp of the event. */
  timestamp: string;
  /** XP gained (always non-negative). */
  amount: number;
  /** Current player level at time of gain. */
  level: number;
  /** Source description (e.g. "kill", "quest", "exploration"). */
  source: string;
}

/** Summary exported for the self-improvement loop. */
export interface SessionSummary {
  /** Unique session identifier. */
  session_id: string;
  /** ISO-8601 timestamp when the session started. */
  started_at: string;
  /** ISO-8601 timestamp when the session ended (or last updated). */
  ended_at: string;
  /** Duration of the session in milliseconds. */
  duration_ms: number;
  /** Total XP gained this session. */
  total_xp: number;
  /** XP per hour for this session. */
  xp_per_hour: number;
  /** Total kills this session. */
  total_kills: number;
  /** Kills per hour for this session. */
  kills_per_hour: number;
  /** Number of named / rare kills this session. */
  named_kills: number;
  /** Total platinum earned this session. */
  total_plat: number;
  /** Platinum per hour for this session. */
  plat_per_hour: number;
  /** Breakdown of kills by NPC type. */
  kills_by_type: Record<string, number>;
  /** Breakdown of XP by level. */
  xp_by_level: Record<string, number>;
}

/** Options for constructing a tracker. */
export interface TrackerOptions {
  /** Unique session identifier. */
  session_id: string;
  /** ISO-8601 timestamp when the session started. */
  started_at?: string;
}

/** Base class with shared timing / persistence helpers. */
class BaseTracker {
  readonly session_id: string;
  readonly started_at: string;

  constructor(opts: TrackerOptions) {
    this.session_id = opts.session_id;
    this.started_at = opts.started_at ?? new Date().toISOString();
  }

  /** Return elapsed time in milliseconds since session start. */
  protected elapsedMs(): number {
    return Date.now() - new Date(this.started_at).getTime();
  }

  /** Return elapsed time in hours (minimum 1 minute to avoid division by zero). */
  protected elapsedHours(): number {
    const ms = this.elapsedMs();
    const minutes = ms / 60_000;
    return Math.max(minutes, 1) / 60;
  }
}

/** Track XP gained per hour, per session, per level. */
export class MQ2XPTracker extends BaseTracker {
  private events: XPEvent[] = [];

  /** Record an XP gain event. */
  record(amount: number, level: number, source = "kill"): void {
    if (amount < 0) {
      throw new RangeError("XP amount cannot be negative");
    }
    this.events.push({
      timestamp: new Date().toISOString(),
      amount,
      level,
      source,
    });
  }

  /** Total XP gained this session. */
  totalXP(): number {
    return this.events.reduce((sum, e) => sum + e.amount, 0);
  }

  /** XP per hour for this session. */
  xpPerHour(): number {
    return this.totalXP() / this.elapsedHours();
  }

  /** Breakdown of XP gained per level. */
  xpByLevel(): Record<string, number> {
    const map: Record<string, number> = {};
    for (const e of this.events) {
      const key = String(e.level);
      map[key] = (map[key] ?? 0) + e.amount;
    }
    return map;
  }

  /** All recorded events. */
  getEvents(): readonly XPEvent[] {
    return this.events;
  }
}

/** Track kills per hour, kill counts by NPC type, named kills. */
export class MQ2KillTracker extends BaseTracker {
  private events: KillEvent[] = [];

  /** Record a kill event. */
  record(npc_name: string, named = false, npc_type?: string): void {
    this.events.push({
      timestamp: new Date().toISOString(),
      npc_name,
      named,
      npc_type,
    });
  }

  /** Total kills this session. */
  totalKills(): number {
    return this.events.length;
  }

  /** Kills per hour for this session. */
  killsPerHour(): number {
    return this.totalKills() / this.elapsedHours();
  }

  /** Number of named / rare kills this session. */
  namedKills(): number {
    return this.events.filter((e) => e.named).length;
  }

  /** Breakdown of kills by NPC type. */
  killsByType(): Record<string, number> {
    const map: Record<string, number> = {};
    for (const e of this.events) {
      const key = e.npc_type ?? "unknown";
      map[key] = (map[key] ?? 0) + 1;
    }
    return map;
  }

  /** All recorded events. */
  getEvents(): readonly KillEvent[] {
    return this.events;
  }
}

/** Track platinum earned per hour, per session, loot value. */
export class MQ2PlatTracker extends BaseTracker {
  private events: PlatEvent[] = [];

  /** Record a platinum gain / spend event. */
  record(amount: number, source = "loot"): void {
    this.events.push({
      timestamp: new Date().toISOString(),
      amount,
      source,
    });
  }

  /** Total platinum earned this session (sum of all amounts). */
  totalPlat(): number {
    return this.events.reduce((sum, e) => sum + e.amount, 0);
  }

  /** Platinum per hour for this session. */
  platPerHour(): number {
    return this.totalPlat() / this.elapsedHours();
  }

  /** Breakdown of platinum by source. */
  platBySource(): Record<string, number> {
    const map: Record<string, number> = {};
    for (const e of this.events) {
      map[e.source] = (map[e.source] ?? 0) + e.amount;
    }
    return map;
  }

  /** All recorded events. */
  getEvents(): readonly PlatEvent[] {
    return this.events;
  }
}

/**
 * Aggregate session tracker that combines XP, Kill, and Plat trackers
 * and produces a unified {@link SessionSummary}.
 */
export class SessionTracker {
  readonly xp: MQ2XPTracker;
  readonly kills: MQ2KillTracker;
  readonly plat: MQ2PlatTracker;

  constructor(opts: TrackerOptions) {
    this.xp = new MQ2XPTracker(opts);
    this.kills = new MQ2KillTracker(opts);
    this.plat = new MQ2PlatTracker(opts);
  }

  /** Generate a {@link SessionSummary} for the self-improvement loop. */
  summary(): SessionSummary {
    const ended_at = new Date().toISOString();
    const duration_ms = Date.now() - new Date(this.xp.started_at).getTime();
    const hours = Math.max(duration_ms / 60_000, 1) / 60;

    return {
      session_id: this.xp.session_id,
      started_at: this.xp.started_at,
      ended_at,
      duration_ms,
      total_xp: this.xp.totalXP(),
      xp_per_hour: this.xp.totalXP() / hours,
      total_kills: this.kills.totalKills(),
      kills_per_hour: this.kills.totalKills() / hours,
      named_kills: this.kills.namedKills(),
      total_plat: this.plat.totalPlat(),
      plat_per_hour: this.plat.totalPlat() / hours,
      kills_by_type: this.kills.killsByType(),
      xp_by_level: this.xp.xpByLevel(),
    };
  }
}

/** Persist a session summary to JSON string. */
export function exportSummary(summary: SessionSummary): string {
  return JSON.stringify(summary, null, 2);
}

/** Parse a persisted session summary from JSON string. */
export function importSummary(json: string): SessionSummary {
  const parsed = JSON.parse(json) as unknown;
  if (typeof parsed !== "object" || parsed === null) {
    throw new TypeError("Invalid summary JSON");
  }
  return parsed as SessionSummary;
}
