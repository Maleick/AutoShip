/**
 * GroupBuilder — Enhanced group composition editor.
 *
 * Features:
 *  - Drag-and-drop character assignment between group slots
 *  - Role auto-suggestion based on EQ class
 *  - Group template save / load (localStorage)
 *  - Bulk operations: multi-select characters, assign group, set role
 *  - Visual composition summary (class / role distribution)
 */

import {
  useState,
  useCallback,
  useRef,
  type DragEvent,
} from "react";
import {
  UsersThree,
  User,
  Plus,
  Trash,
  FloppyDisk,
  FolderOpen,
  ArrowsDownUp,
  CheckSquare,
  Square,
  MagicWand,
  ChartBar,
  X,
  LockSimple,
  LockSimpleOpen,
} from "@phosphor-icons/react";
import type { Character, GroupTemplate, GroupSlot, Role, EQClass } from "../types";
import { demoCharacters, demoGroupTemplates } from "../data/demo";

// ── Constants ────────────────────────────────────────────────────────────────

const ROLE_COLORS: Record<Role, string> = {
  Tank:    "text-blue-400 border-blue-500/40 bg-blue-500/10",
  Healer:  "text-green-400 border-green-500/40 bg-green-500/10",
  DPS:     "text-red-400 border-red-500/40 bg-red-500/10",
  Support: "text-yellow-400 border-yellow-500/40 bg-yellow-500/10",
  Puller:  "text-orange-400 border-orange-500/40 bg-orange-500/10",
  CC:      "text-purple-400 border-purple-500/40 bg-purple-500/10",
};

const ROLE_BADGE_COLORS: Record<Role, string> = {
  Tank:    "bg-blue-500/20 text-blue-300",
  Healer:  "bg-green-500/20 text-green-300",
  DPS:     "bg-red-500/20 text-red-300",
  Support: "bg-yellow-500/20 text-yellow-300",
  Puller:  "bg-orange-500/20 text-orange-300",
  CC:      "bg-purple-500/20 text-purple-300",
};

const CLASS_ROLE_MAP: Record<EQClass, Role> = {
  Warrior:       "Tank",
  Paladin:       "Tank",
  "Shadow Knight": "Tank",
  Ranger:        "Puller",
  Monk:          "Puller",
  Bard:          "Support",
  Rogue:         "DPS",
  Berserker:     "DPS",
  Cleric:        "Healer",
  Druid:         "Healer",
  Shaman:        "Support",
  Necromancer:   "DPS",
  Wizard:        "DPS",
  Magician:      "DPS",
  Enchanter:     "CC",
  Beastlord:     "DPS",
};

const CLASS_COLORS: Record<EQClass, string> = {
  Warrior:         "text-orange-300",
  Paladin:         "text-yellow-300",
  "Shadow Knight": "text-purple-400",
  Ranger:          "text-green-400",
  Monk:            "text-amber-400",
  Bard:            "text-pink-400",
  Rogue:           "text-gray-400",
  Berserker:       "text-red-400",
  Cleric:          "text-white",
  Druid:           "text-emerald-400",
  Shaman:          "text-teal-400",
  Necromancer:     "text-violet-400",
  Wizard:          "text-blue-400",
  Magician:        "text-cyan-400",
  Enchanter:       "text-fuchsia-400",
  Beastlord:       "text-lime-400",
};

const ALL_ROLES: Role[] = ["Tank", "Healer", "DPS", "Support", "Puller", "CC"];
const STORAGE_KEY = "textquest_group_templates";

// ── Helpers ──────────────────────────────────────────────────────────────────

function suggestRole(eqClass: EQClass): Role {
  return CLASS_ROLE_MAP[eqClass];
}

function makeDefaultTemplate(): GroupTemplate {
  return {
    id: `tpl-${Date.now()}`,
    name: "New Group",
    description: "Custom group composition",
    slots: [
      { role: "Tank",   characterId: null, locked: false },
      { role: "Healer", characterId: null, locked: false },
      { role: "CC",     characterId: null, locked: false },
      { role: "DPS",    characterId: null, locked: false },
      { role: "DPS",    characterId: null, locked: false },
      { role: "DPS",    characterId: null, locked: false },
    ],
  };
}

