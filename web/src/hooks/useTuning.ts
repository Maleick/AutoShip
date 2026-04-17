import { useState, useEffect, useCallback } from "react";
import type { CharacterConfig } from "../types";

const DEFAULT_AUTO_CAMP_ON_DEATH = {
  enabled: false,
  camp_delay_secs: 30,
  relog_wait_secs: 900,
};

const DEFAULT_AUTO_REZ_CONFIG = {
  enabled: false,
  min_xp_pct: 90,
  trusted_casters: [],
  decline_if_untrusted: false,
  delay_ms: 3000,
};

const DEFAULT_TRIBUTE_PREFERENCES = {
  auto_activate: false,
  warning_threshold_secs: 300,
  preferred_tributes: [],
};

const DEFAULT_TRIBUTE_STATUS = {
  active: false,
  remaining_secs: 0,
  point_balance: 0,
  active_tributes: [],
  alert_state: "expired" as const,
};

const DEFAULT_WINDOW_TITLE_FORMAT =
  "[{server}] {character} ({level} {class_short})";

function normalizeConfig(config: CharacterConfig): CharacterConfig {
  return {
    ...config,
    auto_rez: {
      ...DEFAULT_AUTO_REZ_CONFIG,
      ...config.auto_rez,
      trusted_casters: config.auto_rez?.trusted_casters ?? [],
    },
    auto_camp_on_death: {
      ...DEFAULT_AUTO_CAMP_ON_DEATH,
      ...config.auto_camp_on_death,
    },
    reward_automation: {
      rules: config.reward_automation?.rules ?? [],
    },
    window_title_format:
      config.window_title_format?.trim() || DEFAULT_WINDOW_TITLE_FORMAT,
    tribute_preferences: {
      ...DEFAULT_TRIBUTE_PREFERENCES,
      ...config.tribute_preferences,
      preferred_tributes: config.tribute_preferences?.preferred_tributes ?? [],
    },
    tribute_status: {
      ...DEFAULT_TRIBUTE_STATUS,
      ...config.tribute_status,
      active_tributes: config.tribute_status?.active_tributes ?? [],
    },
  };
}

/** Fetch the full list of character configs and expose a save function. */
export function useCharacterConfigs() {
  const [configs, setConfigs] = useState<CharacterConfig[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchConfigs = useCallback(async () => {
    try {
      const res = await fetch("/api/config/characters");
      if (res.status === 404 || res.status === 501) {
        setError("Character config backend unavailable (demo mode)");
        return;
      }
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data: CharacterConfig[] = await res.json();
      setConfigs(data.map(normalizeConfig));
      setError(null);
    } catch (e) {
      setError(
        e instanceof Error
          ? `Network error fetching character configs: ${e.message}`
          : "Failed to fetch character configs",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchConfigs();
  }, [fetchConfigs]);

  const saveConfig = useCallback(
    async (config: CharacterConfig): Promise<void> => {
      const normalized = normalizeConfig(config);
      const { tribute_status: _tributeStatus, ...updatePayload } = normalized;
      const res = await fetch(
        `/api/config/characters/${encodeURIComponent(config.character_name)}`,
        {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(updatePayload),
        },
      );
      if (res.status === 404 || res.status === 501) {
        setError("Character config backend unavailable (demo mode)");
        return;
      }
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      await fetchConfigs();
    },
    [fetchConfigs],
  );

  return { configs, loading, error, saveConfig };
}
