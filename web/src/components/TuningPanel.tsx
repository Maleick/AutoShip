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
  MusicNotes,
} from "@phosphor-icons/react";
import type {
  AutoRezConfig,
  CharacterConfig,
  ClassParams,
  RotationEntry,
  TributeAlertState,
} from "../types";
import { useCharacterConfigs } from "../hooks/useTuning";
import { formatDuration } from "../utils/time";
import BardSongPanel, { makeDefaultConfig } from "./BardSongPanel";

const DEFAULT_AUTO_REZ_CONFIG: AutoRezConfig = {
  enabled: false,
  min_xp_pct: 90,
  trusted_casters: [],
  decline_if_untrusted: false,
  delay_ms: 3000,
};

function parseTrustedCasters(value: string): string[] {
  return Array.from(
    new Set(
      value
        .split(/[\n,]/)
        .map((entry) => entry.trim())
        .filter(Boolean),
    ),
  );
}

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

function tributeTone(alertState: TributeAlertState) {
  switch (alertState) {
    case "expiring":
      return {
        badge: "border-amber-400/40 bg-amber-400/10 text-amber-200",
        label: "Expiring",
      };
    case "expired":
      return {
        badge: "border-red-500/40 bg-red-500/10 text-red-300",
        label: "Expired",
      };
    default:
      return {
        badge: "border-emerald-400/30 bg-emerald-400/10 text-emerald-200",
        label: "Stable",
      };
  }
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

const DEFAULT_AUTO_CAMP_ON_DEATH = {
  enabled: false,
  camp_delay_secs: 30,
  relog_wait_secs: 900,
};

function CharacterEditor({ config, onSave }: CharacterEditorProps) {
  const [draft, setDraft] = useState<CharacterConfig>({
    ...config,
    auto_rez: config.auto_rez ?? DEFAULT_AUTO_REZ_CONFIG,
  });
  const [saving, setSaving] = useState(false);
  const [saveMsg, setSaveMsg] = useState<string | null>(null);

  // Reset draft when selected character changes.
  useEffect(() => {
    setDraft({
      ...config,
      auto_rez: config.auto_rez ?? DEFAULT_AUTO_REZ_CONFIG,
    });
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

  const autoRez = draft.auto_rez ?? DEFAULT_AUTO_REZ_CONFIG;
  const deathRecovery = draft.auto_camp_on_death ?? DEFAULT_AUTO_CAMP_ON_DEATH;

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

      {/* Death auto-camp */}
      {(() => {
        const deathCfg =
          draft.auto_camp_on_death ?? DEFAULT_AUTO_CAMP_ON_DEATH;

        return (
          <section>
            <h4 className="font-archaic text-xs uppercase tracking-widest text-white/50 mb-3 flex items-center gap-2">
              <Faders size={12} className="text-red-400" />
              Death Recovery
            </h4>
            <div className="bg-violet/20 border border-white/5 p-4 flex flex-col gap-4">
              <button
                onClick={() =>
                  setDraft({
                    ...draft,
                    auto_camp_on_death: {
                      ...deathCfg,
                      enabled: !deathCfg.enabled,
                    },
                  })
                }
                className="flex items-center gap-2 text-sm font-tech transition-colors hover:text-red-300"
              >
                {deathCfg.enabled ? (
                  <CheckSquare weight="fill" size={16} className="text-red-300" />
                ) : (
                  <Square size={16} className="text-white/40" />
                )}
                <span className="text-white/70">
                  Auto-camp to desktop after death
                </span>
              </button>
              <p className="text-[10px] font-rune text-white/35">
                Wait for a rez window, then camp out and hand off to AutoLogin for
                a delayed return.
              </p>
              <div className="grid grid-cols-2 gap-3">
                <label className="flex flex-col gap-1 text-[10px] font-rune uppercase tracking-widest text-white/45">
                  Camp Delay (s)
                  <input
                    type="number"
                    min={0}
                    value={deathCfg.camp_delay_secs}
                    onChange={(e) =>
                      setDraft({
                        ...draft,
                        auto_camp_on_death: {
                          ...deathCfg,
                          camp_delay_secs: Math.max(0, Number(e.target.value) || 0),
                        },
                      })
                    }
                    className="bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-red-300"
                  />
                </label>
                <label className="flex flex-col gap-1 text-[10px] font-rune uppercase tracking-widest text-white/45">
                  Relog Wait (s)
                  <input
                    type="number"
                    min={0}
                    value={deathCfg.relog_wait_secs}
                    onChange={(e) =>
                      setDraft({
                        ...draft,
                        auto_camp_on_death: {
                          ...deathCfg,
                          relog_wait_secs: Math.max(0, Number(e.target.value) || 0),
                        },
                      })
                    }
                    className="bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-red-300"
                  />
                </label>
              </div>
            </div>
          </section>
        );
      })()}

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

      <section>
        <h4 className="font-archaic text-xs uppercase tracking-widest text-white/50 mb-3 flex items-center gap-2">
          <Faders size={12} className="text-green-400" />
          Resurrection Offers
        </h4>
        <div className="bg-violet/20 border border-white/5 p-4 flex flex-col gap-5">
          <button
            onClick={() =>
              setDraft({
                ...draft,
                auto_rez: { ...autoRez, enabled: !autoRez.enabled },
              })
            }
            className="flex items-center gap-2 text-sm font-tech transition-colors hover:text-magentaglow"
          >
            {autoRez.enabled ? (
              <CheckSquare weight="fill" size={16} className="text-magentaglow" />
            ) : (
              <Square size={16} className="text-white/40" />
            )}
            <span className="text-white/70">
              Auto-handle incoming resurrection offers
            </span>
          </button>

          <ThresholdSlider
            label="Minimum Rez XP %"
            value={autoRez.min_xp_pct}
            color="text-green-400"
            onChange={(v) =>
              setDraft({
                ...draft,
                auto_rez: { ...autoRez, min_xp_pct: v },
              })
            }
          />

          <div className="flex items-center gap-4">
            <button
              onClick={() =>
                setDraft({
                  ...draft,
                  auto_rez: {
                    ...autoRez,
                    decline_if_untrusted: !autoRez.decline_if_untrusted,
                  },
                })
              }
              className="flex items-center gap-2 text-sm font-tech transition-colors hover:text-magentaglow"
            >
              {autoRez.decline_if_untrusted ? (
                <CheckSquare weight="fill" size={16} className="text-magentaglow" />
              ) : (
                <Square size={16} className="text-white/40" />
              )}
              <span className="text-white/70">
                Auto-decline offers that fail policy checks
              </span>
            </button>

            <label className="ml-auto flex items-center gap-2 text-xs font-tech text-white/70">
              Delay
              <input
                type="number"
                min={0}
                max={15000}
                step={100}
                value={autoRez.delay_ms}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    auto_rez: {
                      ...autoRez,
                      delay_ms: Math.min(
                        15000,
                        Math.max(0, Math.trunc(Number(e.target.value) || 0)),
                      ),
                    },
                  })
                }
                className="w-28 bg-void border border-white/20 text-white text-xs px-3 py-1
                  focus:outline-none focus:border-magentaglow font-rune"
              />
              <span className="text-white/40 font-rune">ms</span>
            </label>
          </div>

          <label className="flex flex-col gap-2 text-xs font-tech text-white/70">
            Trusted Casters
            <textarea
              rows={4}
              value={autoRez.trusted_casters.join("\n")}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  auto_rez: {
                    ...autoRez,
                    trusted_casters: parseTrustedCasters(e.target.value),
                  },
                })
              }
              placeholder="One character per line or comma-separated"
              className="bg-void border border-white/20 text-white text-xs px-3 py-2
                focus:outline-none focus:border-magentaglow font-rune placeholder:text-white/30
                resize-y min-h-24"
            />
          </label>

          <p className="text-[10px] text-white/35 font-rune">
            Offers are only accepted when the rez percent meets the threshold
            and the caster appears in the trust list. The delay leaves a manual
            override window before TextQuest clicks the popup.
          </p>
        </div>
      </section>

      {/* Tribute automation */}
      <section>
        <h4 className="font-archaic text-xs uppercase tracking-widest text-white/50 mb-3 flex items-center gap-2">
          <Faders size={12} className="text-amber-300" />
          Tribute Automation
        </h4>
        <div className="bg-violet/20 border border-white/5 p-4 flex flex-col gap-4">
          <div className="grid grid-cols-4 gap-3">
            <div className="border border-white/10 bg-void/40 px-3 py-2">
              <div className="text-[10px] uppercase tracking-widest text-white/35 font-rune">
                Status
              </div>
              <div className="mt-2">
                <span
                  className={`inline-flex items-center border px-2 py-1 text-[10px] uppercase tracking-widest font-rune ${
                    tributeTone(draft.tribute_status.alert_state).badge
                  }`}
                >
                  {draft.tribute_status.active ? "Active" : "Inactive"} ·{" "}
                  {tributeTone(draft.tribute_status.alert_state).label}
                </span>
              </div>
            </div>
            <div className="border border-white/10 bg-void/40 px-3 py-2">
              <div className="text-[10px] uppercase tracking-widest text-white/35 font-rune">
                Time Remaining
              </div>
              <div className="mt-2 font-rune text-lg text-white">
                {formatDuration(draft.tribute_status.remaining_secs)}
              </div>
            </div>
            <div className="border border-white/10 bg-void/40 px-3 py-2">
              <div className="text-[10px] uppercase tracking-widest text-white/35 font-rune">
                Tribute Balance
              </div>
              <div className="mt-2 font-rune text-lg text-amber-200">
                {draft.tribute_status.point_balance.toLocaleString()}
              </div>
            </div>
            <div className="border border-white/10 bg-void/40 px-3 py-2">
              <div className="text-[10px] uppercase tracking-widest text-white/35 font-rune">
                Active Bonuses
              </div>
              <div className="mt-2 text-xs text-white/70 font-tech">
                {draft.tribute_status.active_tributes.length > 0
                  ? draft.tribute_status.active_tributes.join(", ")
                  : "None"}
              </div>
            </div>
          </div>

          <div className="flex items-center gap-3 border border-white/10 bg-void/30 px-3 py-2">
            <button
              onClick={() =>
                setDraft({
                  ...draft,
                  tribute_preferences: {
                    ...draft.tribute_preferences,
                    auto_activate: !draft.tribute_preferences.auto_activate,
                  },
                })
              }
              className="flex items-center gap-2 text-sm font-tech transition-colors hover:text-magentaglow"
            >
              {draft.tribute_preferences.auto_activate ? (
                <CheckSquare weight="fill" size={16} className="text-magentaglow" />
              ) : (
                <Square size={16} className="text-white/40" />
              )}
              <span className="text-white/70">Auto-activate on expiry</span>
            </button>
          </div>

          <div className="grid grid-cols-[180px_1fr] gap-3 items-start">
            <label
              htmlFor="tribute-warning-threshold"
              className="text-[10px] uppercase tracking-widest text-white/35 font-rune pt-2"
            >
              Warning Lead Time
            </label>
            <input
              id="tribute-warning-threshold"
              type="number"
              min={0}
              value={draft.tribute_preferences.warning_threshold_secs}
              onChange={(e) => {
                const nextValue = e.currentTarget.valueAsNumber;
                if (!Number.isFinite(nextValue)) {
                  return;
                }
                setDraft({
                  ...draft,
                  tribute_preferences: {
                    ...draft.tribute_preferences,
                    warning_threshold_secs: Math.max(0, Math.trunc(nextValue)),
                  },
                });
              }}
              className="w-40 bg-void border border-white/20 text-white text-xs px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
            />
          </div>

          <div className="grid grid-cols-[180px_1fr] gap-3 items-start">
            <label
              htmlFor="preferred-tributes"
              className="text-[10px] uppercase tracking-widest text-white/35 font-rune pt-2"
            >
              Preferred Tributes
            </label>
            <input
              id="preferred-tributes"
              type="text"
              value={draft.tribute_preferences.preferred_tributes.join(", ")}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  tribute_preferences: {
                    ...draft.tribute_preferences,
                    preferred_tributes: e.target.value
                      .split(",")
                      .map((entry) => entry.trim())
                      .filter(Boolean),
                  },
                })
              }
              className="w-full bg-void border border-white/20 text-white text-xs px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
            />
          </div>
          <p className="text-[10px] text-white/35 font-rune">
            Comma-separated tribute names. The monitor warns before expiry and
            re-activates this list when points are available.
          </p>
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

      {/* Bard song configuration */}
      {draft.class === "Bard" && (
        <section>
          <h4 className="font-archaic text-xs uppercase tracking-widest text-white/50 mb-3 flex items-center gap-2">
            <MusicNotes size={12} className="text-magentaglow" />
            Bard Song Automation
          </h4>
          <div className="bg-violet/20 border border-white/5 p-4">
            <BardSongPanel
              config={draft.bard ?? makeDefaultConfig(draft.character_name)}
              onSave={async (bardConfig) => {
                setDraft({ ...draft, bard: bardConfig });
              }}
            />
          </div>
        </section>
      )}
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
    auto_rez: {
      enabled: true,
      min_xp_pct: 96,
      trusted_casters: ["Highclerk", "Leafbinder"],
      decline_if_untrusted: true,
      delay_ms: 5000,
    },
    group_override: false,
    auto_camp_on_death: {
      enabled: true,
      camp_delay_secs: 30,
      relog_wait_secs: 900,
    },
    tribute_preferences: {
      auto_activate: true,
      warning_threshold_secs: 300,
      preferred_tributes: ["Marr's Gift", "Champion's Aura"],
    },
    tribute_status: {
      active: true,
      remaining_secs: 240,
      point_balance: 3200,
      active_tributes: ["Marr's Gift"],
      alert_state: "expiring",
    },
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
    auto_rez: {
      enabled: false,
      min_xp_pct: 90,
      trusted_casters: ["Frostreaver"],
      decline_if_untrusted: false,
      delay_ms: 3000,
    },
    group_override: false,
    auto_camp_on_death: {
      enabled: false,
      camp_delay_secs: 30,
      relog_wait_secs: 900,
    },
    tribute_preferences: {
      auto_activate: true,
      warning_threshold_secs: 420,
      preferred_tributes: ["Stalwart Ward", "Champion's Aura"],
    },
    tribute_status: {
      active: true,
      remaining_secs: 3600,
      point_balance: 1950,
      active_tributes: ["Stalwart Ward", "Champion's Aura"],
      alert_state: "ok",
    },
  },
  {
    character_name: "Melodica",
    class: "Bard",
    role: "Support",
    heal_at_pct: 60,
    mana_sit_pct: 25,
    nuke_at_pct: 90,
    rotation: [
      { id: "b1", name: "Celestial Clarity", priority: 1, enabled: true },
      { id: "b2", name: "Aeon's Harmony", priority: 2, enabled: true },
      { id: "b3", name: "Blade Chords", priority: 3, enabled: true },
      { id: "b4", name: "Crescendo of the Siren", priority: 4, enabled: true },
      { id: "b5", name: "Warless Superbia", priority: 5, enabled: false },
    ],
    class_params: {},
    auto_rez: {
      enabled: false,
      min_xp_pct: 90,
      trusted_casters: [],
      decline_if_untrusted: false,
      delay_ms: 3000,
    },
    group_override: false,
    tribute_preferences: {
      auto_activate: true,
      warning_threshold_secs: 300,
      preferred_tributes: [],
    },
    tribute_status: {
      active: false,
      remaining_secs: 0,
      point_balance: 0,
      active_tributes: [],
      alert_state: "ok",
    },
    bard: makeDefaultConfig("Melodica"),
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
