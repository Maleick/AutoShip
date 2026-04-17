import { useCallback, useEffect, useState } from "react";

import type {
  AutoClaimPreferences,
  CollectionRoute,
  CollectionRoutingRule,
  ConsumablePreferences,
  CursorAction,
  CursorRule,
  InventoryUtilityConfig,
  LegacyAdapterWarning,
  PluginCoverageStatus,
  PluginMapping,
  RelocationRule,
  RewardPreference,
  RewardRoutingRule,
  TrophyPreferences,
  VendorWatchRule,
} from "../types";

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function asStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.filter((entry): entry is string => typeof entry === "string");
}

function asBoolean(value: unknown, fallback = false): boolean {
  return typeof value === "boolean" ? value : fallback;
}

const U32_MAX = 4_294_967_295;

function toNonNegativeSafeIntegerOrNull(
  value: unknown,
  max = Number.MAX_SAFE_INTEGER,
): number | null {
  if (
    typeof value !== "number" ||
    !Number.isFinite(value) ||
    value < 0 ||
    value > max
  ) {
    return null;
  }

  return Math.trunc(value);
}

function asNumberOrNull(value: unknown): number | null {
  return toNonNegativeSafeIntegerOrNull(value, U32_MAX);
}

function asNonNegativeNumber(value: unknown, fallback = 0): number {
  const normalizedValue = toNonNegativeSafeIntegerOrNull(value, U32_MAX);
  const normalizedFallback = toNonNegativeSafeIntegerOrNull(fallback, U32_MAX);

  return normalizedValue ?? normalizedFallback ?? 0;
}

function asPositiveNumber(value: unknown, fallback = 1): number {
  const normalizedValue = toNonNegativeSafeIntegerOrNull(value);
  const normalizedFallback = toNonNegativeSafeIntegerOrNull(fallback);

  if (normalizedValue !== null && normalizedValue >= 1) {
    return normalizedValue;
  }

  if (normalizedFallback !== null && normalizedFallback >= 1) {
    return normalizedFallback;
  }

  return 1;
}

function normalizeStatus(value: unknown): PluginCoverageStatus {
  return value === "native" || value === "adapted" || value === "deferred"
    ? value
    : "adapted";
}

function normalizeCursorAction(value: unknown): CursorAction {
  return value === "keep" || value === "sell" || value === "destroy" || value === "consume"
    ? value
    : "keep";
}

function normalizeCollectionRoute(value: unknown): CollectionRoute {
  return value === "keep" || value === "bank" || value === "tribute" || value === "sell"
    ? value
    : "keep";
}

function normalizePluginMapping(value: unknown): PluginMapping {
  if (!isObjectRecord(value)) {
    return {
      plugin: "",
      owner: "",
      status: "adapted",
      config_surface: "",
      notes: "",
    };
  }

  return {
    plugin: typeof value.plugin === "string" ? value.plugin : "",
    owner: typeof value.owner === "string" ? value.owner : "",
    status: normalizeStatus(value.status),
    config_surface: typeof value.config_surface === "string" ? value.config_surface : "",
    notes: typeof value.notes === "string" ? value.notes : "",
  };
}

function normalizeLegacyAdapter(value: unknown): LegacyAdapterWarning {
  if (!isObjectRecord(value)) {
    return {
      plugin: "",
      source_reference: "",
      adapted_into: "",
      unsupported_fields: [],
    };
  }

  return {
    plugin: typeof value.plugin === "string" ? value.plugin : "",
    source_reference:
      typeof value.source_reference === "string" ? value.source_reference : "",
    adapted_into: typeof value.adapted_into === "string" ? value.adapted_into : "",
    unsupported_fields: asStringArray(value.unsupported_fields),
  };
}

function normalizeCursorRule(value: unknown): CursorRule {
  if (!isObjectRecord(value)) {
    return {
      item_matcher: "",
      action: "keep",
      keep_at_or_below: null,
      overflow_action: null,
    };
  }

  return {
    item_matcher: typeof value.item_matcher === "string" ? value.item_matcher : "",
    action: normalizeCursorAction(value.action),
    keep_at_or_below: asNumberOrNull(value.keep_at_or_below),
    overflow_action:
      value.overflow_action === null || value.overflow_action === undefined
        ? null
        : normalizeCursorAction(value.overflow_action),
  };
}

function normalizeCollectionRoutingRule(value: unknown): CollectionRoutingRule {
  if (!isObjectRecord(value)) {
    return {
      set_matcher: "",
      incomplete_route: "keep",
      completed_route: "bank",
      duplicate_route: "bank",
    };
  }

  return {
    set_matcher: typeof value.set_matcher === "string" ? value.set_matcher : "",
    incomplete_route: normalizeCollectionRoute(value.incomplete_route),
    completed_route: normalizeCollectionRoute(value.completed_route),
    duplicate_route: normalizeCollectionRoute(value.duplicate_route),
  };
}

function normalizeRewardPreference(value: unknown): RewardPreference {
  if (!isObjectRecord(value) || value.kind === "by_position") {
    return {
      kind: "by_position",
      reward_position: isObjectRecord(value)
        ? asPositiveNumber(value.reward_position, 1)
        : 1,
    };
  }

  return {
    kind: "by_name",
    reward_name: typeof value.reward_name === "string" ? value.reward_name : "",
  };
}

function normalizeRewardRoutingRule(value: unknown): RewardRoutingRule {
  if (!isObjectRecord(value)) {
    return {
      task_matcher: "",
      preference: { kind: "by_position", reward_position: 1 },
      auto_claim: false,
    };
  }

  return {
    task_matcher: typeof value.task_matcher === "string" ? value.task_matcher : "",
    preference: normalizeRewardPreference(value.preference),
    auto_claim: asBoolean(value.auto_claim),
  };
}

