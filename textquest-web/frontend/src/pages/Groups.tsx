import { useState } from "react";
import { Plus, Play, RotateCcw, Trash2, UsersRound, Zap } from "lucide-react";
import { PageHeader } from "../components/PageHeader.tsx";
import { SaveButton } from "../components/SaveButton.tsx";
import { SortableList, SortableItem } from "../components/SortableList.tsx";
import { useSave } from "../hooks/useSave.ts";
import { MOCK_AUTO_GROUP, MOCK_CHARACTERS } from "../lib/mocks.ts";
import type { AutoGroupConfig, AutoGroupMember, GroupRole } from "../lib/types.ts";

const ROLES: GroupRole[] = ["tank", "healer", "dps", "support", "puller", "mez", "slow"];

const ROLE_COLOR: Record<GroupRole, string> = {
  tank: "#60a5fa",
  healer: "#34d399",
  dps: "#ef4444",
  support: "#fbbf24",
  puller: "#cc44ff",
  mez: "#cc44ff",
  slow: "#00e5ff",
};

const TICK_SECS = 6;

export function Groups() {
  const [cfg, setCfg] = useState(MOCK_AUTO_GROUP);
  const [phase] = useState("idle");
  const save = useSave<AutoGroupConfig>("PUT", "/auto-group/");

  const addMember = () => {
    const remaining = Object.keys(MOCK_CHARACTERS).filter(
      (n) => !cfg.members.some((m) => m.name === n),
    );
    if (remaining.length === 0) return;
    setCfg({
      ...cfg,
      members: [...cfg.members, { name: remaining[0], role: "dps" }],
    });
  };

  const removeMember = (name: string) => {
    setCfg({ ...cfg, members: cfg.members.filter((m) => m.name !== name) });
  };

  const updateMember = (name: string, patch: Partial<AutoGroupMember>) => {
    setCfg({
      ...cfg,
      members: cfg.members.map((m) => (m.name === name ? { ...m, ...patch } : m)),
    });
  };

  return (
    <div>
      <PageHeader
        title="Groups"
        subtitle={
          <>
            <UsersRound className="w-3.5 h-3.5 text-[#34d399]" strokeWidth={1.75} />
            <span>{cfg.members.length} members</span>
            <span className="text-[#503c6e]">·</span>
            <span className={cfg.enabled ? "text-[#34d399]" : "text-[#a096b4]"}>
              {cfg.enabled ? "automation on" : "automation off"}
            </span>
            <span className="text-[#503c6e]">·</span>
            <span className="text-[#cc44ff] uppercase tracking-[0.2em]">{phase}</span>
          </>
        }
        meta={
          <span className="text-[10px] font-mono text-[#fbbf24] border border-[#fbbf24]/40 bg-[#fbbf24]/5 rounded-sm px-2 py-1 uppercase tracking-[0.2em]">
            mock data
          </span>
        }
      />

      <div className="p-6 grid grid-cols-1 xl:grid-cols-[1fr_340px] gap-4">
        <section className="border border-[#503c6e] rounded-md bg-[#1a0a2e]">
          <div className="flex items-center gap-2 px-3 py-2 border-b border-[#503c6e] text-xs font-mono text-[#a096b4] uppercase tracking-[0.15em]">
            <UsersRound className="w-3.5 h-3.5 text-[#34d399]" strokeWidth={1.75} />
            members
            <button
              onClick={addMember}
              className="ml-auto flex items-center gap-1 text-[#cc44ff] hover:text-[#ff00ff]"
            >
              <Plus className="w-3 h-3" strokeWidth={2} />
              add
            </button>
          </div>
          <div className="p-2">
            {cfg.members.length === 0 ? (
              <div className="px-3 py-8 text-center text-[#503c6e] font-mono text-sm italic">
                no members configured
              </div>
            ) : (
              <SortableList
                items={cfg.members.map((m) => ({ ...m, id: m.name }))}
                onReorder={(items) =>
                  setCfg({ ...cfg, members: items.map(({ name, role }) => ({ name, role })) })
                }
              >
                {(m, i) => (
                  <SortableItem id={m.id}>
                    {({ handle }) => (
                      <div className="group flex items-center gap-3 px-2 py-2 border border-[#503c6e]/40 rounded-sm bg-[#0d0618] hover:bg-[#2d1e41]/50">
                        {handle}
                        <span className="font-mono text-[10px] text-[#503c6e] w-6 text-center tabular-nums">
                          {i + 1}
                        </span>
                        <select
                          value={m.name}
                          onChange={(e) => updateMember(m.name, { name: e.target.value })}
                          className="bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1 font-mono text-sm text-[#e2d7f4] focus:border-[#cc44ff] outline-none min-w-[140px]"
                        >
                          {Object.keys(MOCK_CHARACTERS).map((n) => (
                            <option key={n} value={n}>
                              {n}
                            </option>
                          ))}
                        </select>
                        <span className="font-mono text-[10px] text-[#cc44ff] uppercase tracking-[0.2em]">
                          {MOCK_CHARACTERS[m.name]?.class ?? "—"}
                        </span>
                        <select
                          value={m.role}
                          onChange={(e) =>
                            updateMember(m.name, { role: e.target.value as GroupRole })
                          }
                          className="bg-[#0d0618] border rounded-sm px-2 py-1 font-mono text-xs focus:outline-none"
                          style={{
                            borderColor: `${ROLE_COLOR[m.role]}66`,
                            color: ROLE_COLOR[m.role],
                          }}
                        >
                          {ROLES.map((r) => (
                            <option key={r} value={r}>
                              {r}
                            </option>
                          ))}
                        </select>
                        <button
                          onClick={() => removeMember(m.name)}
                          className="ml-auto opacity-0 group-hover:opacity-100 text-[#ef4444] hover:text-[#fca5a5] transition-opacity"
                        >
                          <Trash2 className="w-3.5 h-3.5" strokeWidth={1.75} />
                        </button>
                      </div>
                    )}
                  </SortableItem>
                )}
              </SortableList>
            )}
          </div>
        </section>

        <aside className="space-y-4">
          <section className="border border-[#503c6e] rounded-md bg-[#1a0a2e]">
            <div className="flex items-center gap-2 px-3 py-2 border-b border-[#503c6e] text-xs font-mono text-[#a096b4] uppercase tracking-[0.15em]">
              <Zap className="w-3.5 h-3.5 text-[#fbbf24]" strokeWidth={1.75} />
              automation
            </div>
            <div className="p-4 space-y-3">
              <label className="flex items-center gap-2 font-mono text-sm text-[#e2d7f4]">
                <input
                  type="checkbox"
                  checked={cfg.enabled}
                  onChange={() => setCfg({ ...cfg, enabled: !cfg.enabled })}
                  className="accent-[#cc44ff]"
                />
                enabled
              </label>

              <label className="flex flex-col gap-1">
                <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-[#a096b4]">
                  completion command
                </span>
                <input
                  value={cfg.completion_command ?? ""}
                  onChange={(e) => setCfg({ ...cfg, completion_command: e.target.value })}
                  placeholder="/g ready"
                  className="bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 font-mono text-sm text-[#e2d7f4] focus:border-[#cc44ff] outline-none"
                />
              </label>

              <label className="flex flex-col gap-1">
                <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-[#a096b4]">
                  max retries
                </span>
                <input
                  type="number"
                  min={0}
                  max={20}
                  value={cfg.max_retries}
                  onChange={(e) => setCfg({ ...cfg, max_retries: Number(e.target.value) })}
                  className="bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 font-mono text-sm text-[#e2d7f4] focus:border-[#cc44ff] outline-none"
                />
              </label>

              <label className="flex flex-col gap-1">
                <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-[#a096b4]">
                  invite interval (ticks)
                </span>
                <input
                  type="number"
                  min={1}
                  value={cfg.invite_interval_ticks}
                  onChange={(e) =>
                    setCfg({ ...cfg, invite_interval_ticks: Number(e.target.value) })
                  }
                  className="bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 font-mono text-sm text-[#e2d7f4] focus:border-[#cc44ff] outline-none"
                />
                <span className="font-mono text-[10px] text-[#503c6e]">
                  ≈ {cfg.invite_interval_ticks * TICK_SECS}s
                </span>
              </label>

              <label className="flex flex-col gap-1">
                <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-[#a096b4]">
                  member wait (ticks)
                </span>
                <input
                  type="number"
                  min={1}
                  value={cfg.member_wait_ticks}
                  onChange={(e) => setCfg({ ...cfg, member_wait_ticks: Number(e.target.value) })}
                  className="bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 font-mono text-sm text-[#e2d7f4] focus:border-[#cc44ff] outline-none"
                />
                <span className="font-mono text-[10px] text-[#503c6e]">
                  ≈ {cfg.member_wait_ticks * TICK_SECS}s
                </span>
              </label>
            </div>
          </section>

          <section className="border border-[#cc44ff]/40 rounded-md bg-[#1a0a2e] p-3 space-y-2">
            <SaveButton
              state={save.state}
              error={save.error}
              onClick={() => save.save(cfg)}
              label="save config"
            />
            <div className="flex gap-2">
              <button
                disabled={!cfg.enabled}
                className="flex-1 flex items-center justify-center gap-2 bg-[#cc44ff]/20 border border-[#cc44ff] text-[#cc44ff] hover:bg-[#cc44ff]/30 disabled:opacity-40 rounded-sm py-2 text-xs font-mono uppercase tracking-[0.2em]"
              >
                <Play className="w-3.5 h-3.5" strokeWidth={2} />
                start
              </button>
              <button className="flex items-center justify-center gap-2 border border-[#503c6e] hover:border-[#fbbf24] text-[#a096b4] hover:text-[#fbbf24] rounded-sm px-4 py-2 text-xs font-mono uppercase tracking-[0.2em]">
                <RotateCcw className="w-3.5 h-3.5" strokeWidth={1.75} />
                reset
              </button>
            </div>
          </section>
        </aside>
      </div>
    </div>
  );
}
