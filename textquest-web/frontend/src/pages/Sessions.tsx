import { useCallback, useMemo, useState } from "react";
import { Pause, Play, Radio, Send, Terminal, Users, AlertTriangle } from "lucide-react";
import { useApi } from "../hooks/useApi.ts";
import { api } from "../lib/api.ts";
import { MOCK_SESSIONS, USE_MOCKS } from "../lib/mocks.ts";
import { PageHeader } from "../components/PageHeader.tsx";
import { StatBar } from "../components/StatBar.tsx";
import type {
  CommandScope,
  SessionControlRecord,
  SessionState,
  SlashCommandResponse,
} from "../lib/types.ts";

const GROUP_IDS = [1, 2, 3, 4, 5, 6, 7, 8];

const STATE_COLOR: Record<SessionState, string> = {
  active: "text-state-ok",
  idle: "text-neriak-muted",
  paused: "text-state-warn",
  error: "text-state-danger",
};

const STATE_DOT: Record<SessionState, string> = {
  active: "bg-state-ok shadow-[0_0_6px_#34d399]",
  idle: "bg-neriak-muted",
  paused: "bg-state-warn",
  error: "bg-state-danger animate-pulse",
};

function bulkEndpoint(id: number, op: "pause" | "resume") {
  return `/sessions/${id}/${op}`;
}