function normalizeConsumablePreferences(value: unknown): ConsumablePreferences {
  if (!isObjectRecord(value)) {
    return {
      enabled: true,
      preferred_food: [],
      preferred_drink: [],
      ignored_items: [],
    };
  }

  return {
    enabled: asBoolean(value.enabled, true),
    preferred_food: asStringArray(value.preferred_food),
    preferred_drink: asStringArray(value.preferred_drink),
    ignored_items: asStringArray(value.ignored_items),
  };
}

function normalizeVendorWatchRule(value: unknown): VendorWatchRule {
  if (!isObjectRecord(value)) {
    return {
      item_name: "",
      max_price_pp: null,
      notify: true,
    };
  }

  return {
    item_name: typeof value.item_name === "string" ? value.item_name : "",
    max_price_pp: asNumberOrNull(value.max_price_pp),
    notify: asBoolean(value.notify, true),
  };
}

function normalizeRelocationRule(value: unknown): RelocationRule {
  if (!isObjectRecord(value)) {
    return {
      destination: "",
      required_option_id: null,
      keep_on_hand: 1,
      notify_if_unavailable: true,
    };
  }

  return {
    destination: typeof value.destination === "string" ? value.destination : "",
    required_option_id:
      typeof value.required_option_id === "string" ? value.required_option_id : null,
    keep_on_hand: asNonNegativeNumber(value.keep_on_hand, 1),
    notify_if_unavailable: asBoolean(value.notify_if_unavailable, true),
  };
}

function normalizeTrophyPreferences(value: unknown): TrophyPreferences {
  if (!isObjectRecord(value)) {
    return {
      enabled: false,
      auto_equip: true,
      restore_after_craft: true,
      trophy_items: [],
    };
  }

  return {
    enabled: asBoolean(value.enabled),
    auto_equip: asBoolean(value.auto_equip, true),
    restore_after_craft: asBoolean(value.restore_after_craft, true),
    trophy_items: asStringArray(value.trophy_items),
  };
}

function normalizeAutoClaimPreferences(value: unknown): AutoClaimPreferences {
  if (!isObjectRecord(value)) {
    return {
      enabled: false,
      claim_membership_grants: true,
      claim_task_windows: true,
      once_per_session: true,
    };
  }

  return {
    enabled: asBoolean(value.enabled),
    claim_membership_grants: asBoolean(value.claim_membership_grants, true),
    claim_task_windows: asBoolean(value.claim_task_windows, true),
    once_per_session: asBoolean(value.once_per_session, true),
  };
}

export function createDefaultInventoryUtilityConfig(): InventoryUtilityConfig {
  return {
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
  };
}

function normalizeInventoryUtilityConfig(config: unknown): InventoryUtilityConfig {
  if (!isObjectRecord(config)) {
    return createDefaultInventoryUtilityConfig();
  }

  return {
    plugin_mappings: Array.isArray(config.plugin_mappings)
      ? config.plugin_mappings.map(normalizePluginMapping)
      : [],
    legacy_adapters: Array.isArray(config.legacy_adapters)
      ? config.legacy_adapters.map(normalizeLegacyAdapter)
      : [],
    item_knowledge: isObjectRecord(config.item_knowledge)
      ? {
          link_sources: asStringArray(config.item_knowledge.link_sources),
          show_provenance: asBoolean(config.item_knowledge.show_provenance, true),
          show_unsupported_fields: asBoolean(
            config.item_knowledge.show_unsupported_fields,
            true,
          ),
        }
      : createDefaultInventoryUtilityConfig().item_knowledge,
    cursor_rules: Array.isArray(config.cursor_rules)
      ? config.cursor_rules.map(normalizeCursorRule)
      : [],
    collection_routing: Array.isArray(config.collection_routing)
      ? config.collection_routing.map(normalizeCollectionRoutingRule)
      : [],
    reward_routing: Array.isArray(config.reward_routing)
      ? config.reward_routing.map(normalizeRewardRoutingRule)
      : [],
    consumption: normalizeConsumablePreferences(config.consumption),
    vendor_watch: Array.isArray(config.vendor_watch)
      ? config.vendor_watch.map(normalizeVendorWatchRule)
      : [],
    relocation_rules: Array.isArray(config.relocation_rules)
      ? config.relocation_rules.map(normalizeRelocationRule)
      : [],
    trophy_preferences: normalizeTrophyPreferences(config.trophy_preferences),
    auto_claim: normalizeAutoClaimPreferences(config.auto_claim),
  };
}

export function useInventoryUtilityConfig() {
  const [config, setConfig] = useState<InventoryUtilityConfig>(
    createDefaultInventoryUtilityConfig,
  );
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const [loaded, setLoaded] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const response = await fetch("/api/loot/inventory-utility");
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      const payload = normalizeInventoryUtilityConfig(await response.json());
      setConfig(payload);
      setError(null);
      setLoaded(true);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load inventory utility config",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const save = useCallback(async (next: InventoryUtilityConfig): Promise<InventoryUtilityConfig> => {
    setSaving(true);
    try {
      const normalized = normalizeInventoryUtilityConfig(next);
      const response = await fetch("/api/loot/inventory-utility", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(normalized),
      });
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      setConfig(normalized);
      setSavedAt(Date.now());
      setError(null);
      setLoaded(true);
      return normalized;
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Failed to save inventory utility config";
      setError(message);
      throw new Error(message);
    } finally {
      setSaving(false);
    }
  }, []);

  return {
    config,
    loading,
    saving,
    error,
    savedAt,
    loaded,
    refresh,
    save,
  };
}