function loadSavedTemplates(): GroupTemplate[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return JSON.parse(raw) as GroupTemplate[];
  } catch {
    // ignore corrupt data
  }
  return [];
}

function saveTemplates(templates: GroupTemplate[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(templates));
  } catch {
    // ignore storage errors
  }
}

// ── Sub-components ────────────────────────────────────────────────────────────

function RoleBadge({ role }: { role: Role }) {
  return (
    <span
      className={`text-[9px] uppercase tracking-widest px-1.5 py-0.5 rounded-sm font-tech font-bold ${ROLE_BADGE_COLORS[role]}`}
    >
      {role}
    </span>
  );
}

function StatusDot({ status }: { status: Character["status"] }) {
  const color =
    status === "online"  ? "bg-green-500" :
    status === "idle"    ? "bg-yellow-500" :
                           "bg-gray-600";
  return <span className={`inline-block w-1.5 h-1.5 rounded-full ${color}`} />;
}

// ── Character card (draggable source) ─────────────────────────────────────────

interface CharCardProps {
  char: Character;
  selected: boolean;
  assigned: boolean;
  onToggleSelect: (id: string) => void;
  onDragStart: (e: DragEvent, charId: string) => void;
}

function CharCard({ char, selected, assigned, onToggleSelect, onDragStart }: CharCardProps) {
  const suggested = suggestRole(char.eqClass);
  return (
    <div
      draggable
      onDragStart={(e) => onDragStart(e, char.id)}
      onClick={() => onToggleSelect(char.id)}
      className={`
        flex items-center gap-2 px-3 py-2 cursor-grab active:cursor-grabbing border transition-all select-none
        ${selected
          ? "border-magentaglow/60 bg-magentaglow/10"
          : assigned
          ? "border-white/5 bg-void/60 opacity-50"
          : "border-white/10 bg-void/40 hover:border-spectral/40 hover:bg-violet/20"
        }
      `}
    >
      <div className="flex-shrink-0">
        {selected
          ? <CheckSquare className="text-magentaglow" size={14} />
          : <Square className="text-white/30" size={14} />
        }
      </div>
      <div className="flex-shrink-0 w-6 h-6 border border-white/20 bg-void flex items-center justify-center">
        <User size={12} className="text-white/40" />
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5">
          <StatusDot status={char.status} />
          <span className="text-xs font-medium text-white truncate">{char.name}</span>
        </div>
        <div className="flex items-center gap-1 mt-0.5">
          <span className={`text-[10px] font-rune ${CLASS_COLORS[char.eqClass]}`}>
            {char.eqClass}
          </span>
          <span className="text-white/20">·</span>
          <span className="text-[10px] text-white/40 font-rune">Lv{char.level}</span>
        </div>
      </div>
      <RoleBadge role={suggested} />
    </div>
  );
}

// ── Group slot (drop target) ──────────────────────────────────────────────────

interface SlotProps {
  slot: GroupSlot;
  slotIndex: number;
  char: Character | undefined;
  isOver: boolean;
  onDrop: (e: DragEvent, slotIndex: number) => void;
  onDragOver: (e: DragEvent, slotIndex: number) => void;
  onDragLeave: () => void;
  onRemove: (slotIndex: number) => void;
  onToggleLock: (slotIndex: number) => void;
  onRoleChange: (slotIndex: number, role: Role) => void;
}

