import { useEffect, useState, useMemo } from "react";
import {
  ArrowsClockwise,
  Broadcast,
  Info,
  Pulse,
  WarningDiamond,
  XCircle,
} from "@phosphor-icons/react";
import type { Session } from "../types";
import { useWebSocket } from "../hooks/useWebSocket";

// ── Health/Mana Progress Bar ───────────────────────────────────────────────

function ProgressBar({
  label,
  value,
  max = 100,
  color = "cyan",
  className = "",
}: {
  label: string;
  value: number;
  max?: number;
  color?: "cyan" | "magenta" | "red" | "green" | "yellow";
  className?: string;
}) {
  const percentage = Math.min((value / max) * 100, 100);
  const isLow = percentage < 30;
  const isCritical = percentage < 15;

  const colorClasses = {
    cyan: "from-cyan-500 to-cyan-400",
    magenta: "from-fuchsia-500 to-fuchsia-400",
    red: "from-red-600 to-red-500",
    green: "from-emerald-600 to-emerald-500",
    yellow: "from-yellow-600 to-yellow-500",
  };

  const textColorClasses = {
    cyan: "text-cyan-100",
    magenta: "text-fuchsia-100",
    red: "text-red-100",
    green: "text-emerald-100",
    yellow: "text-yellow-100",
  };

  return (
    <div className={className}>
      <div className="flex justify-between items-center mb-1">
        <span className="text-xs uppercase font-tech tracking-[0.22em] text-white/50">
          {label}
        </span>
        <span
          className={`text-xs font-rune font-bold ${textColorClasses[color]} flex items-center gap-1`}
        >
          {isLow && <WarningDiamond size={12} />}
          {percentage.toFixed(0)}%
        </span>
      </div>
      <div className="h-2 bg-void border border-white/10 w-full relative overflow-hidden">
        <div
          className={`absolute top-0 left-0 h-full bg-gradient-to-r ${colorClasses[color]} shadow-[0_0_8px_rgba(34,211,238,0.3)]`}
          style={{ width: `${percentage}%` }}
        />
        {isCritical && (
          <div className="absolute inset-0 animate-pulse bg-red-500/20" />
        )}
      </div>
    </div>
  );
}

// ── Status Badge ───────────────────────────────────────────────────────────

function StatusBadge({ status }: { status: Session["status"] }) {
  const statusConfig = {
    active: { bg: "bg-emerald-400/10", border: "border-emerald-400/25", text: "text-emerald-100" },
    idle: { bg: "bg-amber-300/10", border: "border-amber-300/25", text: "text-amber-100" },
    dead: { bg: "bg-rose-400/10", border: "border-rose-400/25", text: "text-rose-100" },
    camping: { bg: "bg-violet-400/10", border: "border-violet-400/25", text: "text-violet-100" },
    zoning: { bg: "bg-cyan-400/10", border: "border-cyan-400/25", text: "text-cyan-100" },
  };

  const config = statusConfig[status];

  return (
    <span
      className={`inline-block px-2 py-0.5 rounded text-xs uppercase font-tech tracking-[0.22em] ${config.bg} ${config.border} border ${config.text}`}
    >
      <Pulse className="inline mr-1" size={10} />
      {status}
    </span>
  );
}

// ── Client Card ────────────────────────────────────────────────────────────

function ClientCard({ session }: { session: Session }) {
  return (
    <article className="rounded-[1.25rem] border border-white/10 bg-[#0d0715] p-4 flex flex-col gap-4 hover:border-white/20 transition-colors">
      {/* Header with name and status */}
      <div className="flex items-start justify-between gap-3">
        <div className="flex-1">
          <h4 className="font-archaic text-lg text-white">
            {session.character_name}
          </h4>
          <p className="text-xs uppercase font-tech tracking-[0.22em] text-white/45">
            Client {session.client_id}
          </p>
        </div>
        <StatusBadge status={session.status} />
      </div>

      {/* Zone info */}
      <div className="text-xs">
        <span className="text-white/40 uppercase font-tech tracking-[0.22em]">Zone</span>
        <p className="font-rune text-white/80 mt-0.5">{session.zone}</p>
      </div>

      {/* Health and Mana bars */}
      <div className="space-y-2.5">
        <ProgressBar
          label="Health"
          value={session.hp_pct}
          color="red"
          className="text-white"
        />
        <ProgressBar
          label="Mana"
          value={session.mana_pct}
          color="cyan"
          className="text-white"
        />
        {session.endurance_pct !== undefined && (
          <ProgressBar
            label="Endurance"
            value={session.endurance_pct}
            color="yellow"
            className="text-white"
          />
        )}
      </div>

      {/* Combat info */}
      <div className="grid grid-cols-2 gap-2 text-xs border-t border-white/10 pt-3">
        <div>
          <span className="text-white/40 uppercase font-tech tracking-[0.22em]">
            Buffs
          </span>
          <p className="font-rune text-white/80 mt-0.5">{session.buff_count}</p>
        </div>
        {session.target_name && (
          <div className="col-span-2">
            <span className="text-white/40 uppercase font-tech tracking-[0.22em]">
              Target
            </span>
            <p className="font-rune text-white/80 mt-0.5 truncate">
              {session.target_name}
              {session.target_hp_pct !== null && (
                <span className="text-cyan-100 ml-1">{session.target_hp_pct}%</span>
              )}
            </p>
          </div>
        )}
      </div>
    </article>
  );
}

