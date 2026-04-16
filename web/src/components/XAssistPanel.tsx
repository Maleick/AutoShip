import { useState } from "react";
import {
  Crosshair,
  FloppyDisk,
  ArrowClockwise,
  Warning,
  User,
  CheckCircle,
  XCircle,
} from "@phosphor-icons/react";
import { useXAssist } from "../hooks/useXAssist";

export default function XAssistPanel() {
  const { configs, loading, saving, error, refresh, updateConfig } = useXAssist();
  const [editingCharacter, setEditingCharacter] = useState<string | null>(null);
  const [editMaName, setEditMaName] = useState("");
  const [editEnabled, setEditEnabled] = useState(false);

  function startEdit(charName: string, maName: string | null, enabled: boolean) {
    setEditingCharacter(charName);
    setEditMaName(maName ?? "");
    setEditEnabled(enabled);
  }

  async function saveEdit(charName: string) {
    try {
      await updateConfig(charName, {
        ma_name: editMaName.trim() || null,
        enabled: editEnabled,
      });
      setEditingCharacter(null);
    } catch {
      // error is handled by hook
    }
  }

  function cancelEdit() {
    setEditingCharacter(null);
    setEditMaName("");
    setEditEnabled(false);
  }

  return (
    <div className="h-full flex flex-col bg-void">
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-4 border-b border-white/10">
        <div className="flex items-center gap-3">
          <Crosshair weight="fill" className="text-spectral text-xl" />
          <h2 className="font-archaic text-xl text-spectral uppercase tracking-widest">
            X-Assist Configuration
          </h2>
        </div>
        <button
          onClick={refresh}
          className="p-2 text-white/40 hover:text-spectral transition-colors"
          title="Refresh"
        >
          <ArrowClockwise size={18} />
        </button>
      </div>

      {/* Info Banner */}
      <div className="mx-6 mt-4 p-3 border border-cyan-500/20 bg-cyan-500/10 rounded">
        <p className="text-xs text-cyan-300/80 font-tech">
          Cross-Group Outside-Group Assist — Auto-target the Main Assist&apos;s target
          regardless of group membership. Works in raid and cross-group scenarios.
        </p>
      </div>

      {/* Error State */}
      {error && (
        <div className="mx-6 mt-4 p-3 border border-red-500/30 bg-red-500/10 rounded flex items-center gap-2">
          <Warning weight="fill" className="text-red-400 flex-shrink-0" />
          <span className="text-xs text-red-300">{error}</span>
        </div>
      )}

      {/* Loading State */}
      {loading ? (
        <div className="flex-1 flex items-center justify-center">
          <span className="text-white/40 font-tech text-sm animate-pulse">
            Loading X-Assist configs...
          </span>
        </div>
      ) : (
        <div className="flex-1 overflow-y-auto px-6 py-4">
          {configs.length === 0 ? (
            <div className="text-center py-12 text-white/30 font-tech text-sm">
              No X-Assist configurations found.
            </div>
          ) : (
            <div className="space-y-3">
              {configs.map((config) => (
                <div
                  key={config.character_name}
                  className="border border-white/10 bg-white/5 rounded p-4 hover:border-spectral/30 transition-colors"
                >
                  {editingCharacter === config.character_name ? (
                    <div className="space-y-3">
                      <div className="flex items-center gap-2">
                        <User size={14} className="text-spectral" />
                        <span className="font-tech text-spectral">
                          {config.character_name}
                        </span>
                      </div>

                      <div>
                        <label className="block text-xs text-white/50 mb-1 font-tech uppercase tracking-wider">
                          Main Assist Name
                        </label>
                        <input
                          type="text"
                          value={editMaName}
                          onChange={(e) => setEditMaName(e.target.value)}
                          placeholder="Enter MA character name..."
                          className="w-full bg-void border border-white/20 rounded px-3 py-2 text-sm text-white placeholder:text-white/30 focus:outline-none focus:border-spectral/60 font-mono"
                        />
                      </div>

                      <label className="flex items-center gap-2 cursor-pointer">
                        <div
                          onClick={() => setEditEnabled(!editEnabled)}
                          className={`w-10 h-5 rounded-full transition-colors relative ${
                            editEnabled ? "bg-spectral" : "bg-white/20"
                          }`}
                        >
                          <div
                            className={`absolute top-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
                              editEnabled ? "translate-x-5" : "translate-x-0.5"
                            }`}
                          />
                        </div>
                        <span className="text-xs text-white/70 font-tech">
                          Enable X-Assist
                        </span>
                      </label>

                      <div className="flex gap-2 pt-2">
                        <button
                          onClick={() => saveEdit(config.character_name)}
                          disabled={saving}
                          className="flex items-center gap-1.5 px-3 py-1.5 bg-spectral/20 border border-spectral/40 text-spectral text-xs font-tech rounded hover:bg-spectral/30 transition-colors disabled:opacity-50"
                        >
                          <FloppyDisk size={14} />
                          Save
                        </button>
                        <button
                          onClick={cancelEdit}
                          className="px-3 py-1.5 bg-white/5 border border-white/20 text-white/60 text-xs font-tech rounded hover:bg-white/10 transition-colors"
                        >
                          Cancel
                        </button>
                      </div>
                    </div>
                  ) : (
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <div className="flex items-center gap-2 mb-1">
                          <User size={14} className="text-spectral" />
                          <span className="font-tech text-spectral">
                            {config.character_name}
                          </span>
                          {config.enabled ? (
                            <span className="flex items-center gap-1 text-xs text-green-400">
                              <CheckCircle size={12} weight="fill" />
                              Active
                            </span>
                          ) : (
                            <span className="flex items-center gap-1 text-xs text-white/40">
                              <XCircle size={12} />
                              Disabled
                            </span>
                          )}
                        </div>
                        <p className="text-sm text-white/60 font-mono">
                          MA:{" "}
                          <span className="text-cyan-300">
                            {config.ma_name ?? "(not set)"}
                          </span>
                        </p>
                      </div>
                      <button
                        onClick={() =>
                          startEdit(
                            config.character_name,
                            config.ma_name,
                            config.enabled
                          )
                        }
                        className="px-3 py-1.5 bg-white/5 border border-white/20 text-white/60 text-xs font-tech rounded hover:bg-white/10 transition-colors"
                      >
                        Edit
                      </button>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
