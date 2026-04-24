interface StatBarProps {
  pct: number;
  kind: "hp" | "mp";
  width?: number;
}

function hpColor(pct: number): string {
  if (pct <= 15) return "#ef4444"; // danger
  if (pct <= 40) return "#fbbf24"; // warn
  return "#34d399"; // ok
}

function mpColor(pct: number): string {
  if (pct <= 10) return "#ef4444";
  if (pct <= 30) return "#fbbf24";
  return "#60a5fa"; // mana blue
}

export function StatBar({ pct, kind, width = 64 }: StatBarProps) {
  const clamped = Math.max(0, Math.min(100, pct));
  const color = kind === "hp" ? hpColor(clamped) : mpColor(clamped);
  return (
    <span className="inline-flex items-center gap-2">
      <span
        className="relative inline-block h-2 rounded-sm bg-void border border-neriak-dim overflow-hidden"
        style={{ width }}
      >
        <span
          className="absolute inset-y-0 left-0 rounded-sm"
          style={{
            width: `${clamped}%`,
            background: color,
            boxShadow: `0 0 6px ${color}80`,
          }}
        />
      </span>
      <span className="font-mono text-[11px] tabular-nums w-8 text-right" style={{ color }}>
        {clamped}%
      </span>
    </span>
  );
}
