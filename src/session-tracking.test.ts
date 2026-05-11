import { describe, it, expect, beforeEach, vi } from "vitest";
import {
  MQ2XPTracker,
  MQ2KillTracker,
  MQ2PlatTracker,
  SessionTracker,
  exportSummary,
  importSummary,
} from "./session-tracking.js";

describe("MQ2XPTracker", () => {
  let tracker: MQ2XPTracker;

  beforeEach(() => {
    tracker = new MQ2XPTracker({ session_id: "test-xp-1", started_at: new Date().toISOString() });
  });

  it("records XP events", () => {
    tracker.record(100, 5, "kill");
    tracker.record(200, 5, "quest");
    expect(tracker.totalXP()).toBe(300);
    expect(tracker.getEvents()).toHaveLength(2);
  });

  it("rejects negative XP", () => {
    expect(() => tracker.record(-10, 1)).toThrow(RangeError);
  });

  it("computes XP by level", () => {
    tracker.record(100, 5);
    tracker.record(200, 6);
    tracker.record(50, 5);
    expect(tracker.xpByLevel()).toEqual({ "5": 150, "6": 200 });
  });

  it("computes XP per hour", () => {
    const started = new Date(Date.now() - 3_600_000).toISOString(); // 1 hour ago
    tracker = new MQ2XPTracker({ session_id: "test-xp-2", started_at: started });
    tracker.record(1200, 10);
    expect(tracker.xpPerHour()).toBeCloseTo(1200, 0);
  });

  it("returns zero when no events", () => {
    expect(tracker.totalXP()).toBe(0);
    expect(tracker.xpByLevel()).toEqual({});
    expect(tracker.getEvents()).toHaveLength(0);
  });
});

describe("MQ2KillTracker", () => {
  let tracker: MQ2KillTracker;

  beforeEach(() => {
    tracker = new MQ2KillTracker({ session_id: "test-kill-1", started_at: new Date().toISOString() });
  });

  it("records kill events", () => {
    tracker.record("a_skeleton", false, "undead");
    tracker.record("a_bear", false, "animal");
    expect(tracker.totalKills()).toBe(2);
  });

  it("counts named kills", () => {
    tracker.record("Fippy Darkpaw", true, "gnoll");
    tracker.record("a_gnoll", false, "gnoll");
    expect(tracker.namedKills()).toBe(1);
    expect(tracker.totalKills()).toBe(2);
  });

  it("computes kills by type", () => {
    tracker.record("a_skeleton", false, "undead");
    tracker.record("a_zombie", false, "undead");
    tracker.record("a_bear", false, "animal");
    expect(tracker.killsByType()).toEqual({ undead: 2, animal: 1 });
  });

  it("computes kills per hour", () => {
    const started = new Date(Date.now() - 3_600_000).toISOString();
    tracker = new MQ2KillTracker({ session_id: "test-kill-2", started_at: started });
    tracker.record("a_rat", false, "animal");
    tracker.record("a_rat", false, "animal");
    expect(tracker.killsPerHour()).toBeCloseTo(2, 0);
  });

  it("defaults unknown type", () => {
    tracker.record("a_mystery");
    expect(tracker.killsByType()).toEqual({ unknown: 1 });
  });

  it("returns zero when no events", () => {
    expect(tracker.totalKills()).toBe(0);
    expect(tracker.namedKills()).toBe(0);
    expect(tracker.killsByType()).toEqual({});
  });
});

describe("MQ2PlatTracker", () => {
  let tracker: MQ2PlatTracker;

  beforeEach(() => {
    tracker = new MQ2PlatTracker({ session_id: "test-plat-1", started_at: new Date().toISOString() });
  });

  it("records plat events", () => {
    tracker.record(50, "loot");
    tracker.record(20, "vendor");
    expect(tracker.totalPlat()).toBe(70);
    expect(tracker.getEvents()).toHaveLength(2);
  });

  it("handles negative amounts (spending)", () => {
    tracker.record(100, "loot");
    tracker.record(-30, "trade");
    expect(tracker.totalPlat()).toBe(70);
  });

  it("computes plat by source", () => {
    tracker.record(50, "loot");
    tracker.record(20, "loot");
    tracker.record(-10, "trade");
    expect(tracker.platBySource()).toEqual({ loot: 70, trade: -10 });
  });

  it("computes plat per hour", () => {
    const started = new Date(Date.now() - 3_600_000).toISOString();
    tracker = new MQ2PlatTracker({ session_id: "test-plat-2", started_at: started });
    tracker.record(300, "loot");
    expect(tracker.platPerHour()).toBeCloseTo(300, 0);
  });

  it("returns zero when no events", () => {
    expect(tracker.totalPlat()).toBe(0);
    expect(tracker.platBySource()).toEqual({});
  });
});

describe("SessionTracker", () => {
  let tracker: SessionTracker;

  beforeEach(() => {
    tracker = new SessionTracker({ session_id: "test-session-1", started_at: new Date().toISOString() });
  });

  it("aggregates all sub-trackers", () => {
    tracker.xp.record(100, 5);
    tracker.kills.record("a_rat", false, "animal");
    tracker.plat.record(50, "loot");

    const s = tracker.summary();
    expect(s.session_id).toBe("test-session-1");
    expect(s.total_xp).toBe(100);
    expect(s.total_kills).toBe(1);
    expect(s.total_plat).toBe(50);
    expect(s.kills_by_type).toEqual({ animal: 1 });
    expect(s.xp_by_level).toEqual({ "5": 100 });
  });

  it("produces a valid summary after time passes", () => {
    const started = new Date(Date.now() - 3_600_000).toISOString();
    tracker = new SessionTracker({ session_id: "test-session-2", started_at: started });
    tracker.xp.record(600, 10);
    tracker.kills.record("a_goblin", false, "humanoid");
    tracker.kills.record("a_goblin", false, "humanoid");
    tracker.plat.record(120, "loot");

    const s = tracker.summary();
    expect(s.xp_per_hour).toBeCloseTo(600, 0);
    expect(s.kills_per_hour).toBeCloseTo(2, 0);
    expect(s.plat_per_hour).toBeCloseTo(120, 0);
    expect(s.duration_ms).toBeGreaterThanOrEqual(3_600_000);
  });

  it("exports and imports summary round-trip", () => {
    tracker.xp.record(50, 3);
    tracker.kills.record("a_wolf", false, "animal");
    tracker.plat.record(25, "loot");

    const s = tracker.summary();
    const json = exportSummary(s);
    const restored = importSummary(json);
    expect(restored.session_id).toBe(s.session_id);
    expect(restored.total_xp).toBe(s.total_xp);
    expect(restored.total_kills).toBe(s.total_kills);
    expect(restored.total_plat).toBe(s.total_plat);
  });

  it("throws on invalid importSummary input", () => {
    expect(() => importSummary("not json")).toThrow();
    expect(() => importSummary("123")).toThrow(TypeError);
  });
});
