import type { ReactNode } from "react";

interface PageHeaderProps {
  title: string;
  subtitle?: ReactNode;
  meta?: ReactNode;
}

export function PageHeader({ title, subtitle, meta }: PageHeaderProps) {
  return (
    <div className="px-6 pt-6 pb-4 border-b border-neriak-dim/50 flex items-baseline justify-between">
      <div className="flex items-baseline gap-4">
        <h1 className="font-[Cinzel,serif] text-3xl tracking-[0.1em] text-neriak-text uppercase">
          {title}
        </h1>
        {subtitle && (
          <div className="font-mono text-xs text-neriak-muted flex items-center gap-2">
            {subtitle}
          </div>
        )}
      </div>
      {meta && (
        <div className="font-mono text-xs text-neriak-muted flex items-center gap-2">{meta}</div>
      )}
    </div>
  );
}
