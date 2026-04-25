import { describe, expect, it } from "vitest";

import type {
  CampConfiguration,
  CombatSettings,
  Goal,
  Group,
  RecommendRequest,
} from "../types";
import {
  DEFAULT_OBJECTIVE_WEIGHTS,
  prepareRecommendations,
  recommend,
  rerankRecommendations,
  type RandomSource,
} from "./camp-recommender";

function makeCamp(
  id: string,
  overrides: Partial<CampConfiguration> = {},
): CampConfiguration {
  return {
    id,
    group_id: `${id}-group`,
    template_name: `${id} template`,
    camp_zone: id,
    camp_center: { x: 0, y: 0, z: 0 },
    pull_radius: 80,
    pull_points: [],
    pull_targets: [{ name: "Placeholder", enabled: true }],
    safe_zone_markers: [],
    combat_settings: {
      hp_buff_threshold_pct: 60,
      mana_buff_threshold_pct: 40,
      pull_strategy: "balanced",
    } satisfies CombatSettings,
    created_at: "2026-04-25T00:00:00Z",
    updated_at: "2026-04-25T00:00:00Z",
    ...overrides,
  };
}

function makeGroup(overrides: Partial<Group> = {}): Group {
  return {
    id: "group-1",
    name: "Group One",
    zone: "crushbone",
    members: [
      {
        character_name: "Frostreaver",
        class: "Warrior",
        role: "main_tank",
        order: 1,
        level: 50,
      },
      {
        character_name: "Aelrindel",
        class: "Cleric",
        role: "healer",
        order: 2,
        level: 49,
      },
    ],
    created_at: "2026-04-25T00:00:00Z",
    updated_at: "2026-04-25T00:00:00Z",
    ...overrides,
  };
}

function makeRequest(
  overrides: Partial<RecommendRequest> = {},
): RecommendRequest {
  const group = makeGroup();
  return {
    party: {
      members: group.members.map((member) => ({
        name: member.character_name,
        class: member.class,
        role: member.role,
        level: member.level,
      })),
    },
    current_zone: "crushbone",
    goal: { kind: "xp" } satisfies Goal,
    time_budget_min: 30,
    weights: DEFAULT_OBJECTIVE_WEIGHTS,
    min_confidence: 0.45,
    ...overrides,
  };
}

function makeRng(sequence: number[]): RandomSource {
  let index = 0;
  return () => {
    const value = sequence[index % sequence.length];
    index += 1;
    return value;
  };
}

describe("camp recommender", () => {
  it("drops dominated camps from the Pareto front", () => {
    const request = makeRequest();
    const camps = [
      makeCamp("strong", {
        pull_radius: 60,
        safe_zone_markers: [{ name: "safe", center: { x: 1, y: 1, z: 0 }, radius: 20 }],
      }),
      makeCamp("dominated", {
        pull_radius: 180,
        pull_targets: [],
        combat_settings: {
          hp_buff_threshold_pct: 50,
          mana_buff_threshold_pct: 30,
          pull_strategy: "caster",
        },
      }),
      makeCamp("different", {
        pull_radius: 30,
        pull_targets: [
          { name: "A", enabled: true },
          { name: "B", enabled: true },
        ],
        safe_zone_markers: [
          { name: "safe", center: { x: 1, y: 1, z: 0 }, radius: 20 },
        ],
      }),
    ];

    const run = prepareRecommendations(request, camps, {
      topN: 3,
      explorePct: 0,
      seed: "pareto",
    });

    expect(run.prepared.map((item) => item.camp_id)).not.toContain("dominated");
    expect(run.rejected.some((entry) => entry.camp_id === "dominated")).toBe(true);
    expect(
      run.rejected.find((entry) => entry.camp_id === "dominated")?.reason,
    ).toMatch(/dominated by/i);
  });

  it("reranks the prepared pool when weights change", () => {
    const request = makeRequest({
      weights: { xp: 0.4, plat: 0.2, upgrades: 0.2, safety: 0.2 },
    });
    const camps = [
      makeCamp("xp", {
        pull_radius: 150,
        pull_targets: [
          { name: "A", enabled: true },
          { name: "B", enabled: true },
        ],
      }),
      makeCamp("safety", {
        pull_radius: 40,
        safe_zone_markers: [
          { name: "safe", center: { x: 0, y: 0, z: 0 }, radius: 25 },
          { name: "safe-2", center: { x: 5, y: 5, z: 0 }, radius: 20 },
        ],
      }),
    ];

    const run = prepareRecommendations(request, camps, {
      topN: 2,
      explorePct: 0,
      seed: "rerank",
    });

    const xpRank = rerankRecommendations(run.prepared, {
      xp: 0.8,
      plat: 0.05,
      upgrades: 0.05,
      safety: 0.1,
    });
    const safetyRank = rerankRecommendations(run.prepared, {
      xp: 0.1,
      plat: 0.1,
      upgrades: 0.1,
      safety: 0.7,
    });

    expect(xpRank[0].camp_id).not.toBe(safetyRank[0].camp_id);
    expect(run.prepared).toHaveLength(2);
  });

  it("selects high-sigma camps for the exploratory slice", () => {
    const request = makeRequest({
      weights: { xp: 0.25, plat: 0.25, upgrades: 0.25, safety: 0.25 },
      min_confidence: 0.2,
    });
    const camps = [
      makeCamp("high-sigma", {
        pull_radius: 280,
        combat_settings: {
          hp_buff_threshold_pct: 55,
          mana_buff_threshold_pct: 25,
          pull_strategy: "caster",
        },
      }),
      makeCamp("low-sigma", {
        pull_radius: 40,
        safe_zone_markers: [
          { name: "safe", center: { x: 0, y: 0, z: 0 }, radius: 15 },
        ],
      }),
    ];

    const run = prepareRecommendations(request, camps, {
      topN: 2,
      explorePct: 0.5,
      seed: "thompson",
      rng: makeRng([0.25, 0, 0.25, 0]),
    });

    const exploratory = run.prepared.find((item) => item.exploratory);
    expect(exploratory?.camp_id).toBe("high-sigma");
    expect(exploratory?.score.thompson).toBeGreaterThan(0);
  });

  it("rejects camps that miss hard constraints with a trace reason", () => {
    const request = makeRequest({
      time_budget_min: 4,
      min_confidence: 0.1,
    });
    const camps = [makeCamp("too-far", { pull_radius: 220 })];

    const run = prepareRecommendations(request, camps, {
      topN: 1,
      explorePct: 0,
      seed: "constraints",
    });

    expect(run.prepared).toHaveLength(0);
    expect(
      run.rejected.find((entry) => entry.camp_id === "too-far")?.reason,
    ).toMatch(/short travel/i);
  });

  it("produces a rationale that cites telemetry, upgrades, risk, or travel", () => {
    const request = makeRequest({
      goal: { kind: "item", slot: "chest", character: "Frostreaver" },
      min_confidence: 0.2,
    });
    const result = recommend(request, [makeCamp("detail", { pull_radius: 50 })], {
      topN: 1,
      explorePct: 0,
      seed: "rationale",
    });

    expect(result.recommendations[0].rationale).toMatch(
      /telemetry advantage|upgrade slot|low risk|short travel/i,
    );
  });
});
