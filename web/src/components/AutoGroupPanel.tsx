import { useEffect, useState } from "react";
import {
  ArrowsClockwise,
  CaretDown,
  CaretUp,
  Crown,
  FloppyDisk,
  Plus,
  Rows,
  Trash,
  UsersThree,
} from "@phosphor-icons/react";

import { useAutoGroupSettings } from "../hooks/useAutoGroupSettings";
import type {
  AutoGroupMember,
  AutoGroupProfile,
  AutoGroupRole,
  AutoGroupSettings,
} from "../types";

type AutoGroupPanelProps = {
  embedded?: boolean;
};

const ROLE_OPTIONS: { value: AutoGroupRole; label: string }[] = [
  { value: "none", label: "No Group Role" },
  { value: "main_tank", label: "Main Tank" },
  { value: "main_assist", label: "Main Assist" },
  { value: "puller", label: "Puller" },
  { value: "mark_npc", label: "Mark NPC" },
  { value: "master_looter", label: "Master Looter" },
];

function blankMember(): AutoGroupMember {
  return { name: "", role: "none" };
}

function blankProfile(index: number): AutoGroupProfile {
  return {
    name: `Group Profile ${index}`,
    leader_name: "",
    enabled: true,
    invite_retry_interval_secs: 5,
    max_invite_retries: 3,
    completion_command: "",
    members: [blankMember()],
  };
}

function Banner({
  tone,
  message,
}: {
  tone: "neutral" | "success" | "error";
  message: string;
}) {
  const className =
    tone === "success"
      ? "border-emerald-400/30 bg-emerald-500/10 text-emerald-200"
      : tone === "error"
        ? "border-rose-400/30 bg-rose-500/10 text-rose-200"
        : "border-white/10 bg-white/5 text-white/70";

  return <div className={`rounded-2xl border px-4 py-3 text-sm ${className}`}>{message}</div>;
}

