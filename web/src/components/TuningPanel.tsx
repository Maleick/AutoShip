import { useState, useEffect } from "react";
import {
  Faders,
  ArrowUp,
  ArrowDown,
  FloppyDisk,
  ArrowsClockwise,
  CheckSquare,
  Square,
  UsersThree,
} from "@phosphor-icons/react";
import type { CharacterConfig, ClassParams, RotationEntry } from "../types";
import { useCharacterConfigs } from "../hooks/useTuning";

// ─── Slider ──────────────────────────────────────────────────────────────────

interface SliderProps {
  label: string;
  value: number;
  min?: number;
  max?: number;
  color?: string;
  onChange: (v: number) => void;
}

function ThresholdSlider({
  label,
  value,
  min = 0,
  max = 100,
  color = "text-magentaglow",
  onChange,
}: SliderProps) {
  return (
    <div className="flex flex-col gap-1">
      <div className="flex justify-between text-xs font-tech">
        <span className="text-white/70">{label}</span>
        <span className={`font-bold ${color}`}>{value}%</span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-full h-1.5 appearance-none cursor-pointer bg-void border border-white/10 rounded-none
          [&::-webkit-slider-thumb]:appearance-none
          [&::-webkit-slider-thumb]:w-3
          [&::-webkit-slider-thumb]:h-3
          [&::-webkit-slider-thumb]:rotate-45
          [&::-webkit-slider-thumb]:bg-magentaglow
          [&::-webkit-slider-thumb]:cursor-pointer
          [&::-webkit-slider-thumb]:shadow-[0_0_6px_#ff00ff]"
      />
      <div className="flex justify-between text-[9px] text-white/30 font-rune">
        <span>{min}%</span>
        <span>{max}%</span>
      </div>
    </div>
  );
}

// ─── Rotation entry row ───────────────────────────────────────────────────────

interface RotationRowProps {
  entry: RotationEntry;
  index: number;
  total: number;
  onMoveUp: () => void;
  onMoveDown: () => void;
  onToggle: () => void;
}

function RotationRow({
  entry,
  index,
  total,
  onMoveUp,
  onMoveDown,
  onToggle,
}: RotationRowProps) {
  return (
    <div
      className={`flex items-center gap-3 px-3 py-2 border transition-colors
        ${entry.enabled ? "border-white/10 bg-violet/30 hover:border-magentadark/50" : "border-white/5 bg-void/50 opacity-50"}`}
    >
      <span className="text-[10px] font-rune text-white/30 w-5 text-right shrink-0">
        {index + 1}
      </span>
      <button
        onClick={onToggle}
        className="text-white/50 hover:text-magentaglow transition-colors shrink-0"
        title={entry.enabled ? "Disable" : "Enable"}
      >
        {entry.enabled ? (
          <CheckSquare weight="fill" size={16} />
        ) : (
          <Square size={16} />
        )}
      </button>
      <span className="flex-1 text-sm font-tech text-white/80">{entry.name}</span>
      <div className="flex gap-1 shrink-0">
        <button
          onClick={onMoveUp}
          disabled={index === 0}
          className="p-1 text-white/40 hover:text-spectral disabled:opacity-20 disabled:cursor-not-allowed transition-colors"
          title="Move up (higher priority)"
        >
          <ArrowUp size={12} />
        </button>
        <button
          onClick={onMoveDown}
          disabled={index === total - 1}
          className="p-1 text-white/40 hover:text-spectral disabled:opacity-20 disabled:cursor-not-allowed transition-colors"
          title="Move down (lower priority)"
        >
          <ArrowDown size={12} />
        </button>
      </div>
    </div>
  );
}

// ─── Class-specific params section ───────────────────────────────────────────

interface ClassParamsEditorProps {
  charClass: string;
  params: ClassParams;
  onChange: (params: ClassParams) => void;
}

function ClassParamsEditor({
  charClass,
  params,
  onChange,
}: ClassParamsEditorProps) {
  const cls = charClass.toLowerCase();
  const hasClericParams = cls === "cleric";
  const hasNecroParams = cls === "necromancer";
  const hasBurnParam =
    cls === "warrior" ||
    cls === "wizard" ||
    cls === "magician" ||
    cls === "enchanter" ||
    cls === "necromancer";
  const hasSlowParam =
    cls === "warrior" || cls === "shaman" || cls === "enchanter";

  if (!hasClericParams && !hasNecroParams && !hasBurnParam && !hasSlowParam) {
    return (
      <p className="text-xs text-white/30 font-rune italic">
        No class-specific parameters for {charClass}.
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      {hasClericParams && (
        <div className="flex flex-col gap-1">
          <div className="flex justify-between text-xs font-tech">
            <span className="text-white/70">CH Chain Timing (ms)</span>
            <span className="font-bold text-spectral">
              {params.ch_chain_timing_ms ?? 200} ms
            </span>
          </div>
          <input
            type="range"
            min={50}
            max={1000}
            step={25}
            value={params.ch_chain_timing_ms ?? 200}
            onChange={(e) =>
              onChange({
                ...params,
                ch_chain_timing_ms: Number(e.target.value),
              })
            }
            className="w-full h-1.5 appearance-none cursor-pointer bg-void border border-white/10
              [&::-webkit-slider-thumb]:appearance-none
              [&::-webkit-slider-thumb]:w-3
              [&::-webkit-slider-thumb]:h-3
              [&::-webkit-slider-thumb]:rotate-45
              [&::-webkit-slider-thumb]:bg-spectral
              [&::-webkit-slider-thumb]:cursor-pointer"
          />
          <div className="flex justify-between text-[9px] text-white/30 font-rune">
            <span>50 ms</span>
            <span>1000 ms</span>
          </div>
        </div>
      )}

      {hasNecroParams && (
        <div className="flex flex-col gap-1">
          <div className="flex justify-between text-xs font-tech">
            <span className="text-white/70">DoT Overlap %</span>
            <span className="font-bold text-magentaglow">
              {params.dot_overlap_pct ?? 20}%
            </span>
          </div>
          <input
            type="range"
            min={0}
            max={50}
            value={params.dot_overlap_pct ?? 20}
            onChange={(e) =>
              onChange({ ...params, dot_overlap_pct: Number(e.target.value) })
            }
            className="w-full h-1.5 appearance-none cursor-pointer bg-void border border-white/10
              [&::-webkit-slider-thumb]:appearance-none
              [&::-webkit-slider-thumb]:w-3
              [&::-webkit-slider-thumb]:h-3
              [&::-webkit-slider-thumb]:rotate-45
              [&::-webkit-slider-thumb]:bg-magentaglow
              [&::-webkit-slider-thumb]:cursor-pointer"
          />
        </div>
      )}

      {hasBurnParam && (
        <div className="flex flex-col gap-1">
          <div className="flex justify-between text-xs font-tech">
            <span className="text-white/70">Burn Phase at HP%</span>
            <span className="font-bold text-red-400">
              {params.burn_at_hp_pct ?? 30}%
            </span>
          </div>
          <input
            type="range"
            min={5}
            max={60}
            value={params.burn_at_hp_pct ?? 30}
            onChange={(e) =>
              onChange({ ...params, burn_at_hp_pct: Number(e.target.value) })
            }
            className="w-full h-1.5 appearance-none cursor-pointer bg-void border border-white/10
              [&::-webkit-slider-thumb]:appearance-none
              [&::-webkit-slider-thumb]:w-3
              [&::-webkit-slider-thumb]:h-3
              [&::-webkit-slider-thumb]:rotate-45
              [&::-webkit-slider-thumb]:bg-red-500
              [&::-webkit-slider-thumb]:cursor-pointer"
          />
        </div>
      )}

      {hasSlowParam && (
        <div className="flex flex-col gap-1">
          <div className="flex justify-between text-xs font-tech">
            <span className="text-white/70">Slow at HP%</span>
            <span className="font-bold text-yellow-400">
              {params.slow_at_hp_pct ?? 80}%
            </span>
          </div>
          <input
            type="range"
            min={10}
            max={100}
            value={params.slow_at_hp_pct ?? 80}
            onChange={(e) =>
              onChange({ ...params, slow_at_hp_pct: Number(e.target.value) })
            }
            className="w-full h-1.5 appearance-none cursor-pointer bg-void border border-white/10
              [&::-webkit-slider-thumb]:appearance-none
              [&::-webkit-slider-thumb]:w-3
              [&::-webkit-slider-thumb]:h-3
              [&::-webkit-slider-thumb]:rotate-45
              [&::-webkit-slider-thumb]:bg-yellow-400
              [&::-webkit-slider-thumb]:cursor-pointer"
          />
        </div>
      )}
    </div>
  );
}

// ─── Character editor ─────────────────────────────────────────────────────────

interface CharacterEditorProps {
  config: CharacterConfig;
  onSave: (cfg: CharacterConfig) => Promise<void>;
}

function CharacterEditor({ config, onSave }: CharacterEditorProps) {
  const [draft, setDraft] = useState<CharacterConfig>(config);
  const [saving, setSaving] = useState(false);
  const [saveMsg, setSaveMsg] = useState<string | null>(null);

  // Reset draft when selected character changes.
  useEffect(() => {
    setDraft(config);
    setSaveMsg(null);
  }, [config]);

  const isDirty = JSON.stringify(draft) !== JSON.stringify(config);

  const moveRotation = (index: number, dir: -1 | 1) => {
    const rot = [...draft.rotation];
    const target = index + dir;
    if (target < 0 || target >= rot.length) return;
    [rot[index], rot[target]] = [rot[target], rot[index]];
    // Re-assign sequential priorities.
    const reindexed = rot.map((e, i) => ({ ...e, priority: i + 1 }));
    setDraft({ ...draft, rotation: reindexed });
  };

  const toggleRotation = (index: number) => {
    const rot = draft.rotation.map((e, i) =>
      i === index ? { ...e, enabled: !e.enabled } : e,
    );
    setDraft({ ...draft, rotation: rot });
  };

  const handleSave = async () => {
    setSaving(true);
    setSaveMsg(null);
    try {
      await onSave(draft);
      setSaveMsg("Saved");
    } catch (e) {
      console.error("Failed to save character config:", e);
      setSaveMsg(e instanceof Error ? e.message : "Save failed");
    } finally {
      setSaving(false);
      setTimeout(() => setSaveMsg(null), 2500);
    }
  };

  const roleColor =
    draft.role === "Healer"
      ? "text-green-400"
      : draft.role === "Tank"
        ? "text-yellow-400"
        : draft.role === "Support"
          ? "text-spectral"
          : "text-magentaglow";

  return (
    <div className="flex flex-col gap-6 overflow-y-auto pr-1">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="font-archaic text-xl text-white">
            {draft.character_name}
          </h3>
          <p className="text-xs font-rune text-white/50 mt-0.5">
            {draft.class} ·{" "}
            <span className={`font-bold ${roleColor}`}>{draft.role}</span>
          </p>
        </div>
        <div className="flex items-center gap-3">
          {saveMsg && (
            <span
              className={`text-xs font-rune ${saveMsg === "Saved" ? "text-green-400" : "text-red-400"}`}
            >
              {saveMsg}
            </span>
          )}
          {isDirty && !saving && (
            <button
              onClick={() => setDraft(config)}
              className="p-2 text-white/40 hover:text-spectral transition-colors"
              title="Discard changes"
            >
              <ArrowsClockwise size={16} />
            </button>
          )}
          <button
            onClick={handleSave}
            disabled={!isDirty || saving}
            className="flex items-center gap-2 px-4 py-1.5 bg-magentadark/20 border border-magentaglow text-white text-sm font-medium
              hover:bg-magentadark/40 transition-colors uppercase tracking-wider
              shadow-[0_0_10px_rgba(204,68,255,0.2)]
              disabled:opacity-40 disabled:cursor-not-allowed disabled:shadow-none"
          >
            <FloppyDisk size={14} weight="fill" />
            {saving ? "Saving…" : "Commit"}
          </button>
        </div>
      </div>

      {/* Thresholds */}
      <section>
        <h4 className="font-archaic text-xs uppercase tracking-widest text-white/50 mb-3 flex items-center gap-2">
          <Faders size={12} className="text-magentaglow" />
          Thresholds
        </h4>
        <div className="bg-violet/20 border border-white/5 p-4 flex flex-col gap-5">
          <ThresholdSlider
            label="Heal At %"
            value={draft.heal_at_pct}
            color="text-green-400"
            onChange={(v) => setDraft({ ...draft, heal_at_pct: v })}
          />
          <ThresholdSlider
            label="Mana Sit %"
            value={draft.mana_sit_pct}
            color="text-blue-400"
            onChange={(v) => setDraft({ ...draft, mana_sit_pct: v })}
          />
          <ThresholdSlider
            label="Nuke At %"
            value={draft.nuke_at_pct}
            color="text-magentaglow"
            onChange={(v) => setDraft({ ...draft, nuke_at_pct: v })}
          />
        </div>
      </section>

      {/* Rotation priority */}
      <section>
        <h4 className="font-archaic text-xs uppercase tracking-widest text-white/50 mb-3 flex items-center gap-2">
          <ArrowUp size={12} className="text-spectral" />
          DPS / Heal Rotation Priority
        </h4>
        <p className="text-[10px] text-white/30 font-rune mb-3">
          Lower index = higher priority. Use arrows to reorder, checkbox to
          enable/disable.
        </p>
        <div className="flex flex-col gap-1">
          {draft.rotation.map((entry, i) => (
            <RotationRow
              key={entry.id}
              entry={entry}
              index={i}
              total={draft.rotation.length}
              onMoveUp={() => moveRotation(i, -1)}
              onMoveDown={() => moveRotation(i, 1)}
              onToggle={() => toggleRotation(i)}
            />
          ))}
        </div>
      </section>

      {/* Class-specific params */}
      <section>
        <h4 className="font-archaic text-xs uppercase tracking-widest text-white/50 mb-3 flex items-center gap-2">
          <Faders size={12} className="text-yellow-400" />
          Class Strategy Parameters
        </h4>
        <div className="bg-violet/20 border border-white/5 p-4">
          <ClassParamsEditor
            charClass={draft.class}
            params={draft.class_params}
            onChange={(p) => setDraft({ ...draft, class_params: p })}
          />
        </div>
      </section>

      {/* Group override */}
      <section>
        <h4 className="font-archaic text-xs uppercase tracking-widest text-white/50 mb-3 flex items-center gap-2">
          <UsersThree size={12} className="text-spectral" />
          Group Override
        </h4>
        <div className="bg-violet/20 border border-white/5 p-4 flex items-center gap-4">
          <button
            onClick={() =>
              setDraft({ ...draft, group_override: !draft.group_override })
            }
            className="flex items-center gap-2 text-sm font-tech transition-colors hover:text-magentaglow"
          >
            {draft.group_override ? (
              <CheckSquare weight="fill" size={16} className="text-magentaglow" />
            ) : (
              <Square size={16} className="text-white/40" />
            )}
            <span className="text-white/70">Enable group-level overrides</span>
          </button>
          {draft.group_override && (
            <input
              type="text"
              placeholder="Group name…"
              value={draft.group_name ?? ""}
              onChange={(e) =>
                setDraft({ ...draft, group_name: e.target.value || undefined })
              }
              className="ml-auto bg-void border border-white/20 text-white text-xs px-3 py-1
                focus:outline-none focus:border-magentaglow font-rune placeholder:text-white/30 w-40"
            />
          )}
        </div>
      </section>
    </div>
  );
}

// ─── Main panel ───────────────────────────────────────────────────────────────

/** Fallback demo configs shown while the API is loading or unreachable. */
const DEMO_CONFIGS: CharacterConfig[] = [
  {
    character_name: "Frostreaver",
    class: "Cleric",
    role: "Healer",
    heal_at_pct: 70,
    mana_sit_pct: 30,
    nuke_at_pct: 95,
    rotation: [
      { id: "1", name: "Complete Heal", priority: 1, enabled: true },
      { id: "2", name: "Light Healing", priority: 2, enabled: true },
      { id: "3", name: "Minor Healing", priority: 3, enabled: true },
    ],
    class_params: { ch_chain_timing_ms: 200 },
    group_override: false,
  },
  {
    character_name: "Noxus",
    class: "Warrior",
    role: "Tank",
    heal_at_pct: 50,
    mana_sit_pct: 10,
    nuke_at_pct: 100,
    rotation: [
      { id: "1", name: "Kick", priority: 1, enabled: true },
      { id: "2", name: "Bash", priority: 2, enabled: true },
      { id: "3", name: "Taunt", priority: 3, enabled: true },
    ],
    class_params: { burn_at_hp_pct: 30, slow_at_hp_pct: 80 },
    group_override: false,
  },
];

export default function TuningPanel() {
  const { configs: apiConfigs, loading, error, saveConfig } = useCharacterConfigs();
  const configs = loading || error || apiConfigs.length === 0 ? DEMO_CONFIGS : apiConfigs;
  const [selectedName, setSelectedName] = useState<string>("");

  // Derive the effective selection: fall back to the first character if none chosen.
  const effectiveSelectedName =
    selectedName && configs.some((c) => c.character_name === selectedName)
      ? selectedName
      : configs[0]?.character_name ?? "";
  const selected = configs.find((c) => c.character_name === effectiveSelectedName);

  return (
    <section className="flex-1 h-full flex flex-col relative z-20 min-w-[600px]">
      {/* Header */}
      <header className="h-16 border-b border-white/10 flex items-center justify-between px-6 bg-violet/30 backdrop-blur-md shrink-0">
        <div className="flex items-center gap-4">
          <div className="w-8 h-8 rounded border border-magentadark flex items-center justify-center bg-void">
            <Faders weight="fill" className="text-magentaglow" />
          </div>
          <div>
            <h2 className="font-archaic text-lg text-white leading-tight">
              Strategy Tuning
            </h2>
            <p className="text-[10px] uppercase tracking-widest text-white/50 font-rune">
              DPS &amp; Healing Configuration
            </p>
          </div>
        </div>
        {(loading || error) && (
          <span className="text-xs font-rune text-yellow-400/70">
            {loading ? "Loading…" : `Demo mode — ${error}`}
          </span>
        )}
      </header>

      <div className="flex flex-1 overflow-hidden">
        {/* Character tabs */}
        <nav className="w-44 shrink-0 border-r border-white/10 flex flex-col pt-3 overflow-y-auto bg-void/40">
          {configs.map((c) => {
            const isActive = c.character_name === effectiveSelectedName;
            const roleColor =
              c.role === "Healer"
                ? "text-green-400"
                : c.role === "Tank"
                  ? "text-yellow-400"
                  : c.role === "Support"
                    ? "text-spectral"
                    : "text-magentaglow";
            return (
              <button
                key={c.character_name}
                onClick={() => setSelectedName(c.character_name)}
                className={`w-full text-left px-4 py-3 border-b border-white/5 transition-all
                  ${isActive ? "bg-violet/60 border-l-2 border-l-magentaglow" : "hover:bg-violet/20"}`}
              >
                <div className="font-tech text-sm text-white/90 truncate">
                  {c.character_name}
                </div>
                <div className={`text-[10px] font-rune ${roleColor}`}>
                  {c.class}
                </div>
              </button>
            );
          })}
        </nav>

        {/* Editor */}
        <div className="flex-1 overflow-y-auto p-6">
          {selected ? (
            <CharacterEditor
              key={selected.character_name}
              config={selected}
              onSave={saveConfig}
            />
          ) : (
            <p className="text-white/30 font-rune text-sm">
              Select a character to configure.
            </p>
          )}
        </div>
      </div>
    </section>
  );
}
