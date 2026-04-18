import { Clock, Cpu, HardDrive, Pulse } from "@phosphor-icons/react";
import { useAdminMetrics } from "../hooks/useAdminMetrics";

export function PerformancePanel() {
  const { metrics, loading, error } = useAdminMetrics();

  if (error) {
    return (
      <section className="rounded-[1.5rem] border border-rose-400/30 bg-rose-500/10 p-5">
        <div className="mb-3 flex items-center gap-3 text-rose-200">
          <div className="flex h-11 w-11 items-center justify-center rounded-2xl border border-rose-400/30 bg-rose-500/10">
            <Cpu className="text-rose-300" size={22} />
          </div>
          <div>
            <h2 className="font-archaic text-xl">Diagnostics</h2>
            <p className="font-tech text-xs uppercase tracking-[0.24em] text-rose-300/70">
              Error loading metrics
            </p>
          </div>
        </div>
        <p className="text-sm text-rose-200/70">{error}</p>
      </section>
    );
  }

  if (loading || !metrics) {
    return (
      <section className="rounded-[1.5rem] border border-white/10 bg-[#120a1d]/72 p-5">
        <div className="mb-3 flex items-center gap-3 text-white">
          <div className="flex h-11 w-11 items-center justify-center rounded-2xl border border-white/10 bg-white/5">
            <Cpu className="text-cyan-200 animate-pulse" size={22} />
          </div>
          <div>
            <h2 className="font-archaic text-xl">Diagnostics</h2>
            <p className="font-tech text-xs uppercase tracking-[0.24em] text-white/45">
              Loading metrics...
            </p>
          </div>
        </div>
      </section>
    );
  }

  const formatUptime = (secs: number) => {
    const hours = Math.floor(secs / 3600);
    const minutes = Math.floor((secs % 3600) / 60);
    return `${hours}h ${minutes}m`;
  };

  return (
    <section className="rounded-[1.5rem] border border-white/10 bg-[#120a1d]/72 p-5">
      <div className="mb-4 flex items-center gap-3 text-white">
        <div className="flex h-11 w-11 items-center justify-center rounded-2xl border border-white/10 bg-white/5">
          <Cpu className="text-cyan-200" size={22} />
        </div>
        <div>
          <h2 className="font-archaic text-xl">Diagnostics</h2>
          <p className="font-tech text-xs uppercase tracking-[0.24em] text-white/45">
            Performance metrics and diagnostics
          </p>
        </div>
      </div>

      <div className="grid gap-3">
        {/* Response Time */}
        <div className="rounded-lg border border-blue-400/30 bg-blue-500/10 p-4">
          <div className="flex items-center gap-2">
            <Clock className="h-5 w-5 text-blue-300" />
            <span className="text-xs uppercase tracking-[0.18em] text-blue-300/70">Response Time</span>
          </div>
          <div className="mt-2 text-2xl font-bold text-blue-200">
            {metrics.response_time_ms}ms
          </div>
        </div>

        {/* Uptime */}
        <div className="rounded-lg border border-emerald-400/30 bg-emerald-500/10 p-4">
          <div className="flex items-center gap-2">
            <Pulse className="h-5 w-5 text-emerald-300" />
            <span className="text-xs uppercase tracking-[0.18em] text-emerald-300/70">Uptime</span>
          </div>
          <div className="mt-2 text-2xl font-bold text-emerald-200">
            {formatUptime(metrics.uptime_secs)}
          </div>
        </div>

        {/* Memory Usage */}
        <div className="rounded-lg border border-amber-400/30 bg-amber-500/10 p-4">
          <div className="flex items-center gap-2">
            <HardDrive className="h-5 w-5 text-amber-300" />
            <span className="text-xs uppercase tracking-[0.18em] text-amber-300/70">Memory</span>
          </div>
          <div className="mt-2 text-2xl font-bold text-amber-200">
            {metrics.memory_usage_mb}MB
          </div>
        </div>
      </div>

      <div className="mt-4 text-xs text-white/40">
        Updated: {new Date(metrics.timestamp).toLocaleTimeString()}
      </div>
    </section>
  );
}

export default PerformancePanel;
