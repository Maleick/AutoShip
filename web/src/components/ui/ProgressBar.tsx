import React from "react";

type ProgressColor = "hp" | "mana" | "endurance" | "primary" | "danger" | "warning";

interface ProgressBarProps {
  percent: number;
  color?: ProgressColor;
  label?: string;
  showPercent?: boolean;
  className?: string;
  height?: "sm" | "md" | "lg";
}

const colorStyles: Record<ProgressColor, { bar: string; bg: string }> = {
  hp: {
    bar: "bg-gradient-to-r from-cyan-400 to-cyan-300",
    bg: "bg-red-950/30",
  },
  mana: {
    bar: "bg-gradient-to-r from-blue-400 to-blue-300",
    bg: "bg-blue-950/30",
  },
  endurance: {
    bar: "bg-gradient-to-r from-green-400 to-green-300",
    bg: "bg-green-950/30",
  },
  primary: {
    bar: "bg-gradient-to-r from-spectral to-spectral/80",
    bg: "bg-spectral/10",
  },
  danger: {
    bar: "bg-gradient-to-r from-red-500 to-red-400",
    bg: "bg-red-950/30",
  },
  warning: {
    bar: "bg-gradient-to-r from-amber-400 to-amber-300",
    bg: "bg-amber-950/30",
  },
};

const heightStyles = {
  sm: "h-1.5",
  md: "h-2",
  lg: "h-3",
};

export const ProgressBar: React.FC<ProgressBarProps> = ({
  percent,
  color = "hp",
  label,
  showPercent = true,
  className = "",
  height = "md",
}) => {
  const clampedPercent = Math.min(Math.max(percent, 0), 100);
  const colorStyle = colorStyles[color];
  const heightStyle = heightStyles[height];

  return (
    <div className={`flex flex-col gap-1 ${className}`}>
      {label && (
        <div className="flex items-center justify-between text-[11px] text-white/70">
          <span>{label}</span>
          {showPercent && (
            <span className="font-mono text-white/50">{clampedPercent}%</span>
          )}
        </div>
      )}
      <div className={`w-full border border-white/10 overflow-hidden ${colorStyle.bg}`}>
        <div
          className={`${heightStyle} ${colorStyle.bar} transition-all duration-300 ease-out`}
          style={{ width: `${clampedPercent}%` }}
        />
      </div>
    </div>
  );
};

ProgressBar.displayName = "ProgressBar";
