import { Check, Loader2, Save as SaveIcon, AlertTriangle } from "lucide-react";
import type { SaveState } from "../hooks/useSave.ts";

interface SaveButtonProps {
  state: SaveState;
  error: string | null;
  onClick: () => void;
  disabled?: boolean;
  label?: string;
}

export function SaveButton({ state, error, onClick, disabled, label = "save" }: SaveButtonProps) {
  const isSaving = state === "saving";
  const isSaved = state === "saved";
  const isError = state === "error";

  return (
    <div className="flex items-center gap-2">
      <button
        onClick={onClick}
        disabled={disabled || isSaving}
        className={`flex items-center gap-1.5 rounded-sm px-4 py-1.5 text-xs font-mono uppercase tracking-[0.2em] border transition-colors ${
          isSaved
            ? "bg-[#34d399]/20 border-[#34d399] text-[#34d399]"
            : isError
              ? "bg-[#ef4444]/20 border-[#ef4444] text-[#ef4444]"
              : "bg-[#cc44ff]/20 border-[#cc44ff] text-[#cc44ff] hover:bg-[#cc44ff]/30"
        } disabled:opacity-40`}
      >
        {isSaving ? (
          <Loader2 className="w-3.5 h-3.5 animate-spin" strokeWidth={2} />
        ) : isSaved ? (
          <Check className="w-3.5 h-3.5" strokeWidth={2} />
        ) : isError ? (
          <AlertTriangle className="w-3.5 h-3.5" strokeWidth={2} />
        ) : (
          <SaveIcon className="w-3.5 h-3.5" strokeWidth={1.75} />
        )}
        {isSaving ? "saving…" : isSaved ? "saved" : isError ? "retry" : label}
      </button>
      {error && (
        <span className="font-mono text-[10px] text-[#ef4444] uppercase tracking-[0.15em]">
          {error}
        </span>
      )}
    </div>
  );
}
