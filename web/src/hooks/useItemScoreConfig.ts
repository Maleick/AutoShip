import { useCallback, useEffect, useState } from "react";

import type { ItemScoreConfig, StatWeights } from "../types";

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function createDefaultItemScoreConfig(): ItemScoreConfig {
  return {
    min_upgrade_delta: 0,
    class_weights: {},
  };
}

function normalizeWeights(value: unknown): StatWeights {
  if (!isObjectRecord(value)) {
    return {};
  }

  const weights: StatWeights = {};
  for (const [stat, raw] of Object.entries(value)) {
    if (typeof raw === "number" && Number.isFinite(raw)) {
      weights[stat] = raw;
    }
  }
  return weights;
}

function normalizeItemScoreConfig(config: unknown): ItemScoreConfig {
  if (!isObjectRecord(config)) {
    return createDefaultItemScoreConfig();
  }

  const min_upgrade_delta =
    typeof config.min_upgrade_delta === "number" && Number.isFinite(config.min_upgrade_delta)
      ? config.min_upgrade_delta
      : 0;

  const classWeights = isObjectRecord(config.class_weights) ? config.class_weights : {};

  return {
    min_upgrade_delta,
    class_weights: Object.fromEntries(
      Object.entries(classWeights).map(([className, weights]) => [className, normalizeWeights(weights)]),
    ),
  };
}

export function useItemScoreConfig() {
  const [config, setConfig] = useState<ItemScoreConfig>(createDefaultItemScoreConfig);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const [loaded, setLoaded] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const response = await fetch("/api/loot/item-score");
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      const payload = normalizeItemScoreConfig(await response.json());
      setConfig(payload);
      setError(null);
      setLoaded(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load item score config");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const save = useCallback(async (next: ItemScoreConfig): Promise<ItemScoreConfig> => {
    setSaving(true);
    try {
      const normalized = normalizeItemScoreConfig(next);
      const response = await fetch("/api/loot/item-score", {
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
        err instanceof Error ? err.message : "Failed to save item score config";
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