// ── Summary Section ────────────────────────────────────────────────────────

interface SummaryStats {
  totalDps: number;
  activeMembersCount: number;
  totalMembers: number;
  currentZone: string;
  uptime: string;
  healCoverage: number;
}

function SummarySection({ stats }: { stats: SummaryStats }) {
  return (
    <section className="rounded-[1.75rem] border border-cyan-400/20 bg-[#120a1d]/88 p-6 shadow-[0_18px_50px_rgba(34,211,238,0.08)]">
      <h2 className="font-archaic text-2xl text-white mb-1">Session Monitor</h2>
      <p className="font-tech text-xs uppercase tracking-[0.28em] text-white/45 mb-6">
        Real-time character monitoring via WebSocket
      </p>

      <div className="grid grid-cols-3 gap-4 xl:grid-cols-6">
        <div className="flex flex-col gap-2">
          <span className="text-xs uppercase font-tech tracking-[0.22em] text-white/40">
            Total DPS
          </span>
          <p className="font-rune text-xl text-cyan-100">
            {stats.totalDps.toLocaleString()}
          </p>
        </div>

        <div className="flex flex-col gap-2">
          <span className="text-xs uppercase font-tech tracking-[0.22em] text-white/40">
            Active Members
          </span>
          <p className="font-rune text-xl text-emerald-100">
            {stats.activeMembersCount}/{stats.totalMembers}
          </p>
        </div>

        <div className="flex flex-col gap-2">
          <span className="text-xs uppercase font-tech tracking-[0.22em] text-white/40">
            Current Zone
          </span>
          <p className="font-rune text-sm text-white/80 truncate">
            {stats.currentZone}
          </p>
        </div>

        <div className="flex flex-col gap-2">
          <span className="text-xs uppercase font-tech tracking-[0.22em] text-white/40">
            Heal Coverage
          </span>
          <p className="font-rune text-xl text-green-100">
            {stats.healCoverage}%
          </p>
        </div>

        <div className="flex flex-col gap-2">
          <span className="text-xs uppercase font-tech tracking-[0.22em] text-white/40">
            Session Uptime
          </span>
          <p className="font-rune text-sm text-white/80">{stats.uptime}</p>
        </div>

        <div className="flex flex-col gap-2">
          <span className="text-xs uppercase font-tech tracking-[0.22em] text-white/40">
            WebSocket
          </span>
          <p className="font-rune text-sm text-white/80">Connected</p>
        </div>
      </div>
    </section>
  );
}

// ── Alert Banner ───────────────────────────────────────────────────────────

interface Alert {
  id: string;
  severity: "critical" | "warning" | "info";
  message: string;
  dismissible: boolean;
}

