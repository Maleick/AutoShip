import { useState } from "react";
import { Coins, MapPin, Pause, Plus, Play, Trash2, TrendingDown } from "lucide-react";
import { PageHeader } from "../components/PageHeader.tsx";
import { Sparkline } from "../components/Sparkline.tsx";
import { MOCK_KPI, MOCK_VENDOR_ROUTES } from "../lib/mocks.ts";

export function Economy() {
  const [paused, setPaused] = useState(false);
  const [routes, setRoutes] = useState(MOCK_VENDOR_ROUTES);

  const ledger = {
    plat_per_hour: -42.1,
    items_distributed: 184,
    vendor_sales: 26,
  };
  const queues = { loot_queue_len: 3, vendor_backlog_len: 7 };
  const platNow = MOCK_KPI.economy[MOCK_KPI.economy.length - 1];

  const toggleRoute = (id: string) =>
    setRoutes((rs) => rs.map((r) => (r.id === id ? { ...r, enabled: !r.enabled } : r)));
  const deleteRoute = (id: string) => setRoutes((rs) => rs.filter((r) => r.id !== id));

  return (
    <div>
      <PageHeader
        title="Economy"
        subtitle={
          <>
            <Coins className="w-3.5 h-3.5 text-state-warn" strokeWidth={1.75} />
            <span
              className={`uppercase tracking-[0.2em] ${
                paused ? "text-state-warn" : "text-state-ok"
              }`}
            >
              {paused ? "paused" : "active"}
            </span>
            <span className="text-neriak-dim">·</span>
            <span className="text-neriak-muted">vendor loop · loot-queue · krono-sink</span>
          </>
        }
        meta={
          <>
            <button
              onClick={() => setPaused((p) => !p)}
              className={`flex items-center gap-1.5 border rounded-sm px-2.5 py-1 font-mono text-[10px] uppercase tracking-[0.18em] ${
                paused
                  ? "border-state-ok text-state-ok hover:bg-state-ok/10"
                  : "border-state-warn text-state-warn hover:bg-state-warn/10"
              }`}
            >
              {paused ? (
                <Play className="w-3 h-3" strokeWidth={2} />
              ) : (
                <Pause className="w-3 h-3" strokeWidth={2} />
              )}
              {paused ? "resume" : "pause"}
            </button>
            <span className="text-[10px] font-mono text-state-warn border border-state-warn/40 bg-state-warn/5 rounded-sm px-2 py-1 uppercase tracking-[0.2em]">
              mock data
            </span>
          </>
        }
      />

      <div className="p-6 grid grid-cols-1 xl:grid-cols-[1fr_380px] gap-4">
        {/* Vendor routes */}
        <section className="border border-neriak-dim rounded-md bg-panel">
          <div className="flex items-center gap-2 px-3 py-2 border-b border-neriak-dim text-xs font-mono text-neriak-muted uppercase tracking-[0.15em]">
            <MapPin className="w-3.5 h-3.5 text-state-warn" strokeWidth={1.75} />
            vendor routes
            <button className="ml-auto flex items-center gap-1 text-neriak-magenta hover:text-neriak-magenta-bright">
              <Plus className="w-3 h-3" strokeWidth={2} />
              new route
            </button>
          </div>
          <table className="w-full font-mono text-sm">
            <thead className="text-neriak-muted uppercase tracking-[0.15em] text-[10px] bg-void">
              <tr>
                <th className="px-3 py-2 text-left">zone</th>
                <th className="px-3 py-2 text-left">npc</th>
                <th className="px-3 py-2 text-left">categories</th>
                <th className="px-3 py-2 text-left">notes</th>
                <th className="px-3 py-2 text-left">state</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {routes.map((r) => (
                <tr key={r.id} className="group border-t border-neriak-dim/40 hover:bg-elevated/40">
                  <td className="px-3 py-2 text-neriak-text">{r.zone}</td>
                  <td className="px-3 py-2 text-neriak-magenta">{r.npc_name}</td>
                  <td className="px-3 py-2">
                    <div className="flex flex-wrap gap-1">
                      {r.item_categories.map((c) => (
                        <span
                          key={c}
                          className="inline-block px-1.5 py-0.5 border border-state-warn/40 bg-state-warn/5 text-state-warn rounded-sm font-mono text-[10px]"
                        >
                          {c}
                        </span>
                      ))}
                    </div>
                  </td>
                  <td className="px-3 py-2 text-neriak-muted text-xs italic">{r.path_notes}</td>
                  <td className="px-3 py-2">
                    <button
                      onClick={() => toggleRoute(r.id)}
                      className={`font-mono text-[11px] uppercase tracking-[0.18em] ${
                        r.enabled ? "text-state-ok" : "text-neriak-dim"
                      }`}
                    >
                      {r.enabled ? "● on" : "○ off"}
                    </button>
                  </td>
                  <td className="px-3 py-2 text-right">
                    <button
                      onClick={() => deleteRoute(r.id)}
                      className="opacity-0 group-hover:opacity-100 text-neriak-dim hover:text-state-danger transition-opacity"
                    >
                      <Trash2 className="w-3.5 h-3.5" strokeWidth={1.75} />
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        {/* Ledger + queues */}
        <aside className="space-y-4">
          <section
            className="border rounded-md bg-panel p-4 space-y-3"
            style={{
              borderColor: "#fbbf2455",
              boxShadow: "0 0 32px -16px #fbbf24",
            }}
          >
            <div className="flex items-center justify-between">
              <span className="font-mono text-[10px] uppercase tracking-[0.2em] text-neriak-muted">
                plat balance
              </span>
              <span className="flex items-center gap-1 font-mono text-[11px] text-state-danger">
                <TrendingDown className="w-3 h-3" strokeWidth={2} />
                {ledger.plat_per_hour.toFixed(1)} pp/hr
              </span>
            </div>
            <div className="flex items-end justify-between">
              <div>
                <span className="font-mono text-4xl tabular-nums text-state-warn">
                  {platNow.toFixed(1)}
                </span>
                <span className="ml-1.5 font-mono text-xs text-neriak-muted uppercase tracking-wider">
                  pp
                </span>
              </div>
              <Sparkline values={MOCK_KPI.economy} color="#fbbf24" width={120} height={40} />
            </div>
            <div className="grid grid-cols-2 gap-3 pt-2 border-t border-neriak-dim/50 font-mono text-xs">
              <div>
                <div className="text-neriak-dim uppercase tracking-[0.15em] text-[10px]">
                  items distributed
                </div>
                <div className="text-neriak-text tabular-nums text-lg">
                  {ledger.items_distributed}
                </div>
              </div>
              <div>
                <div className="text-neriak-dim uppercase tracking-[0.15em] text-[10px]">
                  vendor sales
                </div>
                <div className="text-neriak-text tabular-nums text-lg">{ledger.vendor_sales}</div>
              </div>
            </div>
          </section>

          <section className="border border-neriak-dim rounded-md bg-panel p-4 space-y-3">
            <div className="font-mono text-[10px] uppercase tracking-[0.2em] text-neriak-muted">
              queues
            </div>
            {[
              { label: "loot queue", value: queues.loot_queue_len, color: "#cc44ff", max: 20 },
              {
                label: "vendor backlog",
                value: queues.vendor_backlog_len,
                color: "#fbbf24",
                max: 20,
              },
            ].map(({ label, value, color, max }) => (
              <div key={label} className="space-y-1">
                <div className="flex items-center justify-between font-mono text-xs">
                  <span className="text-neriak-muted">{label}</span>
                  <span className="tabular-nums" style={{ color }}>
                    {value}
                  </span>
                </div>
                <div className="h-1.5 bg-void border border-neriak-dim rounded-sm overflow-hidden">
                  <div
                    className="h-full"
                    style={{
                      width: `${Math.min(100, (value / max) * 100)}%`,
                      background: color,
                      boxShadow: `0 0 6px ${color}80`,
                    }}
                  />
                </div>
              </div>
            ))}
          </section>
        </aside>
      </div>
    </div>
  );
}
