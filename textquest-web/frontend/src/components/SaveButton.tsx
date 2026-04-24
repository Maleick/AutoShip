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
            ? "bg-state-ok/20 border-state-ok text-state-ok"
            : isError
              ? "bg-state-danger/20 border-state-danger text-state-danger"
              : "bg-neriak-magenta/20 border-neriak-magenta text-neriak-magenta hover:bg-neriak-magenta/30"
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
        <span className="font-mono text-[10px] text-state-danger uppercase tracking-[0.15em]">
          {error}
        </span>
      )}
    </div>
  );
}
