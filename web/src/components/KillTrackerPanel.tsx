import { useState } from "react";
import {
  Crosshair,
  Pulse,
  Skull,
  Sword,
  TrendUp,
  Gear,
} from "@phosphor-icons/react";
import { useKillTracker, type KillTrackerState } from "../hooks/useKillTracker";
import type { KillTrackerSettings } from "../types";

function StatChip({
  label,
  value,
  tone = "neutral",
}: {
  label: string;
  value: string;
  tone?: "neutral" | "good" | "warning" | "critical";
}) {
  const toneClass =
    tone === "good"
      ? "border-emerald-400/25 bg-emerald-400/10 text-emerald-200"
      : tone === "warning"
      ? "border-amber-300/25 bg-amber-300/10 text-amber-100"
      : tone === "critical"
      ? "border-rose-400/25 bg-rose-400/10 text-rose-100"
      : "border-white/10 bg-white/5 text-white";

  return (
    <div className={`rounded-2xl border px-3 py-2 ${toneClass}`}>
      <div className="font-tech text-[10px] uppercase tracking-[0.25em] text-white/50">
        {label}
      </div>
      <div className="mt-1 font-rune text-lg">{value}</div>
    </div>
  );
}

function SettingsForm({
  settings,
  onSave,
  onCancel,
}: {
  settings: KillTrackerSettings;
  onSave: (settings: KillTrackerSettings) => Promise<void>;
  onCancel: () => void;
}) {
  const [form, setForm] = useState<KillTrackerSettings>(settings);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setSaving(true);
    setError(null);
    try {
      await onSave(form);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save settings");
    } finally {
      setSaving(false);
    }
  }

  return (
    <form
      onSubmit={(e) => void handleSubmit(e)}
      className="mt-4 space-y-4 rounded-3xl border border-white/10 bg-[#0d0715] p-4"
    >
      {error && (
        <div className="rounded-xl border border-rose-400/30 bg-rose-400/10 px-3 py-2 text-sm text-rose-100">
          {error}
        </div>
      )}

      <div className="grid gap-4 md:grid-cols-2">
        <label className="flex items-center gap-3">
          <input
            type="checkbox"
            checked={form.enabled}
            onChange={(e) => setForm({ ...form, enabled: e.target.checked })}
            className="h-5 w-5 rounded border-white/20 bg-white/5"
          />
          <span className="text-sm text-white">Enable Kill Tracking</span>
        </label>

        <label className="flex items-center gap-3">
          <input
            type="checkbox"
            checked={form.trackPerCharacter}
            onChange={(e) => setForm({ ...form, trackPerCharacter: e.target.checked })}
            className="h-5 w-5 rounded border-white/20 bg-white/5"
          />
          <span className="text-sm text-white">Track Per Character</span>
        </label>

        <label className="flex items-center gap-3">
          <input
            type="checkbox"
            checked={form.autoReportIncludeMobs}
            onChange={(e) => setForm({ ...form, autoReportIncludeMobs: e.target.checked })}
            className="h-5 w-5 rounded border-white/20 bg-white/5"
          />
          <span className="text-sm text-white">Include Mob Breakdown</span>
        </label>

        <label className="flex items-center gap-3">
          <input
            type="checkbox"
            checked={form.autoReportIncludeKph}
            onChange={(e) => setForm({ ...form, autoReportIncludeKph: e.target.checked })}
            className="h-5 w-5 rounded border-white/20 bg-white/5"
          />
          <span className="text-sm text-white">Include KPH in Reports</span>
        </label>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <div className="grid gap-1">
          <label htmlFor="auto-report-interval" className="text-sm text-white/75">
            Auto-Report Interval (minutes)
          </label>
          <input
            id="auto-report-interval"
            type="number"
            min={0}
            max={120}
            value={form.autoReportIntervalMinutes}
            onChange={(e) =>
              setForm({ ...form, autoReportIntervalMinutes: parseInt(e.target.value, 10) || 0 })
            }
            className="rounded-2xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-fuchsia-300/35"
          />
        </div>

        <div className="grid gap-1">
          <label htmlFor="auto-report-channel" className="text-sm text-white/75">
            Auto-Report Channel
          </label>
          <select
            id="auto-report-channel"
            value={form.autoReportChannel}
            onChange={(e) => setForm({ ...form, autoReportChannel: e.target.value })}
            className="rounded-2xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-fuchsia-300/35"
          >
            <option value="group">Group</option>
            <option value="raid">Raid</option>
            <option value="guild">Guild</option>
            <option value="say">Say</option>
            <option value="shout">Shout</option>
            <option value="ooc">OOC</option>
          </select>
        </div>
      </div>

      <div className="flex justify-end gap-2">
        <button
          type="button"
          onClick={onCancel}
          className="rounded-full border border-white/10 px-4 py-2 text-sm text-white/65 transition hover:border-white/30 hover:bg-white/5"
        >
          Cancel
        </button>
        <button
          type="submit"
          disabled={saving}
          className="rounded-full border border-fuchsia-400/25 bg-fuchsia-400/10 px-4 py-2 text-sm text-fuchsia-100 transition hover:bg-fuchsia-400/20 disabled:opacity-50"
        >
          {saving ? "Saving..." : "Save Settings"}
        </button>
      </div>
    </form>
  );
}

