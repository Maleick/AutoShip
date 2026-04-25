/**
 * GroupsPage — Fleet Formations & Camp Configuration
 *
 * Features:
 *  - Groups List: cards showing each group (name, member count, zone), edit/delete, add new group button
 *  - Group Detail/Edit Modal: group name, member list with drag-and-drop, role assignments
 *  - Camp Configuration Section: camp center coordinates, pull radius, pull targets list, safe zone markers
 *  - Integration with API client for CRUD operations
 */

import { useEffect, useState, type DragEvent } from "react";
import {
  Plus,
  PencilSimple,
  Trash,
  ArrowsDownUp,
  MapPin,
  Target,
  Shield,
  X,
  Warning,
  UsersThree,
} from "@phosphor-icons/react";
import { useGroups, useCampConfiguration } from "../hooks/useGroups";
import type {
  Group,
  ClassSpecificSettings,
  GroupMember,
  GroupMemberRole,
  CampConfiguration,
  CampCoordinate,
  CombatSettings,
  CreateCampConfigPayload,
  PullTarget,
  PullPoint,
  PullStrategy,
  SafeZoneMarker,
  UpdateCampConfigPayload,
} from "../types";

// ── Role colors ──────────────────────────────────────────────────────────────

const ROLE_COLORS: Record<GroupMemberRole, string> = {
  main_tank: "text-blue-400 border-blue-500/40 bg-blue-500/10",
  main_assist: "text-cyan-400 border-cyan-500/40 bg-cyan-500/10",
  puller: "text-orange-400 border-orange-500/40 bg-orange-500/10",
  healer: "text-green-400 border-green-500/40 bg-green-500/10",
  dps: "text-red-400 border-red-500/40 bg-red-500/10",
  support: "text-yellow-400 border-yellow-500/40 bg-yellow-500/10",
  cc: "text-purple-400 border-purple-500/40 bg-purple-500/10",
};

const ROLE_LABELS: Record<GroupMemberRole, string> = {
  main_tank: "Tank",
  main_assist: "Main Assist",
  puller: "Puller",
  healer: "Healer",
  dps: "DPS",
  support: "Utility",
  cc: "CC",
};

const ROLE_OPTIONS: GroupMemberRole[] = [
  "main_tank",
  "main_assist",
  "puller",
  "healer",
  "dps",
  "support",
  "cc",
];

const PULL_STRATEGIES: PullStrategy[] = ["balanced", "melee", "caster"];

const DEFAULT_COMBAT_SETTINGS: CombatSettings = {
  hp_buff_threshold_pct: 60,
  mana_buff_threshold_pct: 40,
  pull_strategy: "balanced",
};

const DEFAULT_CLASS_SETTINGS = {
  pet_management_enabled: false,
  spell_priority: "",
  cc_assignment: "",
};

function finiteNumber(value: string, fallback = 0) {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function hasFiniteCoordinates(point: CampCoordinate) {
  return [point.x, point.y, point.z].every(Number.isFinite);
}

// ── Group Card ───────────────────────────────────────────────────────────────

interface GroupCardProps {
  group: Group;
  onEdit: (group: Group) => void;
  onDelete: (id: string) => void;
}

function GroupCard({ group, onEdit, onDelete }: GroupCardProps) {
  const handleDelete = () => {
    if (
      confirm(
        `Permanently remove group "${group.name}" from the registry? This cannot be undone.`,
      )
    ) {
      onDelete(group.id);
    }
  };

  return (
    <div className="bg-void/50 border border-white/10 p-6 hover:border-white/30 transition-colors">
      <div className="flex items-start justify-between gap-4 mb-4">
        <div className="flex-1">
          <h3 className="font-archaic text-lg text-glow-magenta font-bold uppercase tracking-widest">
            {group.name}
          </h3>
          {group.zone && (
            <p className="text-white/60 text-sm font-tech mt-1">
              Zone: {group.zone}
            </p>
          )}
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => onEdit(group)}
            className="p-2 border border-white/20 hover:border-white/40 text-white/60 hover:text-white transition-colors"
            title="Edit group"
          >
            <PencilSimple size={16} />
          </button>
          <button
            onClick={handleDelete}
            className="p-2 border border-red-900/50 hover:border-red-500 text-red-400/60 hover:text-red-400 transition-colors"
            title="Delete group"
          >
            <Trash size={16} />
          </button>
        </div>
      </div>

      <div className="flex gap-4 text-sm text-white/60">
        <span className="flex items-center gap-1 font-rune">
          <span className="font-archaic text-white/80">
            {group.members.length}
          </span>
          Members
        </span>
      </div>

      {group.members.length > 0 && (
        <div className="mt-4 flex flex-wrap gap-2">
          {group.members.slice(0, 3).map((member) => (
            <span
              key={member.character_name}
              className={`text-xs px-2 py-1 border font-rune ${ROLE_COLORS[member.role]}`}
            >
              {member.character_name} · {ROLE_LABELS[member.role]}
            </span>
          ))}
          {group.members.length > 3 && (
            <span className="text-xs px-2 py-1 text-white/50 font-rune">
              +{group.members.length - 3} more
            </span>
          )}
        </div>
      )}
    </div>
  );
}

// ── Group Member List ────────────────────────────────────────────────────────

interface GroupMemberListProps {
  members: GroupMember[];
  onMembersChange: (members: GroupMember[]) => void;
}

