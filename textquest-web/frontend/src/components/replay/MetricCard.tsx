import type { ReplayMetricCard } from "../../pages/replay-data.ts";
import { formatClock } from "../../pages/replay-data.ts";

interface MetricCardProps {
  metric: ReplayMetricCard;
  selected: boolean;
  activeTs: number;
  onSelect: () => void;
  onJumpToTs: (ts: number) => void;
}

const TONE_STYLES: Record<ReplayMetricCard["tone"], string> = {
  magenta: "border-neriak-magenta/40 bg-neriak-magenta/10 text-neriak-magenta",
  violet: "border-violet-400/30 bg-violet-400/10 text-violet-200",
  teal: "border-emerald-400/30 bg-emerald-400/10 text-emerald-200",
  amber: "border-amber-400/30 bg-amber-400/10 text-amber-200",
  rose: "border-rose-400/30 bg-rose-400/10 text-rose-200",
  cyan: "border-cyan-400/30 bg-cyan-400/10 text-cyan-200",
};

export function MetricCard({ metric, selected, activeTs, onSelect, onJumpToTs }: MetricCardProps) {
  const toneClass = TONE_STYLES[metric.tone];

  return (
    <section
      className={`rounded-lg border px-4 py-3 transition-colors ${
        selected
          ? "border-neriak-magenta bg-panel/90 shadow-[0_0_18px_-12px_var(--color-neriak-magenta)]"
          : "border-neriak-dim bg-panel/70 hover:border-neriak-magenta/50"
      }`}
    >
      <button className="w-full text-left" onClick={onSelect}>
        <div className="flex items-start justify-between gap-3">
          <div>
            <p className="font-mono text-[10px] uppercase tracking-[0.22em] text-neriak-muted">
              {metric.label}
            </p>
            <p className="mt-1 text-3xl font-semibold text-neriak-text">{metric.value}</p>
          </div>
          <span
            className={`mt-1 rounded-full border px-2 py-1 font-mono text-[10px] uppercase tracking-[0.15em] ${toneClass}`}
          >
            {metric.delta}
          </span>
        </div>
      </button>

      <div className="mt-3 space-y-1.5">
        {metric.rows.map((row) => {
          const active = Math.abs(row.ts - activeTs) <= 0.5;
          return (
            <button
              key={row.id}
              onClick={() => onJumpToTs(row.ts)}
              className={`w-full rounded-md border px-3 py-2 text-left transition-colors ${
                active
                  ? "border-neriak-magenta bg-neriak-magenta/10"
                  : "border-transparent hover:border-neriak-dim hover:bg-elevated/70"
              }`}
            >
              <div className="flex items-center justify-between gap-3">
                <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-neriak-muted">
                  {formatClock(row.ts)}
                </span>
                <span className="text-sm text-neriak-text">
                  {row.label ?? row.actor ?? row.kind ?? "event"}
                </span>
                <span className="font-mono text-[10px] text-neriak-muted">
                  {row.value ?? "jump"}
                </span>
              </div>
              <p className="mt-1 text-xs text-neriak-muted">{row.detail}</p>
            </button>
          );
        })}
      </div>
    </section>
  );
}
