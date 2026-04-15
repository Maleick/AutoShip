import { useState, useEffect, useCallback } from "react";
import type { CharacterConfig } from "../types";

function normalizeConfig(config: CharacterConfig): CharacterConfig {
  return {
    ...config,
    auto_camp_on_death: config.auto_camp_on_death ?? {
      enabled: false,
      camp_delay_secs: 30,
      relog_wait_secs: 900,
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
      const res = await fetch(
        `/api/config/characters/${encodeURIComponent(config.character_name)}`,
        {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(config),
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
