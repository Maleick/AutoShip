import { useState, useRef, useCallback } from "react";
import {
  MusicNotes,
  FloppyDisk,
  ArrowsClockwise,
  CheckSquare,
  Square,
  DotsSixVertical,
  Trash,
  Plus,
  CaretUp,
  CaretDown,
} from "@phosphor-icons/react";
import type {
  BardConfig,
  SongSlotConfig,
  InstrumentSet,
  InstrumentType,
  SongCategory,
  InstrumentSlot,
} from "../types";

const CATEGORY_COLORS: Record<SongCategory, string> = {
  Haste: "text-amber-300",
  SpellFocus: "text-blue-300",
  MeleeProc: "text-red-400",
  Crescendo: "text-rose-300",
  Insult: "text-orange-400",
  RunSpeed: "text-emerald-300",
  Regen: "text-green-400",
  Tank: "text-yellow-400",
  Slow: "text-indigo-300",
  Accelerando: "text-cyan-300",
  Mez: "text-violet-300",
  Dot: "text-purple-300",
  Arcane: "text-fuchsia-300",
  Other: "text-white/50",
};

const INSTRUMENT_LABELS: Record<InstrumentType, string> = {
  None: "Default",
  String: "String",
  Brass: "Brass",
  Wind: "Wind",
  Percussion: "Perc",
};

const SLOT_LABELS: Record<InstrumentSlot, string> = {
  Primary: "Primary",
  Secondary: "Secondary",
};

const DEFAULT_SONGS: SongSlotConfig[] = [
  {
    id: "s1",
    gem: 1,
    name: "Celestial Clarity",
    priority: 1,
    enabled: true,
    min_recast_ticks: 66,
    buff_duration_ticks: 240,
    category: "Haste",
    instrument_type: "None",
    instrument_slot: "Primary",
  },
  {
    id: "s2",
    gem: 2,
    name: "Aeon's Harmony",
    priority: 2,
    enabled: true,
    min_recast_ticks: 66,
    buff_duration_ticks: 240,
    category: "SpellFocus",
    instrument_type: "None",
    instrument_slot: "Primary",
  },
  {
    id: "s3",
    gem: 3,
    name: "Blade Chords",
    priority: 3,
    enabled: true,
    min_recast_ticks: 66,
    buff_duration_ticks: 240,
    category: "MeleeProc",
    instrument_type: "None",
    instrument_slot: "Primary",
  },
  {
    id: "s4",
    gem: 4,
    name: "Crescendo of the Siren",
    priority: 4,
    enabled: true,
    min_recast_ticks: 66,
    buff_duration_ticks: 240,
    category: "Crescendo",
    instrument_type: "None",
    instrument_slot: "Primary",
  },
  {
    id: "s5",
    gem: 5,
    name: "Warless Superbia",
    priority: 5,
    enabled: false,
    min_recast_ticks: 66,
    buff_duration_ticks: null,
    category: "Insult",
    instrument_type: "None",
    instrument_slot: "Primary",
  },
];

const DEFAULT_INSTRUMENTS: InstrumentSet[] = [
  { string_item_id: null, brass_item_id: null, wind_item_id: null, percussion_item_id: null },
];

function categoryTone(cat: SongCategory): string {
  return CATEGORY_COLORS[cat] ?? "text-white/50";
}

function makeDefaultConfig(characterName: string): BardConfig {
  return {
    character_name: characterName,
    twist_enabled: true,
    full_rotation_enabled: true,
    instrument_swap_enabled: true,
    songs: DEFAULT_SONGS.map((s, i) => ({ ...s, id: `s${i + 1}` })),
    instruments: DEFAULT_INSTRUMENTS,
  };
}

interface SongRowProps {
  song: SongSlotConfig;
  index: number;
  total: number;
  dragHandleProps: Record<string, unknown>;
  onMoveUp: () => void;
  onMoveDown: () => void;
  onToggle: () => void;
  onRemove: () => void;
  onChange: (s: SongSlotConfig) => void;
}

