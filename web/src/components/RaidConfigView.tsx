import { useState, useRef, useEffect, type ElementType, type ReactNode } from "react";
import {
  Shield,
  Sword,
  UsersThree,
  ArrowsDownUp,
  MapPin,
  Lightning,
  FloppyDisk,
  ArrowClockwise,
  Warning,
  User,
  Crown,
  Crosshair,
  HandFist,
  HeartStraight,
  PersonSimpleRun,
} from "@phosphor-icons/react";
import type { RaidConfig, RaidMember, RaidRole, BehaviorMode, CampPosition } from "../types";
import { useRaidConfig } from "../hooks/useRaidConfig";
import { useWebSocket } from "../hooks/useWebSocket";

// ── Role badge helpers ────────────────────────────────────────────────────────

const ROLE_META: Record<
  RaidRole,
  { label: string; color: string; bg: string; border: string; Icon: ElementType }
> = {
  main_tank:    { label: "MT",  color: "text-yellow-300",  bg: "bg-yellow-900/30",  border: "border-yellow-500/50",  Icon: Shield },
  main_assist:  { label: "MA",  color: "text-spectral",    bg: "bg-cyan-900/30",    border: "border-spectral/50",    Icon: Crosshair },
  ch_chain:     { label: "CH",  color: "text-green-300",   bg: "bg-green-900/30",   border: "border-green-500/50",   Icon: HeartStraight },
  puller:       { label: "PUL", color: "text-orange-300",  bg: "bg-orange-900/30",  border: "border-orange-500/50",  Icon: PersonSimpleRun },
  healer:       { label: "CLR", color: "text-blue-300",    bg: "bg-blue-900/30",    border: "border-blue-500/50",    Icon: HeartStraight },
  dps:          { label: "DPS", color: "text-red-300",     bg: "bg-red-900/30",     border: "border-red-500/50",     Icon: Sword },
  none:         { label: "---", color: "text-white/30",    bg: "bg-void",           border: "border-white/10",       Icon: User },
};

const ALL_ROLES: RaidRole[] = ["none", "main_tank", "main_assist", "ch_chain", "puller", "healer", "dps"];

function RoleBadge({ role }: { role: RaidRole }) {
  const m = ROLE_META[role];
  return (
    <span
      className={`inline-flex items-center gap-1 px-1.5 py-0.5 text-[10px] font-tech uppercase tracking-widest border ${m.bg} ${m.border} ${m.color}`}
    >
      <m.Icon size={10} weight="fill" />
      {m.label}
    </span>
  );
}

// ── Behavior-mode toggle ──────────────────────────────────────────────────────

const MODES: { id: BehaviorMode; label: string; Icon: ElementType; desc: string }[] = [
  { id: "camp",   label: "Camp",   Icon: MapPin,    desc: "Hold position at camp anchor" },
  { id: "hunt",   label: "Hunt",   Icon: Crosshair, desc: "Actively seek and engage mobs" },
  { id: "follow", label: "Follow", Icon: UsersThree, desc: "Follow main assist" },
];

function BehaviorToggle({
  value,
  onChange,
}: {
  value: BehaviorMode;
  onChange: (m: BehaviorMode) => void;
}) {
  return (
    <div className="flex gap-2">
      {MODES.map(({ id, label, Icon, desc }) => (
        <button
          key={id}
          title={desc}
          onClick={() => onChange(id)}
          className={`flex-1 flex flex-col items-center gap-1.5 py-3 border transition-all ${
            value === id
              ? "bg-magentaglow/10 border-magentaglow text-magentaglow shadow-[0_0_12px_rgba(204,68,255,0.25)]"
              : "bg-void border-white/10 text-white/40 hover:border-white/30 hover:text-white/70"
          }`}
        >
          <Icon size={18} weight={value === id ? "fill" : "regular"} />
          <span className="text-[11px] font-tech uppercase tracking-wider">{label}</span>
        </button>
      ))}
    </div>
  );
}

// ── Role assignment dropdown ──────────────────────────────────────────────────

function RoleSelect({
  value,
  onChange,
}: {
  value: RaidRole;
  onChange: (r: RaidRole) => void;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value as RaidRole)}
      className="bg-void border border-white/20 text-white text-xs px-2 py-1 focus:outline-none focus:border-magentaglow font-tech uppercase tracking-wide"
    >
      {ALL_ROLES.map((r) => (
        <option key={r} value={r} className="bg-[#0d0b1a]">
          {ROLE_META[r].label}
        </option>
      ))}
    </select>
  );
}