function GroupMemberList({ members, onMembersChange }: GroupMemberListProps) {
  const [draggedIndex, setDraggedIndex] = useState<number | null>(null);

  const handleDragStart = (index: number) => {
    setDraggedIndex(index);
  };

  const handleDragOver = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
  };

  const handleDrop = (targetIndex: number) => {
    if (draggedIndex === null || draggedIndex === targetIndex) return;

    const newMembers = [...members];
    const [draggedMember] = newMembers.splice(draggedIndex, 1);
    newMembers.splice(targetIndex, 0, draggedMember);

    // Update order based on new positions
    const updated = newMembers.map((m, idx) => ({ ...m, order: idx }));
    onMembersChange(updated);
    setDraggedIndex(null);
  };

  const moveMember = (index: number, direction: -1 | 1) => {
    const targetIndex = index + direction;
    if (targetIndex < 0 || targetIndex >= members.length) return;

    const updated = [...members];
    const [member] = updated.splice(index, 1);
    updated.splice(targetIndex, 0, member);
    onMembersChange(updated.map((m, idx) => ({ ...m, order: idx })));
  };

  const handleRoleChange = (index: number, role: GroupMemberRole) => {
    const updated = [...members];
    updated[index].role = role;
    onMembersChange(updated);
  };

  const handleClassSettingsChange = (
    index: number,
    settings: Partial<ClassSpecificSettings>,
  ) => {
    const updated = [...members];
    updated[index] = {
      ...updated[index],
      class_settings: {
        ...DEFAULT_CLASS_SETTINGS,
        ...updated[index].class_settings,
        ...settings,
      },
    };
    onMembersChange(updated);
  };

  const handleRemoveMember = (index: number) => {
    onMembersChange(members.filter((_, i) => i !== index));
  };

  return (
    <div className="space-y-2">
      {members.length === 0 ? (
        <div className="text-center py-4 text-white/40 text-sm font-tech">
          No members assigned yet
        </div>
      ) : (
        members.map((member, idx) => {
          const classSettings = {
            ...DEFAULT_CLASS_SETTINGS,
            ...member.class_settings,
          };

          return (
            <div
              key={idx}
              draggable
              onDragStart={() => handleDragStart(idx)}
              onDragOver={handleDragOver}
              onDrop={() => handleDrop(idx)}
              className="p-3 bg-void border border-white/10 hover:border-white/20 transition-colors group"
            >
              <div className="flex items-center gap-3">
                <ArrowsDownUp
                  size={14}
                  className="text-white/40 flex-shrink-0"
                />
                <div className="flex-1 min-w-0">
                  <p className="text-white font-rune text-sm truncate">
                    {member.character_name}
                  </p>
                  <p className="text-white/50 text-xs">{member.class}</p>
                </div>
                <div className="flex items-center gap-1">
                  <button
                    type="button"
                    onClick={() => moveMember(idx, -1)}
                    disabled={idx === 0}
                    className="px-2 py-1 border border-white/10 text-white/50 disabled:opacity-30 hover:text-white"
                    aria-label={`Move ${member.character_name} up`}
                  >
                    ↑
                  </button>
                  <button
                    type="button"
                    onClick={() => moveMember(idx, 1)}
                    disabled={idx === members.length - 1}
                    className="px-2 py-1 border border-white/10 text-white/50 disabled:opacity-30 hover:text-white"
                    aria-label={`Move ${member.character_name} down`}
                  >
                    ↓
                  </button>
                </div>
                <select
                  value={member.role}
                  onChange={(e) =>
                    handleRoleChange(idx, e.target.value as GroupMemberRole)
                  }
                  className="bg-void border border-white/20 text-white/80 text-xs px-2 py-1 font-tech"
                  aria-label={`${member.character_name} role`}
                >
                  {ROLE_OPTIONS.map((role) => (
                    <option key={role} value={role}>
                      {ROLE_LABELS[role]}
                    </option>
                  ))}
                </select>
                <button
                  onClick={() => handleRemoveMember(idx)}
                  className="p-1 text-white/40 hover:text-red-400 transition-colors"
                  title="Remove member"
                  aria-label={`Remove ${member.character_name}`}
                >
                  <X size={14} />
                </button>
              </div>
              <details className="mt-3 border-t border-white/10 pt-3">
                <summary className="cursor-pointer text-xs font-tech uppercase tracking-wider text-white/60">
                  Class Settings
                </summary>
                <div className="mt-3 grid grid-cols-1 md:grid-cols-3 gap-3">
                  <label className="flex items-center gap-2 text-xs text-white/70 font-tech">
                    <input
                      type="checkbox"
                      checked={classSettings.pet_management_enabled}
                      onChange={(e) =>
                        handleClassSettingsChange(idx, {
                          pet_management_enabled: e.target.checked,
                        })
                      }
                    />
                    Pet management
                  </label>
                  <input
                    type="text"
                    value={classSettings.spell_priority}
                    onChange={(e) =>
                      handleClassSettingsChange(idx, {
                        spell_priority: e.target.value,
                      })
                    }
                    placeholder="Spell priorities"
                    className="bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
                    aria-label={`${member.character_name} spell priorities`}
                  />
                  <input
                    type="text"
                    value={classSettings.cc_assignment}
                    onChange={(e) =>
                      handleClassSettingsChange(idx, {
                        cc_assignment: e.target.value,
                      })
                    }
                    placeholder="CC assignment"
                    className="bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
                    aria-label={`${member.character_name} CC assignment`}
                  />
                </div>
              </details>
            </div>
          );
        })
      )}
    </div>
  );
}

