import { useMemo, useState } from "react";
import {
  Broadcast,
  Clock,
  Eye,
  Funnel,
  MagnifyingGlass,
  Megaphone,
  Plus,
  Skull,
  Trash,
  WifiHigh,
} from "@phosphor-icons/react";
import {
  spawnAlerts as initialSpawnAlerts,
  spawnAlertConfig as initialConfig,
  spawnAlertStats as initialStats,
} from "../data/demo";
import type {
  SpawnAlertEntry,
  SpawnAlertConfig,
  SpawnAlertStats,
} from "../types";

function Section({
  icon,
  title,
  subtitle,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  subtitle: string;
  children: React.ReactNode;
}) {
  return (
    <section className="bg-violet/20 border border-white/10 p-5">
      <div className="flex items-center gap-3 mb-4">
        <div className="w-8 h-8 border border-magentaglow/40 bg-magentadark/15 text-magentaglow flex items-center justify-center">
          {icon}
        </div>
        <div>
          <h3 className="font-archaic text-base text-white">{title}</h3>
          <p className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
            {subtitle}
          </p>
        </div>
      </div>
      {children}
    </section>
  );
}

function formatDuration(ms: number | null): string {
  if (ms === null) return "First seen";
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  if (hours > 0) {
    return `${hours}h ${minutes % 60}m ago`;
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds % 60}s ago`;
  }
  return `${seconds}s ago`;
}

function formatTimestamp(ts: string): string {
  const date = new Date(ts);
  return date.toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export default function SpawnAlerts() {
  const alerts = initialSpawnAlerts as SpawnAlertEntry[];
  const [config, setConfig] = useState<SpawnAlertConfig>(initialConfig);
  const [stats] = useState<SpawnAlertStats>(initialStats);
  const [search, setSearch] = useState("");
  const [zoneFilter, setZoneFilter] = useState<string>("");
  const [showUpOnly, setShowUpOnly] = useState(false);
  const [newPattern, setNewPattern] = useState("");

  const zones = useMemo(() => {
    const zoneSet = new Set(alerts.map((a) => a.zone));
    return Array.from(zoneSet).sort();
  }, [alerts]);

  const filteredAlerts = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return alerts.filter((alert) => {
      if (needle && !alert.spawn_name.toLowerCase().includes(needle)) {
        return false;
      }
      if (zoneFilter && alert.zone !== zoneFilter) {
        return false;
      }
      if (showUpOnly && !alert.is_up) {
        return false;
      }
      return true;
    });
  }, [alerts, search, zoneFilter, showUpOnly]);

  const activePatterns = config.watch_patterns.filter((p) => p.enabled);
  const spawnsUp = alerts.filter((a) => a.is_up).length;

  function togglePattern(pattern: string) {
    setConfig((prev) => ({
      ...prev,
      watch_patterns: prev.watch_patterns.map((p) =>
        p.pattern === pattern ? { ...p, enabled: !p.enabled } : p
      ),
    }));
  }

  function addPattern() {
    const trimmed = newPattern.trim();
    if (!trimmed) return;
    if (config.watch_patterns.some((p) => p.pattern === trimmed)) {
      return;
    }
    setConfig((prev) => ({
      ...prev,
      watch_patterns: [...prev.watch_patterns, { pattern: trimmed, enabled: true }],
    }));
    setNewPattern("");
  }

  function removePattern(pattern: string) {
    setConfig((prev) => ({
      ...prev,
      watch_patterns: prev.watch_patterns.filter((p) => p.pattern !== pattern),
    }));
  }

  function toggleWatchNamed() {
    setConfig((prev) => ({
      ...prev,
      watch_named_enabled: !prev.watch_named_enabled,
    }));
  }

  function toggleBroadcastWeb() {
    setConfig((prev) => ({
      ...prev,
      broadcast_to_web: !prev.broadcast_to_web,
    }));
  }

  function toggleBroadcastClients() {
    setConfig((prev) => ({
      ...prev,
      broadcast_to_clients: !prev.broadcast_to_clients,
    }));
  }

  return (
    <section className="flex-1 h-full flex flex-col relative z-20 min-w-[700px]">
      <header className="h-16 border-b border-white/10 flex items-center justify-between px-6 bg-violet/30 backdrop-blur-md shrink-0">
        <div className="flex items-center gap-4">
          <div className="w-8 h-8 rounded border border-magentadark flex items-center justify-center bg-void">
            <Eye weight="fill" className="text-magentaglow" />
          </div>
          <div>
            <h2 className="font-archaic text-lg text-white leading-tight">
              Rare Spawn Alerts
            </h2>
            <p className="text-[10px] uppercase tracking-widest text-white/50 font-rune">
              Named NPC monitoring · Watch patterns · Spawn history
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <div className="flex items-center gap-3 mr-4">
            <div className="flex items-center gap-1.5">
              <div className="w-2 h-2 rounded-full bg-green-400 animate-pulse" />
              <span className="text-xs text-white/60 font-rune">
                {spawnsUp} up
              </span>
            </div>
            <div className="flex items-center gap-1.5">
              <div className="w-2 h-2 rounded-full bg-red-400/60" />
              <span className="text-xs text-white/60 font-rune">
                {alerts.length - spawnsUp} down
              </span>
            </div>
          </div>
        </div>
      </header>

      <div className="flex-1 overflow-y-auto p-6 scroll-smooth space-y-6">
        <div className="grid grid-cols-4 gap-4">
          <div className="bg-void/60 border border-white/10 p-4 flex flex-col gap-1">
            <span className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
              Total Alerts
            </span>
            <span className="font-rune text-2xl text-white">{stats.total_alerts}</span>
          </div>
          <div className="bg-void/60 border border-white/10 p-4 flex flex-col gap-1">
            <span className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
              Active Patterns
            </span>
            <span className="font-rune text-2xl text-magentaglow">
              {activePatterns.length}
            </span>
          </div>
          <div className="bg-void/60 border border-white/10 p-4 flex flex-col gap-1">
            <span className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
              Watched Zones
            </span>
            <span className="font-rune text-2xl text-spectral">{zones.length}</span>
          </div>
          <div className="bg-void/60 border border-white/10 p-4 flex flex-col gap-1">
            <span className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
              Broadcast Mode
            </span>
            <span className="font-rune text-2xl text-cyan-400">
              {config.broadcast_to_web ? "Web" : "Local"}
            </span>
          </div>
        </div>

        <Section
          icon={<Funnel size={15} />}
          title="Watch List Configuration"
          subtitle="Pattern matching for spawn alerts"
        >
          <div className="flex items-center gap-4 mb-4">
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={config.watch_named_enabled}
                onChange={toggleWatchNamed}
                className="w-4 h-4 accent-magentaglow"
              />
              <span className="text-sm text-white font-rune">
                Watch all Named NPCs (no article prefix)
              </span>
            </label>
          </div>

          <div className="flex flex-wrap gap-2 mb-4">
            {config.watch_patterns.map((pattern) => (
              <div
                key={pattern.pattern}
                className={`flex items-center gap-2 px-3 py-1.5 border text-xs font-rune ${
                  pattern.enabled
                    ? "border-magentaglow/50 bg-magentadark/20 text-magentaglow"
                    : "border-white/20 bg-void/40 text-white/50"
                }`}
              >
                <span>{pattern.pattern}</span>
                <button
                  onClick={() => togglePattern(pattern.pattern)}
                  className="text-[10px] uppercase tracking-wider hover:underline"
                >
                  {pattern.enabled ? "Disable" : "Enable"}
                </button>
                <button
                  onClick={() => removePattern(pattern.pattern)}
                  className="text-white/40 hover:text-red-400 ml-1"
                >
                  <Trash size={12} />
                </button>
              </div>
            ))}
          </div>

          <div className="flex items-center gap-3">
            <input
              value={newPattern}
              onChange={(e) => setNewPattern(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addPattern()}
              placeholder="Add pattern (e.g., *Maestro* or Emperor Crush)"
              className="flex-1 bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune placeholder:text-white/20"
            />
            <button
              onClick={addPattern}
              className="px-4 py-2 border border-magentaglow/50 text-magentaglow bg-magentadark/20 hover:bg-magentadark/40 text-xs uppercase tracking-wider font-rune flex items-center gap-1.5"
            >
              <Plus size={14} />
              Add
            </button>
          </div>
        </Section>

        <Section
          icon={<Broadcast size={15} />}
          title="Broadcast Settings"
          subtitle="Alert delivery channels"
        >
          <div className="flex items-center gap-6">
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={config.broadcast_to_web}
                onChange={toggleBroadcastWeb}
                className="w-4 h-4 accent-magentaglow"
              />
              <WifiHigh size={16} className="text-spectral" />
              <span className="text-sm text-white font-rune">Web Notifications</span>
            </label>
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={config.broadcast_to_clients}
                onChange={toggleBroadcastClients}
                className="w-4 h-4 accent-magentaglow"
              />
              <Megaphone size={16} className="text-cyan-400" />
              <span className="text-sm text-white font-rune">Broadcast to All Clients</span>
            </label>
          </div>
        </Section>

        <Section
          icon={<Skull size={15} />}
          title="Spawn History"
          subtitle="Recent spawn events and time tracking"
        >
          <div className="flex items-center gap-4 mb-4">
            <div className="relative flex-1">
              <MagnifyingGlass
                size={15}
                className="absolute left-3 top-1/2 -translate-y-1/2 text-white/30"
              />
              <input
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Search by spawn name..."
                className="w-full bg-void border border-white/20 text-white text-sm pl-10 pr-3 py-2 focus:outline-none focus:border-magentaglow font-rune placeholder:text-white/20"
              />
            </div>
            <select
              value={zoneFilter}
              onChange={(e) => setZoneFilter(e.target.value)}
              className="bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune min-w-40"
            >
              <option value="">All Zones</option>
              {zones.map((zone) => (
                <option key={zone} value={zone}>
                  {zone}
                </option>
              ))}
            </select>
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={showUpOnly}
                onChange={(e) => setShowUpOnly(e.target.checked)}
                className="w-4 h-4 accent-magentaglow"
              />
              <span className="text-sm text-white font-rune">Up Only</span>
            </label>
          </div>

          <div className="space-y-2">
            {filteredAlerts.map((alert) => (
              <div
                key={alert.id}
                className={`grid grid-cols-[1.2fr_0.8fr_0.7fr_1fr_auto] gap-3 border bg-void/40 p-3 items-center ${
                  alert.is_up ? "border-green-500/30" : "border-white/10"
                }`}
              >
                <div className="flex items-center gap-2">
                  <div
                    className={`w-2 h-2 rounded-full ${
                      alert.is_up ? "bg-green-400 animate-pulse" : "bg-white/30"
                    }`}
                  />
                  <div>
                    <div className="text-white text-sm">{alert.spawn_name}</div>
                    <div className="text-[10px] uppercase tracking-widest text-white/35 font-rune">
                      {alert.match_source === "Named" ? "Named NPC" : alert.match_source}
                    </div>
                  </div>
                </div>
                <div className="text-white/70 text-sm">{alert.zone}</div>
                <div
                  className={`font-rune text-sm ${
                    alert.is_up ? "text-green-400" : "text-white/50"
                  }`}
                >
                  {alert.is_up ? "UP" : "DOWN"}
                </div>
                <div className="flex items-center gap-1.5 text-white/50 text-xs font-rune">
                  <Clock size={12} />
                  {formatDuration(alert.time_since_last_pop_ms)}
                </div>
                <div className="text-right text-white/55 font-rune text-xs">
                  {formatTimestamp(alert.timestamp)}
                </div>
              </div>
            ))}
          </div>

          {filteredAlerts.length === 0 && (
            <div className="flex items-center justify-center h-24 text-white/30 font-archaic">
              No spawn alerts match your filters
            </div>
          )}
        </Section>
      </div>
    </section>
  );
}
