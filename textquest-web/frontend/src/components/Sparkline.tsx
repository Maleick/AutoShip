interface SparklineProps {
  values: number[];
  color: string;
  width?: number;
  height?: number;
  filled?: boolean;
}

export function Sparkline({
  values,
  color,
  width = 120,
  height = 32,
  filled = true,
}: SparklineProps) {
  if (values.length < 2) return <svg width={width} height={height} />;
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const step = width / (values.length - 1);
  const pts = values.map((v, i) => {
    const x = i * step;
    const y = height - 2 - ((v - min) / span) * (height - 4);
    return [x, y] as const;
  });
  const path = pts
    .map(([x, y], i) => `${i === 0 ? "M" : "L"}${x.toFixed(1)} ${y.toFixed(1)}`)
    .join(" ");
  const area = filled ? `${path} L${width} ${height} L0 ${height} Z` : undefined;
  const last = pts[pts.length - 1];
  return (
    <svg width={width} height={height} className="block">
      {area && <path d={area} fill={`${color}22`} />}
      <path d={path} fill="none" stroke={color} strokeWidth="1.5" />
      <circle
        cx={last[0]}
        cy={last[1]}
        r="2"
        fill={color}
        style={{ filter: `drop-shadow(0 0 3px ${color})` }}
      />
    </svg>
  );
}
