import { useMemo, useState } from "react";
import { Heart, Droplet, Flame, Search, Shield, Sparkles, Users, Wand2 } from "lucide-react";
import { PageHeader } from "../components/PageHeader.tsx";
import { SaveButton } from "../components/SaveButton.tsx";
import { SortableList, SortableItem } from "../components/SortableList.tsx";
import { useSave } from "../hooks/useSave.ts";
import { MOCK_CHARACTERS } from "../lib/mocks.ts";
import { CHAR_ROLES, EQ_CLASSES, type CharacterConfig } from "../lib/types.ts";

function Slider({
  label,
  value,
  color,
  onChange,
}: {
  label: string;
  value: number;
  color: string;
  onChange: (v: number) => void;
}) {
  return (
    <div className="flex items-center gap-3">
      <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-[#a096b4] w-24 shrink-0">
        {label}
      </span>
      <input
        type="range"
        min={0}
        max={100}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="flex-1 accent-[color:var(--slider-color)]"
        style={{ ["--slider-color" as string]: color } as React.CSSProperties}
      />
      <span className="font-mono text-sm tabular-nums w-10 text-right" style={{ color }}>
        {value}%
      </span>
    </div>
  );
}

function Section({
  title,
  icon: Icon,
  children,
}: {
  title: string;
  icon: typeof Users;
  children: React.ReactNode;
}) {
  return (
    <section className="border border-[#503c6e] rounded-md bg-[#1a0a2e]">
      <div className="flex items-center gap-2 px-3 py-2 border-b border-[#503c6e] text-xs font-mono text-[#a096b4] uppercase tracking-[0.15em]">
        <Icon className="w-3.5 h-3.5 text-[#cc44ff]" strokeWidth={1.75} />
        {title}
      </div>
      <div className="p-4 space-y-3">{children}</div>
    </section>
  );
}

export function Characters() {
  const [selectedName, setSelectedName] = useState<string>(Object.keys(MOCK_CHARACTERS)[0]);
  const [filter, setFilter] = useState("");
  const [classFilter, setClassFilter] = useState<string>("");
  const [configs, setConfigs] = useState(MOCK_CHARACTERS);

  const chars = useMemo(() => Object.values(configs), [configs]);

  const filtered = useMemo(() => {
    return chars.filter((c) => {
      const q = filter.trim().toLowerCase();
      const matchName = !q || c.character_name.toLowerCase().includes(q);
      const matchClass = !classFilter || c.class === classFilter;
      return matchName && matchClass;
    });
  }, [chars, filter, classFilter]);

  const selected = configs[selectedName];
  const original = MOCK_CHARACTERS[selectedName];
  const dirty = selected && JSON.stringify(selected) !== JSON.stringify(original);
  const save = useSave<CharacterConfig>("PUT", `/config/characters/${selectedName}`);
  const saveAll = () => selected && save.save(selected);

  const update = (patch: Partial<CharacterConfig>) => {
    setConfigs((prev) => ({
      ...prev,
      [selectedName]: { ...prev[selectedName], ...patch },
    }));
  };

  return (
    <div>
      <PageHeader
        title="Characters"
        subtitle={
          <>
            <Users className="w-3.5 h-3.5 text-[#cc44ff]" strokeWidth={1.75} />
            <span>{chars.length} configured</span>
            <span className="text-[#503c6e]">·</span>
            <span className="text-[#a096b4]">editing {selected?.character_name}</span>
            {dirty && (
              <>
                <span className="text-[#503c6e]">·</span>
                <span className="flex items-center gap-1 text-[#fbbf24]">
                  <span className="w-1.5 h-1.5 rounded-full bg-[#fbbf24] animate-pulse" />
                  unsaved
                </span>
              </>
            )}
          </>
        }
        meta={
          <span className="text-[10px] font-mono text-[#fbbf24] border border-[#fbbf24]/40 bg-[#fbbf24]/5 rounded-sm px-2 py-1 uppercase tracking-[0.2em]">
            mock data
          </span>
        }
      />

      <div className="grid grid-cols-[260px_1fr] min-h-[calc(100vh-11rem)]">
        {/* Character list */}
        <aside className="border-r border-[#503c6e] bg-[#0d0618]/40 flex flex-col">
          <div className="p-3 border-b border-[#503c6e] space-y-2">
            <div className="flex items-center gap-2 bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 focus-within:border-[#cc44ff]">
              <Search className="w-3.5 h-3.5 text-[#a096b4]" strokeWidth={1.75} />
              <input
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                placeholder="search…"
                className="flex-1 bg-transparent font-mono text-xs text-[#e2d7f4] placeholder-[#503c6e] outline-none"
              />
            </div>
            <select
              value={classFilter}
              onChange={(e) => setClassFilter(e.target.value)}
              className="w-full bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 text-xs font-mono text-[#e2d7f4] focus:border-[#00e5ff] outline-none"
            >
              <option value="">all classes</option>
              {EQ_CLASSES.map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
          </div>
          <div className="flex-1 overflow-y-auto">
            {filtered.map((c) => {
              const active = c.character_name === selectedName;
              return (
                <button
                  key={c.character_name}
                  onClick={() => setSelectedName(c.character_name)}
                  className={`w-full text-left px-3 py-2 border-b border-[#503c6e]/30 flex items-center gap-2 font-mono text-sm ${
                    active
                      ? "bg-[#2d1e41] text-[#cc44ff] border-l-2 border-l-[#cc44ff]"
                      : "text-[#e2d7f4] hover:bg-[#1a0a2e] border-l-2 border-l-transparent"
                  }`}
                >
                  <span className="flex-1">{c.character_name}</span>
                  <span
                    className="text-[10px] uppercase tracking-[0.15em]"
                    style={{ color: active ? "#cc44ff" : "#a096b4" }}
                  >
                    {c.class}
                  </span>
                  <span className="text-[10px] text-[#503c6e]">{c.group_name}</span>
                </button>
              );
            })}
            {filtered.length === 0 && (
              <div className="px-3 py-6 text-center text-[#503c6e] font-mono text-sm italic">
                no matches
              </div>
            )}
          </div>
        </aside>

        {/* Detail panel */}
        {selected && (
          <div className="p-6 overflow-y-auto space-y-4">
            <Section title="Identity" icon={Users}>
              <div className="grid grid-cols-3 gap-3">
                <label className="flex flex-col gap-1">
                  <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-[#a096b4]">
                    character
                  </span>
                  <input
                    readOnly
                    value={selected.character_name}
                    className="bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 font-mono text-sm text-[#e2d7f4]"
                  />
                </label>
                <label className="flex flex-col gap-1">
                  <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-[#a096b4]">
                    class
                  </span>
                  <select
                    value={selected.class}
                    onChange={(e) => update({ class: e.target.value })}
                    className="bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 font-mono text-sm text-[#e2d7f4] focus:border-[#cc44ff] outline-none"
                  >
                    {EQ_CLASSES.map((c) => (
                      <option key={c} value={c}>
                        {c}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="flex flex-col gap-1">
                  <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-[#a096b4]">
                    role
                  </span>
                  <select
                    value={selected.role}
                    onChange={(e) => update({ role: e.target.value })}
                    className="bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 font-mono text-sm text-[#e2d7f4] focus:border-[#cc44ff] outline-none"
                  >
                    {CHAR_ROLES.map((r) => (
                      <option key={r} value={r}>
                        {r}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
            </Section>

            <Section title="Thresholds" icon={Heart}>
              <Slider
                label="heal at"
                value={selected.heal_at_pct}
                color="#34d399"
                onChange={(v) => update({ heal_at_pct: v })}
              />
              <Slider
                label="mana sit"
                value={selected.mana_sit_pct}
                color="#60a5fa"
                onChange={(v) => update({ mana_sit_pct: v })}
              />
              <Slider
                label="nuke at"
                value={selected.nuke_at_pct}
                color="#cc44ff"
                onChange={(v) => update({ nuke_at_pct: v })}
              />
            </Section>

            <Section title="Rotation" icon={Flame}>
              <SortableList
                items={selected.rotation}
                onReorder={(rot) =>
                  update({
                    rotation: rot.map((r, i) => ({ ...r, priority: i + 1 })),
                  })
                }
              >
                {(entry, i) => (
                  <SortableItem id={entry.id}>
                    {({ handle }) => (
                      <div className="flex items-center gap-3 px-2 py-1.5 border border-[#503c6e]/50 rounded-sm font-mono text-sm">
                        {handle}
                        <span className="text-[#503c6e] w-6 text-center">{i + 1}</span>
                        <input
                          value={entry.name}
                          onChange={(e) => {
                            const rot = [...selected.rotation];
                            rot[i] = { ...entry, name: e.target.value };
                            update({ rotation: rot });
                          }}
                          className="flex-1 bg-transparent outline-none text-[#e2d7f4]"
                        />
                        <label className="flex items-center gap-1.5 text-xs text-[#a096b4]">
                          <input
                            type="checkbox"
                            checked={entry.enabled}
                            onChange={() => {
                              const rot = [...selected.rotation];
                              rot[i] = { ...entry, enabled: !entry.enabled };
                              update({ rotation: rot });
                            }}
                            className="accent-[#cc44ff]"
                          />
                          enabled
                        </label>
                      </div>
                    )}
                  </SortableItem>
                )}
              </SortableList>
            </Section>

            <Section title="Class Params" icon={Wand2}>
              <div className="grid grid-cols-2 gap-3">
                {(
                  [
                    { key: "ch_chain_timing_ms", label: "CH chain (ms)", max: 15000, step: 100 },
                    { key: "dot_overlap_pct", label: "DoT overlap %", max: 100, step: 5 },
                    { key: "burn_at_hp_pct", label: "burn at HP %", max: 100, step: 5 },
                    { key: "slow_at_hp_pct", label: "slow at HP %", max: 100, step: 5 },
                  ] as const
                ).map(({ key, label, max, step }) => {
                  const val = selected.class_params[key];
                  return (
                    <label key={key} className="flex flex-col gap-1">
                      <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-[#a096b4]">
                        {label}
                      </span>
                      <input
                        type="number"
                        min={0}
                        max={max}
                        step={step}
                        value={val ?? ""}
                        placeholder="—"
                        onChange={(e) => {
                          const v = e.target.value === "" ? null : Number(e.target.value);
                          update({
                            class_params: { ...selected.class_params, [key]: v },
                          });
                        }}
                        className="bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 font-mono text-sm text-[#e2d7f4] focus:border-[#cc44ff] outline-none"
                      />
                    </label>
                  );
                })}
              </div>
            </Section>

            <Section title="Auto-Rez" icon={Sparkles}>
              <label className="flex items-center gap-2 font-mono text-sm text-[#e2d7f4]">
                <input
                  type="checkbox"
                  checked={selected.auto_rez.enabled}
                  onChange={() =>
                    update({
                      auto_rez: { ...selected.auto_rez, enabled: !selected.auto_rez.enabled },
                    })
                  }
                  className="accent-[#cc44ff]"
                />
                auto-accept rez offers
              </label>
              <Slider
                label="min XP %"
                value={selected.auto_rez.min_xp_pct}
                color="#34d399"
                onChange={(v) => update({ auto_rez: { ...selected.auto_rez, min_xp_pct: v } })}
              />
              <div>
                <div className="font-mono text-[10px] uppercase tracking-[0.18em] text-[#a096b4] mb-1">
                  trusted casters
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {selected.auto_rez.trusted_casters.map((caster) => (
                    <span
                      key={caster}
                      className="inline-flex items-center gap-1 px-2 py-0.5 border border-[#cc44ff]/40 bg-[#cc44ff]/5 rounded-sm font-mono text-[11px] text-[#cc44ff]"
                    >
                      {caster}
                    </span>
                  ))}
                </div>
              </div>
              <label className="flex items-center gap-2 font-mono text-sm text-[#e2d7f4]">
                <input
                  type="checkbox"
                  checked={selected.auto_rez.decline_if_untrusted}
                  onChange={() =>
                    update({
                      auto_rez: {
                        ...selected.auto_rez,
                        decline_if_untrusted: !selected.auto_rez.decline_if_untrusted,
                      },
                    })
                  }
                  className="accent-[#cc44ff]"
                />
                decline untrusted rez offers
              </label>
            </Section>

            <Section title="Group & Window" icon={Shield}>
              <label className="flex items-center gap-2 font-mono text-sm text-[#e2d7f4]">
                <input
                  type="checkbox"
                  checked={selected.group_override}
                  onChange={() => update({ group_override: !selected.group_override })}
                  className="accent-[#cc44ff]"
                />
                override default group
              </label>
              <label className="flex items-center gap-3">
                <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-[#a096b4] w-24">
                  group name
                </span>
                <input
                  value={selected.group_name ?? ""}
                  onChange={(e) => update({ group_name: e.target.value })}
                  className="flex-1 bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 font-mono text-sm text-[#e2d7f4] focus:border-[#cc44ff] outline-none"
                />
              </label>
              <label className="flex items-center gap-3">
                <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-[#a096b4] w-24">
                  window title
                </span>
                <input
                  value={selected.window_title_format}
                  onChange={(e) => update({ window_title_format: e.target.value })}
                  className="flex-1 bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 font-mono text-sm text-[#e2d7f4] focus:border-[#cc44ff] outline-none"
                />
              </label>
            </Section>

            <Section title="Tribute" icon={Droplet}>
              <label className="flex items-center gap-2 font-mono text-sm text-[#e2d7f4]">
                <input
                  type="checkbox"
                  checked={selected.tribute_preferences.auto_activate}
                  onChange={() =>
                    update({
                      tribute_preferences: {
                        ...selected.tribute_preferences,
                        auto_activate: !selected.tribute_preferences.auto_activate,
                      },
                    })
                  }
                  className="accent-[#cc44ff]"
                />
                auto-activate tribute
              </label>
              <label className="flex items-center gap-3">
                <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-[#a096b4] w-32">
                  warn threshold (s)
                </span>
                <input
                  type="number"
                  value={selected.tribute_preferences.warning_threshold_secs}
                  onChange={(e) =>
                    update({
                      tribute_preferences: {
                        ...selected.tribute_preferences,
                        warning_threshold_secs: Number(e.target.value),
                      },
                    })
                  }
                  className="flex-1 bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 font-mono text-sm text-[#e2d7f4] focus:border-[#cc44ff] outline-none"
                />
              </label>
            </Section>

            <div className="flex items-center gap-2 pt-2">
              <SaveButton state={save.state} error={save.error} onClick={saveAll} />
              <button className="border border-[#503c6e] hover:border-[#00e5ff] text-[#a096b4] hover:text-[#00e5ff] rounded-sm px-4 py-1.5 text-xs font-mono uppercase tracking-[0.2em]">
                copy to…
              </button>
              <button
                onClick={() =>
                  setConfigs((prev) => ({ ...prev, [selectedName]: MOCK_CHARACTERS[selectedName] }))
                }
                className="ml-auto border border-[#503c6e] hover:border-[#fbbf24] text-[#a096b4] hover:text-[#fbbf24] rounded-sm px-4 py-1.5 text-xs font-mono uppercase tracking-[0.2em]"
              >
                revert
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
