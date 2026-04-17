import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useInventoryUtilityConfig } from "./useInventoryUtilityConfig";

const fetchMock = vi.fn();

describe("useInventoryUtilityConfig", () => {
  beforeEach(() => {
    fetchMock.mockReset();
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("loads inventory utility config on mount", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
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
            link_sources: ["item-db"],
            show_provenance: true,
            show_unsupported_fields: true,
          },
          cursor_rules: [],
          collection_routing: [],
          reward_routing: [],
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
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );

    const { result } = renderHook(() => useInventoryUtilityConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.config.plugin_mappings[0]?.plugin).toBe("MQ2LinkDB");
    expect(result.current.config.item_knowledge.show_provenance).toBe(true);
    expect(result.current.error).toBeNull();
    expect(result.current.loaded).toBe(true);
  });

  it("saves updated inventory utility config", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          plugin_mappings: [],
          legacy_adapters: [],
          item_knowledge: {
            link_sources: [],
            show_provenance: true,
            show_unsupported_fields: true,
          },
          cursor_rules: [],
          collection_routing: [],
          reward_routing: [],
          consumption: {
            enabled: true,
            preferred_food: [],
            preferred_drink: [],
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
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );
    fetchMock.mockResolvedValueOnce(new Response(null, { status: 204 }));

    const { result } = renderHook(() => useInventoryUtilityConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.save({
        ...result.current.config,
        vendor_watch: [{ item_name: "Jacinth", max_price_pp: 450, notify: true }],
        auto_claim: {
          ...result.current.config.auto_claim,
          enabled: true,
        },
      });
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/loot/inventory-utility",
      expect.objectContaining({
        method: "PUT",
        headers: { "Content-Type": "application/json" },
      }),
    );
    expect(result.current.config.auto_claim.enabled).toBe(true);
    expect(result.current.config.vendor_watch[0]?.item_name).toBe("Jacinth");
    expect(result.current.savedAt).not.toBeNull();
    expect(result.current.loaded).toBe(true);
  });

  it("normalizes unsigned integer fields before saving", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          plugin_mappings: [],
          legacy_adapters: [],
          item_knowledge: {
            link_sources: [],
            show_provenance: true,
            show_unsupported_fields: true,
          },
          cursor_rules: [],
          collection_routing: [],
          reward_routing: [],
          consumption: {
            enabled: true,
            preferred_food: [],
            preferred_drink: [],
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
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );
    fetchMock.mockResolvedValueOnce(new Response(null, { status: 204 }));

    const { result } = renderHook(() => useInventoryUtilityConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.save({
        ...result.current.config,
        cursor_rules: [
          {
            item_matcher: "Fish Roll",
            action: "keep",
            keep_at_or_below: -1.5,
            overflow_action: "sell",
          },
        ],
        reward_routing: [
          {
            task_matcher: "Hero's Mission",
            preference: { kind: "by_position", reward_position: 0 },
            auto_claim: true,
          },
        ],
        vendor_watch: [{ item_name: "Jacinth", max_price_pp: 12.75, notify: true }],
        relocation_rules: [
          {
            destination: "guildlobby",
            required_option_id: null,
            keep_on_hand: -2.4,
            notify_if_unavailable: true,
          },
        ],
      });
    });

    const saveCall = fetchMock.mock.calls.find(
      ([url, init]) =>
        url === "/api/loot/inventory-utility" && init?.method === "PUT",
    );
    expect(saveCall).toBeDefined();

    const body = JSON.parse(String(saveCall?.[1]?.body));
    expect(body.cursor_rules[0].keep_at_or_below).toBeNull();
    expect(body.reward_routing[0].preference.reward_position).toBe(1);
    expect(body.vendor_watch[0].max_price_pp).toBe(12);
    expect(body.relocation_rules[0].keep_on_hand).toBe(1);
  });

  it("rejects values above the backend u32 range before saving", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          plugin_mappings: [],
          legacy_adapters: [],
          item_knowledge: {
            link_sources: [],
            show_provenance: true,
            show_unsupported_fields: true,
          },
          cursor_rules: [],
          collection_routing: [],
          reward_routing: [],
          consumption: {
            enabled: true,
            preferred_food: [],
            preferred_drink: [],
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
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );
    fetchMock.mockResolvedValueOnce(new Response(null, { status: 204 }));

    const { result } = renderHook(() => useInventoryUtilityConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.save({
        ...result.current.config,
        cursor_rules: [
          {
            item_matcher: "Fish Roll",
            action: "keep",
            keep_at_or_below: 5_000_000_000,
            overflow_action: "sell",
          },
        ],
        vendor_watch: [
          { item_name: "Jacinth", max_price_pp: 4_294_967_296, notify: true },
        ],
        relocation_rules: [
          {
            destination: "guildlobby",
            required_option_id: null,
            keep_on_hand: 4_294_967_296,
            notify_if_unavailable: true,
          },
        ],
      });
    });

    const saveCall = fetchMock.mock.calls.find(
      ([url, init]) =>
        url === "/api/loot/inventory-utility" && init?.method === "PUT",
    );
    expect(saveCall).toBeDefined();

    const body = JSON.parse(String(saveCall?.[1]?.body));
    expect(body.cursor_rules[0].keep_at_or_below).toBeNull();
    expect(body.vendor_watch[0].max_price_pp).toBeNull();
    expect(body.relocation_rules[0].keep_on_hand).toBe(1);
  });

  it("does not mark the config as loaded after the initial fetch fails", async () => {
    fetchMock.mockResolvedValueOnce(new Response(null, { status: 500 }));

    const { result } = renderHook(() => useInventoryUtilityConfig());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.loaded).toBe(false);
    expect(result.current.error).toBe("HTTP 500");
  });
});
