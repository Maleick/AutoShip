import { useState, useEffect, useCallback } from "react";
import type { PlayerWatchConfig, PlayerFilterMode } from "../types";

type PlayerWatchApiConfig = {
  filter_mode?: PlayerFilterMode;
  sound_on_zone_in?: boolean;
  friends?: string[];
};

function normalizeConfig(config: PlayerWatchApiConfig): PlayerWatchConfig {
  return {
    filter_mode: config.filter_mode ?? "all",
    sound_on_zone_in: config.sound_on_zone_in ?? false,
    friends: config.friends ?? [],
  };
}

function toApiConfig(config: Partial<PlayerWatchConfig>): PlayerWatchApiConfig {
  return {
    filter_mode: config.filter_mode,
    sound_on_zone_in: config.sound_on_zone_in,
    friends: config.friends,
  };
}

export function usePlayerWatchConfig() {
  const [config, setConfig] = useState<PlayerWatchConfig>({
    filter_mode: "all",
    sound_on_zone_in: false,
    friends: [],
  });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchConfig = useCallback(async () => {
    try {
      const res = await fetch("/api/config/player-watch");
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data: PlayerWatchApiConfig = await res.json();
      setConfig(normalizeConfig(data));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to fetch player watch config");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchConfig();
  }, [fetchConfig]);

  const saveConfig = useCallback(async (newConfig: Partial<PlayerWatchConfig>) => {
    const res = await fetch("/api/config/player-watch", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(toApiConfig(newConfig)),
    });
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      throw new Error(body.error ?? `HTTP ${res.status}`);
    }
    const updated: PlayerWatchApiConfig = await res.json();
    setConfig(normalizeConfig(updated));
  }, []);

  const setFilterMode = useCallback(
    async (filter_mode: PlayerFilterMode) => {
      await saveConfig({ filter_mode });
    },
    [saveConfig]
  );

  const setSoundOnZoneIn = useCallback(
    async (sound_on_zone_in: boolean) => {
      await saveConfig({ sound_on_zone_in });
    },
    [saveConfig]
  );

  const addFriend = useCallback(
    async (name: string) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      await saveConfig({
        friends: [...config.friends, trimmed],
      });
    },
    [saveConfig, config.friends]
  );

  const removeFriend = useCallback(
    async (name: string) => {
      await saveConfig({
        friends: config.friends.filter((f) => f !== name),
      });
    },
    [saveConfig, config.friends]
  );

  return {
    config,
    loading,
    error,
    refresh: fetchConfig,
    setFilterMode,
    setSoundOnZoneIn,
    addFriend,
    removeFriend,
    saveConfig,
  };
}