// ── Live Composition Grid ─────────────────────────────────────────────────────

function CompositionGrid({
  members,
  onRoleChange,
  onDragReorder,
}: {
  members: RaidMember[];
  onRoleChange: (name: string, role: RaidRole) => void;
  onDragReorder: (fromIdx: number, toIdx: number) => void;
}) {
  const dragIdx = useRef<number | null>(null);
  const [dragOverIdx, setDragOverIdx] = useState<number | null>(null);

  return (
    <div className="space-y-1.5">
      {members.map((m, idx) => {
        const isDraggingOver = dragOverIdx === idx;
        return (
          <div
            key={m.name}
            draggable
            onDragStart={() => { dragIdx.current = idx; }}
            onDragOver={(e) => { e.preventDefault(); setDragOverIdx(idx); }}
            onDragLeave={() => setDragOverIdx(null)}
            onDrop={() => {
              if (dragIdx.current !== null && dragIdx.current !== idx) {
                onDragReorder(dragIdx.current, idx);
              }
              dragIdx.current = null;
              setDragOverIdx(null);
            }}
            onDragEnd={() => { dragIdx.current = null; setDragOverIdx(null); }}
            className={`flex items-center gap-3 p-2.5 border cursor-grab active:cursor-grabbing transition-all select-none ${
              isDraggingOver
                ? "border-magentaglow bg-magentaglow/10"
                : m.is_online
                ? "border-white/10 bg-void/60 hover:border-white/20 hover:bg-void"
                : "border-red-900/30 bg-red-950/20 opacity-60"
            }`}
          >
            {/* drag handle */}
            <ArrowsDownUp size={12} className="text-white/20 flex-shrink-0" />

            {/* avatar */}
            <div
              className={`w-8 h-8 flex-shrink-0 border flex items-center justify-center ${
                m.is_online ? "border-white/20 text-white/40" : "border-red-500/40 text-red-400/40"
              }`}
            >
              <User weight="fill" size={14} />
            </div>

            {/* name + class */}
            <div className="flex-1 min-w-0">
              <div className="text-sm font-medium text-white truncate">{m.name}</div>
              <div className="text-[10px] text-white/40 uppercase tracking-wide">{m.class}</div>
            </div>

            {/* HP/Mana bars */}
            <div className="w-16 flex flex-col gap-1">
              <div className="h-1 bg-void border border-white/10 w-full relative overflow-hidden">
                <div
                  className="absolute top-0 left-0 h-full bg-green-500"
                  style={{ width: `${m.hp_pct}%` }}
                />
              </div>
              {m.mana_pct !== undefined && (
                <div className="h-1 bg-void border border-white/10 w-full relative overflow-hidden">
                  <div
                    className="absolute top-0 left-0 h-full bg-blue-500"
                    style={{ width: `${m.mana_pct}%` }}
                  />
                </div>
              )}
            </div>

            {/* role badge */}
            <RoleBadge role={m.role as RaidRole} />

            {/* role selector */}
            <RoleSelect
              value={m.role as RaidRole}
              onChange={(r) => onRoleChange(m.name, r)}
            />
          </div>
        );
      })}
    </div>
  );
}

// ── CH Chain editor ───────────────────────────────────────────────────────────

