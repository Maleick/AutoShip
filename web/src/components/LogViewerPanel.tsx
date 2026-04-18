import { Notepad } from "@phosphor-icons/react";
import { useAdminLogs } from "../hooks/useAdminLogs";

export function LogViewerPanel() {
  const { logs, loading, error } = useAdminLogs();

  if (error) {
    return (
      <section className="rounded-[1.5rem] border border-rose-400/30 bg-rose-500/10 p-5">
        <div className="mb-3 flex items-center gap-3 text-rose-200">
          <div className="flex h-11 w-11 items-center justify-center rounded-2xl border border-rose-400/30 bg-rose-500/10">
            <Notepad className="text-rose-300" size={22} />
          </div>
          <div>
            <h2 className="font-archaic text-xl">Log Stream</h2>
            <p className="font-tech text-xs uppercase tracking-[0.24em] text-rose-300/70">
              Error loading logs
            </p>
          </div>
        </div>
        <p className="text-sm text-rose-200/70">{error}</p>
      </section>
    );
  }

  if (loading) {
    return (
      <section className="rounded-[1.5rem] border border-white/10 bg-[#120a1d]/72 p-5">
        <div className="mb-3 flex items-center gap-3 text-white">
          <div className="flex h-11 w-11 items-center justify-center rounded-2xl border border-white/10 bg-white/5">
            <Notepad className="text-fuchsia-200 animate-pulse" size={22} />
          </div>
          <div>
            <h2 className="font-archaic text-xl">Log Stream</h2>
            <p className="font-tech text-xs uppercase tracking-[0.24em] text-white/45">
              Loading logs...
            </p>
          </div>
        </div>
      </section>
    );
  }

  if (logs.length === 0) {
    return (
      <section className="rounded-[1.5rem] border border-white/10 bg-[#120a1d]/72 p-5">
        <div className="mb-3 flex items-center gap-3 text-white">
          <div className="flex h-11 w-11 items-center justify-center rounded-2xl border border-white/10 bg-white/5">
            <Notepad className="text-fuchsia-200" size={22} />
          </div>
          <div>
            <h2 className="font-archaic text-xl">Log Stream</h2>
            <p className="font-tech text-xs uppercase tracking-[0.24em] text-white/45">
              No logs available
            </p>
          </div>
        </div>
        <p className="text-sm leading-6 text-white/60">
          Unified log entries appear here as the system generates them. The log stream is currently empty.
        </p>
      </section>
    );
  }

  return (
    <section className="rounded-[1.5rem] border border-white/10 bg-[#120a1d]/72 p-5">
      <div className="mb-4 flex items-center gap-3 text-white">
        <div className="flex h-11 w-11 items-center justify-center rounded-2xl border border-white/10 bg-white/5">
          <Notepad className="text-fuchsia-200" size={22} />
        </div>
        <div>
          <h2 className="font-archaic text-xl">Log Stream</h2>
          <p className="font-tech text-xs uppercase tracking-[0.24em] text-white/45">
            {logs.length} entries
          </p>
        </div>
      </div>

      <div className="max-h-64 overflow-y-auto rounded-lg border border-white/5 bg-white/5 p-3 font-mono text-xs">
        {logs.map((log, index) => (
          <div
            key={index}
            className={`mb-2 pb-2 border-b border-white/5 last:border-b-0 ${
              log.level === "error"
                ? "text-rose-300"
                : log.level === "warn"
                  ? "text-amber-300"
                  : log.level === "debug"
                    ? "text-blue-300/70"
                    : "text-white/70"
            }`}
          >
            <div className="flex items-start gap-2">
              <span className="text-white/40">
                {new Date(log.timestamp).toLocaleTimeString()}
              </span>
              <span className="inline-block w-12 text-white/50 uppercase">
                {log.level}
              </span>
              {log.source && (
                <span className="inline-block text-white/35 before:content-['['] after:content-[']']">
                  {log.source}
                </span>
              )}
            </div>
            <div className="ml-0 mt-1 text-white/60">{log.message}</div>
          </div>
        ))}
      </div>
    </section>
  );
}

export default LogViewerPanel;
