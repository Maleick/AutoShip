import type { ReactNode } from "react";

interface CardProps {
  title?: string;
  icon?: ReactNode;
  accent?: "magenta" | "cyan" | "ok" | "warn" | "danger" | "info";
  children: ReactNode;
  className?: string;
}

const accentBorder: Record<NonNullable<CardProps["accent"]>, string> = {
  magenta: "border-neriak-magenta/40",
  cyan: "border-neriak-cyan/40",
  ok: "border-state-ok/40",
  warn: "border-state-warn/40",
  danger: "border-state-danger/40",
  info: "border-state-info/40",
};

export function Card({ title, icon, accent, children, className = "" }: CardProps) {
  const border = accent ? accentBorder[accent] : "border-neriak-dim";
  return (
    <div
      className={`bg-panel border ${border} rounded-md p-4 shadow-[0_0_24px_-12px_rgba(204,68,255,0.35)] ${className}`}
    >
      {title && (
        <div className="flex items-center gap-2 mb-3">
          {icon && <span className="text-neriak-muted">{icon}</span>}
          <h2 className="text-xs font-semibold text-neriak-muted uppercase tracking-[0.18em]">
            {title}
          </h2>
        </div>
      )}
      {children}
    </div>
  );
}
