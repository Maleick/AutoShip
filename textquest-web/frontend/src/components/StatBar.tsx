interface StatBarProps {
  pct: number;
  kind: "hp" | "mp";
  width?: number;
}

const fadeColor = (color: string, pct: number) =>
  `color-mix(in srgb, ${color} ${pct}%, transparent)`;

function hpColor(pct: number): string {
  if (pct <= 15) return "var(--color-state-danger)"; // danger
  if (pct <= 40) return "var(--color-state-warn)"; // warn
  return "var(--color-state-ok)"; // ok
}

function mpColor(pct: number): string {
  if (pct <= 10) return "var(--color-state-danger)";
  if (pct <= 30) return "var(--color-state-warn)";
  return "var(--color-state-info)"; // mana blue
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
            boxShadow: `0 0 6px ${fadeColor(color, 50)}`,
          }}
        />
      </span>
      <span className="font-mono text-[11px] tabular-nums w-8 text-right" style={{ color }}>
        {clamped}%
      </span>
    </span>
  );
}
