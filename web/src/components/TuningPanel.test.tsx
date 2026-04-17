import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import TuningPanel from "./TuningPanel";
import { jsonResponse } from "../test/http";

describe("TuningPanel", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders resurrection offer controls for the selected character", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(
      jsonResponse([
        {
          character_name: "Frostreaver",
          class: "Cleric",
          role: "Healer",
          heal_at_pct: 70,
          mana_sit_pct: 20,
          nuke_at_pct: 90,
          rotation: [],
          class_params: {},
          auto_rez: {
            enabled: true,
            min_xp_pct: 96,
            trusted_casters: ["Highclerk", "Leafbinder"],
            decline_if_untrusted: true,
            delay_ms: 5100,
          },
          group_override: false,
        },
      ])
    );

    render(<TuningPanel />);

    await waitFor(() =>
      expect(screen.getByText("Resurrection Offers")).toBeInTheDocument()
    );

    expect(screen.getByText("Minimum Rez XP %")).toBeInTheDocument();
    expect(screen.getByText("Trusted Casters")).toBeInTheDocument();
    expect(screen.getByText("Delay Before Action")).toBeInTheDocument();
  });

  it("renders tribute status and saves updated tribute preferences", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse([
          {
            character_name: "Alpha",
            class: "Cleric",
            role: "Healer",
            heal_at_pct: 70,
            mana_sit_pct: 25,
            nuke_at_pct: 90,
            rotation: [],
            class_params: {},
            group_override: false,
            tribute_preferences: {
              auto_activate: true,
              warning_threshold_secs: 300,
              preferred_tributes: ["Marr's Gift", "Champion's Aura"],
            },
            tribute_status: {
              active: true,
              remaining_secs: 240,
              point_balance: 3200,
              active_tributes: ["Marr's Gift"],
              alert_state: "expiring",
            },
          },
        ])
      )
      .mockResolvedValueOnce(jsonResponse({ updated: true }))
      .mockResolvedValueOnce(
        jsonResponse([
          {
            character_name: "Alpha",
            class: "Cleric",
            role: "Healer",
            heal_at_pct: 70,
            mana_sit_pct: 25,
            nuke_at_pct: 90,
            rotation: [],
            class_params: {},
            group_override: false,
            tribute_preferences: {
              auto_activate: true,
              warning_threshold_secs: 180,
              preferred_tributes: [
                "Marr's Gift",
                "Champion's Aura",
                "Hero's Fortitude",
              ],
            },
            tribute_status: {
              active: true,
              remaining_secs: 240,
              point_balance: 3200,
              active_tributes: ["Marr's Gift"],
              alert_state: "expiring",
            },
          },
        ])
      );

    render(<TuningPanel />);

    await waitFor(() => expect(screen.getAllByText("Alpha")).toHaveLength(2));
    await screen.findByText("Tribute Automation");
    expect(screen.getByText(/3,200|3200/)).toBeInTheDocument();
    expect(screen.getByText(/Expiring/i)).toBeInTheDocument();
    expect(screen.getByDisplayValue("Marr's Gift, Champion's Aura")).toBeInTheDocument();

    await act(async () => {
      fireEvent.change(screen.getByLabelText(/Warning Lead Time/i), {
        target: { value: "180" },
      });
      fireEvent.change(screen.getByLabelText(/Preferred Tributes/i), {
        target: {
          value: "Marr's Gift, Champion's Aura, Hero's Fortitude",
        },
      });
    });

    await waitFor(() =>
      expect(screen.getByLabelText(/Warning Lead Time/i)).toHaveValue(180),
    );
    expect(screen.getByLabelText(/Preferred Tributes/i)).toHaveValue(
      "Marr's Gift, Champion's Aura, Hero's Fortitude",
    );

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Commit/i }));
    });

    await waitFor(() =>
      expect(fetchMock).toHaveBeenNthCalledWith(
        2,
        "/api/config/characters/Alpha",
        expect.objectContaining({ method: "PUT" }),
      ),
    );

    const saveCall = fetchMock.mock.calls[1];
    const body = JSON.parse(String(saveCall[1]?.body));
    expect(body.tribute_status).toBeUndefined();
    expect(body.tribute_preferences.warning_threshold_secs).toBe(180);
    expect(body.tribute_preferences.preferred_tributes).toEqual([
      "Marr's Gift",
      "Champion's Aura",
      "Hero's Fortitude",
    ]);
  });

  it("renders and saves the per-character window title format", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse([
          {
            character_name: "Alpha",
            class: "Wizard",
            role: "DPS",
            heal_at_pct: 70,
            mana_sit_pct: 25,
            nuke_at_pct: 90,
            rotation: [],
            class_params: {},
            auto_rez: {
              enabled: false,
              min_xp_pct: 90,
              trusted_casters: [],
              decline_if_untrusted: false,
              delay_ms: 3000,
            },
            group_override: false,
            window_title_format: "[{server}] {character}",
            tribute_preferences: {
              auto_activate: true,
              warning_threshold_secs: 300,
              preferred_tributes: [],
            },
            tribute_status: {
              active: false,
              remaining_secs: 0,
              point_balance: 0,
              active_tributes: [],
              alert_state: "expired",
            },
          },
        ]),
      )
      .mockResolvedValueOnce(jsonResponse({ updated: true }))
      .mockResolvedValueOnce(
        jsonResponse([
          {
            character_name: "Alpha",
            class: "Wizard",
            role: "DPS",
            heal_at_pct: 70,
            mana_sit_pct: 25,
            nuke_at_pct: 90,
            rotation: [],
            class_params: {},
            auto_rez: {
              enabled: false,
              min_xp_pct: 90,
              trusted_casters: [],
              decline_if_untrusted: false,
              delay_ms: 3000,
            },
            group_override: false,
            window_title_format: "[{server}] {character} ({level} {class_short})",
            tribute_preferences: {
              auto_activate: true,
              warning_threshold_secs: 300,
              preferred_tributes: [],
            },
            tribute_status: {
              active: false,
              remaining_secs: 0,
              point_balance: 0,
              active_tributes: [],
              alert_state: "expired",
            },
          },
        ]),
      );

    render(<TuningPanel />);

    await waitFor(() =>
      expect(screen.getByLabelText(/Window Title Format/i)).toHaveValue(
        "[{server}] {character}",
      ),
    );
    const input = screen.getByLabelText(/Window Title Format/i);

    await act(async () => {
      fireEvent.change(input, {
        target: {
          value: "[{server}] {character} ({level} {class_short})",
        },
      });
    });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Commit/i }));
    });

    const saveCall = fetchMock.mock.calls[1];
    const body = JSON.parse(String(saveCall[1]?.body));
    expect(body.window_title_format).toBe(
      "[{server}] {character} ({level} {class_short})",
    );
  });
});