function SongRow({
  song,
  index,
  total,
  dragHandleProps,
  onMoveUp,
  onMoveDown,
  onToggle,
  onRemove,
  onChange,
}: SongRowProps) {
  const categories: SongCategory[] = [
    "Haste", "SpellFocus", "MeleeProc", "Crescendo", "Insult",
    "RunSpeed", "Regen", "Tank", "Slow", "Accelerando",
    "Mez", "Dot", "Arcane", "Other",
  ];
  const instrumentTypes: InstrumentType[] = ["None", "String", "Brass", "Wind", "Percussion"];
  const instrumentSlots: InstrumentSlot[] = ["Primary", "Secondary"];

  return (
    <div
      className={`flex flex-col gap-1 px-3 py-2 border transition-colors rounded-xl mb-1
        ${song.enabled ? "border-white/10 bg-violet/20 hover:border-magentadark/50" : "border-white/5 bg-void/50 opacity-50"}`}
    >
      <div className="flex items-center gap-2">
        <span
          className="text-[10px] font-rune text-white/30 w-5 text-right shrink-0 cursor-grab active:cursor-grabbing"
          {...dragHandleProps}
          title="Drag to reorder"
        >
          <DotsSixVertical size={14} className="mx-auto" />
        </span>
        <span className="text-[10px] font-rune text-white/30 w-5 text-right shrink-0">
          {index + 1}
        </span>
        <button
          onClick={onToggle}
          className="text-white/50 hover:text-magentaglow transition-colors shrink-0"
          title={song.enabled ? "Disable" : "Enable"}
        >
          {song.enabled ? (
            <CheckSquare weight="fill" size={14} />
          ) : (
            <Square size={14} />
          )}
        </button>
        <input
          type="text"
          value={song.name}
          onChange={(e) => onChange({ ...song, name: e.target.value })}
          className="flex-1 bg-transparent border-none text-sm font-tech text-white/80 focus:outline-none focus:text-white placeholder:text-white/20"
          placeholder="Song name..."
        />
        <span className={`text-[10px] font-rune shrink-0 ${categoryTone(song.category)}`}>
          {song.category}
        </span>
        <div className="flex gap-1 shrink-0">
          <button
            onClick={onMoveUp}
            disabled={index === 0}
            className="p-1 text-white/40 hover:text-spectral disabled:opacity-20 disabled:cursor-not-allowed transition-colors"
            title="Move up (higher priority)"
          >
            <CaretUp size={10} />
          </button>
          <button
            onClick={onMoveDown}
            disabled={index === total - 1}
            className="p-1 text-white/40 hover:text-spectral disabled:opacity-20 disabled:cursor-not-allowed transition-colors"
            title="Move down (lower priority)"
          >
            <CaretDown size={10} />
          </button>
          <button
            onClick={onRemove}
            className="p-1 text-white/30 hover:text-red-400 transition-colors"
            title="Remove song"
          >
            <Trash size={12} />
          </button>
        </div>
      </div>
      <div className="flex items-center gap-3 pl-[4.5rem] text-xs font-rune text-white/40">
        <label className="flex items-center gap-1">
          Gem
          <input
            type="number"
            min={1}
            max={16}
            value={song.gem}
            onChange={(e) =>
              onChange({ ...song, gem: Math.min(16, Math.max(1, Number(e.target.value) || 1)) })
            }
            className="w-12 bg-void border border-white/10 text-white text-xs px-2 py-0.5 text-center focus:outline-none focus:border-magentaglow font-rune"
          />
        </label>
        <label className="flex items-center gap-1">
          Category
          <select
            value={song.category}
            onChange={(e) => onChange({ ...song, category: e.target.value as SongCategory })}
            className="bg-void border border-white/10 text-white/70 text-xs px-2 py-0.5 focus:outline-none focus:border-magentaglow font-rune"
          >
            {categories.map((c) => (
              <option key={c} value={c} className="bg-[#120a1d]">
                {c}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-1">
          Instrument
          <select
            value={song.instrument_type}
            onChange={(e) => onChange({ ...song, instrument_type: e.target.value as InstrumentType })}
            className="bg-void border border-white/10 text-white/70 text-xs px-2 py-0.5 focus:outline-none focus:border-magentaglow font-rune"
          >
            {instrumentTypes.map((t) => (
              <option key={t} value={t} className="bg-[#120a1d]">
                {INSTRUMENT_LABELS[t]}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-1">
          Slot
          <select
            value={song.instrument_slot}
            onChange={(e) => onChange({ ...song, instrument_slot: e.target.value as InstrumentSlot })}
            className="bg-void border border-white/10 text-white/70 text-xs px-2 py-0.5 focus:outline-none focus:border-magentaglow font-rune"
          >
            {instrumentSlots.map((s) => (
              <option key={s} value={s} className="bg-[#120a1d]">
                {SLOT_LABELS[s]}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-1">
          Recast
          <input
            type="number"
            min={1}
            max={300}
            value={song.min_recast_ticks}
            onChange={(e) =>
              onChange({ ...song, min_recast_ticks: Math.max(1, Number(e.target.value) || 66) })
            }
            className="w-14 bg-void border border-white/10 text-white text-xs px-2 py-0.5 text-center focus:outline-none focus:border-magentaglow font-rune"
          />
          ticks
        </label>
        <label className="flex items-center gap-1">
          Buff
          <input
            type="number"
            min={0}
            max={600}
            value={song.buff_duration_ticks ?? ""}
            onChange={(e) =>
              onChange({
                ...song,
                buff_duration_ticks: e.target.value ? Math.max(0, Number(e.target.value)) : null,
              })
            }
            className="w-14 bg-void border border-white/10 text-white text-xs px-2 py-0.5 text-center focus:outline-none focus:border-magentaglow font-rune"
            placeholder="---"
          />
          ticks
        </label>
      </div>
    </div>
  );
}

interface DragDropSongListProps {
  songs: SongSlotConfig[];
  onChange: (songs: SongSlotConfig[]) => void;
}

function DragDropSongList({ songs, onChange }: DragDropSongListProps) {
  const [draggedIndex, setDraggedIndex] = useState<number | null>(null);
  const [overIndex, setOverIndex] = useState<number | null>(null);
  const dragNode = useRef<number | null>(null);

  const handleDragStart = useCallback((e: React.DragEvent, index: number) => {
    dragNode.current = index;
    setDraggedIndex(index);
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", String(index));
  }, []);

  const handleDragEnter = useCallback((e: React.DragEvent, index: number) => {
    e.preventDefault();
    setOverIndex(index);
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent, targetIndex: number) => {
      e.preventDefault();
      const fromIndex = dragNode.current;
      if (fromIndex === null || fromIndex === targetIndex) {
        setDraggedIndex(null);
        setOverIndex(null);
        dragNode.current = null;
        return;
      }
      const reordered = [...songs];
      const [moved] = reordered.splice(fromIndex, 1);
      reordered.splice(targetIndex, 0, moved);
      const reindexed = reordered.map((s, i) => ({ ...s, priority: i + 1 }));
      onChange(reindexed);
      setDraggedIndex(null);
      setOverIndex(null);
      dragNode.current = null;
    },
    [songs, onChange],
  );

  const handleDragEnd = useCallback(() => {
    setDraggedIndex(null);
    setOverIndex(null);
    dragNode.current = null;
  }, []);

  return (
    <div className="flex flex-col gap-1">
      {songs.map((song, i) => (
        <div
          key={song.id}
          draggable
          onDragStart={(e) => handleDragStart(e, i)}
          onDragEnter={(e) => handleDragEnter(e, i)}
          onDragOver={handleDragOver}
          onDrop={(e) => handleDrop(e, i)}
          onDragEnd={handleDragEnd}
          className={
            i === draggedIndex
              ? "opacity-30"
              : i === overIndex && draggedIndex !== null && draggedIndex !== i
              ? "border-l-2 border-l-magentaglow"
              : ""
          }
        >
          <SongRow
            song={song}
            index={i}
            total={songs.length}
            dragHandleProps={{}}
            onMoveUp={() => {
              if (i === 0) return;
              const reordered = [...songs];
              [reordered[i - 1], reordered[i]] = [reordered[i], reordered[i - 1]];
              onChange(reordered.map((s, idx) => ({ ...s, priority: idx + 1 })));
            }}
            onMoveDown={() => {
              if (i === songs.length - 1) return;
              const reordered = [...songs];
              [reordered[i], reordered[i + 1]] = [reordered[i + 1], reordered[i]];
              onChange(reordered.map((s, idx) => ({ ...s, priority: idx + 1 })));
            }}
            onToggle={() => {
              const updated = songs.map((s, idx) => (idx === i ? { ...s, enabled: !s.enabled } : s));
              onChange(updated);
            }}
            onRemove={() => {
              const updated = songs.filter((_, idx) => idx !== i);
              onChange(updated.map((s, idx) => ({ ...s, priority: idx + 1 })));
            }}
            onChange={(s) => {
              const updated = songs.map((existing) => (existing.id === s.id ? s : existing));
              onChange(updated);
            }}
          />
        </div>
      ))}
    </div>
  );
}

interface InstrumentEditorProps {
  instruments: InstrumentSet[];
  onChange: (instruments: InstrumentSet[]) => void;
}

function InstrumentEditor({ instruments, onChange }: InstrumentEditorProps) {
  const instTypes: { key: keyof InstrumentSet; label: string }[] = [
    { key: "string_item_id", label: "String" },
    { key: "brass_item_id", label: "Brass" },
    { key: "wind_item_id", label: "Wind" },
    { key: "percussion_item_id", label: "Percussion" },
  ];

  const addSet = () => {
    onChange([...instruments, { string_item_id: null, brass_item_id: null, wind_item_id: null, percussion_item_id: null }]);
  };

  const removeSet = (index: number) => {
    if (instruments.length <= 1) return;
    onChange(instruments.filter((_, i) => i !== index));
  };

  const updateSet = (index: number, key: keyof InstrumentSet, value: number | null) => {
    const updated = instruments.map((set, i) =>
      i === index ? { ...set, [key]: value } : set,
    );
    onChange(updated);
  };

  return (
    <div className="flex flex-col gap-3">
      {instruments.map((set, i) => (
        <div key={i} className="border border-white/10 bg-void/40 rounded-xl px-4 py-3">
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs font-rune text-white/50 uppercase tracking-widest">
              Instrument Set {i + 1}
            </span>
            {instruments.length > 1 && (
              <button
                onClick={() => removeSet(i)}
                className="text-white/30 hover:text-red-400 transition-colors"
                title="Remove set"
              >
                <Trash size={12} />
              </button>
            )}
          </div>
          <div className="grid grid-cols-4 gap-3">
            {instTypes.map(({ key, label }) => (
              <div key={key} className="flex flex-col gap-1">
                <label className="text-[10px] font-rune text-white/40 uppercase tracking-widest">
                  {label} Item ID
                </label>
                <input
                  type="number"
                  min={0}
                  value={set[key] ?? ""}
                  onChange={(e) => {
                    const val = e.target.value;
                    updateSet(i, key, val ? Math.max(0, Number(val)) : null);
                  }}
                  placeholder="---"
                  className="bg-void border border-white/10 text-white text-xs px-2 py-1.5 text-center focus:outline-none focus:border-magentaglow font-rune placeholder:text-white/20"
                />
              </div>
            ))}
          </div>
        </div>
      ))}
      <button
        onClick={addSet}
        className="flex items-center justify-center gap-2 border border-dashed border-white/20 text-white/50 hover:border-white/40 hover:text-white/70 text-xs py-2 rounded-xl transition-colors"
      >
        <Plus size={12} />
        Add Instrument Set
      </button>
    </div>
  );
}

interface BardSongPanelProps {
  config: BardConfig;
  onSave: (cfg: BardConfig) => Promise<void>;
}

export default function BardSongPanel({ config, onSave }: BardSongPanelProps) {
  const [draft, setDraft] = useState<BardConfig>({ ...config });
  const [saving, setSaving] = useState(false);
  const [saveMsg, setSaveMsg] = useState<string | null>(null);

  const isDirty = JSON.stringify(draft) !== JSON.stringify(config);

  const handleSave = async () => {
    setSaving(true);
    setSaveMsg(null);
    try {
      await onSave(draft);
      setSaveMsg("Saved");
    } catch (e) {
      console.error("Failed to save bard config:", e);
      setSaveMsg(e instanceof Error ? e.message : "Save failed");
    } finally {
      setSaving(false);
      setTimeout(() => setSaveMsg(null), 2500);
    }
  };

  return (
    <div className="flex flex-col gap-6 overflow-y-auto pr-1">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="font-archaic text-xl text-white">
            {draft.character_name} — Bard Configuration
          </h3>
          <p className="text-xs font-rune text-white/50 mt-0.5">
            Song scheduling, twist rotation, and instrument swap
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

      <section>
        <h4 className="font-archaic text-xs uppercase tracking-widest text-white/50 mb-3 flex items-center gap-2">
          <MusicNotes size={12} className="text-magentaglow" />
          Engine Control
        </h4>
        <div className="bg-violet/20 border border-white/5 p-4 flex flex-wrap gap-6">
          <button
            onClick={() => setDraft({ ...draft, twist_enabled: !draft.twist_enabled })}
            className="flex items-center gap-2 text-sm font-tech transition-colors hover:text-magentaglow"
          >
            {draft.twist_enabled ? (
              <CheckSquare weight="fill" size={16} className="text-magentaglow" />
            ) : (
              <Square size={16} className="text-white/40" />
            )}
            <span className="text-white/70">Enable Song Twisting</span>
          </button>
          <button
            onClick={() =>
              setDraft({ ...draft, full_rotation_enabled: !draft.full_rotation_enabled })
            }
            className="flex items-center gap-2 text-sm font-tech transition-colors hover:text-magentaglow"
          >
            {draft.full_rotation_enabled ? (
              <CheckSquare weight="fill" size={16} className="text-magentaglow" />
            ) : (
              <Square size={16} className="text-white/40" />
            )}
            <span className="text-white/70">Full Rotation (song weaving)</span>
          </button>
          <button
            onClick={() =>
              setDraft({ ...draft, instrument_swap_enabled: !draft.instrument_swap_enabled })
            }
            className="flex items-center gap-2 text-sm font-tech transition-colors hover:text-magentaglow"
          >
            {draft.instrument_swap_enabled ? (
              <CheckSquare weight="fill" size={16} className="text-magentaglow" />
            ) : (
              <Square size={16} className="text-white/40" />
            )}
            <span className="text-white/70">Instrument Swap</span>
          </button>
        </div>
      </section>

      <section>
        <h4 className="font-archaic text-xs uppercase tracking-widest text-white/50 mb-2 flex items-center gap-2">
          <MusicNotes size={12} className="text-spectral" />
          Song List — Drag to Reorder
        </h4>
        <p className="text-[10px] text-white/30 font-rune mb-3">
          Higher priority = earlier in twist cycle. Songs maintain overlap when
          recast before expiry. Enable/disable individual songs, drag rows to
          reorder, or use arrow buttons.
        </p>
        <DragDropSongList
          songs={draft.songs}
          onChange={(songs) => setDraft({ ...draft, songs })}
        />
      </section>

      <section>
        <h4 className="font-archaic text-xs uppercase tracking-widest text-white/50 mb-3 flex items-center gap-2">
          <MusicNotes size={12} className="text-amber-300" />
          Instrument Inventory
        </h4>
        <p className="text-[10px] text-white/30 font-rune mb-3">
          Configure instruments by type. The engine selects the first available
          item matching the song&apos;s required type. Supports multiple instrument
          sets for different situations.
        </p>
        <InstrumentEditor
          instruments={draft.instruments}
          onChange={(instruments) => setDraft({ ...draft, instruments })}
        />
      </section>
    </div>
  );
}

export { makeDefaultConfig, DEFAULT_SONGS, DEFAULT_INSTRUMENTS };
