import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import LootConfig from "./LootConfig";
import { jsonResponse } from "../test/http";

describe("LootConfig inventory utility tab", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);

      if (url === "/api/loot/item-score") {
        if (init?.method === "PUT") {
          return Promise.resolve(new Response(null, { status: 204 }));
        }

        return Promise.resolve(
          jsonResponse({
            min_upgrade_delta: 1.25,
            class_weights: {
              Warrior: { STR: 1.2, AC: 0.9 },
            },
          }),
        );
      }

      if (url === "/api/loot/inventory-utility") {
        if (init?.method === "PUT") {
          return Promise.resolve(new Response(null, { status: 204 }));
        }

        return Promise.resolve(
          jsonResponse({
            plugin_mappings: [
              {
                plugin: "MQ2LinkDB",
                owner: "inventory_utility.item_knowledge",
                status: "adapted",
                config_surface: "Loot Config > Inventory Utilities",
                notes: "Uses provenance-aware item knowledge.",
              },
            ],
            legacy_adapters: [
              {
                plugin: "MQ2LinkDB",
                source_reference: "legacy_ini:itemdb",
                adapted_into: "item_knowledge",
                unsupported_fields: ["custom link color formatting"],
              },
            ],
            item_knowledge: {
              link_sources: ["item-db", "loot-history"],
              show_provenance: true,
              show_unsupported_fields: true,
            },
            cursor_rules: [
              {
                item_matcher: "Fish Roll",
                action: "consume",
                keep_at_or_below: 40,
                overflow_action: "sell",
              },
            ],
            collection_routing: [],
            reward_routing: [
              {
                task_matcher: "heroic-adventure:*",
                preference: {
                  kind: "by_position",
                  reward_position: 1,
                },
                auto_claim: true,
              },
            ],
            consumption: {
              enabled: true,
              preferred_food: ["Fish Roll"],
              preferred_drink: ["Water Flask"],
              ignored_items: [],
            },
            vendor_watch: [],
            relocation_rules: [],
            trophy_preferences: {
              enabled: false,
              auto_equip: true,
              restore_after_craft: true,
              trophy_items: [],
            },
            auto_claim: {
              enabled: false,
              claim_membership_grants: true,
              claim_task_windows: true,
              once_per_session: true,
            },
          }),
        );
      }

      return Promise.reject(new Error(`Unexpected fetch ${url}`));
    }));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("renders provenance warnings, removes reward rules, and saves inventory utility changes", async () => {
    const fetchMock = vi.mocked(fetch);

    render(<LootConfig />);

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith("/api/loot/inventory-utility"),
    );

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Inventory Utilities/i }));
    });

    expect(await screen.findByText("Plugin Coverage")).toBeInTheDocument();
    expect(screen.getAllByText("MQ2LinkDB")).toHaveLength(2);
    expect(screen.getByText(/custom link color formatting/i)).toBeInTheDocument();

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Remove reward rule 1/i }));
    });

    await act(async () => {
      fireEvent.click(screen.getByLabelText(/Enable Auto-Claim/i));
    });

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Save Inventory Utility/i }));
    });

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/loot/inventory-utility",
        expect.objectContaining({
          method: "PUT",
          headers: { "Content-Type": "application/json" },
        }),
      ),
    );

    const saveCall = fetchMock.mock.calls.find(
      ([url, init]) =>
        url === "/api/loot/inventory-utility" && init?.method === "PUT",
    );
    expect(saveCall).toBeDefined();
    const body = JSON.parse(String(saveCall?.[1]?.body));
    expect(body.auto_claim.enabled).toBe(true);
    expect(body.item_knowledge.show_provenance).toBe(true);
    expect(body.reward_routing).toEqual([]);
  });
});