// ── Group Edit Modal ─────────────────────────────────────────────────────────

interface GroupEditModalProps {
  group: Group | null;
  onSave: (group: Group) => void;
  onCancel: () => void;
}

function GroupEditModal({ group, onSave, onCancel }: GroupEditModalProps) {
  const [formName, setFormName] = useState(group?.name || "");
  const [formZone, setFormZone] = useState(group?.zone || "");
  const [formMembers, setFormMembers] = useState<GroupMember[]>(
    group?.members || [],
  );
  const [newMemberName, setNewMemberName] = useState("");
  const [newMemberClass, setNewMemberClass] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setFormName(group?.name || "");
    setFormZone(group?.zone || "");
    setFormMembers(group?.members || []);
    setNewMemberName("");
    setNewMemberClass("");
    setError(null);
  }, [group]);

  const handleAddMember = () => {
    if (!newMemberName.trim() || !newMemberClass.trim()) {
      setError("Member name and class are required");
      return;
    }
    if (formMembers.length >= 6) {
      setError("Groups support a maximum of 6 members");
      return;
    }
    if (
      formMembers.some(
        (member) =>
          member.character_name.toLowerCase() ===
          newMemberName.trim().toLowerCase(),
      )
    ) {
      setError("That character is already assigned to this group");
      return;
    }

    const newMember: GroupMember = {
      character_name: newMemberName.trim(),
      class: newMemberClass.trim(),
      role: "dps",
      order: formMembers.length,
      class_settings: DEFAULT_CLASS_SETTINGS,
    };

    setFormMembers([...formMembers, newMember]);
    setNewMemberName("");
    setNewMemberClass("");
    setError(null);
  };

  const handleSave = () => {
    if (!formName.trim()) {
      setError("Group name is required");
      return;
    }
    if (formMembers.length > 6) {
      setError("Groups support a maximum of 6 members");
      return;
    }
    const uniqueMembers = new Set(
      formMembers.map((member) => member.character_name.trim().toLowerCase()),
    );
    if (uniqueMembers.size !== formMembers.length) {
      setError("Each character can only appear once in a group");
      return;
    }

    if (group) {
      onSave({
        ...group,
        name: formName.trim(),
        zone: formZone.trim() || null,
        members: formMembers.map((member, index) => ({
          ...member,
          character_name: member.character_name.trim(),
          class: member.class.trim(),
          order: index,
          class_settings: {
            ...DEFAULT_CLASS_SETTINGS,
            ...member.class_settings,
          },
        })),
      });
    }
  };

  if (!group) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
      <div className="bg-void border border-white/20 p-6 w-full max-w-2xl shadow-[0_0_30px_rgba(147,51,234,0.15)] max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between mb-6">
          <h2 className="font-archaic text-2xl text-glow-magenta font-bold uppercase tracking-widest">
            {group.id ? "Edit Group" : "New Group"}
          </h2>
          <button
            onClick={onCancel}
            className="p-2 hover:bg-white/5 transition-colors"
          >
            <X size={20} />
          </button>
        </div>

        {error && (
          <div className="mb-4 p-3 border border-red-500/30 bg-red-500/10 text-red-300 text-sm font-tech">
            {error}
          </div>
        )}

        {/* Group Name */}
        <div className="mb-6">
          <label className="block text-sm font-tech text-white/60 uppercase tracking-wider mb-2">
            Group Name
          </label>
          <input
            type="text"
            value={formName}
            onChange={(e) => setFormName(e.target.value)}
            className="w-full bg-void border border-white/20 px-4 py-2 text-white font-tech focus:border-white/40 focus:outline-none"
            placeholder="e.g., Guild Raid Team"
            required
          />
        </div>

        {/* Zone */}
        <div className="mb-6">
          <label className="block text-sm font-tech text-white/60 uppercase tracking-wider mb-2">
            Zone (Optional)
          </label>
          <input
            type="text"
            value={formZone}
            onChange={(e) => setFormZone(e.target.value)}
            className="w-full bg-void border border-white/20 px-4 py-2 text-white font-tech focus:border-white/40 focus:outline-none"
            placeholder="e.g., North Temple of Veeshan"
          />
        </div>

        {/* Members */}
        <div className="mb-6">
          <h3 className="font-archaic text-sm text-white/70 uppercase tracking-widest mb-3">
            Members (Drag to reorder)
          </h3>
          <GroupMemberList
            members={formMembers}
            onMembersChange={setFormMembers}
          />

          {/* Add Member */}
          <div className="mt-4 p-4 border border-white/10 bg-void/50">
            <h4 className="text-sm font-tech text-white/60 uppercase tracking-wider mb-3">
              Add Member
            </h4>
            <div className="flex gap-2 mb-2">
              <input
                type="text"
                value={newMemberName}
                onChange={(e) => setNewMemberName(e.target.value)}
                placeholder="Character name"
                className="flex-1 bg-void border border-white/20 px-3 py-2 text-sm text-white font-tech focus:border-white/40 focus:outline-none"
              />
              <input
                type="text"
                value={newMemberClass}
                onChange={(e) => setNewMemberClass(e.target.value)}
                placeholder="Class"
                className="w-24 bg-void border border-white/20 px-3 py-2 text-sm text-white font-tech focus:border-white/40 focus:outline-none"
              />
              <button
                onClick={handleAddMember}
                className="px-4 py-2 bg-magentadark/30 border border-magenta/50 text-magenta text-sm hover:bg-magentadark/50 transition-colors font-tech uppercase tracking-wider"
              >
                <Plus size={16} />
              </button>
            </div>
          </div>
        </div>

        {/* Actions */}
        <div className="flex gap-3 justify-end">
          <button
            onClick={onCancel}
            className="px-4 py-2 border border-white/20 text-white/60 text-sm hover:bg-white/5 transition-colors font-tech uppercase tracking-wider"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            className="px-4 py-2 bg-magentadark/30 border border-magenta/50 text-magenta text-sm hover:bg-magentadark/50 transition-colors font-tech uppercase tracking-wider"
          >
            Save Group
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Camp Configuration Panel ─────────────────────────────────────────────────