function GroupSlotCard({
  slot, slotIndex, char, isOver,
  onDrop, onDragOver, onDragLeave,
  onRemove, onToggleLock, onRoleChange,
}: SlotProps) {
  const [editingRole, setEditingRole] = useState(false);
  const roleStyle = ROLE_COLORS[slot.role];

  return (
    <div
      onDrop={(e) => onDrop(e, slotIndex)}
      onDragOver={(e) => onDragOver(e, slotIndex)}
      onDragLeave={onDragLeave}
      className={`
        relative border p-2.5 transition-all min-h-[64px] flex items-center gap-2
        ${isOver
          ? "border-spectral/80 bg-spectral/10 shadow-[0_0_12px_rgba(0,229,255,0.3)]"
          : slot.locked
          ? "border-white/5 bg-void/30 opacity-70"
          : char
          ? `${roleStyle} border`
          : "border-dashed border-white/15 bg-void/20 hover:border-white/30"
        }
      `}
    >
      {/* Slot number */}
      <span className="text-[9px] text-white/20 font-rune absolute top-1 left-1">
        {slotIndex + 1}
      </span>

      {/* Role badge / selector */}
      <div className="flex-shrink-0">
        {editingRole ? (
          <select
            autoFocus
            className="bg-void border border-white/20 text-white text-[10px] font-tech px-1 py-0.5 focus:outline-none focus:border-magentaglow"
            value={slot.role}
            onChange={(e) => {
              onRoleChange(slotIndex, e.target.value as Role);
              setEditingRole(false);
            }}
            onBlur={() => setEditingRole(false)}
          >
            {ALL_ROLES.map((r) => (
              <option key={r} value={r}>{r}</option>
            ))}
          </select>
        ) : (
          <button
            className={`text-[9px] uppercase tracking-widest px-1.5 py-0.5 rounded-sm font-tech font-bold border cursor-pointer ${ROLE_BADGE_COLORS[slot.role]} border-current/20 hover:opacity-80`}
            onClick={() => !slot.locked && setEditingRole(true)}
            title="Click to change role"
          >
            {slot.role}
          </button>
        )}
      </div>

      {/* Character info or empty prompt */}
      <div className="flex-1 min-w-0">
        {char ? (
          <div>
            <div className="flex items-center gap-1.5">
              <StatusDot status={char.status} />
              <span className="text-sm font-medium text-white truncate">{char.name}</span>
            </div>
            <span className={`text-[10px] font-rune ${CLASS_COLORS[char.eqClass]}`}>
              {char.eqClass}
            </span>
          </div>
        ) : (
          <span className="text-[10px] text-white/25 font-rune italic">
            {isOver ? "Drop here" : "— empty —"}
          </span>
        )}
      </div>

      {/* Actions */}
      <div className="flex-shrink-0 flex gap-1">
        <button
          onClick={() => onToggleLock(slotIndex)}
          className="text-white/30 hover:text-white/70 transition-colors"
          title={slot.locked ? "Unlock slot" : "Lock slot"}
        >
          {slot.locked
            ? <LockSimple size={13} />
            : <LockSimpleOpen size={13} />
          }
        </button>
        {char && !slot.locked && (
          <button
            onClick={() => onRemove(slotIndex)}
            className="text-white/30 hover:text-magentaglow transition-colors"
            title="Remove character"
          >
            <X size={13} />
          </button>
        )}
      </div>
    </div>
  );
}

// ── Composition summary bar ───────────────────────────────────────────────────

