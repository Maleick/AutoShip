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
            <UsersRound className="w-3.5 h-3.5 text-state-ok" strokeWidth={1.75} />
            <span>{cfg.members.length} members</span>
            <span className="text-neriak-dim">·</span>
            <span className={cfg.enabled ? "text-state-ok" : "text-neriak-muted"}>
              {cfg.enabled ? "automation on" : "automation off"}
            </span>
            <span className="text-neriak-dim">·</span>
            <span className="text-neriak-magenta uppercase tracking-[0.2em]">{phase}</span>
          </>
        }
        meta={
          <span className="text-[10px] font-mono text-state-warn border border-state-warn/40 bg-state-warn/5 rounded-sm px-2 py-1 uppercase tracking-[0.2em]">
            mock data
          </span>
        }
      />

      <div className="p-6 grid grid-cols-1 xl:grid-cols-[1fr_340px] gap-4">
        <section className="border border-neriak-dim rounded-md bg-panel">
          <div className="flex items-center gap-2 px-3 py-2 border-b border-neriak-dim text-xs font-mono text-neriak-muted uppercase tracking-[0.15em]">
            <UsersRound className="w-3.5 h-3.5 text-state-ok" strokeWidth={1.75} />
            members
            <button
              onClick={addMember}
              className="ml-auto flex items-center gap-1 text-neriak-magenta hover:text-neriak-magenta-bright"
            >
              <Plus className="w-3 h-3" strokeWidth={2} />
              add
            </button>
          </div>
          <div className="p-2">
            {cfg.members.length === 0 ? (
              <div className="px-3 py-8 text-center text-neriak-dim font-mono text-sm italic">
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
                      <div className="group flex items-center gap-3 px-2 py-2 border border-neriak-dim/40 rounded-sm bg-void hover:bg-elevated/50">
                        {handle}
                        <span className="font-mono text-[10px] text-neriak-dim w-6 text-center tabular-nums">
                          {i + 1}
                        </span>
                        <select
                          value={m.name}
                          onChange={(e) => updateMember(m.name, { name: e.target.value })}
                          className="bg-void border border-neriak-dim rounded-sm px-2 py-1 font-mono text-sm text-neriak-text focus:border-neriak-magenta outline-none min-w-[140px]"
                        >
                          {Object.keys(MOCK_CHARACTERS).map((n) => (
                            <option key={n} value={n}>
                              {n}
                            </option>
                          ))}
                        </select>
                        <span className="font-mono text-[10px] text-neriak-magenta uppercase tracking-[0.2em]">
                          {MOCK_CHARACTERS[m.name]?.class ?? "—"}
                        </span>
                        <select
                          aria-label={`Role for ${m.name}`}
                          value={m.role}
                          onChange={(e) =>
                            updateMember(m.name, { role: e.target.value as GroupRole })
                          }
                          className="bg-void border rounded-sm px-2 py-1 font-mono text-xs focus:outline-none"
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
                          className="ml-auto opacity-0 group-hover:opacity-100 text-state-danger hover:text-[#fca5a5] transition-opacity"
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
          <section className="border border-neriak-dim rounded-md bg-panel">
            <div className="flex items-center gap-2 px-3 py-2 border-b border-neriak-dim text-xs font-mono text-neriak-muted uppercase tracking-[0.15em]">
              <Zap className="w-3.5 h-3.5 text-state-warn" strokeWidth={1.75} />
              automation
            </div>
            <div className="p-4 space-y-3">
              <label className="flex items-center gap-2 font-mono text-sm text-neriak-text">
                <input
                  type="checkbox"
                  checked={cfg.enabled}
                  onChange={() => setCfg({ ...cfg, enabled: !cfg.enabled })}
                  className="accent-neriak-magenta"
                />
                enabled
              </label>

              <label className="flex flex-col gap-1">
                <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-neriak-muted">
                  completion command
                </span>
                <input
                  value={cfg.completion_command ?? ""}
                  onChange={(e) => setCfg({ ...cfg, completion_command: e.target.value })}
                  placeholder="/g ready"
                  className="bg-void border border-neriak-dim rounded-sm px-2 py-1.5 font-mono text-sm text-neriak-text focus:border-neriak-magenta outline-none"
                />
              </label>

              <label className="flex flex-col gap-1">
                <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-neriak-muted">
                  max retries
                </span>
                <input
                  type="number"
                  min={0}
                  max={20}
                  value={cfg.max_retries}
                  onChange={(e) => setCfg({ ...cfg, max_retries: Number(e.target.value) })}
                  className="bg-void border border-neriak-dim rounded-sm px-2 py-1.5 font-mono text-sm text-neriak-text focus:border-neriak-magenta outline-none"
                />
              </label>

              <label className="flex flex-col gap-1">
                <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-neriak-muted">
                  invite interval (ticks)
                </span>
                <input
                  type="number"
                  min={1}
                  value={cfg.invite_interval_ticks}
                  onChange={(e) =>
                    setCfg({ ...cfg, invite_interval_ticks: Number(e.target.value) })
                  }
                  className="bg-void border border-neriak-dim rounded-sm px-2 py-1.5 font-mono text-sm text-neriak-text focus:border-neriak-magenta outline-none"
                />
                <span className="font-mono text-[10px] text-neriak-dim">
                  ≈ {cfg.invite_interval_ticks * TICK_SECS}s
                </span>
              </label>

              <label className="flex flex-col gap-1">
                <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-neriak-muted">
                  member wait (ticks)
                </span>
                <input
                  type="number"
                  min={1}
                  value={cfg.member_wait_ticks}
                  onChange={(e) => setCfg({ ...cfg, member_wait_ticks: Number(e.target.value) })}
                  className="bg-void border border-neriak-dim rounded-sm px-2 py-1.5 font-mono text-sm text-neriak-text focus:border-neriak-magenta outline-none"
                />
                <span className="font-mono text-[10px] text-neriak-dim">
                  ≈ {cfg.member_wait_ticks * TICK_SECS}s
                </span>
              </label>
            </div>
          </section>

          <section className="border border-neriak-magenta/40 rounded-md bg-panel p-3 space-y-2">
            <SaveButton
              state={save.state}
              error={save.error}
              onClick={() => save.save(cfg)}
              label="save config"
            />
            <div className="flex gap-2">
              <button
                disabled={!cfg.enabled}
                className="flex-1 flex items-center justify-center gap-2 bg-neriak-magenta/20 border border-neriak-magenta text-neriak-magenta hover:bg-neriak-magenta/30 disabled:opacity-40 rounded-sm py-2 text-xs font-mono uppercase tracking-[0.2em]"
              >
                <Play className="w-3.5 h-3.5" strokeWidth={2} />
                start
              </button>
              <button className="flex items-center justify-center gap-2 border border-neriak-dim hover:border-state-warn text-neriak-muted hover:text-state-warn rounded-sm px-4 py-2 text-xs font-mono uppercase tracking-[0.2em]">
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