interface CampConfigPanelProps {
  group: Group;
  campConfig: CampConfiguration | null;
  saving: boolean;
  onSave: (
    id: string | null,
    payload: CreateCampConfigPayload | UpdateCampConfigPayload,
  ) => Promise<void>;
}

function CampConfigPanel({
  group,
  campConfig,
  saving,
  onSave,
}: CampConfigPanelProps) {
  const [templateName, setTemplateName] = useState(
    campConfig?.template_name || group.name,
  );
  const [campZone, setCampZone] = useState(
    campConfig?.camp_zone || group.zone || "",
  );
  const [campCenter, setCampCenter] = useState<CampCoordinate>(
    campConfig?.camp_center || { x: 0, y: 0, z: 0 },
  );
  const [pullRadius, setPullRadius] = useState(campConfig?.pull_radius || 100);
  const [pullPoints, setPullPoints] = useState<PullPoint[]>(
    campConfig?.pull_points || [],
  );
  const [pullTargets, setPullTargets] = useState<PullTarget[]>(
    campConfig?.pull_targets || [],
  );
  const [safeZones, setSafeZones] = useState<SafeZoneMarker[]>(
    campConfig?.safe_zone_markers || [],
  );
  const [combatSettings, setCombatSettings] = useState<CombatSettings>({
    ...DEFAULT_COMBAT_SETTINGS,
    ...campConfig?.combat_settings,
  });
  const [newTargetName, setNewTargetName] = useState("");
  const [newPullPointLabel, setNewPullPointLabel] = useState("");
  const [newPullPointLocation, setNewPullPointLocation] =
    useState<CampCoordinate>({ x: 0, y: 0, z: 0 });
  const [newZoneName, setNewZoneName] = useState("");
  const [newZoneCenter, setNewZoneCenter] = useState<CampCoordinate>({
    x: 0,
    y: 0,
    z: 0,
  });
  const [newZoneRadius, setNewZoneRadius] = useState(50);
  const [formError, setFormError] = useState<string | null>(null);

  useEffect(() => {
    setTemplateName(campConfig?.template_name || group.name);
    setCampZone(campConfig?.camp_zone || group.zone || "");
    setCampCenter(campConfig?.camp_center || { x: 0, y: 0, z: 0 });
    setPullRadius(campConfig?.pull_radius || 100);
    setPullPoints(campConfig?.pull_points || []);
    setPullTargets(campConfig?.pull_targets || []);
    setSafeZones(campConfig?.safe_zone_markers || []);
    setCombatSettings({
      ...DEFAULT_COMBAT_SETTINGS,
      ...campConfig?.combat_settings,
    });
    setNewTargetName("");
    setNewPullPointLabel("");
    setNewPullPointLocation({ x: 0, y: 0, z: 0 });
    setNewZoneName("");
    setNewZoneCenter({ x: 0, y: 0, z: 0 });
    setNewZoneRadius(50);
    setFormError(null);
  }, [campConfig, group]);

  const validateCampConfig = () => {
    if (!campZone.trim()) return "Camp zone is required";
    if (!hasFiniteCoordinates(campCenter)) {
      return "Camp center coordinates must be valid numbers";
    }
    if (!Number.isFinite(pullRadius) || pullRadius < 10 || pullRadius > 500) {
      return "Pull radius must be between 10 and 500";
    }
    if (
      combatSettings.hp_buff_threshold_pct < 1 ||
      combatSettings.hp_buff_threshold_pct > 100 ||
      combatSettings.mana_buff_threshold_pct < 1 ||
      combatSettings.mana_buff_threshold_pct > 100
    ) {
      return "HP and mana thresholds must be between 1 and 100";
    }
    if (pullPoints.some((point) => !hasFiniteCoordinates(point.location))) {
      return "Pull point coordinates must be valid numbers";
    }
    if (
      safeZones.some(
        (zone) => zone.radius <= 0 || !hasFiniteCoordinates(zone.center),
      )
    ) {
      return "Safe zones require a positive radius and valid coordinates";
    }
    return null;
  };

  const handleAddPullTarget = () => {
    if (!newTargetName.trim()) return;
    setPullTargets([
      ...pullTargets,
      { name: newTargetName.trim(), enabled: true },
    ]);
    setNewTargetName("");
  };

  const handleRemovePullTarget = (index: number) => {
    setPullTargets(pullTargets.filter((_, i) => i !== index));
  };

  const handleTogglePullTarget = (index: number) => {
    const updated = [...pullTargets];
    updated[index].enabled = !updated[index].enabled;
    setPullTargets(updated);
  };

  const handleAddPullPoint = () => {
    if (!newPullPointLabel.trim()) {
      setFormError("Pull point label is required");
      return;
    }
    if (!hasFiniteCoordinates(newPullPointLocation)) {
      setFormError("Pull point coordinates must be valid numbers");
      return;
    }
    setPullPoints([
      ...pullPoints,
      {
        label: newPullPointLabel.trim(),
        location: newPullPointLocation,
        enabled: true,
      },
    ]);
    setNewPullPointLabel("");
    setNewPullPointLocation({ x: 0, y: 0, z: 0 });
    setFormError(null);
  };

  const handleRemovePullPoint = (index: number) => {
    setPullPoints(pullPoints.filter((_, i) => i !== index));
  };

  const handleTogglePullPoint = (index: number) => {
    const updated = [...pullPoints];
    updated[index].enabled = !updated[index].enabled;
    setPullPoints(updated);
  };

  const handleAddSafeZone = () => {
    if (!newZoneName.trim()) return;
    setSafeZones([
      ...safeZones,
      {
        name: newZoneName.trim(),
        center: newZoneCenter,
        radius: newZoneRadius,
      },
    ]);
    setNewZoneName("");
    setNewZoneCenter({ x: 0, y: 0, z: 0 });
    setNewZoneRadius(50);
  };

  const handleRemoveSafeZone = (index: number) => {
    setSafeZones(safeZones.filter((_, i) => i !== index));
  };

  const handleSaveCamp = async () => {
    const validationError = validateCampConfig();
    if (validationError) {
      setFormError(validationError);
      return;
    }

    const payload: CreateCampConfigPayload = {
      group_id: group.id,
      template_name: templateName.trim() || group.name,
      camp_zone: campZone.trim(),
      camp_center: campCenter,
      pull_radius: pullRadius,
      pull_points: pullPoints,
      pull_targets: pullTargets,
      safe_zone_markers: safeZones,
      combat_settings: combatSettings,
    };

    try {
      await onSave(campConfig?.id || null, payload);
      setFormError(null);
    } catch (e) {
      setFormError(
        e instanceof Error ? e.message : "Failed to save camp configuration",
      );
    }
  };

  return (
    <div className="bg-void/50 border border-white/10 p-6 space-y-6">
      <h3 className="font-archaic text-xl text-glow-magenta font-bold uppercase tracking-widest flex items-center gap-2">
        <MapPin size={20} />
        Camp Configuration: {group.name}
      </h3>

      {formError && (
        <div className="p-3 border border-red-500/30 bg-red-500/10 text-red-300 text-sm font-tech">
          {formError}
        </div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div>
          <label className="block text-xs font-tech text-white/60 uppercase tracking-wider mb-1">
            Template Name
          </label>
          <input
            type="text"
            value={templateName}
            onChange={(e) => setTemplateName(e.target.value)}
            className="w-full bg-void border border-white/20 px-3 py-2 text-white font-tech text-sm focus:border-white/40 focus:outline-none"
            placeholder="e.g., Velks Frenzy"
          />
        </div>
        <div>
          <label className="block text-xs font-tech text-white/60 uppercase tracking-wider mb-1">
            Camp Zone
          </label>
          <input
            type="text"
            value={campZone}
            onChange={(e) => setCampZone(e.target.value)}
            className="w-full bg-void border border-white/20 px-3 py-2 text-white font-tech text-sm focus:border-white/40 focus:outline-none"
            placeholder="e.g., velketor"
            required
          />
        </div>
      </div>

      {/* Camp Center Coordinates */}
      <div className="grid grid-cols-3 gap-4">
        <div>
          <label className="block text-xs font-tech text-white/60 uppercase tracking-wider mb-1">
            Center X
          </label>
          <input
            type="number"
            value={campCenter.x}
            onChange={(e) =>
              setCampCenter({
                ...campCenter,
                x: finiteNumber(e.target.value),
              })
            }
            className="w-full bg-void border border-white/20 px-3 py-2 text-white font-tech text-sm focus:border-white/40 focus:outline-none"
          />
        </div>
        <div>
          <label className="block text-xs font-tech text-white/60 uppercase tracking-wider mb-1">
            Center Y
          </label>
          <input
            type="number"
            value={campCenter.y}
            onChange={(e) =>
              setCampCenter({
                ...campCenter,
                y: finiteNumber(e.target.value),
              })
            }
            className="w-full bg-void border border-white/20 px-3 py-2 text-white font-tech text-sm focus:border-white/40 focus:outline-none"
          />
        </div>
        <div>
          <label className="block text-xs font-tech text-white/60 uppercase tracking-wider mb-1">
            Center Z
          </label>
          <input
            type="number"
            value={campCenter.z}
            onChange={(e) =>
              setCampCenter({
                ...campCenter,
                z: finiteNumber(e.target.value),
              })
            }
            className="w-full bg-void border border-white/20 px-3 py-2 text-white font-tech text-sm focus:border-white/40 focus:outline-none"
          />
        </div>
      </div>

      {/* Pull Radius */}
      <div>
        <label className="block text-xs font-tech text-white/60 uppercase tracking-wider mb-2">
          Pull Radius: <span className="text-white/80">{pullRadius}m</span>
        </label>
        <input
          type="range"
          min="10"
          max="500"
          step="10"
          value={pullRadius}
          onChange={(e) => setPullRadius(Number.parseInt(e.target.value, 10))}
          className="w-full"
        />
      </div>

      {/* Combat Settings */}
      <div className="border border-white/10 bg-void/40 p-4">
        <h4 className="font-archaic text-sm text-white/70 uppercase tracking-widest mb-3">
          Combat Settings
        </h4>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div>
            <label className="block text-xs font-tech text-white/60 uppercase tracking-wider mb-1">
              HP Buff Threshold
            </label>
            <input
              type="number"
              min="1"
              max="100"
              value={combatSettings.hp_buff_threshold_pct}
              onChange={(e) =>
                setCombatSettings({
                  ...combatSettings,
                  hp_buff_threshold_pct: finiteNumber(e.target.value, 1),
                })
              }
              className="w-full bg-void border border-white/20 px-3 py-2 text-white font-tech text-sm focus:border-white/40 focus:outline-none"
            />
          </div>
          <div>
            <label className="block text-xs font-tech text-white/60 uppercase tracking-wider mb-1">
              Mana Buff Threshold
            </label>
            <input
              type="number"
              min="1"
              max="100"
              value={combatSettings.mana_buff_threshold_pct}
              onChange={(e) =>
                setCombatSettings({
                  ...combatSettings,
                  mana_buff_threshold_pct: finiteNumber(e.target.value, 1),
                })
              }
              className="w-full bg-void border border-white/20 px-3 py-2 text-white font-tech text-sm focus:border-white/40 focus:outline-none"
            />
          </div>
          <div>
            <label className="block text-xs font-tech text-white/60 uppercase tracking-wider mb-1">
              Pull Strategy
            </label>
            <select
              value={combatSettings.pull_strategy}
              onChange={(e) =>
                setCombatSettings({
                  ...combatSettings,
                  pull_strategy: e.target.value as PullStrategy,
                })
              }
              className="w-full bg-void border border-white/20 px-3 py-2 text-white font-tech text-sm focus:border-white/40 focus:outline-none"
            >
              {PULL_STRATEGIES.map((strategy) => (
                <option key={strategy} value={strategy}>
                  {strategy}
                </option>
              ))}
            </select>
          </div>
        </div>
      </div>

      {/* Pull Points */}
      <div>
        <h4 className="font-archaic text-sm text-white/70 uppercase tracking-widest mb-3 flex items-center gap-2">
          <MapPin size={16} />
          Pull Points
        </h4>
        <div className="space-y-2 mb-3">
          {pullPoints.length === 0 ? (
            <div className="text-center py-2 text-white/40 text-xs font-tech">
              No pull points defined
            </div>
          ) : (
            pullPoints.map((point, idx) => (
              <div
                key={`${point.label}-${idx}`}
                className="flex items-center gap-2 p-2 bg-void border border-white/10"
              >
                <input
                  type="checkbox"
                  checked={point.enabled}
                  onChange={() => handleTogglePullPoint(idx)}
                  className="cursor-pointer"
                  aria-label={`Enable ${point.label}`}
                />
                <span className="flex-1 text-white font-rune text-sm">
                  {point.label}
                </span>
                <span className="text-white/50 text-xs font-tech">
                  ({point.location.x}, {point.location.y}, {point.location.z})
                </span>
                <button
                  onClick={() => handleRemovePullPoint(idx)}
                  className="p-1 text-white/40 hover:text-red-400 transition-colors"
                  aria-label={`Remove ${point.label}`}
                >
                  <X size={12} />
                </button>
              </div>
            ))
          )}
        </div>
        <div className="grid grid-cols-2 md:grid-cols-5 gap-2">
          <input
            type="text"
            value={newPullPointLabel}
            onChange={(e) => setNewPullPointLabel(e.target.value)}
            placeholder="Point label"
            className="col-span-2 bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
          />
          <input
            type="number"
            value={newPullPointLocation.x}
            onChange={(e) =>
              setNewPullPointLocation({
                ...newPullPointLocation,
                x: finiteNumber(e.target.value),
              })
            }
            placeholder="X"
            className="bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
          />
          <input
            type="number"
            value={newPullPointLocation.y}
            onChange={(e) =>
              setNewPullPointLocation({
                ...newPullPointLocation,
                y: finiteNumber(e.target.value),
              })
            }
            placeholder="Y"
            className="bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
          />
          <div className="flex gap-2">
            <input
              type="number"
              value={newPullPointLocation.z}
              onChange={(e) =>
                setNewPullPointLocation({
                  ...newPullPointLocation,
                  z: finiteNumber(e.target.value),
                })
              }
              placeholder="Z"
              className="min-w-0 flex-1 bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
            />
            <button
              onClick={handleAddPullPoint}
              className="px-3 py-2 bg-magentadark/30 border border-magenta/50 text-magenta text-xs hover:bg-magentadark/50 transition-colors"
              aria-label="Add pull point"
            >
              <Plus size={14} />
            </button>
          </div>
        </div>
      </div>

      {/* Pull Targets */}
      <div>
        <h4 className="font-archaic text-sm text-white/70 uppercase tracking-widest mb-3 flex items-center gap-2">
          <Target size={16} />
          Pull Targets
        </h4>
        <div className="space-y-2 mb-3">
          {pullTargets.length === 0 ? (
            <div className="text-center py-2 text-white/40 text-xs font-tech">
              No pull targets defined
            </div>
          ) : (
            pullTargets.map((target, idx) => (
              <div
                key={idx}
                className="flex items-center gap-2 p-2 bg-void border border-white/10"
              >
                <input
                  type="checkbox"
                  checked={target.enabled}
                  onChange={() => handleTogglePullTarget(idx)}
                  className="cursor-pointer"
                  aria-label={`Enable ${target.name}`}
                />
                <span className="flex-1 text-white font-rune text-sm">
                  {target.name}
                </span>
                <button
                  onClick={() => handleRemovePullTarget(idx)}
                  className="p-1 text-white/40 hover:text-red-400 transition-colors"
                  aria-label={`Remove ${target.name}`}
                >
                  <X size={12} />
                </button>
              </div>
            ))
          )}
        </div>
        <div className="flex gap-2">
          <input
            type="text"
            value={newTargetName}
            onChange={(e) => setNewTargetName(e.target.value)}
            placeholder="Target name (e.g., Named Mob)"
            className="flex-1 bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
          />
          <button
            onClick={handleAddPullTarget}
            className="px-3 py-2 bg-magentadark/30 border border-magenta/50 text-magenta text-xs hover:bg-magentadark/50 transition-colors"
            aria-label="Add pull target"
          >
            <Plus size={14} />
          </button>
        </div>
      </div>

      {/* Safe Zone Markers */}
      <div>
        <h4 className="font-archaic text-sm text-white/70 uppercase tracking-widest mb-3 flex items-center gap-2">
          <Shield size={16} />
          Safe Zone Markers
        </h4>
        <div className="space-y-2 mb-4">
          {safeZones.length === 0 ? (
            <div className="text-center py-2 text-white/40 text-xs font-tech">
              No safe zones defined
            </div>
          ) : (
            safeZones.map((zone, idx) => (
              <div
                key={idx}
                className="p-3 bg-void border border-white/10 text-xs font-tech"
              >
                <div className="flex items-center justify-between mb-2">
                  <span className="text-white font-rune">{zone.name}</span>
                  <button
                    onClick={() => handleRemoveSafeZone(idx)}
                    className="p-1 text-white/40 hover:text-red-400 transition-colors"
                    aria-label={`Remove ${zone.name}`}
                  >
                    <X size={12} />
                  </button>
                </div>
                <div className="text-white/60 space-y-1">
                  <div>
                    Center: ({zone.center.x}, {zone.center.y},{zone.center.z})
                  </div>
                  <div>Radius: {zone.radius}m</div>
                </div>
              </div>
            ))
          )}
        </div>
        <div className="grid grid-cols-2 gap-2 mb-2">
          <input
            type="text"
            value={newZoneName}
            onChange={(e) => setNewZoneName(e.target.value)}
            placeholder="Zone name"
            className="col-span-2 bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
          />
          <input
            type="number"
            value={newZoneCenter.x}
            onChange={(e) =>
              setNewZoneCenter({
                ...newZoneCenter,
                x: finiteNumber(e.target.value),
              })
            }
            placeholder="X"
            className="bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
          />
          <input
            type="number"
            value={newZoneCenter.y}
            onChange={(e) =>
              setNewZoneCenter({
                ...newZoneCenter,
                y: finiteNumber(e.target.value),
              })
            }
            placeholder="Y"
            className="bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
          />
          <input
            type="number"
            value={newZoneCenter.z}
            onChange={(e) =>
              setNewZoneCenter({
                ...newZoneCenter,
                z: finiteNumber(e.target.value),
              })
            }
            placeholder="Z"
            className="bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
          />
          <input
            type="number"
            value={newZoneRadius}
            onChange={(e) => setNewZoneRadius(finiteNumber(e.target.value, 1))}
            placeholder="Radius"
            min="1"
            className="bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
          />
          <button
            onClick={handleAddSafeZone}
            className="px-3 py-2 bg-magentadark/30 border border-magenta/50 text-magenta text-xs hover:bg-magentadark/50 transition-colors"
            aria-label="Add safe zone"
          >
            <Plus size={14} />
          </button>
        </div>
      </div>

      {/* Preview and Save */}
      <div className="grid grid-cols-1 lg:grid-cols-[1fr_auto] gap-4 items-end border-t border-white/10 pt-4">
        <div
          className="bg-black/20 border border-white/10 p-4 text-sm font-tech text-white/70"
          aria-live="polite"
        >
          <div className="text-white font-archaic uppercase tracking-widest mb-2">
            Current Configuration Preview
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
            <span>Template: {templateName || group.name}</span>
            <span>Zone: {campZone || "unassigned"}</span>
            <span>
              Center: {campCenter.x}, {campCenter.y}, {campCenter.z}
            </span>
            <span>Pull radius: {pullRadius}m</span>
            <span>
              Buffs: HP {combatSettings.hp_buff_threshold_pct}% / Mana{" "}
              {combatSettings.mana_buff_threshold_pct}%
            </span>
            <span>Strategy: {combatSettings.pull_strategy}</span>
            <span>
              Pull points: {pullPoints.filter((point) => point.enabled).length}{" "}
              active
            </span>
            <span>
              CC markers: {safeZones.length} safe /{" "}
              {pullTargets.filter((target) => target.enabled).length} targets
            </span>
          </div>
        </div>
        <button
          onClick={handleSaveCamp}
          disabled={saving}
          className="px-4 py-3 bg-magentadark/30 border border-magenta/50 text-magenta text-sm hover:bg-magentadark/50 disabled:opacity-50 transition-colors font-tech uppercase tracking-wider"
        >
          {campConfig ? "Save Camp" : "Save Camp Template"}
        </button>
      </div>
    </div>
  );
}

// ── Main GroupsPage ──────────────────────────────────────────────────────────

export default function GroupsPage() {
  const { groups, loading, error, createGroup, updateGroup, deleteGroup } =
    useGroups();
  const { campConfigs, createCampConfig, updateCampConfig } =
    useCampConfiguration();

  const [editingGroup, setEditingGroup] = useState<Group | null>(null);
  const [selectedGroup, setSelectedGroup] = useState<Group | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saveLoading, setSaveLoading] = useState(false);
  const [campSaveLoading, setCampSaveLoading] = useState(false);

  const handleCreateGroup = () => {
    setSaveError(null);
    setEditingGroup({
      id: "",
      name: "",
      zone: null,
      members: [],
      created_at: "",
      updated_at: "",
    });
  };

  const handleEditGroup = (group: Group) => {
    setEditingGroup(group);
  };

  const handleSaveGroup = async (updatedGroup: Group) => {
    setSaveLoading(true);
    setSaveError(null);
    try {
      const payload = {
        name: updatedGroup.name,
        zone: updatedGroup.zone,
        members: updatedGroup.members,
      };
      const savedGroup = updatedGroup.id
        ? await updateGroup(updatedGroup.id, payload)
        : await createGroup(payload);
      if (selectedGroup?.id === savedGroup.id) {
        setSelectedGroup(savedGroup);
      }
      setEditingGroup(null);
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : "Failed to save group");
    } finally {
      setSaveLoading(false);
    }
  };

  const handleDeleteGroup = async (id: string) => {
    setSaveLoading(true);
    setSaveError(null);
    try {
      await deleteGroup(id);
      if (selectedGroup?.id === id) {
        setSelectedGroup(null);
      }
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : "Failed to delete group");
    } finally {
      setSaveLoading(false);
    }
  };

  const getCampConfigForGroup = (groupId: string) => {
    return campConfigs.find((c) => c.group_id === groupId) || null;
  };

  const handleSaveCamp = async (
    id: string | null,
    payload: CreateCampConfigPayload | UpdateCampConfigPayload,
  ) => {
    setCampSaveLoading(true);
    try {
      if (id) {
        await updateCampConfig(id, payload);
      } else {
        await createCampConfig(payload as CreateCampConfigPayload);
      }
    } finally {
      setCampSaveLoading(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-white/60 font-tech">Loading groups...</div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full w-full gap-6 overflow-y-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <h2 className="font-archaic text-3xl text-glow-magenta font-bold uppercase tracking-widest flex items-center gap-2">
          <UsersThree size={32} weight="fill" />
          Fleet Formations
        </h2>
        <button
          onClick={handleCreateGroup}
          disabled={saveLoading}
          className="flex items-center gap-2 px-4 py-2 bg-magentadark/30 border border-magenta/50 text-magenta hover:bg-magentadark/50 disabled:opacity-50 transition-colors font-tech uppercase tracking-wider text-sm"
        >
          <Plus size={16} />
          New Group
        </button>
      </div>

      {/* Error display */}
      {(error || saveError) && (
        <div className="p-4 border border-red-500/30 bg-red-500/10 text-red-300 text-sm font-tech flex items-start gap-3">
          <Warning size={16} className="flex-shrink-0 mt-0.5" />
          <div>
            <p className="font-bold mb-1">Error</p>
            <p>{error || saveError}</p>
          </div>
        </div>
      )}

      {/* Groups Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {groups.map((group) => (
          <div key={group.id} className="flex flex-col gap-2">
            <GroupCard
              group={group}
              onEdit={handleEditGroup}
              onDelete={handleDeleteGroup}
            />
            <button
              onClick={() => setSelectedGroup(group)}
              className={`px-3 py-2 border text-sm font-tech uppercase tracking-wider transition-colors ${
                selectedGroup?.id === group.id
                  ? "bg-magenta/20 border-magenta/50 text-magenta"
                  : "border-white/20 text-white/60 hover:border-white/40 hover:text-white/80"
              }`}
            >
              {selectedGroup?.id === group.id
                ? "Camp Config (Active)"
                : "Camp Config"}
            </button>
          </div>
        ))}
      </div>

      {groups.length === 0 && (
        <div className="text-center py-12">
          <div className="text-white/40 font-tech text-sm mb-4">
            No groups configured yet
          </div>
          <button
            onClick={handleCreateGroup}
            disabled={saveLoading}
            className="inline-flex items-center gap-2 px-4 py-2 bg-magentadark/30 border border-magenta/50 text-magenta hover:bg-magentadark/50 disabled:opacity-50 transition-colors font-tech uppercase tracking-wider text-sm"
          >
            <Plus size={16} />
            Create First Group
          </button>
        </div>
      )}

      {/* Camp Configuration Panel */}
      {selectedGroup && (
        <div className="mt-6">
          <CampConfigPanel
            group={selectedGroup}
            campConfig={getCampConfigForGroup(selectedGroup.id)}
            saving={campSaveLoading}
            onSave={handleSaveCamp}
          />
        </div>
      )}

      {/* Group Edit Modal */}
      <GroupEditModal
        group={editingGroup}
        onSave={handleSaveGroup}
        onCancel={() => setEditingGroup(null)}
      />
    </div>
  );
}
