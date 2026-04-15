import { useState } from "react";
import { usePlayerWatchConfig } from "../hooks/usePlayerWatchConfig";
import type { PlayerFilterMode } from "../types";

export default function PlayerWatchPanel() {
  const {
    config,
    loading,
    error,
    setFilterMode,
    setSoundOnZoneIn,
    addFriend,
    removeFriend,
  } = usePlayerWatchConfig();

  const [newFriend, setNewFriend] = useState("");

  const handleAddFriend = async (e: React.FormEvent) => {
    e.preventDefault();
    if (newFriend.trim()) {
      await addFriend(newFriend.trim());
      setNewFriend("");
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-muted">Loading player watch config...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-red-500">Error: {error}</div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full gap-6 p-4">
      <div className="space-y-1">
        <h2 className="text-xl font-semibold text-text">Player Watch</h2>
        <p className="text-sm text-muted">
          Configure zone entry/exit notifications for nearby players
        </p>
      </div>

      <div className="space-y-4">
        <div className="space-y-2">
          <label className="block text-sm font-medium text-text">
            Filter Mode
          </label>
          <div className="flex gap-2">
            {(["all", "strangers_only", "friends_only"] as PlayerFilterMode[]).map((mode) => (
              <button
                key={mode}
                onClick={() => setFilterMode(mode)}
                className={`px-3 py-1.5 text-sm rounded border transition-colors ${
                  config.filter_mode === mode
                    ? "bg-violet text-void border-violet"
                    : "bg-transparent text-text border-muted hover:border-violet"
                }`}
              >
                {mode.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase())}
              </button>
            ))}
          </div>
        </div>

        <div className="flex items-center gap-3">
          <input
            type="checkbox"
            id="sound-toggle"
            checked={config.sound_on_zone_in}
            onChange={(e) => setSoundOnZoneIn(e.target.checked)}
            className="w-4 h-4 accent-violet"
          />
          <label htmlFor="sound-toggle" className="text-sm text-text">
            Play sound on player zone-in
          </label>
        </div>
      </div>

      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <label className="block text-sm font-medium text-text">
            Friends List
          </label>
          <span className="text-xs text-muted">{config.friends.length} friends</span>
        </div>

        <div className="flex gap-2">
          <input
            type="text"
            value={newFriend}
            onChange={(e) => setNewFriend(e.target.value)}
            placeholder="Add friend name..."
            className="flex-1 px-3 py-2 text-sm bg-void border border-muted rounded text-text placeholder:text-muted focus:outline-none focus:border-violet"
          />
          <button
            onClick={handleAddFriend}
            className="px-4 py-2 text-sm font-medium bg-violet text-void rounded hover:bg-violet/90 transition-colors"
          >
            Add
          </button>
        </div>

        {config.friends.length > 0 ? (
          <div className="flex flex-wrap gap-2">
            {config.friends.map((friend) => (
              <span
                key={friend}
                className="inline-flex items-center gap-1.5 px-2.5 py-1 text-sm bg-void border border-violet/50 rounded-full text-violet"
              >
                {friend}
                <button
                  onClick={() => removeFriend(friend)}
                  className="w-4 h-4 flex items-center justify-center rounded-full hover:bg-violet/20 transition-colors"
                  aria-label={`Remove ${friend}`}
                >
                  ×
                </button>
              </span>
            ))}
          </div>
        ) : (
          <p className="text-sm text-muted">No friends added yet.</p>
        )}
      </div>

      <div className="mt-auto space-y-2 text-xs text-muted">
        <p>
          <strong>All:</strong> Announce all player zone events
        </p>
        <p>
          <strong>Strangers Only:</strong> Announce only non-friend players
        </p>
        <p>
          <strong>Friends Only:</strong> Announce only friends from your list
        </p>
      </div>
    </div>
  );
}
