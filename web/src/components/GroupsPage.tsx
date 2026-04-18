/**
 * GroupsPage — Fleet Formations & Camp Configuration
 *
 * Features:
 *  - Groups List: cards showing each group (name, member count, zone), edit/delete, add new group button
 *  - Group Detail/Edit Modal: group name, member list with drag-and-drop, role assignments
 *  - Camp Configuration Section: camp center coordinates, pull radius, pull targets list, safe zone markers
 *  - Integration with API client for CRUD operations
 */

import {
  useState,
  type DragEvent,
} from "react";
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
  GroupMember,
  GroupMemberRole,
  CampConfiguration,
  CampCoordinate,
  PullTarget,
  SafeZoneMarker,
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
        `Permanently remove group "${group.name}" from the registry? This cannot be undone.`
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
              {member.character_name}
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

function GroupMemberList({
  members,
  onMembersChange,
}: GroupMemberListProps) {
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

  const handleRoleChange = (index: number, role: GroupMemberRole) => {
    const updated = [...members];
    updated[index].role = role;
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
        members.map((member, idx) => (
          <div
            key={idx}
            draggable
            onDragStart={() => handleDragStart(idx)}
            onDragOver={handleDragOver}
            onDrop={() => handleDrop(idx)}
            className="flex items-center gap-3 p-3 bg-void border border-white/10 cursor-move hover:border-white/20 transition-colors group"
          >
            <ArrowsDownUp size={14} className="text-white/40 flex-shrink-0" />
            <div className="flex-1 min-w-0">
              <p className="text-white font-rune text-sm truncate">
                {member.character_name}
              </p>
              <p className="text-white/50 text-xs">{member.class}</p>
            </div>
            <select
              value={member.role}
              onChange={(e) =>
                handleRoleChange(idx, e.target.value as GroupMemberRole)
              }
              className="bg-void border border-white/20 text-white/80 text-xs px-2 py-1 font-tech"
            >
              <option value="main_tank">Main Tank</option>
              <option value="main_assist">Main Assist</option>
              <option value="puller">Puller</option>
              <option value="healer">Healer</option>
              <option value="dps">DPS</option>
              <option value="support">Support</option>
              <option value="cc">CC</option>
            </select>
            <button
              onClick={() => handleRemoveMember(idx)}
              className="p-1 text-white/40 hover:text-red-400 transition-colors"
              title="Remove member"
            >
              <X size={14} />
            </button>
          </div>
        ))
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
    group?.members || []
  );
  const [newMemberName, setNewMemberName] = useState("");
  const [newMemberClass, setNewMemberClass] = useState("");
  const [error, setError] = useState<string | null>(null);

  const handleAddMember = () => {
    if (!newMemberName.trim() || !newMemberClass.trim()) {
      setError("Member name and class are required");
      return;
    }

    const newMember: GroupMember = {
      character_name: newMemberName,
      class: newMemberClass,
      role: "dps",
      order: formMembers.length,
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

    if (group) {
      onSave({
        ...group,
        name: formName,
        zone: formZone || null,
        members: formMembers,
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
          <GroupMemberList members={formMembers} onMembersChange={setFormMembers} />

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
}

function CampConfigPanel({
  group,
  campConfig,
}: CampConfigPanelProps) {
  const [campCenter, setCampCenter] = useState<CampCoordinate>(
    campConfig?.camp_center || { x: 0, y: 0, z: 0 }
  );
  const [pullRadius, setPullRadius] = useState(
    campConfig?.pull_radius || 100
  );
  const [pullTargets, setPullTargets] = useState<PullTarget[]>(
    campConfig?.pull_targets || []
  );
  const [safeZones, setSafeZones] = useState<SafeZoneMarker[]>(
    campConfig?.safe_zone_markers || []
  );
  const [newTargetName, setNewTargetName] = useState("");
  const [newZoneName, setNewZoneName] = useState("");
  const [newZoneCenter, setNewZoneCenter] = useState<CampCoordinate>({
    x: 0,
    y: 0,
    z: 0,
  });
  const [newZoneRadius, setNewZoneRadius] = useState(50);

  const handleAddPullTarget = () => {
    if (!newTargetName.trim()) return;
    setPullTargets([
      ...pullTargets,
      { name: newTargetName, enabled: true },
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

  const handleAddSafeZone = () => {
    if (!newZoneName.trim()) return;
    setSafeZones([
      ...safeZones,
      {
        name: newZoneName,
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

  return (
    <div className="bg-void/50 border border-white/10 p-6 space-y-6">
      <h3 className="font-archaic text-xl text-glow-magenta font-bold uppercase tracking-widest flex items-center gap-2">
        <MapPin size={20} />
        Camp Configuration: {group.name}
      </h3>

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
              setCampCenter({ ...campCenter, x: parseFloat(e.target.value) })
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
              setCampCenter({ ...campCenter, y: parseFloat(e.target.value) })
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
              setCampCenter({ ...campCenter, z: parseFloat(e.target.value) })
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
          onChange={(e) => setPullRadius(parseInt(e.target.value))}
          className="w-full"
        />
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
                />
                <span className="flex-1 text-white font-rune text-sm">
                  {target.name}
                </span>
                <button
                  onClick={() => handleRemovePullTarget(idx)}
                  className="p-1 text-white/40 hover:text-red-400 transition-colors"
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
                  >
                    <X size={12} />
                  </button>
                </div>
                <div className="text-white/60 space-y-1">
                  <div>
                    Center: ({zone.center.x}, {zone.center.y},
                    {zone.center.z})
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
              setNewZoneCenter({ ...newZoneCenter, x: parseFloat(e.target.value) })
            }
            placeholder="X"
            className="bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
          />
          <input
            type="number"
            value={newZoneCenter.y}
            onChange={(e) =>
              setNewZoneCenter({ ...newZoneCenter, y: parseFloat(e.target.value) })
            }
            placeholder="Y"
            className="bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
          />
          <input
            type="number"
            value={newZoneCenter.z}
            onChange={(e) =>
              setNewZoneCenter({ ...newZoneCenter, z: parseFloat(e.target.value) })
            }
            placeholder="Z"
            className="bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
          />
          <input
            type="number"
            value={newZoneRadius}
            onChange={(e) => setNewZoneRadius(parseFloat(e.target.value))}
            placeholder="Radius"
            min="1"
            className="bg-void border border-white/20 px-3 py-2 text-white font-tech text-xs focus:border-white/40 focus:outline-none"
          />
          <button
            onClick={handleAddSafeZone}
            className="px-3 py-2 bg-magentadark/30 border border-magenta/50 text-magenta text-xs hover:bg-magentadark/50 transition-colors"
          >
            <Plus size={14} />
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Main GroupsPage ──────────────────────────────────────────────────────────

export default function GroupsPage() {
  const { groups, loading, error, createGroup, updateGroup, deleteGroup } =
    useGroups();
  const { campConfigs } =
    useCampConfiguration();

  const [editingGroup, setEditingGroup] = useState<Group | null>(null);
  const [selectedGroup, setSelectedGroup] = useState<Group | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saveLoading, setSaveLoading] = useState(false);

  const handleCreateGroup = async () => {
    setSaveLoading(true);
    setSaveError(null);
    try {
      const newGroup = await createGroup({
        name: "New Group",
        zone: null,
        members: [],
      });
      setEditingGroup(newGroup);
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : "Failed to create group");
    } finally {
      setSaveLoading(false);
    }
  };

  const handleEditGroup = (group: Group) => {
    setEditingGroup(group);
  };

  const handleSaveGroup = async (updatedGroup: Group) => {
    setSaveLoading(true);
    setSaveError(null);
    try {
      await updateGroup(updatedGroup.id, {
        name: updatedGroup.name,
        zone: updatedGroup.zone,
        members: updatedGroup.members,
      });
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
            onSave={createCampConfig}
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
