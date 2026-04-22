import { useCallback, useEffect, useState } from "react";
import type {
  InventoryUtilityParityConfig,
  AutoClaimRule,
  CollectionRoutingRule,
  CursorRule,
  FoodRule,
  RelocationRule,
  RewardRoutingRule,
} from "../types";

export interface UseInventoryUtilityParityState {
  config: InventoryUtilityParityConfig;
  loading: boolean;
  error: string | null;
  save: (config: InventoryUtilityParityConfig) => Promise<void>;
  saving: boolean;
  addRewardRoute: (rule: RewardRoutingRule) => void;
  removeRewardRoute: (index: number) => void;
  updateRewardRoute: (index: number, rule: RewardRoutingRule) => void;
  addCursorRule: (rule: CursorRule) => void;
  removeCursorRule: (index: number) => void;
  updateCursorRule: (index: number, rule: CursorRule) => void;
  addCollectionRoute: (rule: CollectionRoutingRule) => void;
  removeCollectionRoute: (index: number) => void;
  updateCollectionRoute: (index: number, rule: CollectionRoutingRule) => void;
  addFoodRule: (rule: FoodRule) => void;
  removeFoodRule: (index: number) => void;
  updateFoodRule: (index: number, rule: FoodRule) => void;
  addRelocationRule: (rule: RelocationRule) => void;
  removeRelocationRule: (index: number) => void;
  updateRelocationRule: (index: number, rule: RelocationRule) => void;
  addAutoClaim: (rule: AutoClaimRule) => void;
  removeAutoClaim: (index: number) => void;
  updateAutoClaim: (index: number, rule: AutoClaimRule) => void;
}

const defaultConfig: InventoryUtilityParityConfig = {
  plugin_mappings: [],
  provenance: [],
  reward_routing_rules: [],
  collection_routing_rules: [],
  cursor_rules: [],
  food_rules: [],
  relocation_rules: [],
  auto_claim_rules: [],
};

export function useInventoryUtilityParity(): UseInventoryUtilityParityState {
  const [config, setConfig] = useState<InventoryUtilityParityConfig>(defaultConfig);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    const fetch_config = async () => {
      try {
        const res = await fetch("/api/config/inventory-utility-parity");
        if (!res.ok) {
          throw new Error(`HTTP ${res.status}`);
        }
        const data = (await res.json()) as InventoryUtilityParityConfig;
        setConfig(data);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Unknown error");
      } finally {
        setLoading(false);
      }
    };

    fetch_config();
  }, []);

  const save = useCallback(
    async (newConfig: InventoryUtilityParityConfig) => {
      setSaving(true);
      try {
        const res = await fetch("/api/config/inventory-utility-parity", {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(newConfig),
        });
        if (!res.ok) {
          throw new Error(`HTTP ${res.status}`);
        }
        const saved = (await res.json()) as InventoryUtilityParityConfig;
        setConfig(saved);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Unknown error");
        throw err;
      } finally {
        setSaving(false);
      }
    },
    []
  );

  const addRewardRoute = useCallback((rule: RewardRoutingRule) => {
    setConfig((prev) => ({
      ...prev,
      reward_routing_rules: [...prev.reward_routing_rules, rule],
    }));
  }, []);

  const removeRewardRoute = useCallback((index: number) => {
    setConfig((prev) => ({
      ...prev,
      reward_routing_rules: prev.reward_routing_rules.filter((_, i) => i !== index),
    }));
  }, []);

  const updateRewardRoute = useCallback((index: number, rule: RewardRoutingRule) => {
    setConfig((prev) => ({
      ...prev,
      reward_routing_rules: prev.reward_routing_rules.map((r, i) => (i === index ? rule : r)),
    }));
  }, []);

  const addCursorRule = useCallback((rule: CursorRule) => {
    setConfig((prev) => ({
      ...prev,
      cursor_rules: [...prev.cursor_rules, rule],
    }));
  }, []);

  const removeCursorRule = useCallback((index: number) => {
    setConfig((prev) => ({
      ...prev,
      cursor_rules: prev.cursor_rules.filter((_, i) => i !== index),
    }));
  }, []);

  const updateCursorRule = useCallback((index: number, rule: CursorRule) => {
    setConfig((prev) => ({
      ...prev,
      cursor_rules: prev.cursor_rules.map((r, i) => (i === index ? rule : r)),
    }));
  }, []);

  const addCollectionRoute = useCallback((rule: CollectionRoutingRule) => {
    setConfig((prev) => ({
      ...prev,
      collection_routing_rules: [...prev.collection_routing_rules, rule],
    }));
  }, []);

  const removeCollectionRoute = useCallback((index: number) => {
    setConfig((prev) => ({
      ...prev,
      collection_routing_rules: prev.collection_routing_rules.filter((_, i) => i !== index),
    }));
  }, []);

  const updateCollectionRoute = useCallback((index: number, rule: CollectionRoutingRule) => {
    setConfig((prev) => ({
      ...prev,
      collection_routing_rules: prev.collection_routing_rules.map((r, i) => (i === index ? rule : r)),
    }));
  }, []);

  const addFoodRule = useCallback((rule: FoodRule) => {
    setConfig((prev) => ({
      ...prev,
      food_rules: [...prev.food_rules, rule],
    }));
  }, []);

  const removeFoodRule = useCallback((index: number) => {
    setConfig((prev) => ({
      ...prev,
      food_rules: prev.food_rules.filter((_, i) => i !== index),
    }));
  }, []);

  const updateFoodRule = useCallback((index: number, rule: FoodRule) => {
    setConfig((prev) => ({
      ...prev,
      food_rules: prev.food_rules.map((r, i) => (i === index ? rule : r)),
    }));
  }, []);

  const addRelocationRule = useCallback((rule: RelocationRule) => {
    setConfig((prev) => ({
      ...prev,
      relocation_rules: [...prev.relocation_rules, rule],
    }));
  }, []);

  const removeRelocationRule = useCallback((index: number) => {
    setConfig((prev) => ({
      ...prev,
      relocation_rules: prev.relocation_rules.filter((_, i) => i !== index),
    }));
  }, []);

  const updateRelocationRule = useCallback((index: number, rule: RelocationRule) => {
    setConfig((prev) => ({
      ...prev,
      relocation_rules: prev.relocation_rules.map((r, i) => (i === index ? rule : r)),
    }));
  }, []);

  const addAutoClaim = useCallback((rule: AutoClaimRule) => {
    setConfig((prev) => ({
      ...prev,
      auto_claim_rules: [...prev.auto_claim_rules, rule],
    }));
  }, []);

  const removeAutoClaim = useCallback((index: number) => {
    setConfig((prev) => ({
      ...prev,
      auto_claim_rules: prev.auto_claim_rules.filter((_, i) => i !== index),
    }));
  }, []);

  const updateAutoClaim = useCallback((index: number, rule: AutoClaimRule) => {
    setConfig((prev) => ({
      ...prev,
      auto_claim_rules: prev.auto_claim_rules.map((r, i) => (i === index ? rule : r)),
    }));
  }, []);

  return {
    config,
    loading,
    error,
    save,
    saving,
    addRewardRoute,
    removeRewardRoute,
    updateRewardRoute,
    addCursorRule,
    removeCursorRule,
    updateCursorRule,
    addCollectionRoute,
    removeCollectionRoute,
    updateCollectionRoute,
    addFoodRule,
    removeFoodRule,
    updateFoodRule,
    addRelocationRule,
    removeRelocationRule,
    updateRelocationRule,
    addAutoClaim,
    removeAutoClaim,
    updateAutoClaim,
  };
}
