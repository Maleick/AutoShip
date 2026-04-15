import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import TuningPanel from "./TuningPanel";

vi.mock("../hooks/useTuning", () => ({
  useCharacterConfigs: () => ({
    configs: [
      {
        character_name: "Frostreaver",
        class: "Cleric",
        role: "Healer",
        heal_at_pct: 70,
        mana_sit_pct: 30,
        nuke_at_pct: 95,
        rotation: [],
        class_params: {
          ch_chain_timing_ms: 2500,
          cross_client_heal_enabled: true,
          cross_client_heal_threshold_pct: 85,
          cross_client_heal_priority: 10,
          cross_client_claim_timeout_ms: 3000,
        },
        auto_rez: {
          enabled: false,
          min_xp_pct: 90,
          trusted_casters: [],
          decline_if_untrusted: false,
          delay_ms: 0,
        },
        group_override: false,
      },
    ],
    loading: false,
    error: null,
    saveConfig: vi.fn(),
  }),
}));

describe("TuningPanel", () => {
  it("shows cross-client heal controls for healer classes", () => {
    render(<TuningPanel />);

    expect(screen.getByText("Cross-Client Heal")).toBeInTheDocument();
    expect(screen.getByText("Claim Timeout (ms)")).toBeInTheDocument();
    expect(screen.getByText("Response Priority")).toBeInTheDocument();
    expect(screen.getByText("Cross-Client Heal At %")).toBeInTheDocument();
  });
});