export default function KillTrackerPanel() {
  const { dashboard, loading, error, refresh, updateSettings } = useKillTracker();
  const [settingsOpen, setSettingsOpen] = useState(false);

  if (loading && !dashboard) {
    return (
      <div className="rounded-[1.5rem] border border-fuchsia-400/20 bg-[#120a1d]/88 p-5 backdrop-blur shadow-[0_12px_40px_rgba(217,70,239,0.08)]">
        <div className="flex items-center gap-3">
          <div className="h-11 w-11 animate-pulse rounded-2xl border border-white/10 bg-white/5" />
          <div className="h-5 w-32 animate-pulse rounded bg-white/10" />
        </div>
      </div>
    );
  }

  const session = dashboard?.currentSession;
  const settings = dashboard?.settings;

  return (
    <div className="rounded-[1.5rem] border border-fuchsia-400/20 bg-[#120a1d]/88 p-5 backdrop-blur shadow-[0_12px_40px_rgba(217,70,239,0.08)]">
      <div className="mb-4 flex items-start justify-between gap-4">
        <div className="flex items-start gap-3">
          <div className="mt-0.5 flex h-11 w-11 items-center justify-center rounded-2xl border border-white/10 bg-white/5 text-white">
            <Crosshair size={20} />
          </div>
          <div>
            <h2 className="font-archaic text-xl text-white">Kill Tracker</h2>
            <p className="font-tech text-xs uppercase tracking-[0.28em] text-white/45">
              Session kills, KPH, and auto-reporting
            </p>
          </div>
        </div>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => void refresh()}
            className="rounded-full border border-white/10 bg-white/5 px-3 py-2 text-sm text-white/70 transition hover:border-white/25 hover:bg-white/10"
          >
            <Pulse size={16} />
          </button>
          <button
            type="button"
            onClick={() => setSettingsOpen((o) => !o)}
            className={`rounded-full border px-3 py-2 text-sm transition ${
              settingsOpen
                ? "border-fuchsia-400/25 bg-fuchsia-400/10 text-fuchsia-100"
                : "border-white/10 bg-white/5 text-white/70 hover:border-white/25 hover:bg-white/10"
            }`}
          >
            <Gear size={16} />
          </button>
        </div>
      </div>

      {error && (
        <div className="mb-4 rounded-xl border border-rose-400/30 bg-rose-400/10 px-3 py-2 text-sm text-rose-100">
          {error}
        </div>
      )}

      {settingsOpen && settings && (
        <SettingsForm
          settings={settings}
          onSave={updateSettings}
          onCancel={() => setSettingsOpen(false)}
        />
      )}

      {!session ? (
        <div className="rounded-2xl border border-dashed border-white/10 bg-[#0d0715] p-6 text-center text-sm text-white/45">
          No active session. Start farming to see kill statistics.
        </div>
      ) : (
        <div className="space-y-4">
          <div className="grid gap-3 sm:grid-cols-4">
            <StatChip
              label="Total Kills"
              value={String(session.totalKills)}
              tone={session.totalKills > 0 ? "good" : "neutral"}
            />
            <StatChip
              label="Deaths"
              value={String(session.totalDeaths)}
              tone={session.totalDeaths > 0 ? "warning" : "good"}
            />
            <StatChip
              label="Kills/Hour"
              value={session.killsPerHour.toFixed(1)}
              tone={session.killsPerHour > 30 ? "good" : "neutral"}
            />
            <StatChip
              label="Efficiency"
              value={`${session.efficiency.score.toFixed(0)}/100`}
              tone={
                session.efficiency.score >= 70
                  ? "good"
                  : session.efficiency.score >= 40
                    ? "warning"
                    : "critical"
              }
            />
          </div>

          <div className="rounded-3xl border border-white/10 bg-[#0d0715] p-4">
            <div className="mb-3 flex items-center gap-2 text-white">
              <Sword size={18} className="text-fuchsia-200" />
              <h3 className="font-archaic text-lg">Top Mobs</h3>
            </div>
            <div className="space-y-3">
              {session.topMobs.length === 0 ? (
                <p className="text-sm text-white/45">No mob data yet.</p>
              ) : (
                session.topMobs.map(([mobName, count]) => {
                  const pct =
                    session.totalKills > 0
                      ? Math.round((count / session.totalKills) * 100)
                      : 0;
                  return (
                    <div key={mobName}>
                      <div className="mb-2 flex items-center justify-between text-sm text-white/75">
                        <span className="font-rune">{mobName}</span>
                        <span className="text-white/50">
                          {count} kills ({pct}%)
                        </span>
                      </div>
                      <div className="h-2 overflow-hidden rounded-full bg-white/5">
                        <div
                          className="h-full rounded-full bg-gradient-to-r from-fuchsia-500 to-cyan-300"
                          style={{ width: `${pct}%` }}
                        />
                      </div>
                    </div>
                  );
                })
              )}
            </div>
          </div>

          {session.mobStats.length > 0 && (
            <div className="rounded-3xl border border-white/10 bg-[#0d0715] p-4">
              <div className="mb-3 flex items-center gap-2 text-white">
                <TrendUp size={18} className="text-cyan-200" />
                <h3 className="font-archaic text-lg">Mob Statistics</h3>
              </div>
              <div className="space-y-2">
                {session.mobStats.slice(0, 5).map((mob) => (
                  <div
                    key={mob.mobName}
                    className="grid grid-cols-4 gap-2 rounded-xl border border-white/5 bg-white/[0.03] px-3 py-2 text-xs"
                  >
                    <span className="font-rune text-white truncate">{mob.mobName}</span>
                    <span className="text-white/50">
                      <span className="text-white/75">{mob.killCount}</span> kills
                    </span>
                    <span className="text-white/50">
                      Avg: <span className="text-white/75">{(mob.avgTimeMs / 1000).toFixed(1)}s</span>
                    </span>
                    <span className="text-white/50">
                      DPS: <span className="text-white/75">{mob.avgDps.toFixed(0)}</span>
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}

          <div className="flex items-center justify-between rounded-2xl border border-white/10 bg-[#0d0715] px-4 py-2 text-xs text-white/45">
            <span>
              Session started: {new Date(session.sessionStart).toLocaleString()}
            </span>
            <span>Zone: {session.zone}</span>
          </div>
        </div>
      )}
    </div>
  );
}
