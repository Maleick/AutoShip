import { Clock, HardDrive, Pulse } from "@phosphor-icons/react";
import { useAdminMetrics } from "../hooks/useAdminMetrics";

export default function PerformancePanel() {
  const { metrics, loading, error } = useAdminMetrics();

  if (error) {
    return (
      <div className="rounded-2xl border border-rose-400/30 bg-rose-500/10 p-6 text-rose-200">
        <div className="text-sm font-semibold">Metrics Unavailable</div>
        <div className="mt-2 text-xs text-rose-200/70">{error}</div>
      </div>
    );
  }

  if (loading || !metrics) {
    return (
      <div className="rounded-2xl border border-white/10 bg-white/5 p-6">
        <div className="animate-pulse text-white/50">Loading metrics...</div>
      </div>
    );
  }

  const formatUptime = (secs: number) => {
    const hours = Math.floor(secs / 3600);
    const minutes = Math.floor((secs % 3600) / 60);
    return `${hours}h ${minutes}m`;
  };

  return (
    <div className="space-y-4">
      <div className="text-lg font-bold text-white">Performance Metrics</div>

      <div className="grid grid-cols-3 gap-4">
        {/* Response Time */}
        <div className="rounded-2xl border border-blue-400/30 bg-blue-500/10 p-4">
          <div className="flex items-center gap-2">
            <Clock className="h-5 w-5 text-blue-300" />
            <span className="text-xs uppercase tracking-[0.18em] text-blue-300/70">Response Time</span>
          </div>
          <div className="mt-2 text-2xl font-bold text-blue-200">
            {metrics.response_time_ms}ms
          </div>
        </div>

        {/* Uptime */}
        <div className="rounded-2xl border border-emerald-400/30 bg-emerald-500/10 p-4">
          <div className="flex items-center gap-2">
            <Pulse className="h-5 w-5 text-emerald-300" />
            <span className="text-xs uppercase tracking-[0.18em] text-emerald-300/70">Uptime</span>
          </div>
          <div className="mt-2 text-2xl font-bold text-emerald-200">
            {formatUptime(metrics.uptime_secs)}
          </div>
        </div>

        {/* Memory Usage */}
        <div className="rounded-2xl border border-amber-400/30 bg-amber-500/10 p-4">
          <div className="flex items-center gap-2">
            <HardDrive className="h-5 w-5 text-amber-300" />
            <span className="text-xs uppercase tracking-[0.18em] text-amber-300/70">Memory</span>
          </div>
          <div className="mt-2 text-2xl font-bold text-amber-200">
            {metrics.memory_usage_mb}MB
          </div>
        </div>
      </div>

      <div className="text-xs text-white/40">
        Updated: {new Date(metrics.timestamp).toLocaleTimeString()}
      </div>
    </div>
  );
}
