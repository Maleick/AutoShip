import { WarningCircle } from "@phosphor-icons/react";
import { useAdminLogs } from "../hooks/useAdminLogs";

function getLevelColor(level: string) {
  switch (level.toLowerCase()) {
    case "error":
    case "critical":
      return "text-rose-300";
    case "warn":
    case "warning":
      return "text-amber-300";
    case "info":
      return "text-blue-300";
    default:
      return "text-white/60";
  }
}

function getLevelBgColor(level: string) {
  switch (level.toLowerCase()) {
    case "error":
    case "critical":
      return "bg-rose-500/10";
    case "warn":
    case "warning":
      return "bg-amber-500/10";
    case "info":
      return "bg-blue-500/10";
    default:
      return "bg-white/5";
  }
}

export default function LogViewerPanel() {
  const { logs, loading, error } = useAdminLogs();

  if (error) {
    return (
      <div className="rounded-2xl border border-rose-400/30 bg-rose-500/10 p-6 text-rose-200">
        <div className="text-sm font-semibold">Log Viewer Unavailable</div>
        <div className="mt-2 text-xs text-rose-200/70">{error}</div>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="rounded-2xl border border-white/10 bg-white/5 p-6">
        <div className="animate-pulse text-white/50">Loading logs...</div>
      </div>
    );
  }

  if (logs.length === 0) {
    return (
      <div className="rounded-2xl border border-white/10 bg-white/5 p-6">
        <div className="flex items-center gap-3">
          <WarningCircle className="h-5 w-5 text-white/40" />
          <span className="text-sm text-white/40">No recent logs</span>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="text-lg font-bold text-white">Recent Logs</div>

      <div className="max-h-96 space-y-2 overflow-y-auto rounded-2xl border border-white/10 bg-white/5 p-4">
        {logs.map((log, idx) => (
          <div
            key={idx}
            className={`rounded-lg ${getLevelBgColor(log.level)} p-3 text-xs`}
          >
            <div className="flex items-start justify-between gap-2">
              <span className={`font-semibold ${getLevelColor(log.level)}`}>
                [{log.level.toUpperCase()}]
              </span>
              <span className="text-white/40">
                {new Date(log.timestamp).toLocaleTimeString()}
              </span>
            </div>
            <div className="mt-1 text-white/70">{log.message}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
