import { useState } from "react";
import { CheckCircle2, RotateCcw, FileUp, Loader } from "lucide-react";
import { api } from "../lib/api.ts";

interface ConfigControlProps {
  sessionId: number;
  onOperationComplete?: () => void;
}

export function ConfigControl({ sessionId, onOperationComplete }: ConfigControlProps) {
  const [busy, setBusy] = useState<string | null>(null);
  const [lastMessage, setLastMessage] = useState<string | null>(null);
  const [promoteConfigName, setPromoteConfigName] = useState("");

  const handleAccept = async () => {
    setBusy("accept");
    try {
      const response = await api.post(`/admin/sessions/${sessionId}/config/accept`, {
        changes: {},
      });
      setLastMessage(`✓ ${(response as any).message}`);
      onOperationComplete?.();
    } catch (e) {
      setLastMessage(`✗ Accept failed: ${e instanceof Error ? e.message : "unknown error"}`);
    } finally {
      setBusy(null);
    }
  };

  const handleUndo = async () => {
    setBusy("undo");
    try {
      const response = await api.post(`/admin/sessions/${sessionId}/config/undo`, {});
      setLastMessage(`✓ ${(response as any).message}`);
      onOperationComplete?.();
    } catch (e) {
      setLastMessage(`✗ Undo failed: ${e instanceof Error ? e.message : "unknown error"}`);
    } finally {
      setBusy(null);
    }
  };

  const handlePromote = async () => {
    if (!promoteConfigName.trim()) {
      setLastMessage("✗ Config name required");
      return;
    }

    setBusy("promote");
    try {
      const response = await api.post(`/admin/sessions/${sessionId}/config/promote`, {
        config_name: promoteConfigName,
      });
      setLastMessage(`✓ ${(response as any).message}`);
      setPromoteConfigName("");
      onOperationComplete?.();
    } catch (e) {
      setLastMessage(`✗ Promote failed: ${e instanceof Error ? e.message : "unknown error"}`);
    } finally {
      setBusy(null);
    }
  };

  return (
    <section className="border border-neriak-dim rounded-sm p-3 space-y-2 bg-void/50">
      <h3 className="text-xs font-mono uppercase text-neriak-muted tracking-wider">
        Config Workflow
      </h3>

      <div className="flex flex-wrap gap-2">
        <button
          onClick={handleAccept}
          disabled={busy !== null}
          className="flex items-center gap-1 bg-state-ok/20 border border-state-ok text-state-ok hover:bg-state-ok/30 disabled:opacity-40 rounded-sm px-2.5 py-1 text-xs font-mono uppercase tracking-wider"
        >
          {busy === "accept" ? (
            <Loader className="w-3 h-3 animate-spin" strokeWidth={1.75} />
          ) : (
            <CheckCircle2 className="w-3 h-3" strokeWidth={1.75} />
          )}
          accept
        </button>

        <button
          onClick={handleUndo}
          disabled={busy !== null}
          className="flex items-center gap-1 bg-state-warn/20 border border-state-warn text-state-warn hover:bg-state-warn/30 disabled:opacity-40 rounded-sm px-2.5 py-1 text-xs font-mono uppercase tracking-wider"
        >
          {busy === "undo" ? (
            <Loader className="w-3 h-3 animate-spin" strokeWidth={1.75} />
          ) : (
            <RotateCcw className="w-3 h-3" strokeWidth={1.75} />
          )}
          undo
        </button>

        <div className="flex items-center gap-1 flex-1">
          <input
            value={promoteConfigName}
            onChange={(e) => setPromoteConfigName(e.target.value)}
            placeholder="config name"
            disabled={busy !== null}
            className="flex-1 min-w-[120px] bg-void border border-neriak-dim rounded-sm px-2 py-1 text-xs font-mono text-neriak-text placeholder-neriak-dim focus:border-neriak-cyan outline-none disabled:opacity-40"
          />
          <button
            onClick={handlePromote}
            disabled={busy !== null || !promoteConfigName.trim()}
            className="flex items-center gap-1 bg-neriak-cyan/20 border border-neriak-cyan text-neriak-cyan hover:bg-neriak-cyan/30 disabled:opacity-40 rounded-sm px-2.5 py-1 text-xs font-mono uppercase tracking-wider"
          >
            {busy === "promote" ? (
              <Loader className="w-3 h-3 animate-spin" strokeWidth={1.75} />
            ) : (
              <FileUp className="w-3 h-3" strokeWidth={1.75} />
            )}
            promote
          </button>
        </div>
      </div>

      {lastMessage && <div className="text-xs font-mono text-neriak-dim">{lastMessage}</div>}
    </section>
  );
}
