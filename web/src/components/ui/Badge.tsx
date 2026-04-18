import React from "react";

type BadgeVariant =
  | "status-active"
  | "status-idle"
  | "status-offline"
  | "status-error"
  | "count-primary"
  | "count-danger"
  | "count-warning";

interface BadgeProps {
  variant?: BadgeVariant;
  children: React.ReactNode;
  count?: number | string;
  className?: string;
}

const variantStyles: Record<BadgeVariant, string> = {
  "status-active":
    "border border-cyan-300/40 bg-cyan-400/10 text-cyan-200 font-rune text-[10px] uppercase tracking-[0.25em]",
  "status-idle":
    "border border-yellow-300/40 bg-yellow-400/10 text-yellow-200 font-rune text-[10px] uppercase tracking-[0.25em]",
  "status-offline":
    "border border-white/20 bg-white/5 text-white/40 font-rune text-[10px] uppercase tracking-[0.25em]",
  "status-error":
    "border border-red-400/40 bg-red-500/10 text-red-300 font-rune text-[10px] uppercase tracking-[0.25em]",
  "count-primary":
    "inline-flex items-center justify-center h-6 w-6 rounded-full border border-spectral/30 bg-spectral/10 text-spectral text-xs font-bold",
  "count-danger":
    "inline-flex items-center justify-center h-6 w-6 rounded-full border border-red-400/30 bg-red-500/10 text-red-300 text-xs font-bold",
  "count-warning":
    "inline-flex items-center justify-center h-6 w-6 rounded-full border border-amber-300/30 bg-amber-400/10 text-amber-200 text-xs font-bold",
};

export const Badge: React.FC<BadgeProps> = ({
  variant = "count-primary",
  children,
  count,
  className = "",
}) => {
  const variantStyle = variantStyles[variant];

  // For count variants, render as a circle with number
  if (variant.startsWith("count-")) {
    return (
      <span className={`${variantStyle} ${className}`}>{count || children}</span>
    );
  }

  // For status variants, render as a pill/chip
  return (
    <span className={`inline-block px-2 py-1 ${variantStyle} ${className}`}>
      {children}
    </span>
  );
};

Badge.displayName = "Badge";