function ChChainEditor({
  chain,
  members,
  onReorder,
  onAdd,
  onRemove,
}: {
  chain: string[];
  members: RaidMember[];
  onReorder: (from: number, to: number) => void;
  onAdd: (name: string) => void;
  onRemove: (idx: number) => void;
}) {
  const dragIdx = useRef<number | null>(null);
  const [dragOverIdx, setDragOverIdx] = useState<number | null>(null);

  const healers = members
    .filter((m) => ["healer", "ch_chain"].includes(m.role) || m.class.toLowerCase().includes("cleric"))
    .map((m) => m.name)
    .filter((n) => !chain.includes(n));

  return (
    <div className="space-y-2">
      {chain.length === 0 && (
        <p className="text-xs text-white/30 italic font-rune text-center py-2">
          No healers in CH chain — add below
        </p>
      )}
      {chain.map((name, idx) => {
        const isOver = dragOverIdx === idx;
        return (
          <div
            key={name}
            draggable
            onDragStart={() => { dragIdx.current = idx; }}
            onDragOver={(e) => { e.preventDefault(); setDragOverIdx(idx); }}
            onDragLeave={() => setDragOverIdx(null)}
            onDrop={() => {
              if (dragIdx.current !== null && dragIdx.current !== idx) {
                onReorder(dragIdx.current, idx);
              }
              dragIdx.current = null;
              setDragOverIdx(null);
            }}
            onDragEnd={() => { dragIdx.current = null; setDragOverIdx(null); }}
            className={`flex items-center gap-3 px-3 py-2 border cursor-grab active:cursor-grabbing select-none transition-all ${
              isOver
                ? "border-green-400 bg-green-900/20"
                : "border-green-900/40 bg-green-950/10 hover:border-green-500/40"
            }`}
          >
            <span className="w-5 text-center font-rune text-green-500/50 text-xs">{idx + 1}</span>
            <ArrowsDownUp size={11} className="text-white/20 flex-shrink-0" />
            <span className="flex-1 text-sm text-green-200">{name}</span>
            <button
              onClick={() => onRemove(idx)}
              className="text-white/20 hover:text-red-400 transition-colors text-xs px-1"
              title="Remove from chain"
            >
              ✕
            </button>
          </div>
        );
      })}

      {healers.length > 0 && (
        <select
          defaultValue=""
          onChange={(e) => { if (e.target.value) { onAdd(e.target.value); e.target.value = ""; } }}
          className="w-full bg-void border border-white/20 text-white/60 text-xs px-3 py-2 focus:outline-none focus:border-green-500 font-tech mt-1"
        >
          <option value="">+ Add healer to chain…</option>
          {healers.map((n) => (
            <option key={n} value={n} className="bg-[#0d0b1a]">{n}</option>
          ))}
        </select>
      )}
    </div>
  );
}

// ── Camp position editor ──────────────────────────────────────────────────────

function CampPositionEditor({
  value,
  onChange,
}: {
  value: CampPosition | null;
  onChange: (pos: CampPosition | null) => void;
}) {
  const pos = value ?? { x: 0, y: 0, z: 0 };

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-3">
        <label className="text-xs text-white/50 w-4">X</label>
        <input
          type="number"
          step="0.1"
          value={pos.x}
          onChange={(e) => onChange({ ...pos, x: parseFloat(e.target.value) || 0 })}
          className="flex-1 bg-void border border-white/20 text-white text-sm px-3 py-1.5 focus:outline-none focus:border-magentaglow font-rune"
        />
      </div>
      <div className="flex items-center gap-3">
        <label className="text-xs text-white/50 w-4">Y</label>
        <input
          type="number"
          step="0.1"
          value={pos.y}
          onChange={(e) => onChange({ ...pos, y: parseFloat(e.target.value) || 0 })}
          className="flex-1 bg-void border border-white/20 text-white text-sm px-3 py-1.5 focus:outline-none focus:border-magentaglow font-rune"
        />
      </div>
      <div className="flex items-center gap-3">
        <label className="text-xs text-white/50 w-4">Z</label>
        <input
          type="number"
          step="0.1"
          value={pos.z}
          onChange={(e) => onChange({ ...pos, z: parseFloat(e.target.value) || 0 })}
          className="flex-1 bg-void border border-white/20 text-white text-sm px-3 py-1.5 focus:outline-none focus:border-magentaglow font-rune"
        />
      </div>
      <div className="flex gap-2">
        {value === null ? (
          <button
            onClick={() => onChange({ x: 0, y: 0, z: 0 })}
            className="text-xs text-spectral/70 hover:text-spectral border border-spectral/20 hover:border-spectral/40 px-3 py-1 transition-colors"
          >
            Set camp position
          </button>
        ) : (
          <button
            onClick={() => onChange(null)}
            className="text-xs text-white/30 hover:text-red-400 border border-white/10 hover:border-red-400/30 px-3 py-1 transition-colors"
          >
            Clear position
          </button>
        )}
      </div>
    </div>
  );
}

// ── Named assignment dropdown ─────────────────────────────────────────────────