export default function AutoGroupPanel({ embedded = false }: AutoGroupPanelProps) {
  const { settings, loading, saving, error, savedAt, refresh, saveSettings } =
    useAutoGroupSettings();
  const [draft, setDraft] = useState<AutoGroupSettings>(settings);

  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  function updateProfile(
    profileIndex: number,
    updater: (profile: AutoGroupProfile) => AutoGroupProfile,
  ) {
    setDraft((current) => ({
      groups: current.groups.map((profile, index) =>
        index === profileIndex ? updater(profile) : profile,
      ),
    }));
  }

  function updateMember(
    profileIndex: number,
    memberIndex: number,
    updater: (member: AutoGroupMember) => AutoGroupMember,
  ) {
    updateProfile(profileIndex, (profile) => ({
      ...profile,
      members: profile.members.map((member, index) =>
        index === memberIndex ? updater(member) : member,
      ),
    }));
  }

  function moveMember(profileIndex: number, memberIndex: number, direction: -1 | 1) {
    updateProfile(profileIndex, (profile) => {
      const nextIndex = memberIndex + direction;
      if (nextIndex < 0 || nextIndex >= profile.members.length) {
        return profile;
      }

      const members = [...profile.members];
      const [member] = members.splice(memberIndex, 1);
      members.splice(nextIndex, 0, member);
      return { ...profile, members };
    });
  }

  async function handleSave() {
    const sanitized: AutoGroupSettings = {
      groups: draft.groups.map((profile) => ({
        ...profile,
        name: profile.name.trim(),
        leader_name: profile.leader_name.trim(),
        completion_command: profile.completion_command?.trim() || null,
        members: profile.members.map((member) => ({
          ...member,
          name: member.name.trim(),
        })),
      })),
    };
    await saveSettings(sanitized);
  }

  const banner = loading
    ? { tone: "neutral" as const, message: "Loading auto-group profiles..." }
    : error
      ? { tone: "error" as const, message: error }
      : savedAt
        ? {
            tone: "success" as const,
            message: `Saved ${new Date(savedAt).toLocaleTimeString()}`,
          }
        : {
            tone: "neutral" as const,
            message:
              "Define leader, invite order, role assignment, and completion command for each group.",
          };

  return (
    <section className={`relative z-20 overflow-hidden ${embedded ? "" : "min-w-[860px] flex-1"}`}>
      <div
        className={`flex flex-col border border-white/10 bg-void/70 backdrop-blur-md ${
          embedded ? "rounded-[1.5rem]" : "h-full"
        }`}
      >
        <header
          className={`border-b border-white/10 bg-fuchsia-500/10 ${embedded ? "px-6 py-5" : "px-8 py-6"}`}
        >
          <div className="flex items-start justify-between gap-6">
            <div>
              <div className="flex items-center gap-3">
                <div className="flex h-11 w-11 items-center justify-center border border-fuchsia-400/40 bg-fuchsia-400/10 text-fuchsia-100">
                  <UsersThree size={20} weight="fill" />
                </div>
                <div>
                  <h2 className="font-archaic text-2xl tracking-wide text-white">
                    Auto-Group Profiles
                  </h2>
                  <p className="mt-1 text-xs uppercase tracking-[0.3em] text-white/45">
                    MQ2AutoGroup parity for unattended session bring-up
                  </p>
                </div>
              </div>
              <p className="mt-5 max-w-3xl text-sm leading-6 text-white/60">
                Invite characters in a fixed order, assign group roles when the roster is full,
                and run a post-formation slash command without touching the TUI.
              </p>
            </div>

            <div className="flex items-center gap-3">
              <button
                onClick={() => {
                  void refresh();
                }}
                className="flex items-center gap-2 border border-white/15 bg-white/5 px-4 py-2 text-xs uppercase tracking-[0.2em] text-white/65 transition-colors hover:border-white/30 hover:text-white"
              >
                <ArrowsClockwise size={14} />
                Refresh
              </button>
              <button
                onClick={() => {
                  void handleSave();
                }}
                disabled={saving || loading}
                className="flex items-center gap-2 border border-fuchsia-400/40 bg-fuchsia-500/15 px-4 py-2 text-xs uppercase tracking-[0.2em] text-white transition-colors hover:bg-fuchsia-500/25 disabled:cursor-wait disabled:opacity-70"
              >
                <FloppyDisk size={14} />
                {saving ? "Saving" : "Save Profiles"}
              </button>
            </div>
          </div>
        </header>

        <div className={`${embedded ? "px-6 py-5" : "px-8 py-6"} space-y-6`}>
          <Banner tone={banner.tone} message={banner.message} />

          <div className="flex items-center justify-between">
            <div>
              <h3 className="font-archaic text-lg text-white">Profiles</h3>
              <p className="mt-1 text-xs uppercase tracking-[0.18em] text-white/35">
                Each profile maps one leader to one ordered member roster
              </p>
            </div>
            <button
              type="button"
              onClick={() =>
                setDraft((current) => ({
                  groups: [...current.groups, blankProfile(current.groups.length + 1)],
                }))
              }
              className="flex items-center gap-2 rounded-full border border-cyan-300/25 bg-cyan-300/10 px-4 py-2 text-xs uppercase tracking-[0.2em] text-cyan-100 transition hover:bg-cyan-300/20"
            >
              <Plus size={14} />
              Add Profile
            </button>
          </div>

          {draft.groups.length === 0 ? (
            <div className="rounded-3xl border border-dashed border-white/10 bg-[#0d0715] px-5 py-8 text-center text-sm text-white/45">
              No auto-group profiles yet. Add one to define a leader and invite order.
            </div>
          ) : (
            <div className="space-y-5">
              {draft.groups.map((profile, profileIndex) => (
                <section
                  key={`${profile.name}-${profileIndex}`}
                  className="rounded-3xl border border-white/10 bg-[#0d0715] p-5"
                >
                  <div className="flex flex-wrap items-start justify-between gap-4">
                    <div className="grid flex-1 gap-4 md:grid-cols-2">
                      <label className="space-y-2">
                        <span className="text-xs uppercase tracking-[0.18em] text-white/40">
                          Profile Name
                        </span>
                        <input
                          value={profile.name}
                          onChange={(event) =>
                            updateProfile(profileIndex, (current) => ({
                              ...current,
                              name: event.target.value,
                            }))
                          }
                          placeholder="Fire Team"
                          className="w-full rounded-2xl border border-white/10 bg-white/5 px-4 py-3 text-sm text-white outline-none focus:border-fuchsia-300/40"
                        />
                      </label>

                      <label className="space-y-2">
                        <span className="text-xs uppercase tracking-[0.18em] text-white/40">
                          Leader Character
                        </span>
                        <input
                          value={profile.leader_name}
                          onChange={(event) =>
                            updateProfile(profileIndex, (current) => ({
                              ...current,
                              leader_name: event.target.value,
                            }))
                          }
                          placeholder="Alpha"
                          className="w-full rounded-2xl border border-white/10 bg-white/5 px-4 py-3 text-sm text-white outline-none focus:border-fuchsia-300/40"
                        />
                      </label>
                    </div>

                    <div className="flex items-center gap-3">
                      <label className="flex items-center gap-3 rounded-full border border-white/10 bg-white/5 px-4 py-3 text-xs uppercase tracking-[0.22em] text-white/70">
                        <input
                          type="checkbox"
                          checked={profile.enabled}
                          onChange={(event) =>
                            updateProfile(profileIndex, (current) => ({
                              ...current,
                              enabled: event.target.checked,
                            }))
                          }
                          className="h-4 w-4 accent-fuchsia-400"
                        />
                        Enabled
                      </label>
                      <button
                        type="button"
                        onClick={() =>
                          setDraft((current) => ({
                            groups: current.groups.filter((_, index) => index !== profileIndex),
                          }))
                        }
                        className="flex items-center gap-2 rounded-full border border-rose-400/25 bg-rose-400/10 px-4 py-3 text-xs uppercase tracking-[0.22em] text-rose-100 transition hover:bg-rose-400/20"
                      >
                        <Trash size={14} />
                        Remove
                      </button>
                    </div>
                  </div>

                  <div className="mt-5 grid gap-4 md:grid-cols-[0.7fr_0.7fr_1fr]">
                    <label className="space-y-2">
                      <span className="text-xs uppercase tracking-[0.18em] text-white/40">
                        Retry Interval
                      </span>
                      <input
                        type="number"
                        min={1}
                        value={profile.invite_retry_interval_secs}
                        onChange={(event) =>
                          updateProfile(profileIndex, (current) => ({
                            ...current,
                            invite_retry_interval_secs: Number(event.target.value) || 1,
                          }))
                        }
                        className="w-full rounded-2xl border border-white/10 bg-white/5 px-4 py-3 text-sm text-white outline-none focus:border-fuchsia-300/40"
                      />
                    </label>

                    <label className="space-y-2">
                      <span className="text-xs uppercase tracking-[0.18em] text-white/40">
                        Max Invite Retries
                      </span>
                      <input
                        type="number"
                        min={1}
                        value={profile.max_invite_retries}
                        onChange={(event) =>
                          updateProfile(profileIndex, (current) => ({
                            ...current,
                            max_invite_retries: Number(event.target.value) || 1,
                          }))
                        }
                        className="w-full rounded-2xl border border-white/10 bg-white/5 px-4 py-3 text-sm text-white outline-none focus:border-fuchsia-300/40"
                      />
                    </label>

                    <label className="space-y-2">
                      <span className="text-xs uppercase tracking-[0.18em] text-white/40">
                        Completion Command
                      </span>
                      <input
                        value={profile.completion_command ?? ""}
                        onChange={(event) =>
                          updateProfile(profileIndex, (current) => ({
                            ...current,
                            completion_command: event.target.value,
                          }))
                        }
                        placeholder="/say group ready"
                        className="w-full rounded-2xl border border-white/10 bg-white/5 px-4 py-3 text-sm text-white outline-none focus:border-fuchsia-300/40"
                      />
                    </label>
                  </div>

                  <div className="mt-6">
                    <div className="mb-3 flex items-center justify-between">
                      <div>
                        <div className="flex items-center gap-2 text-white">
                          <Rows size={16} className="text-cyan-200" />
                          <h4 className="font-archaic text-lg">Member Order</h4>
                        </div>
                        <p className="mt-1 text-xs uppercase tracking-[0.18em] text-white/35">
                          The controller invites the first missing member, then waits before moving on
                        </p>
                      </div>
                      <button
                        type="button"
                        onClick={() =>
                          updateProfile(profileIndex, (current) => ({
                            ...current,
                            members: [...current.members, blankMember()],
                          }))
                        }
                        className="flex items-center gap-2 rounded-full border border-cyan-300/25 bg-cyan-300/10 px-4 py-2 text-xs uppercase tracking-[0.2em] text-cyan-100 transition hover:bg-cyan-300/20"
                      >
                        <Plus size={14} />
                        Add Member
                      </button>
                    </div>

                    <div className="space-y-3">
                      {profile.members.map((member, memberIndex) => (
                        <div
                          key={`${member.name}-${memberIndex}`}
                          className="grid gap-3 rounded-2xl border border-white/8 bg-white/[0.03] px-4 py-4 lg:grid-cols-[60px_1.2fr_0.9fr_auto]"
                        >
                          <div className="rounded-2xl border border-white/10 bg-white/5 px-3 py-3 text-center font-rune text-sm text-white/80">
                            {memberIndex + 1}
                          </div>

                          <label className="space-y-2">
                            <span className="text-xs uppercase tracking-[0.18em] text-white/40">
                              Character
                            </span>
                            <input
                              value={member.name}
                              onChange={(event) =>
                                updateMember(profileIndex, memberIndex, (current) => ({
                                  ...current,
                                  name: event.target.value,
                                }))
                              }
                              placeholder="Bravo"
                              className="w-full rounded-2xl border border-white/10 bg-[#120a1d] px-4 py-3 text-sm text-white outline-none focus:border-fuchsia-300/40"
                            />
                          </label>

                          <label className="space-y-2">
                            <span className="text-xs uppercase tracking-[0.18em] text-white/40">
                              Group Role
                            </span>
                            <select
                              value={member.role}
                              onChange={(event) =>
                                updateMember(profileIndex, memberIndex, (current) => ({
                                  ...current,
                                  role: event.target.value as AutoGroupRole,
                                }))
                              }
                              className="w-full rounded-2xl border border-white/10 bg-[#120a1d] px-4 py-3 text-sm text-white outline-none focus:border-fuchsia-300/40"
                            >
                              {ROLE_OPTIONS.map((option) => (
                                <option key={option.value} value={option.value}>
                                  {option.label}
                                </option>
                              ))}
                            </select>
                          </label>

                          <div className="flex items-end justify-end gap-2">
                            <button
                              type="button"
                              onClick={() => moveMember(profileIndex, memberIndex, -1)}
                              disabled={memberIndex === 0}
                              className="rounded-full border border-white/10 bg-white/5 p-3 text-white/70 transition hover:border-white/25 hover:text-white disabled:cursor-not-allowed disabled:opacity-35"
                              title="Move earlier"
                            >
                              <CaretUp size={14} />
                            </button>
                            <button
                              type="button"
                              onClick={() => moveMember(profileIndex, memberIndex, 1)}
                              disabled={memberIndex === profile.members.length - 1}
                              className="rounded-full border border-white/10 bg-white/5 p-3 text-white/70 transition hover:border-white/25 hover:text-white disabled:cursor-not-allowed disabled:opacity-35"
                              title="Move later"
                            >
                              <CaretDown size={14} />
                            </button>
                            <button
                              type="button"
                              onClick={() =>
                                updateProfile(profileIndex, (current) => ({
                                  ...current,
                                  members: current.members.filter((_, index) => index !== memberIndex),
                                }))
                              }
                              disabled={profile.members.length === 1}
                              className="rounded-full border border-rose-400/20 bg-rose-400/10 p-3 text-rose-100 transition hover:bg-rose-400/20 disabled:cursor-not-allowed disabled:opacity-35"
                              title="Remove member"
                            >
                              <Trash size={14} />
                            </button>
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>

                  <div className="mt-5 grid gap-3 sm:grid-cols-3">
                    <div className="rounded-2xl border border-white/10 bg-white/5 px-4 py-3">
                      <div className="text-[10px] uppercase tracking-[0.24em] text-white/40">
                        Leader
                      </div>
                      <div className="mt-2 flex items-center gap-2 font-rune text-sm text-white">
                        <Crown size={16} className="text-amber-200" />
                        {profile.leader_name.trim() || "Unset"}
                      </div>
                    </div>
                    <div className="rounded-2xl border border-white/10 bg-white/5 px-4 py-3">
                      <div className="text-[10px] uppercase tracking-[0.24em] text-white/40">
                        Members
                      </div>
                      <div className="mt-2 font-rune text-sm text-white">{profile.members.length}</div>
                    </div>
                    <div className="rounded-2xl border border-white/10 bg-white/5 px-4 py-3">
                      <div className="text-[10px] uppercase tracking-[0.24em] text-white/40">
                        Completion
                      </div>
                      <div className="mt-2 text-sm text-white/70">
                        {profile.completion_command?.trim() || "No follow-up command"}
                      </div>
                    </div>
                  </div>
                </section>
              ))}
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