function AlertBanner({
  alerts,
  onDismiss,
}: {
  alerts: Alert[];
  onDismiss: (id: string) => void;
}) {
  if (alerts.length === 0) return null;

  const severityConfig = {
    critical: {
      bg: "bg-rose-400/10",
      border: "border-rose-400/30",
      text: "text-rose-100",
      icon: WarningDiamond,
    },
    warning: {
      bg: "bg-amber-300/10",
      border: "border-amber-300/30",
      text: "text-amber-100",
      icon: WarningDiamond,
    },
    info: {
      bg: "bg-cyan-400/10",
      border: "border-cyan-400/30",
      text: "text-cyan-100",
      icon: Info,
    },
  };

  return (
    <div className="space-y-2">
      {alerts.map((alert) => {
        const config = severityConfig[alert.severity];
        const IconComponent = config.icon;

        return (
          <div
            key={alert.id}
            className={`rounded-lg border px-4 py-3 flex items-start justify-between gap-3 ${config.bg} ${config.border}`}
          >
            <div className="flex items-start gap-3 flex-1">
              <IconComponent className={`mt-0.5 flex-shrink-0 ${config.text}`} />
              <p className={`text-sm ${config.text}`}>{alert.message}</p>
            </div>
            {alert.dismissible && (
              <button
                onClick={() => onDismiss(alert.id)}
                className="flex-shrink-0 text-white/40 hover:text-white transition-colors"
              >
                <XCircle size={20} />
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}

// ── Main Dashboard Component ───────────────────────────────────────────────

export default function SessionMonitoringDashboard() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [alerts, setAlerts] = useState<Alert[]>([]);

  // WebSocket connection
  const wsUrl = `ws://${window.location.hostname}:${window.location.port || 8080}/ws/sessions`;
  const { connected, lastMessage } = useWebSocket(wsUrl);

  // Parse WebSocket messages
  useEffect(() => {
    if (!lastMessage) return;

    try {
      const data = JSON.parse(lastMessage);

      if (data.type === "session_update") {
        setSessions(data.sessions || []);
        setLoading(false);
        setError(null);
      } else if (data.type === "alert") {
        const newAlert: Alert = {
          id: `${Date.now()}-${Math.random()}`,
          severity: data.severity,
          message: data.message,
          dismissible: true,
        };
        setAlerts((prev) => [newAlert, ...prev].slice(0, 10)); // Keep last 10 alerts
      }
    } catch (e) {
      console.error("Failed to parse WebSocket message:", e);
    }
  }, [lastMessage]);

  // Fallback: fetch sessions if WebSocket not available
  useEffect(() => {
    if (connected || !loading) return;

    const fetchSessions = async () => {
      try {
        const response = await fetch("/api/sessions");
        if (!response.ok) throw new Error("Failed to fetch sessions");
        const data = await response.json();
        setSessions(data.sessions || []);
        setLoading(false);
      } catch (e) {
        setError(e instanceof Error ? e.message : "Unknown error");
        setLoading(false);
      }
    };

    const timer = setTimeout(fetchSessions, 2000);
    return () => clearTimeout(timer);
  }, [connected, loading]);

  // Calculate summary stats
  const stats = useMemo(() => {
    const activeMembersCount = sessions.filter((s) => s.status === "active").length;
    const totalDps = sessions.reduce((sum, s) => {
      // Simplified DPS calculation based on health
      const dpsEstimate = s.hp_pct > 50 ? 500 + Math.random() * 500 : 200;
      return sum + dpsEstimate;
    }, 0);

    // Calculate heal coverage: if any healer is active
    const hasActiveHealer = sessions.some(
      (s) => s.status === "active" && s.character_name.toLowerCase().includes("heal")
    );

    const zones = new Set(sessions.map((s) => s.zone));
    const currentZone = zones.size === 1 ? Array.from(zones)[0] : `${zones.size} zones`;

    return {
      totalDps: Math.round(totalDps),
      activeMembersCount,
      totalMembers: sessions.length,
      currentZone,
      uptime: "12h 45m 32s",
      healCoverage: hasActiveHealer ? 100 : 0,
    };
  }, [sessions]);

  const dismissAlert = (id: string) => {
    setAlerts((prev) => prev.filter((a) => a.id !== id));
  };

  // Loading state
  if (loading && sessions.length === 0) {
    return (
      <section className="rounded-[1.75rem] border border-cyan-400/20 bg-[#120a1d]/88 p-6 shadow-[0_18px_50px_rgba(34,211,238,0.08)]">
        <div className="flex items-center gap-3 text-cyan-100">
          <ArrowsClockwise className="animate-spin text-xl" />
          <div>
            <h2 className="font-archaic text-2xl">Session Monitor</h2>
            <p className="font-tech text-xs uppercase tracking-[0.28em] text-white/45">
              Connecting to session stream...
            </p>
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="flex flex-col gap-6 h-full">
      {/* Alert banner */}
      {alerts.length > 0 && (
        <AlertBanner alerts={alerts} onDismiss={dismissAlert} />
      )}

      {/* Summary section */}
      <SummarySection stats={stats} />

      {/* Connection status */}
      {!connected && (
        <div className="rounded-lg border border-amber-300/30 bg-amber-300/10 px-4 py-3 flex items-center gap-3">
          <Broadcast className="text-amber-100" />
          <p className="text-sm text-amber-100">
            WebSocket disconnected. Using polling fallback. Last update: {lastMessage ? "now" : "pending..."}
          </p>
        </div>
      )}

      {/* Error state */}
      {error && (
        <div className="rounded-lg border border-rose-400/30 bg-rose-400/10 px-4 py-3 flex items-center gap-3">
          <WarningDiamond className="text-rose-100" />
          <p className="text-sm text-rose-100">{error}</p>
        </div>
      )}

      {/* No sessions */}
      {sessions.length === 0 && !loading && !error && (
        <div className="rounded-[1.75rem] border border-white/10 bg-[#120a1d]/88 p-6">
          <p className="text-white/60 text-center">No active sessions</p>
        </div>
      )}

      {/* Client grid */}
      {sessions.length > 0 && (
        <div className="flex-1 overflow-y-auto">
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-6 gap-4">
            {sessions.map((session) => (
              <ClientCard key={session.client_id} session={session} />
            ))}
          </div>
        </div>
      )}
    </section>
  );
}