function NameSelect({
  value,
  members,
  placeholder,
  onChange,
}: {
  value: string | null;
  members: RaidMember[];
  placeholder: string;
  onChange: (name: string | null) => void;
}) {
  return (
    <select
      value={value ?? ""}
      onChange={(e) => onChange(e.target.value || null)}
      className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
    >
      <option value="" className="bg-[#0d0b1a] text-white/50">{placeholder}</option>
      {members
        .filter((m) => m.is_online)
        .map((m) => (
          <option key={m.name} value={m.name} className="bg-[#0d0b1a]">
            {m.name} ({m.class})
          </option>
        ))}
    </select>
  );
}

// ── Section wrapper ───────────────────────────────────────────────────────────

function Section({
  title,
  icon: Icon,
  children,
}: {
  title: string;
  icon: ElementType;
  children: ReactNode;
}) {
  return (
    <div className="bg-violet/20 border border-white/8 p-5">
      <h3 className="font-archaic text-sm text-white/70 uppercase tracking-widest flex items-center gap-2 mb-4 pb-2 border-b border-white/8">
        <Icon size={14} className="text-magentaglow" weight="fill" />
        {title}
      </h3>
      {children}
    </div>
  );
}

// ── Main view ─────────────────────────────────────────────────────────────────

export default function RaidConfigView() {
  const { config, setConfig, loading, saving, error, saveConfig, refresh } = useRaidConfig();
  const { lastMessage, connected } = useWebSocket(
    `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws`
  );
  const [saveSuccess, setSaveSuccess] = useState(false);

  // Apply incoming WebSocket raid_config_updated events.
  useEffect(() => {
    if (!lastMessage) return;
    try {
      const evt = JSON.parse(lastMessage);
      if (evt.type === "raid_config_updated" && evt.data) {
        setConfig(evt.data as RaidConfig);
      }
    } catch {
      // not a JSON event — ignore
    }
  }, [lastMessage, setConfig]);

  // ── local config mutations ────────────────────────────────────────────────

  const updateConfig = (patch: Partial<RaidConfig>) =>
    setConfig((prev) => ({ ...prev, ...patch }));

  const handleRoleChange = (name: string, role: RaidRole) => {
    setConfig((prev) => ({
      ...prev,
      members: prev.members.map((m) => (m.name === name ? { ...m, role } : m)),
      main_tank:   role === "main_tank"   ? name : prev.main_tank === name   ? null : prev.main_tank,
      main_assist: role === "main_assist" ? name : prev.main_assist === name ? null : prev.main_assist,
    }));
  };

  const handleMemberReorder = (from: number, to: number) => {
    setConfig((prev) => {
      const list = [...prev.members];
      const [item] = list.splice(from, 1);
      list.splice(to, 0, item);
      return { ...prev, members: list };
    });
  };

  const handleChReorder = (from: number, to: number) => {
    setConfig((prev) => {
      const list = [...prev.ch_chain];
      const [item] = list.splice(from, 1);
      list.splice(to, 0, item);
      return { ...prev, ch_chain: list };
    });
  };

  const handleChAdd = (name: string) =>
    setConfig((prev) => ({ ...prev, ch_chain: [...prev.ch_chain, name] }));

  const handleChRemove = (idx: number) =>
    setConfig((prev) => ({
      ...prev,
      ch_chain: prev.ch_chain.filter((_, i) => i !== idx),
    }));

  const handleSave = async () => {
    await saveConfig(config);
    setSaveSuccess(true);
    setTimeout(() => setSaveSuccess(false), 2000);
  };

  // ── render ────────────────────────────────────────────────────────────────

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center text-white/40 font-rune">
        Loading raid configuration…
      </div>
    );
  }

  return (
    <section className="flex-1 h-full flex flex-col relative z-20 min-w-[600px]">
      {/* Header */}
      <header className="h-16 border-b border-white/10 flex items-center justify-between px-6 bg-violet/30 backdrop-blur-md flex-shrink-0">
        <div className="flex items-center gap-4">
          <div className="w-8 h-8 rounded border border-magentadark flex items-center justify-center bg-void">
            <UsersThree weight="fill" className="text-magentaglow" />
          </div>
          <div>
            <h2 className="font-archaic text-lg text-white leading-tight">
              Raid Configuration
            </h2>
            <p className="text-[10px] uppercase tracking-widest text-white/50 font-rune flex items-center gap-2">
              Live Composition Editor
              <span
                className={`w-1.5 h-1.5 rounded-full inline-block ${
                  connected ? "bg-green-400" : "bg-red-400"
                }`}
                title={connected ? "WebSocket connected" : "WebSocket disconnected"}
              />
            </p>
          </div>
        </div>
        <div className="flex gap-3 items-center">
          {error && (
            <span className="flex items-center gap-1 text-xs text-yellow-400 font-tech">
              <Warning size={12} weight="fill" /> {error}
            </span>
          )}
          <button
            onClick={refresh}
            title="Refresh from server"
            className="px-3 py-1.5 border border-white/20 text-white/50 text-sm hover:border-spectral hover:text-spectral transition-colors"
          >
            <ArrowClockwise size={14} />
          </button>
          <button
            onClick={handleSave}
            disabled={saving}
            className={`px-4 py-1.5 border text-sm font-medium uppercase tracking-wider flex items-center gap-2 transition-all ${
              saveSuccess
                ? "border-green-500 text-green-400 bg-green-900/20"
                : saving
                ? "border-white/20 text-white/30"
                : "bg-magentadark/20 border-magentaglow text-white hover:bg-magentadark/40 shadow-[0_0_15px_rgba(204,68,255,0.3)]"
            }`}
          >
            <FloppyDisk size={14} weight="fill" />
            {saveSuccess ? "Saved!" : saving ? "Saving…" : "Save Config"}
          </button>
        </div>
      </header>

      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto p-6 scroll-smooth">
        <div className="grid grid-cols-[1fr_1fr] gap-6 mb-6">
          {/* Main Roles */}
          <Section title="Main Roles" icon={Crown}>
            <div className="space-y-4">
              <div>
                <label className="text-xs text-white/50 uppercase tracking-widest font-tech flex items-center gap-1.5 mb-2">
                  <Shield size={11} weight="fill" className="text-yellow-400" />
                  Main Tank
                </label>
                <NameSelect
                  value={config.main_tank}
                  members={config.members}
                  placeholder="— Unassigned —"
                  onChange={(n) => updateConfig({ main_tank: n })}
                />
              </div>
              <div>
                <label className="text-xs text-white/50 uppercase tracking-widest font-tech flex items-center gap-1.5 mb-2">
                  <Crosshair size={11} weight="fill" className="text-spectral" />
                  Main Assist
                </label>
                <NameSelect
                  value={config.main_assist}
                  members={config.members}
                  placeholder="— Unassigned —"
                  onChange={(n) => updateConfig({ main_assist: n })}
                />
              </div>
            </div>
          </Section>

          {/* Behavior Mode */}
          <Section title="Raid Behavior Mode" icon={Lightning}>
            <BehaviorToggle
              value={config.behavior_mode}
              onChange={(m) => updateConfig({ behavior_mode: m })}
            />
            <p className="text-[10px] text-white/30 font-rune mt-3 italic text-center">
              {MODES.find((m) => m.id === config.behavior_mode)?.desc}
            </p>
          </Section>
        </div>

        <div className="grid grid-cols-[1fr_1fr] gap-6 mb-6">
          {/* CH Chain */}
          <Section title="Complete Heal Chain" icon={HeartStraight}>
            <ChChainEditor
              chain={config.ch_chain}
              members={config.members}
              onReorder={handleChReorder}
              onAdd={handleChAdd}
              onRemove={handleChRemove}
            />
          </Section>

          {/* Pull target + camp position */}
          <div className="space-y-6">
            <Section title="Pull Target" icon={HandFist}>
              <input
                type="text"
                value={config.pull_target ?? ""}
                onChange={(e) => updateConfig({ pull_target: e.target.value || null })}
                placeholder="Target NPC name…"
                className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune placeholder:text-white/30"
              />
            </Section>
            <Section title="Camp Position" icon={MapPin}>
              <CampPositionEditor
                value={config.camp_position}
                onChange={(pos) => updateConfig({ camp_position: pos })}
              />
            </Section>
          </div>
        </div>

        {/* Live Composition */}
        <Section title={`Live Raid Composition (${config.members.length} members)`} icon={UsersThree}>
          {config.members.length === 0 ? (
            <p className="text-xs text-white/30 font-rune italic text-center py-4">
              No members — connect to a live session to populate
            </p>
          ) : (
            <CompositionGrid
              members={config.members}
              onRoleChange={handleRoleChange}
              onDragReorder={handleMemberReorder}
            />
          )}
        </Section>
      </div>
    </section>
  );
}
