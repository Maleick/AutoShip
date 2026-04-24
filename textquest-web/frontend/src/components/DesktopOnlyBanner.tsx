import { Monitor } from "lucide-react";

/**
 * Shown above pages whose fixed multi-column editor layouts don't degrade
 * gracefully below ~900px. Hidden on `md` breakpoint (≥768px) and up.
 */
export function DesktopOnlyBanner({ page }: { page: string }) {
  return (
    <div className="md:hidden border-b border-state-warn/40 bg-state-warn/10 px-4 py-3 flex items-start gap-3">
      <Monitor className="w-4 h-4 text-state-warn shrink-0 mt-0.5" strokeWidth={1.75} />
      <div className="font-mono text-xs text-neriak-text space-y-1">
        <div className="text-state-warn uppercase tracking-[0.2em] text-[10px]">
          desktop recommended
        </div>
        <p className="text-neriak-muted">
          The {page} editor uses a fixed multi-column layout that requires at least 900px of
          horizontal space. Rotate your device or switch to a larger screen for the full experience.
        </p>
      </div>
    </div>
  );
}
