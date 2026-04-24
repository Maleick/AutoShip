import { Activity, Coins, TrendingDown, TrendingUp, Users, UsersRound } from "lucide-react";
import { PageHeader } from "../components/PageHeader.tsx";
import { Sparkline } from "../components/Sparkline.tsx";
import { StatBar } from "../components/StatBar.tsx";
import { MOCK_KPI, MOCK_LOG, MOCK_SESSIONS } from "../lib/mocks.ts";
import type { LogLevel } from "../lib/mocks.ts";

const LEVEL_COLOR: Record<LogLevel, string> = {
  INFO: "#a096b4",
  OK: "#34d399",
  CAST: "#60a5fa",
  WARN: "#fbbf24",
  LOOT: "#cc44ff",
  CH: "#00e5ff",
  ALERT: "#ef4444",
  ROUTE: "#a096b4",
  ECON: "#fbbf24",
  DMG: "#ef4444",
};

interface KpiCardProps {
  title: string;
  value: string;
  unit?: string;
  caption: string;
  delta: number;
  icon: typeof Users;
  accentColor: string;
  sparkline: number[];
}

function KpiCard({
  title,
  value,
  unit,
  caption,
  delta,
  icon: Icon,
  accentColor,
  sparkline,
}: KpiCardProps) {
  const deltaUp = delta >= 0;
  return (
    <div
      className="relative border rounded-md bg-[#1a0a2e] p-4 flex flex-col gap-3 overflow-hidden"
      style={{
        borderColor: `${accentColor}55`,
        boxShadow: `0 0 32px -16px ${accentColor}`,
      }}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Icon className="w-4 h-4" strokeWidth={1.75} style={{ color: accentColor }} />
          <span className="font-mono text-[10px] uppercase tracking-[0.2em] text-[#a096b4]">
            {title}
          </span>
        </div>
        <span
          className={`flex items-center gap-1 font-mono text-[11px] ${
            deltaUp ? "text-[#34d399]" : "text-[#ef4444]"
          }`}
        >
          {deltaUp ? (
            <TrendingUp className="w-3 h-3" strokeWidth={2} />
          ) : (
            <TrendingDown className="w-3 h-3" strokeWidth={2} />
          )}
          {deltaUp ? "▲" : "▼"} {Math.abs(delta).toFixed(1)}%
        </span>
      </div>
      <div className="flex items-end justify-between gap-3">
        <div>
          <span
            className="font-mono text-5xl tabular-nums font-light"
            style={{ color: accentColor }}
          >
            {value}
          </span>
          {unit && (
            <span className="ml-1.5 font-mono text-xs text-[#a096b4] uppercase tracking-wider">
              {unit}
            </span>
          )}
        </div>
        <Sparkline values={sparkline} color={accentColor} width={110} height={36} />
      </div>
      <div className="font-mono text-[11px] text-[#a096b4] border-t border-[#503c6e]/50 pt-2">
        {caption}
      </div>
    </div>
  );
}

