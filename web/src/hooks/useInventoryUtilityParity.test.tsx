import { renderHook, waitFor, act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useInventoryUtilityParity } from "./useInventoryUtilityParity";
import { jsonResponse } from "../test/http";

describe("useInventoryUtilityParity", () => {
  beforeEach(() => {
    vi.stubGlobal("fetch", vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("loads plugin mappings and provenance warnings", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        plugin_mappings: [
          {
            plugin_name: "MQ2LinkDB",
            owner: "loot.item_knowledge",
            support_level: "adapted",
            native_surface: "Inventory Utility Parity",
            notes: "Legacy link database imports",
          },
        ],
        provenance: [
          {
            plugin_name: "MQ2LinkDB",
            support_level: "adapted",
            source: "legacy/linkdb.ini",
            imported_records: 42,
            unsupported_fields: ["LinkBotChannel"],
          },
        ],
        reward_routing_rules: [],
        collection_routing_rules: [],
        cursor_rules: [],
        food_rules: [],
        relocation_rules: [],
        auto_claim_rules: [],
      }),
    );

    const { result } = renderHook(() => useInventoryUtilityParity());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.config.plugin_mappings).toHaveLength(1);
    expect(result.current.config.provenance[0].unsupported_fields).toEqual([
      "LinkBotChannel",
    ]);
  });

  it("saves edited cursor and reward routing rules", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock
      .mockResolvedValueOnce(
        jsonResponse({
          plugin_mappings: [],
          provenance: [],
          reward_routing_rules: [],
          collection_routing_rules: [],
          cursor_rules: [],
          food_rules: [],
          relocation_rules: [],
          auto_claim_rules: [],
        }),
      )
      .mockResolvedValueOnce(
        jsonResponse({
          plugin_mappings: [],
          provenance: [],
          reward_routing_rules: [
            {
              task_matcher: "*",
              reward_name: "Ancient Coin",
              target: "bank",
            },
          ],
          collection_routing_rules: [],
          cursor_rules: [
            {
              item_name: "Water Flask",
              max_quantity: 4,
              overflow_action: "consume",
            },
          ],
          food_rules: [],
          relocation_rules: [],
          auto_claim_rules: [],
        }),
      );

    const { result } = renderHook(() => useInventoryUtilityParity());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.save({
        ...result.current.config,
        reward_routing_rules: [
          {
            task_matcher: "*",
            reward_name: "Ancient Coin",
            target: "bank",
          },
        ],
        cursor_rules: [
          {
            item_name: "Water Flask",
            max_quantity: 4,
            overflow_action: "consume",
          },
        ],
      });
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/config/inventory-utility-parity",
      expect.objectContaining({ method: "PUT" }),
    );
    const [, init] = fetchMock.mock.calls[1]!;
    expect(JSON.parse(String(init?.body))).toMatchObject({
      reward_routing_rules: [
        {
          task_matcher: "*",
          reward_name: "Ancient Coin",
          target: "bank",
        },
      ],
      cursor_rules: [
        {
          item_name: "Water Flask",
          max_quantity: 4,
          overflow_action: "consume",
        },
      ],
    });
  });
});
