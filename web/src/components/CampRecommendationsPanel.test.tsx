import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { CampRecommendationsPanel } from "./CampRecommendationsPanel";
import type { CampConfiguration, CombatSettings, Group } from "../types";

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
    pull_radius: 100,
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

function makeGroup(): Group {
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
  };
}

describe("CampRecommendationsPanel", () => {
  it("renders ranked recommendation cards and exploratory markers", async () => {
    const group = makeGroup();
    const campConfigs = [
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
        pull_targets: [
          { name: "Placeholder", enabled: true },
          { name: "Bonus", enabled: true },
        ],
        safe_zone_markers: [
          { name: "safe", center: { x: 0, y: 0, z: 0 }, radius: 15 },
          { name: "safe-2", center: { x: 1, y: 1, z: 0 }, radius: 12 },
          { name: "safe-3", center: { x: 2, y: 2, z: 0 }, radius: 10 },
        ],
      }),
    ];

    render(
      <CampRecommendationsPanel
        selectedGroup={group}
        campConfigs={campConfigs}
      />,
    );

    expect(screen.getByText(/camp recommendations/i)).toBeInTheDocument();
    expect(await screen.findByText(/high-sigma template/i)).toBeInTheDocument();
    expect(await screen.findByText(/low-sigma template/i)).toBeInTheDocument();
    expect(screen.getByText(/uncharted/i)).toBeInTheDocument();
    expect(screen.getByText(/operator weights/i)).toBeInTheDocument();
    expect(screen.getByText(/current zone/i)).toBeInTheDocument();
  });
});