export function Dashboard() {
  const rows = MOCK_SESSIONS;
  const clients = rows.length;
  const activeCount = rows.filter((r) => r.state === "active").length;
  const pausedCount = rows.filter((r) => r.state === "paused").length;
  const stuckCount = rows.filter((r) => r.hp_pct !== undefined && r.hp_pct < 20).length;
  const groups = new Set(rows.map((r) => r.group_id)).size;
  const economy = MOCK_KPI.economy[MOCK_KPI.economy.length - 1];

  const first = MOCK_KPI.economy[0];
  const econDelta = ((economy - first) / first) * 100;
  const clientsDelta = ((clients - MOCK_KPI.clients[0]) / MOCK_KPI.clients[0]) * 100;

  return (
    <div>
      <PageHeader
        title="Dashboard"
        subtitle={
          <>
            <span className="text-[#a096b4]">G2 Fear Core</span>
            <span className="text-[#503c6e]">·</span>
            <span className="text-[#a096b4]">Plane of Fear</span>
            <span className="text-[#503c6e]">·</span>
            <span className="text-[#ff00ff]">Bertoxxulous</span>
          </>
        }
        meta={
          <>
            <span className="text-[#503c6e]">uptime</span>
            <span className="text-[#e2d7f4] tabular-nums">03:47:12</span>
            <span className="text-[#503c6e]">·</span>
            <span className="text-[#503c6e]">tick</span>
            <span className="text-[#e2d7f4] tabular-nums">16 Hz</span>
          </>
        }
      />

      <div className="p-6 space-y-4">
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          <KpiCard
            title="Clients"
            value={String(clients)}
            unit="online"
            caption={`${activeCount} active · ${pausedCount} paused · ${stuckCount} stuck`}
            delta={clientsDelta}
            icon={Users}
            accentColor="#00e5ff"
            sparkline={MOCK_KPI.clients}
          />
          <KpiCard
            title="Groups"
            value={String(groups)}
            unit="active"
            caption="G1 recovery · G2 hunt · G3 nav"
            delta={0}
            icon={UsersRound}
            accentColor="#34d399"
            sparkline={MOCK_KPI.groups}
          />
          <KpiCard
            title="Economy"
            value={economy.toFixed(1)}
            unit="pp"
            caption={`${econDelta >= 0 ? "+" : ""}${econDelta.toFixed(1)} since 12:00 · loot queue 3`}
            delta={econDelta}
            icon={Coins}
            accentColor="#fbbf24"
            sparkline={MOCK_KPI.economy}
          />
        </div>

        <div className="grid grid-cols-1 xl:grid-cols-2 gap-4">
          {/* Active sessions */}
          <section className="border border-[#503c6e] rounded-md bg-[#1a0a2e] overflow-hidden">
            <div className="flex items-center gap-2 px-3 py-2 border-b border-[#503c6e] text-xs font-mono text-[#a096b4] uppercase tracking-[0.15em]">
              <Activity className="w-3.5 h-3.5 text-[#00e5ff]" strokeWidth={1.75} />
              active sessions
              <span className="text-[#503c6e]">·</span>
              <span>{clients} clients</span>
            </div>
            <table className="w-full font-mono text-xs">
              <thead className="text-[#a096b4] uppercase tracking-[0.15em] text-[9px]">
                <tr>
                  <th className="px-3 py-1.5 text-left">character</th>
                  <th className="px-3 py-1.5 text-left">class</th>
                  <th className="px-3 py-1.5 text-left">zone</th>
                  <th className="px-3 py-1.5 text-left">hp</th>
                  <th className="px-3 py-1.5 text-left">mp</th>
                  <th className="px-3 py-1.5 text-left">status</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((r) => {
                  const statusColor =
                    r.state === "active"
                      ? "#34d399"
                      : r.state === "paused"
                        ? "#fbbf24"
                        : r.state === "error"
                          ? "#ef4444"
                          : "#a096b4";
                  return (
                    <tr
                      key={r.session_id}
                      className="border-t border-[#503c6e]/40 hover:bg-[#2d1e41]/40"
                    >
                      <td className="px-3 py-1.5 text-[#e2d7f4]">{r.character_name}</td>
                      <td className="px-3 py-1.5">
                        <span className="text-[#cc44ff]">{r.class}</span>
                      </td>
                      <td className="px-3 py-1.5 text-[#a096b4]">{r.zone}</td>
                      <td className="px-3 py-1.5">
                        {r.hp_pct !== undefined && <StatBar pct={r.hp_pct} kind="hp" width={48} />}
                      </td>
                      <td className="px-3 py-1.5">
                        {r.mana_pct !== undefined && (
                          <StatBar pct={r.mana_pct} kind="mp" width={48} />
                        )}
                      </td>
                      <td className="px-3 py-1.5">
                        <span className="flex items-center gap-1.5" style={{ color: statusColor }}>
                          <span
                            className="inline-block w-1.5 h-1.5 rounded-full"
                            style={{ background: statusColor, boxShadow: `0 0 4px ${statusColor}` }}
                          />
                          {r.state}
                        </span>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </section>

          {/* Event log */}
          <section className="border border-[#503c6e] rounded-md bg-[#1a0a2e] overflow-hidden flex flex-col">
            <div className="flex items-center gap-2 px-3 py-2 border-b border-[#503c6e] text-xs font-mono text-[#a096b4] uppercase tracking-[0.15em]">
              <Activity className="w-3.5 h-3.5 text-[#cc44ff]" strokeWidth={1.75} />
              event log
              <span className="text-[#503c6e]">·</span>
              <span className="font-mono normal-case">tail -f</span>
              <span className="ml-auto text-[#503c6e]">{MOCK_LOG.length} evts</span>
            </div>
            <div className="p-2 max-h-[480px] overflow-y-auto space-y-0.5 font-mono text-[11px]">
              {MOCK_LOG.map((entry, i) => (
                <div
                  key={i}
                  className="flex items-start gap-2 px-2 py-0.5 hover:bg-[#2d1e41]/40 rounded-sm"
                >
                  <span className="text-[#503c6e] tabular-nums shrink-0">{entry.ts}</span>
                  <span
                    className="font-semibold shrink-0 w-10"
                    style={{ color: LEVEL_COLOR[entry.level] }}
                  >
                    {entry.level}
                  </span>
                  <span className="text-[#e2d7f4]">{entry.text}</span>
                </div>
              ))}
            </div>
            <div className="px-3 py-1.5 border-t border-[#503c6e] font-mono text-[11px] text-[#a096b4] flex items-center gap-2">
              <span className="text-[#cc44ff]">:</span>
              <span className="text-[#503c6e]">command</span>
              <span className="inline-block w-2 h-3 bg-[#cc44ff] animate-pulse" />
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