export function Sessions() {
  const apiResult = useApi<SessionControlRecord[]>("/api/sessions/control");
  const useMockData = USE_MOCKS || (!!apiResult.error && !apiResult.data);
  const data = useMockData ? MOCK_SESSIONS : apiResult.data;
  const loading = useMockData ? false : apiResult.loading;
  const error = useMockData ? null : apiResult.error;
  const refetch = apiResult.refetch;
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [cmdScope, setCmdScope] = useState<CommandScope>("group");
  const [cmdText, setCmdText] = useState("");
  const [cmdLog, setCmdLog] = useState<SlashCommandResponse[]>([]);
  const [busy, setBusy] = useState<number | null>(null);

  const rows = useMemo(() => data ?? [], [data]);

  const toggleSelect = useCallback((id: number) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const toggleAll = useCallback(() => {
    setSelected((prev) =>
      prev.size === rows.length ? new Set() : new Set(rows.map((r) => r.session_id)),
    );
  }, [rows]);

  const runOp = async (id: number, op: "pause" | "resume") => {
    setBusy(id);
    try {
      await api.put<SessionControlRecord>(bulkEndpoint(id, op), {});
      refetch();
    } catch (e) {
      console.error(e);
    } finally {
      setBusy(null);
    }
  };

  const bulk = async (op: "pause" | "resume") => {
    await Promise.all(
      [...selected].map((id) =>
        api.put<SessionControlRecord>(bulkEndpoint(id, op), {}).catch(() => null),
      ),
    );
    refetch();
  };

  const moveToGroup = async (id: number, group_id: number) => {
    setBusy(id);
    try {
      await api.put<SessionControlRecord>(`/sessions/${id}/group`, { group_id });
      refetch();
    } finally {
      setBusy(null);
    }
  };

  const sendCommand = async () => {
    if (!cmdText.trim()) return;
    const targets =
      cmdScope === "all"
        ? rows.map((r) => r.session_id)
        : selected.size > 0
          ? [...selected]
          : rows.filter((r) => r.state === "active").map((r) => r.session_id);

    const responses = await Promise.all(
      targets.map((id) =>
        api
          .post<SlashCommandResponse>(`/sessions/${id}/command`, {
            command: cmdText,
            scope: cmdScope,
          })
          .catch((err: unknown) => ({
            session_id: id,
            command: cmdText,
            scope_used: cmdScope,
            accepted: false,
            message: err instanceof Error ? err.message : "error",
          })),
      ),
    );
    setCmdLog((prev) => [...responses.reverse(), ...prev].slice(0, 50));
    setCmdText("");
  };

  const activeCount = rows.filter((r) => r.state === "active").length;
  const pausedCount = rows.filter((r) => r.state === "paused").length;
  const errorCount = rows.filter((r) => r.state === "error").length;

  return (
    <div>
      <PageHeader
        title="Sessions"
        subtitle={
          <>
            <Users className="w-3.5 h-3.5 text-neriak-cyan" strokeWidth={1.75} />
            <span>{rows.length} clients</span>
            <span className="text-neriak-dim">·</span>
            <span className="text-state-ok">{activeCount} active</span>
            {pausedCount > 0 && (
              <>
                <span className="text-neriak-dim">·</span>
                <span className="text-state-warn">{pausedCount} paused</span>
              </>
            )}
            {errorCount > 0 && (
              <>
                <span className="text-neriak-dim">·</span>
                <span className="text-state-danger">{errorCount} error</span>
              </>
            )}
          </>
        }
        meta={
          <>
            {loading && <span>loading…</span>}
            {error && (
              <span className="flex items-center gap-1 text-state-danger">
                <AlertTriangle className="w-3.5 h-3.5" /> {error}
              </span>
            )}
            <button
              onClick={refetch}
              className="border border-neriak-dim hover:border-neriak-cyan rounded-sm px-2 py-1"
            >
              refresh
            </button>
          </>
        }
      />
      <div className="p-6 space-y-4">
        {useMockData && (
          <div className="text-[10px] font-mono text-state-warn border border-state-warn/40 bg-state-warn/5 rounded-sm px-2 py-1 inline-block uppercase tracking-[0.2em]">
            mock data · backend offline
          </div>
        )}

        {selected.size > 0 && (
          <div className="flex items-center gap-3 border border-neriak-magenta/50 bg-panel rounded-sm px-3 py-2 font-mono text-xs">
            <span className="text-neriak-magenta">{selected.size} selected</span>
            <button
              onClick={() => bulk("pause")}
              className="flex items-center gap-1 text-state-warn hover:text-[#fde68a]"
            >
              <Pause className="w-3.5 h-3.5" strokeWidth={1.75} /> pause all
            </button>
            <button
              onClick={() => bulk("resume")}
              className="flex items-center gap-1 text-state-ok hover:text-[#6ee7b7]"
            >
              <Play className="w-3.5 h-3.5" strokeWidth={1.75} /> resume all
            </button>
            <button
              onClick={() => setSelected(new Set())}
              className="ml-auto text-neriak-muted hover:text-neriak-text"
            >
              clear
            </button>
          </div>
        )}

        <div className="border border-neriak-dim rounded-md overflow-hidden overflow-x-auto">
          <table className="w-full min-w-[720px] font-mono text-sm">
            <thead className="bg-void text-neriak-muted uppercase tracking-[0.15em] text-[10px]">
              <tr>
                <th className="w-8 px-3 py-2 text-left">
                  <input
                    type="checkbox"
                    checked={selected.size === rows.length && rows.length > 0}
                    onChange={toggleAll}
                    className="accent-neriak-magenta"
                  />
                </th>
                <th className="px-3 py-2 text-left">id</th>
                <th className="px-3 py-2 text-left">character</th>
                <th className="px-3 py-2 text-left">zone</th>
                <th className="px-3 py-2 text-left">hp</th>
                <th className="px-3 py-2 text-left">mp</th>
                <th className="px-3 py-2 text-left">group</th>
                <th className="px-3 py-2 text-left">scope</th>
                <th className="px-3 py-2 text-left">state</th>
                <th className="px-3 py-2 text-right">actions</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => {
                const isSel = selected.has(r.session_id);
                const isBusy = busy === r.session_id;
                return (
                  <tr
                    key={r.session_id}
                    className={`border-t border-neriak-dim/50 hover:bg-elevated/60 ${
                      isSel ? "bg-elevated" : ""
                    }`}
                  >
                    <td className="px-3 py-2">
                      <input
                        type="checkbox"
                        checked={isSel}
                        onChange={() => toggleSelect(r.session_id)}
                        className="accent-neriak-magenta"
                      />
                    </td>
                    <td className="px-3 py-2 text-neriak-muted">{r.session_id}</td>
                    <td className="px-3 py-2 text-neriak-text">
                      {r.character_name ?? "—"}
                      {r.class && (
                        <span className="ml-2 text-[10px] text-neriak-dim">
                          {r.class.toUpperCase()}
                        </span>
                      )}
                      {r.level && (
                        <span className="ml-1 text-[10px] text-neriak-dim">L{r.level}</span>
                      )}
                    </td>
                    <td className="px-3 py-2 text-neriak-muted text-xs">{r.zone ?? "—"}</td>
                    <td className="px-3 py-2">
                      {r.hp_pct !== undefined ? (
                        <StatBar pct={r.hp_pct} kind="hp" />
                      ) : (
                        <span className="text-neriak-dim">—</span>
                      )}
                    </td>
                    <td className="px-3 py-2">
                      {r.mana_pct !== undefined ? (
                        <StatBar pct={r.mana_pct} kind="mp" />
                      ) : (
                        <span className="text-neriak-dim">—</span>
                      )}
                    </td>
                    <td className="px-3 py-2">
                      <select
                        value={r.group_id}
                        onChange={(e) => moveToGroup(r.session_id, Number(e.target.value))}
                        disabled={isBusy}
                        className="bg-void border border-neriak-dim rounded-sm px-1.5 py-0.5 text-neriak-text focus:border-neriak-cyan outline-none"
                      >
                        {GROUP_IDS.map((g) => (
                          <option key={g} value={g}>
                            G{g}
                          </option>
                        ))}
                      </select>
                    </td>
                    <td className="px-3 py-2 text-neriak-muted">{r.routing_scope}</td>
                    <td className="px-3 py-2">
                      <span className={`flex items-center gap-2 ${STATE_COLOR[r.state]}`}>
                        <span
                          className={`inline-block w-1.5 h-1.5 rounded-full ${STATE_DOT[r.state]}`}
                        />
                        {r.state}
                      </span>
                    </td>
                    <td className="px-3 py-2 text-right">
                      {r.state === "paused" ? (
                        <button
                          disabled={isBusy}
                          onClick={() => runOp(r.session_id, "resume")}
                          className="inline-flex items-center gap-1 text-state-ok hover:text-[#6ee7b7] disabled:opacity-40"
                        >
                          <Play className="w-3.5 h-3.5" strokeWidth={1.75} /> resume
                        </button>
                      ) : (
                        <button
                          disabled={isBusy || r.state === "error"}
                          onClick={() => runOp(r.session_id, "pause")}
                          className="inline-flex items-center gap-1 text-state-warn hover:text-[#fde68a] disabled:opacity-40"
                        >
                          <Pause className="w-3.5 h-3.5" strokeWidth={1.75} /> pause
                        </button>
                      )}
                    </td>
                  </tr>
                );
              })}
              {rows.length === 0 && !loading && (
                <tr>
                  <td colSpan={10} className="px-3 py-8 text-center text-neriak-dim italic">
                    no sessions
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        <section className="border border-neriak-dim rounded-md bg-panel">
          <div className="flex items-center gap-2 px-3 py-2 border-b border-neriak-dim text-xs font-mono text-neriak-muted uppercase tracking-[0.15em]">
            <Terminal className="w-3.5 h-3.5 text-neriak-cyan" strokeWidth={1.75} />
            command relay
          </div>
          <div className="flex items-center gap-2 p-3">
            <select
              aria-label="Command scope"
              value={cmdScope}
              onChange={(e) => setCmdScope(e.target.value as CommandScope)}
              className="bg-void border border-neriak-dim rounded-sm px-2 py-1.5 text-xs font-mono text-neriak-text focus:border-neriak-cyan outline-none"
            >
              <option value="self">self</option>
              <option value="group">group</option>
              <option value="all">all</option>
            </select>
            <div className="flex-1 flex items-center gap-2 bg-void border border-neriak-dim rounded-sm px-2 py-1.5 focus-within:border-neriak-magenta">
              <Radio className="w-3.5 h-3.5 text-neriak-magenta" strokeWidth={1.75} />
              <input
                aria-label="Slash command"
                value={cmdText}
                onChange={(e) => setCmdText(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && sendCommand()}
                placeholder="/shout camp check"
                className="flex-1 bg-transparent font-mono text-sm text-neriak-text placeholder-neriak-dim outline-none"
              />
            </div>
            <button
              onClick={sendCommand}
              disabled={!cmdText.trim()}
              className="flex items-center gap-1 bg-neriak-magenta/20 border border-neriak-magenta text-neriak-magenta hover:bg-neriak-magenta/30 disabled:opacity-40 rounded-sm px-3 py-1.5 text-xs font-mono uppercase tracking-wider"
            >
              <Send className="w-3.5 h-3.5" strokeWidth={1.75} /> send
            </button>
          </div>
          {cmdLog.length > 0 && (
            <div className="max-h-40 overflow-y-auto border-t border-neriak-dim p-2 space-y-1 font-mono text-[11px]">
              {cmdLog.map((r, i) => (
                <div key={i} className="flex items-start gap-2">
                  <span className={r.accepted ? "text-state-ok" : "text-state-danger"}>
                    {r.accepted ? "✓" : "✗"}
                  </span>
                  <span className="text-neriak-dim">#{r.session_id}</span>
                  <span className="text-neriak-muted">[{r.scope_used}]</span>
                  <span className="text-neriak-text">{r.command}</span>
                  <span className="ml-auto text-neriak-dim">{r.message}</span>
                </div>
              ))}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
