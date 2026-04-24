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
  active: "text-[#34d399]",
  idle: "text-[#a096b4]",
  paused: "text-[#fbbf24]",
  error: "text-[#ef4444]",
};

const STATE_DOT: Record<SessionState, string> = {
  active: "bg-[#34d399] shadow-[0_0_6px_#34d399]",
  idle: "bg-[#a096b4]",
  paused: "bg-[#fbbf24]",
  error: "bg-[#ef4444] animate-pulse",
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
            <Users className="w-3.5 h-3.5 text-[#00e5ff]" strokeWidth={1.75} />
            <span>{rows.length} clients</span>
            <span className="text-[#503c6e]">·</span>
            <span className="text-[#34d399]">{activeCount} active</span>
            {pausedCount > 0 && (
              <>
                <span className="text-[#503c6e]">·</span>
                <span className="text-[#fbbf24]">{pausedCount} paused</span>
              </>
            )}
            {errorCount > 0 && (
              <>
                <span className="text-[#503c6e]">·</span>
                <span className="text-[#ef4444]">{errorCount} error</span>
              </>
            )}
          </>
        }
        meta={
          <>
            {loading && <span>loading…</span>}
            {error && (
              <span className="flex items-center gap-1 text-[#ef4444]">
                <AlertTriangle className="w-3.5 h-3.5" /> {error}
              </span>
            )}
            <button
              onClick={refetch}
              className="border border-[#503c6e] hover:border-[#00e5ff] rounded-sm px-2 py-1"
            >
              refresh
            </button>
          </>
        }
      />
      <div className="p-6 space-y-4">
        {useMockData && (
          <div className="text-[10px] font-mono text-[#fbbf24] border border-[#fbbf24]/40 bg-[#fbbf24]/5 rounded-sm px-2 py-1 inline-block uppercase tracking-[0.2em]">
            mock data · backend offline
          </div>
        )}

        {selected.size > 0 && (
          <div className="flex items-center gap-3 border border-[#cc44ff]/50 bg-[#1a0a2e] rounded-sm px-3 py-2 font-mono text-xs">
            <span className="text-[#cc44ff]">{selected.size} selected</span>
            <button
              onClick={() => bulk("pause")}
              className="flex items-center gap-1 text-[#fbbf24] hover:text-[#fde68a]"
            >
              <Pause className="w-3.5 h-3.5" strokeWidth={1.75} /> pause all
            </button>
            <button
              onClick={() => bulk("resume")}
              className="flex items-center gap-1 text-[#34d399] hover:text-[#6ee7b7]"
            >
              <Play className="w-3.5 h-3.5" strokeWidth={1.75} /> resume all
            </button>
            <button
              onClick={() => setSelected(new Set())}
              className="ml-auto text-[#a096b4] hover:text-[#e2d7f4]"
            >
              clear
            </button>
          </div>
        )}

        <div className="border border-[#503c6e] rounded-md overflow-hidden">
          <table className="w-full font-mono text-sm">
            <thead className="bg-[#0d0618] text-[#a096b4] uppercase tracking-[0.15em] text-[10px]">
              <tr>
                <th className="w-8 px-3 py-2 text-left">
                  <input
                    type="checkbox"
                    checked={selected.size === rows.length && rows.length > 0}
                    onChange={toggleAll}
                    className="accent-[#cc44ff]"
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
                    className={`border-t border-[#503c6e]/50 hover:bg-[#2d1e41]/60 ${
                      isSel ? "bg-[#2d1e41]" : ""
                    }`}
                  >
                    <td className="px-3 py-2">
                      <input
                        type="checkbox"
                        checked={isSel}
                        onChange={() => toggleSelect(r.session_id)}
                        className="accent-[#cc44ff]"
                      />
                    </td>
                    <td className="px-3 py-2 text-[#a096b4]">{r.session_id}</td>
                    <td className="px-3 py-2 text-[#e2d7f4]">
                      {r.character_name ?? "—"}
                      {r.class && (
                        <span className="ml-2 text-[10px] text-[#503c6e]">
                          {r.class.toUpperCase()}
                        </span>
                      )}
                      {r.level && (
                        <span className="ml-1 text-[10px] text-[#503c6e]">L{r.level}</span>
                      )}
                    </td>
                    <td className="px-3 py-2 text-[#a096b4] text-xs">{r.zone ?? "—"}</td>
                    <td className="px-3 py-2">
                      {r.hp_pct !== undefined ? (
                        <StatBar pct={r.hp_pct} kind="hp" />
                      ) : (
                        <span className="text-[#503c6e]">—</span>
                      )}
                    </td>
                    <td className="px-3 py-2">
                      {r.mana_pct !== undefined ? (
                        <StatBar pct={r.mana_pct} kind="mp" />
                      ) : (
                        <span className="text-[#503c6e]">—</span>
                      )}
                    </td>
                    <td className="px-3 py-2">
                      <select
                        value={r.group_id}
                        onChange={(e) => moveToGroup(r.session_id, Number(e.target.value))}
                        disabled={isBusy}
                        className="bg-[#0d0618] border border-[#503c6e] rounded-sm px-1.5 py-0.5 text-[#e2d7f4] focus:border-[#00e5ff] outline-none"
                      >
                        {GROUP_IDS.map((g) => (
                          <option key={g} value={g}>
                            G{g}
                          </option>
                        ))}
                      </select>
                    </td>
                    <td className="px-3 py-2 text-[#a096b4]">{r.routing_scope}</td>
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
                          className="inline-flex items-center gap-1 text-[#34d399] hover:text-[#6ee7b7] disabled:opacity-40"
                        >
                          <Play className="w-3.5 h-3.5" strokeWidth={1.75} /> resume
                        </button>
                      ) : (
                        <button
                          disabled={isBusy || r.state === "error"}
                          onClick={() => runOp(r.session_id, "pause")}
                          className="inline-flex items-center gap-1 text-[#fbbf24] hover:text-[#fde68a] disabled:opacity-40"
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
                  <td colSpan={10} className="px-3 py-8 text-center text-[#503c6e] italic">
                    no sessions
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        <section className="border border-[#503c6e] rounded-md bg-[#1a0a2e]">
          <div className="flex items-center gap-2 px-3 py-2 border-b border-[#503c6e] text-xs font-mono text-[#a096b4] uppercase tracking-[0.15em]">
            <Terminal className="w-3.5 h-3.5 text-[#00e5ff]" strokeWidth={1.75} />
            command relay
          </div>
          <div className="flex items-center gap-2 p-3">
            <select
              value={cmdScope}
              onChange={(e) => setCmdScope(e.target.value as CommandScope)}
              className="bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 text-xs font-mono text-[#e2d7f4] focus:border-[#00e5ff] outline-none"
            >
              <option value="self">self</option>
              <option value="group">group</option>
              <option value="all">all</option>
            </select>
            <div className="flex-1 flex items-center gap-2 bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 focus-within:border-[#cc44ff]">
              <Radio className="w-3.5 h-3.5 text-[#cc44ff]" strokeWidth={1.75} />
              <input
                value={cmdText}
                onChange={(e) => setCmdText(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && sendCommand()}
                placeholder="/shout camp check"
                className="flex-1 bg-transparent font-mono text-sm text-[#e2d7f4] placeholder-[#503c6e] outline-none"
              />
            </div>
            <button
              onClick={sendCommand}
              disabled={!cmdText.trim()}
              className="flex items-center gap-1 bg-[#cc44ff]/20 border border-[#cc44ff] text-[#cc44ff] hover:bg-[#cc44ff]/30 disabled:opacity-40 rounded-sm px-3 py-1.5 text-xs font-mono uppercase tracking-wider"
            >
              <Send className="w-3.5 h-3.5" strokeWidth={1.75} /> send
            </button>
          </div>
          {cmdLog.length > 0 && (
            <div className="max-h-40 overflow-y-auto border-t border-[#503c6e] p-2 space-y-1 font-mono text-[11px]">
              {cmdLog.map((r, i) => (
                <div key={i} className="flex items-start gap-2">
                  <span className={r.accepted ? "text-[#34d399]" : "text-[#ef4444]"}>
                    {r.accepted ? "✓" : "✗"}
                  </span>
                  <span className="text-[#503c6e]">#{r.session_id}</span>
                  <span className="text-[#a096b4]">[{r.scope_used}]</span>
                  <span className="text-[#e2d7f4]">{r.command}</span>
                  <span className="ml-auto text-[#503c6e]">{r.message}</span>
                </div>
              ))}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
