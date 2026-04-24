import type { ReactNode } from "react";

interface CardProps {
  title?: string;
  icon?: ReactNode;
  accent?: "magenta" | "cyan" | "ok" | "warn" | "danger" | "info";
  children: ReactNode;
  className?: string;
}

const accentBorder: Record<NonNullable<CardProps["accent"]>, string> = {
  magenta: "border-[#cc44ff]/40",
  cyan: "border-[#00e5ff]/40",
  ok: "border-[#34d399]/40",
  warn: "border-[#fbbf24]/40",
  danger: "border-[#ef4444]/40",
  info: "border-[#60a5fa]/40",
};

export function Card({ title, icon, accent, children, className = "" }: CardProps) {
  const border = accent ? accentBorder[accent] : "border-[#503c6e]";
  return (
    <div
      className={`bg-[#1a0a2e] border ${border} rounded-md p-4 shadow-[0_0_24px_-12px_rgba(204,68,255,0.35)] ${className}`}
    >
      {title && (
        <div className="flex items-center gap-2 mb-3">
          {icon && <span className="text-[#a096b4]">{icon}</span>}
          <h2 className="text-xs font-semibold text-[#a096b4] uppercase tracking-[0.18em]">
            {title}
          </h2>
        </div>
      )}
      {children}
    </div>
  );
}
