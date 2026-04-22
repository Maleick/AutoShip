import React from "react";
import {
  Check,
  X,
  Warning,
  Clock,
} from "@phosphor-icons/react";

type StatusType = "online" | "offline" | "error" | "warning" | "idle";

interface StatusIconProps {
  status: StatusType;
  size?: "sm" | "md" | "lg";
  label?: string;
  className?: string;
  showLabel?: boolean;
}

const statusConfig: Record<
  StatusType,
  {
    icon: React.FC<{ size: number; weight: "bold" | "fill"; className?: string }>;
    color: string;
    bgColor: string;
    label: string;
  }
> = {
  online: {
    icon: Check,
    color: "text-cyan-300",
    bgColor: "bg-cyan-400/10",
    label: "Online",
  },
  offline: {
    icon: X,
    color: "text-white/40",
    bgColor: "bg-white/5",
    label: "Offline",
  },
  error: {
    icon: X,
    color: "text-red-400",
    bgColor: "bg-red-500/10",
    label: "Error",
  },
  warning: {
    icon: Warning,
    color: "text-amber-300",
    bgColor: "bg-amber-400/10",
    label: "Warning",
  },
  idle: {
    icon: Clock,
    color: "text-yellow-300",
    bgColor: "bg-yellow-400/10",
    label: "Idle",
  },
};

const sizeMap = {
  sm: 12,
  md: 16,
  lg: 20,
};

export const StatusIcon: React.FC<StatusIconProps> = ({
  status,
  size = "md",
  label,
  className = "",
  showLabel = true,
}) => {
  const config = statusConfig[status];
  const IconComponent = config.icon;
  const iconSize = sizeMap[size];

  return (
    <div className={`flex items-center gap-2 ${className}`}>
      <div className={`p-1.5 rounded-full border border-white/10 ${config.bgColor}`}>
        <IconComponent
          size={iconSize}
          weight="fill"
          className={config.color}
        />
      </div>
      {showLabel && (
        <span className="text-[11px] uppercase tracking-[0.25em] text-white/70 font-rune">
          {label || config.label}
        </span>
      )}
    </div>
  );
};

StatusIcon.displayName = "StatusIcon";