function CompositionSummary({ slots, characters }: { slots: GroupSlot[]; characters: Character[] }) {
  const roleCounts: Partial<Record<Role, number>> = {};
  const classCounts: Partial<Record<EQClass, number>> = {};

  for (const slot of slots) {
    roleCounts[slot.role] = (roleCounts[slot.role] ?? 0) + 1;
    if (slot.characterId) {
      const char = characters.find((c) => c.id === slot.characterId);
      if (char) {
        classCounts[char.eqClass] = (classCounts[char.eqClass] ?? 0) + 1;
      }
    }
  }

  const filled = slots.filter((s) => s.characterId !== null).length;
  const total = slots.length;

  return (
    <div className="bg-violet/20 border border-white/5 p-3 space-y-3">
      {/* Fill rate */}
      <div>
        <div className="flex justify-between text-[10px] font-tech mb-1">
          <span className="text-white/50 uppercase tracking-wider">Fill Rate</span>
          <span className="text-spectral">{filled}/{total}</span>
        </div>
        <div className="h-1.5 bg-void border border-white/10 w-full overflow-hidden">
          <div
            className="h-full bg-gradient-to-r from-spectral/60 to-spectral transition-all"
            style={{ width: `${(filled / Math.max(total, 1)) * 100}%` }}
          />
        </div>
      </div>

      {/* Role distribution */}
      <div>
        <div className="text-[10px] font-tech text-white/50 uppercase tracking-wider mb-1.5">
          Role Breakdown
        </div>
        <div className="flex flex-wrap gap-1.5">
          {(Object.entries(roleCounts) as [Role, number][]).map(([role, count]) => (
            <span
              key={role}
              className={`text-[10px] px-2 py-0.5 border rounded-sm font-tech ${ROLE_BADGE_COLORS[role]}`}
            >
              {count}× {role}
            </span>
          ))}
        </div>
      </div>

      {/* Class breakdown */}
      {Object.keys(classCounts).length > 0 && (
        <div>
          <div className="text-[10px] font-tech text-white/50 uppercase tracking-wider mb-1.5">
            Classes Assigned
          </div>
          <div className="flex flex-wrap gap-1.5">
            {(Object.entries(classCounts) as [EQClass, number][]).map(([cls, count]) => (
              <span
                key={cls}
                className={`text-[10px] font-rune ${CLASS_COLORS[cls]}`}
              >
                {count > 1 ? `${count}× ` : ""}{cls}
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// ── Save/Load modal ───────────────────────────────────────────────────────────

interface SaveLoadModalProps {
  mode: "save" | "load";
  currentName: string;
  saved: GroupTemplate[];
  onSave: (name: string) => void;
  onLoad: (tpl: GroupTemplate) => void;
  onDelete: (id: string) => void;
  onClose: () => void;
}

function SaveLoadModal({ mode, currentName, saved, onSave, onLoad, onDelete, onClose }: SaveLoadModalProps) {
  const [name, setName] = useState(currentName);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-void/80 backdrop-blur-sm">
      <div className="bg-violet/90 border border-magentaglow/30 p-6 w-[420px] shadow-[0_0_30px_rgba(204,68,255,0.2)]">
        <div className="flex justify-between items-center mb-5">
          <h3 className="font-archaic text-lg text-white text-glow-magenta">
            {mode === "save" ? "Save Template" : "Load Template"}
          </h3>
          <button onClick={onClose} className="text-white/40 hover:text-white transition-colors">
            <X size={18} />
          </button>
        </div>

        {mode === "save" ? (
          <div className="space-y-4">
            <div>
              <label className="text-[10px] text-white/50 uppercase tracking-wider font-tech block mb-1">
                Template Name
              </label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
                placeholder="Enter template name..."
                autoFocus
              />
            </div>
            <div className="flex gap-2 justify-end">
              <button
                onClick={onClose}
                className="px-4 py-1.5 border border-white/20 text-white/60 text-sm font-tech hover:bg-white/5 transition-colors uppercase tracking-wider"
              >
                Cancel
              </button>
              <button
                onClick={() => { onSave(name.trim() || currentName); onClose(); }}
                className="px-4 py-1.5 bg-magentadark/20 border border-magentaglow text-white text-sm font-tech hover:bg-magentadark/40 transition-colors uppercase tracking-wider shadow-[0_0_10px_rgba(204,68,255,0.3)]"
              >
                Save
              </button>
            </div>
          </div>
        ) : (
          <div className="space-y-2 max-h-[300px] overflow-y-auto">
            {saved.length === 0 ? (
              <p className="text-white/40 text-sm font-rune text-center py-6">
                No saved templates found
              </p>
            ) : (
              saved.map((tpl) => (
                <div
                  key={tpl.id}
                  className="flex items-center gap-3 p-3 border border-white/10 hover:border-spectral/40 hover:bg-violet/30 transition-colors group"
                >
                  <div className="flex-1 min-w-0">
                    <div className="text-sm font-medium text-white truncate">{tpl.name}</div>
                    <div className="text-[10px] text-white/40 font-rune mt-0.5">
                      {tpl.slots.length} slots · {tpl.slots.filter((s) => s.characterId).length} filled
                    </div>
                  </div>
                  <button
                    onClick={() => { onLoad(tpl); onClose(); }}
                    className="text-spectral text-xs font-tech uppercase tracking-wider hover:text-white transition-colors"
                  >
                    Load
                  </button>
                  <button
                    onClick={() => onDelete(tpl.id)}
                    className="text-white/20 hover:text-magentaglow transition-colors opacity-0 group-hover:opacity-100"
                  >
                    <Trash size={13} />
                  </button>
                </div>
              ))
            )}
          </div>
        )}
      </div>
    </div>
  );
}

// ── Main GroupBuilder component ───────────────────────────────────────────────

export default function GroupBuilder() {
  const [templates, setTemplates] = useState<GroupTemplate[]>(() => {
    // Use saved templates when available; fall back to demo templates otherwise.
    const saved = loadSavedTemplates();
    return saved.length > 0 ? saved : demoGroupTemplates;
  });
  const [activeId, setActiveId] = useState<string>(templates[0]?.id ?? "");
  const [characters] = useState<Character[]>(demoCharacters);
  const [selectedCharIds, setSelectedCharIds] = useState<Set<string>>(new Set());
  const [overSlotIndex, setOverSlotIndex] = useState<number | null>(null);
  const [savedTemplates, setSavedTemplates] = useState<GroupTemplate[]>(loadSavedTemplates);
  const [modal, setModal] = useState<"save" | "load" | null>(null);
  const [bulkRole, setBulkRole] = useState<Role>("DPS");
  const dragCharId = useRef<string | null>(null);

  const activeTemplate = templates.find((t) => t.id === activeId);

  // Characters already assigned in current template
  const assignedIds = new Set(
    (activeTemplate?.slots ?? [])
      .map((s) => s.characterId)
      .filter(Boolean) as string[]
  );

  const updateTemplate = useCallback(
    (updater: (tpl: GroupTemplate) => GroupTemplate) => {
      setTemplates((prev) =>
        prev.map((t) => (t.id === activeId ? updater(t) : t))
      );
    },
    [activeId]
  );

  // ── Drag-drop handlers ───────────────────────────────────────────────────

  const handleDragStart = useCallback((e: DragEvent, charId: string) => {
    dragCharId.current = charId;
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", charId);
  }, []);

  const handleDragOver = useCallback((e: DragEvent, slotIndex: number) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    setOverSlotIndex(slotIndex);
  }, []);

  const handleDragLeave = useCallback(() => {
    setOverSlotIndex(null);
  }, []);

  const handleDrop = useCallback(
    (e: DragEvent, slotIndex: number) => {
      e.preventDefault();
      setOverSlotIndex(null);
      const charId = e.dataTransfer.getData("text/plain") || dragCharId.current;
      if (!charId) return;
      updateTemplate((tpl) => {
        if (tpl.slots[slotIndex]?.locked) return tpl;
        const newSlots = tpl.slots.map((s, i) => {
          if (s.characterId === charId) return { ...s, characterId: null };
          if (i === slotIndex) return { ...s, characterId: charId };
          return s;
        });
        // Suggest role when the slot was previously empty
        const wasEmpty = !tpl.slots[slotIndex]?.characterId;
        const char = characters.find((c) => c.id === charId);
        if (char && wasEmpty) {
          const suggested = suggestRole(char.eqClass);
          if (newSlots[slotIndex].role !== suggested) {
            newSlots[slotIndex] = { ...newSlots[slotIndex], role: suggested };
          }
        }
        return { ...tpl, slots: newSlots };
      });
      dragCharId.current = null;
    },
    [updateTemplate, characters]
  );

  // ── Slot manipulation ────────────────────────────────────────────────────

  const handleRemove = useCallback(
    (slotIndex: number) => {
      updateTemplate((tpl) => ({
        ...tpl,
        slots: tpl.slots.map((s, i) => (i === slotIndex ? { ...s, characterId: null } : s)),
      }));
    },
    [updateTemplate]
  );

  const handleToggleLock = useCallback(
    (slotIndex: number) => {
      updateTemplate((tpl) => ({
        ...tpl,
        slots: tpl.slots.map((s, i) => (i === slotIndex ? { ...s, locked: !s.locked } : s)),
      }));
    },
    [updateTemplate]
  );

  const handleRoleChange = useCallback(
    (slotIndex: number, role: Role) => {
      updateTemplate((tpl) => ({
        ...tpl,
        slots: tpl.slots.map((s, i) => (i === slotIndex ? { ...s, role } : s)),
      }));
    },
    [updateTemplate]
  );

  const handleAddSlot = useCallback(() => {
    updateTemplate((tpl) => ({
      ...tpl,
      slots: [...tpl.slots, { role: "DPS", characterId: null, locked: false }],
    }));
  }, [updateTemplate]);

  // ── Auto-fill ────────────────────────────────────────────────────────────

  const handleAutoFill = useCallback(() => {
    updateTemplate((tpl) => {
      const assigned = new Set(
        tpl.slots.map((s) => s.characterId).filter(Boolean) as string[]
      );
      const available = characters.filter(
        (c) => !assigned.has(c.id) && c.status !== "offline"
      );
      const newSlots = [...tpl.slots];
      for (let i = 0; i < newSlots.length; i++) {
        if (newSlots[i].characterId || newSlots[i].locked) continue;
        const match = available.find(
          (c) => suggestRole(c.eqClass) === newSlots[i].role && !assigned.has(c.id)
        );
        if (match) {
          newSlots[i] = { ...newSlots[i], characterId: match.id };
          assigned.add(match.id);
        }
      }
      return { ...tpl, slots: newSlots };
    });
  }, [updateTemplate, characters]);

  const handleClearAll = useCallback(() => {
    updateTemplate((tpl) => ({
      ...tpl,
      slots: tpl.slots.map((s) => (s.locked ? s : { ...s, characterId: null })),
    }));
  }, [updateTemplate]);

  // ── Selection & bulk ops ─────────────────────────────────────────────────

  const toggleSelect = useCallback((id: string) => {
    setSelectedCharIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const handleBulkAssign = useCallback(() => {
    if (selectedCharIds.size === 0 || !activeTemplate) return;
    const ids = [...selectedCharIds];
    updateTemplate((tpl) => {
      const newSlots = [...tpl.slots];
      let idx = 0;
      for (const charId of ids) {
        while (idx < newSlots.length && (newSlots[idx].characterId || newSlots[idx].locked)) idx++;
        if (idx >= newSlots.length) break;
        newSlots[idx] = { ...newSlots[idx], characterId: charId };
        idx++;
      }
      return { ...tpl, slots: newSlots };
    });
    setSelectedCharIds(new Set());
  }, [selectedCharIds, activeTemplate, updateTemplate]);

  const handleBulkSetRole = useCallback(() => {
    if (selectedCharIds.size === 0 || !activeTemplate) return;
    updateTemplate((tpl) => ({
      ...tpl,
      slots: tpl.slots.map((s) =>
        s.characterId && selectedCharIds.has(s.characterId)
          ? { ...s, role: bulkRole }
          : s
      ),
    }));
    setSelectedCharIds(new Set());
  }, [selectedCharIds, activeTemplate, bulkRole, updateTemplate]);

  // ── Template save/load ───────────────────────────────────────────────────

  const handleSave = useCallback(
    (name: string) => {
      if (!activeTemplate) return;
      const updated = { ...activeTemplate, name };
      setSavedTemplates((prev) => {
        const next = prev.filter((t) => t.id !== updated.id);
        next.push(updated);
        saveTemplates(next);
        return next;
      });
    },
    [activeTemplate]
  );

  const handleLoad = useCallback(
    (tpl: GroupTemplate) => {
      const exists = templates.find((t) => t.id === tpl.id);
      if (exists) {
        setActiveId(tpl.id);
      } else {
        setTemplates((prev) => [...prev, tpl]);
        setActiveId(tpl.id);
      }
    },
    [templates]
  );

  const handleDeleteSaved = useCallback((id: string) => {
    setSavedTemplates((prev) => {
      const next = prev.filter((t) => t.id !== id);
      saveTemplates(next);
      return next;
    });
  }, []);

  const handleNewTemplate = useCallback(() => {
    const tpl = makeDefaultTemplate();
    setTemplates((prev) => [...prev, tpl]);
    setActiveId(tpl.id);
  }, []);

  if (!activeTemplate) return null;

  return (
    <div className="flex-1 h-full flex flex-col overflow-hidden">
      {/* Header */}
      <header className="h-16 border-b border-white/10 flex items-center justify-between px-6 bg-violet/30 backdrop-blur-md flex-shrink-0">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 border border-spectral/40 flex items-center justify-center bg-void">
            <UsersThree weight="fill" className="text-spectral" />
          </div>
          <div>
            <h2 className="font-archaic text-lg text-white leading-tight">Fleet Formations</h2>
            <p className="text-[10px] uppercase tracking-widest text-white/50 font-rune">
              Group Composition Editor
            </p>
          </div>
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => setModal("load")}
            className="px-3 py-1.5 border border-spectral/30 text-spectral text-xs font-tech hover:bg-spectral/10 transition-colors uppercase tracking-wider flex items-center gap-1.5"
          >
            <FolderOpen size={13} /> Load
          </button>
          <button
            onClick={() => setModal("save")}
            className="px-3 py-1.5 bg-magentadark/20 border border-magentaglow text-white text-xs font-tech hover:bg-magentadark/40 transition-colors uppercase tracking-wider flex items-center gap-1.5 shadow-[0_0_10px_rgba(204,68,255,0.2)]"
          >
            <FloppyDisk size={13} /> Save
          </button>
        </div>
      </header>

      {/* Body */}
      <div className="flex-1 flex overflow-hidden">
        {/* Left: Character roster */}
        <div className="w-[260px] flex-shrink-0 border-r border-white/10 flex flex-col bg-void/30">
          <div className="px-4 py-3 border-b border-white/5">
            <div className="flex items-center justify-between mb-2">
              <span className="text-[10px] font-tech text-white/50 uppercase tracking-wider">
                Available Characters
              </span>
              <span className="text-[10px] font-rune text-white/40">
                {characters.filter((c) => !assignedIds.has(c.id)).length} free
              </span>
            </div>
          </div>
          <div className="flex-1 overflow-y-auto px-3 py-2 space-y-1.5">
            {characters.map((char) => (
              <CharCard
                key={char.id}
                char={char}
                selected={selectedCharIds.has(char.id)}
                assigned={assignedIds.has(char.id)}
                onToggleSelect={toggleSelect}
                onDragStart={handleDragStart}
              />
            ))}
          </div>

          {/* Bulk ops */}
          {selectedCharIds.size > 0 && (
            <div className="border-t border-white/10 p-3 bg-magentaglow/5 space-y-2">
              <div className="text-[10px] font-tech text-magentaglow uppercase tracking-wider">
                {selectedCharIds.size} selected
              </div>
              <div className="flex gap-2">
                <button
                  onClick={handleBulkAssign}
                  className="flex-1 py-1 text-[10px] font-tech uppercase tracking-wider border border-spectral/40 text-spectral hover:bg-spectral/10 transition-colors"
                >
                  Assign to Group
                </button>
              </div>
              <div className="flex gap-2 items-center">
                <select
                  value={bulkRole}
                  onChange={(e) => setBulkRole(e.target.value as Role)}
                  className="flex-1 bg-void border border-white/20 text-white text-[10px] font-tech px-2 py-1 focus:outline-none focus:border-magentaglow"
                >
                  {ALL_ROLES.map((r) => (
                    <option key={r} value={r}>{r}</option>
                  ))}
                </select>
                <button
                  onClick={handleBulkSetRole}
                  className="px-2 py-1 text-[10px] font-tech uppercase tracking-wider border border-magentaglow/40 text-magentaglow hover:bg-magentaglow/10 transition-colors"
                >
                  Set Role
                </button>
              </div>
            </div>
          )}
        </div>

        {/* Center: Group editor */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Template tabs */}
          <div className="flex items-center gap-0 border-b border-white/10 px-4 pt-3 flex-shrink-0 overflow-x-auto">
            {templates.map((tpl) => (
              <button
                key={tpl.id}
                onClick={() => setActiveId(tpl.id)}
                className={`
                  px-4 py-2 text-xs font-tech uppercase tracking-wider border-b-2 transition-all whitespace-nowrap
                  ${activeId === tpl.id
                    ? "border-b-magentaglow text-white bg-violet/20"
                    : "border-b-transparent text-white/40 hover:text-white/70 hover:bg-violet/10"
                  }
                `}
              >
                {tpl.name}
              </button>
            ))}
            <button
              onClick={handleNewTemplate}
              className="ml-2 px-2 py-2 text-white/30 hover:text-spectral transition-colors"
              title="New template"
            >
              <Plus size={14} />
            </button>
          </div>

          {/* Toolbar */}
          <div className="flex items-center gap-2 px-4 py-2 border-b border-white/5 bg-void/20 flex-shrink-0">
            <button
              onClick={handleAutoFill}
              className="flex items-center gap-1.5 px-3 py-1 text-[10px] font-tech uppercase tracking-wider border border-spectral/30 text-spectral hover:bg-spectral/10 transition-colors"
            >
              <MagicWand size={12} /> Auto-Fill
            </button>
            <button
              onClick={handleAddSlot}
              className="flex items-center gap-1.5 px-3 py-1 text-[10px] font-tech uppercase tracking-wider border border-white/20 text-white/50 hover:text-white hover:border-white/40 transition-colors"
            >
              <Plus size={12} /> Add Slot
            </button>
            <button
              onClick={handleClearAll}
              className="flex items-center gap-1.5 px-3 py-1 text-[10px] font-tech uppercase tracking-wider border border-red-500/20 text-red-400/60 hover:text-red-400 hover:border-red-500/40 transition-colors"
            >
              <ArrowsDownUp size={12} /> Clear
            </button>
            <div className="ml-auto flex items-center gap-1 text-[10px] font-rune text-white/30">
              <span className="text-spectral">{activeTemplate.slots.filter((s) => s.characterId).length}</span>
              <span>/</span>
              <span>{activeTemplate.slots.length}</span>
              <span className="ml-1">slots filled</span>
            </div>
          </div>

          {/* Slots grid */}
          <div className="flex-1 overflow-y-auto p-4">
            <div className="grid grid-cols-2 gap-2">
              {activeTemplate.slots.map((slot, i) => {
                const char = characters.find((c) => c.id === slot.characterId);
                return (
                  <GroupSlotCard
                    key={i}
                    slot={slot}
                    slotIndex={i}
                    char={char}
                    isOver={overSlotIndex === i}
                    onDrop={handleDrop}
                    onDragOver={handleDragOver}
                    onDragLeave={handleDragLeave}
                    onRemove={handleRemove}
                    onToggleLock={handleToggleLock}
                    onRoleChange={handleRoleChange}
                  />
                );
              })}
              {/* Empty drop zone hint when slots are few */}
              {activeTemplate.slots.length < 6 && (
                <div className="col-span-2 flex items-center justify-center h-12 border border-dashed border-white/10 text-[10px] text-white/20 font-rune">
                  click "+ Add Slot" to add more
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Right: Summary panel */}
        <div className="w-[220px] flex-shrink-0 border-l border-white/10 flex flex-col bg-void/20">
          <div className="px-4 py-3 border-b border-white/5">
            <span className="flex items-center gap-1.5 text-[10px] font-tech text-white/50 uppercase tracking-wider">
              <ChartBar size={12} /> Composition
            </span>
          </div>
          <div className="flex-1 overflow-y-auto p-3">
            <CompositionSummary
              slots={activeTemplate.slots}
              characters={characters}
            />
          </div>
          <div className="p-3 border-t border-white/5">
            <div className="text-[10px] font-tech text-white/30 uppercase tracking-wider mb-2">
              Tip
            </div>
            <p className="text-[10px] font-rune text-white/40 leading-relaxed">
              Drag characters from the roster onto slots. Role is auto-suggested from class on drop.
            </p>
          </div>
        </div>
      </div>

      {/* Save/Load modal */}
      {modal && (
        <SaveLoadModal
          mode={modal}
          currentName={activeTemplate.name}
          saved={savedTemplates}
          onSave={handleSave}
          onLoad={handleLoad}
          onDelete={handleDeleteSaved}
          onClose={() => setModal(null)}
        />
      )}
    </div>
  );
}
