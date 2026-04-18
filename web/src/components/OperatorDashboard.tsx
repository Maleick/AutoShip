import { useMemo, useState, type FormEvent, type ReactNode } from "react";
import {
  ArrowsClockwise,
  Broadcast,
  Coins,
  CompassTool,
  Cpu,
  Heartbeat,
  Notepad,
  Plus,
  Pulse,
  ShieldChevron,
  ShieldWarning,
  Skull,
  Sparkle,
  Sword,
  TrendUp,
  UsersThree,
  WarningDiamond,
} from "@phosphor-icons/react";

import BoxChatPanel from "./BoxChatPanel";
import ChatLogPanel from "./ChatLogPanel";
import GmAlertPanel from "./GmAlertPanel";
import KillTrackerPanel from "./KillTrackerPanel";
import AutoGroupPanel from "./AutoGroupPanel";
import SpawnFinderPanel from "./SpawnFinderPanel";
import TradeskillTrophyPanel from "./TradeskillTrophyPanel";
import type {
  DashboardActionRequest,
  DashboardSnapshot,
  GroupCard,
  RouteCard,
  Waypoint,
} from "../dashboard";
import AutoAcceptPanel from "./AutoAcceptPanel";
import { useDashboard } from "../hooks/useDashboard";
import DiscordConfigPanel from "./DiscordConfigPanel";

function panelClasses(accent: "magenta" | "cyan" | "amber" = "magenta") {
  const accentClass =
    accent === "cyan"
      ? "border-cyan-400/20 shadow-[0_12px_40px_rgba(34,211,238,0.08)]"
      : accent === "amber"
      ? "border-amber-300/20 shadow-[0_12px_40px_rgba(251,191,36,0.08)]"
      : "border-fuchsia-400/20 shadow-[0_12px_40px_rgba(217,70,239,0.08)]";

  return `rounded-[1.5rem] border bg-[#120a1d]/88 p-5 backdrop-blur ${accentClass}`;
}

function Panel({
  title,
  subtitle,
  icon,
  accent,
  actions,
  children,
}: {
  title: string;
  subtitle: string;
  icon: ReactNode;
  accent?: "magenta" | "cyan" | "amber";
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className={panelClasses(accent)}>
      <div className="mb-4 flex items-start justify-between gap-4">
        <div className="flex items-start gap-3">
          <div className="mt-0.5 flex h-11 w-11 items-center justify-center rounded-2xl border border-white/10 bg-white/5 text-white">
            {icon}
          </div>
          <div>
            <h2 className="font-archaic text-xl text-white">{title}</h2>
            <p className="font-tech text-xs uppercase tracking-[0.28em] text-white/45">
              {subtitle}
            </p>
          </div>
        </div>
        {actions}
      </div>
      {children}
    </section>
  );
}

function StatChip({
  label,
  value,
  tone = "neutral",
}: {
  label: string;
  value: string;
  tone?: "neutral" | "good" | "warning" | "critical";
}) {
  const toneClass =
    tone === "good"
      ? "border-emerald-400/25 bg-emerald-400/10 text-emerald-200"
      : tone === "warning"
      ? "border-amber-300/25 bg-amber-300/10 text-amber-100"
      : tone === "critical"
      ? "border-rose-400/25 bg-rose-400/10 text-rose-100"
      : "border-white/10 bg-white/5 text-white";

  return (
    <div className={`rounded-2xl border px-3 py-2 ${toneClass}`}>
      <div className="font-tech text-[10px] uppercase tracking-[0.25em] text-white/50">
        {label}
      </div>
      <div className="mt-1 font-rune text-lg">{value}</div>
    </div>
  );
}

function TrendBars({
  points,
  colorClass,
}: {
  points: { label: string; value: number }[];
  colorClass: string;
}) {
  const maxValue = Math.max(...points.map((point) => point.value), 1);

  return (
    <div className="rounded-2xl border border-white/10 bg-[#0d0715] p-4">
      <div className="flex h-32 items-end gap-2">
        {points.map((point) => (
          <div key={point.label} className="flex flex-1 flex-col items-center gap-2">
            <div className="relative flex h-24 w-full items-end overflow-hidden rounded-t-2xl border border-white/5 bg-white/[0.03]">
              <div
                className={`w-full rounded-t-2xl ${colorClass}`}
                style={{ height: `${Math.max((point.value / maxValue) * 100, 12)}%` }}
              />
            </div>
            <div className="font-rune text-[10px] uppercase tracking-[0.2em] text-white/40">
              {point.label}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function PolylineChart({
  series,
}: {
  series: DashboardSnapshot["combat"]["dpsSeries"];
}) {
  const maxValue = Math.max(...series.flatMap((entry) => entry.samples), 1);

  return (
    <div className="rounded-2xl border border-white/10 bg-[#0d0715] p-4">
      <svg viewBox="0 0 240 112" className="h-36 w-full">
        <path
          d="M16 12 V96 H224"
          fill="none"
          stroke="rgba(255,255,255,0.15)"
          strokeWidth="1"
        />
        {series.map((entry, seriesIndex) => {
          const step = entry.samples.length > 1 ? 180 / (entry.samples.length - 1) : 0;
          const points = entry.samples
            .map((sample, index) => {
              const x = 28 + index * step;
              const y = 96 - (sample / maxValue) * 72 - seriesIndex * 2;
              return `${x},${y}`;
            })
            .join(" ");

          return (
            <polyline
              key={entry.characterName}
              fill="none"
              points={points}
              stroke={entry.color}
              strokeWidth="3"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          );
        })}
      </svg>
      <div className="mt-3 flex flex-wrap gap-2">
        {series.map((entry) => (
          <div
            key={entry.characterName}
            className="rounded-full border border-white/10 px-3 py-1 font-rune text-xs text-white/80"
          >
            <span
              className="mr-2 inline-block h-2 w-2 rounded-full align-middle"
              style={{ backgroundColor: entry.color }}
            />
            {entry.characterName}
          </div>
        ))}
      </div>
    </div>
  );
}

function WaypointMap({ route }: { route: RouteCard | undefined }) {
  if (!route) {
    return (
      <div className="rounded-2xl border border-dashed border-white/10 bg-[#0d0715] p-4 text-sm text-white/45">
        Select or create a route to preview its waypoint map.
      </div>
    );
  }

  return (
    <div className="rounded-2xl border border-white/10 bg-[#0d0715] p-4">
      <svg viewBox="0 0 100 100" className="h-56 w-full rounded-2xl bg-[radial-gradient(circle_at_top,#16253d,#08050f_68%)]">
        <defs>
          <pattern
            id="grid"
            width="10"
            height="10"
            patternUnits="userSpaceOnUse"
          >
            <path
              d="M 10 0 L 0 0 0 10"
              fill="none"
              stroke="rgba(255,255,255,0.08)"
              strokeWidth="0.4"
            />
          </pattern>
        </defs>
        <rect width="100" height="100" fill="url(#grid)" />
        <polyline
          fill="none"
          stroke="#7dd3fc"
          strokeWidth="1.5"
          points={route.waypoints.map((point) => `${point.x},${100 - point.y}`).join(" ")}
        />
        {route.waypoints.map((point) => (
          <g key={point.id}>
            <circle cx={point.x} cy={100 - point.y} r="2.8" fill="#f0abfc" />
            <text
              x={point.x + 3}
              y={100 - point.y - 3}
              fill="#e5e7eb"
              fontSize="4"
            >
              {point.label}
            </text>
          </g>
        ))}
      </svg>
      <div className="mt-3 flex items-center justify-between gap-3 text-sm text-white/65">
        <span>{route.zone}</span>
        <span>{route.destination}</span>
      </div>
    </div>
  );
}

function severityTone(severity: "info" | "warning" | "critical") {
  if (severity === "critical") {
    return "border-rose-400/30 bg-rose-400/10 text-rose-100";
  }
  if (severity === "warning") {
    return "border-amber-300/30 bg-amber-300/10 text-amber-100";
  }
  return "border-cyan-300/30 bg-cyan-300/10 text-cyan-100";
}

function statusTone(status: "online" | "offline" | "stuck" | "healthy" | "warning" | "critical") {
  if (status === "online" || status === "healthy") {
    return "border-emerald-400/25 bg-emerald-400/10 text-emerald-200";
  }
  if (status === "stuck" || status === "warning") {
    return "border-amber-300/25 bg-amber-300/10 text-amber-100";
  }
  return "border-rose-400/25 bg-rose-400/10 text-rose-100";
}

function automationTone(mode: "automatic" | "paused" | "camp" | "chase" | "manual") {
  if (mode === "automatic" || mode === "chase") {
    return "border-emerald-400/25 bg-emerald-400/10 text-emerald-200";
  }
  if (mode === "paused" || mode === "manual") {
    return "border-amber-300/25 bg-amber-300/10 text-amber-100";
  }
  return "border-cyan-300/25 bg-cyan-300/10 text-cyan-100";
}

function parseWaypoints(raw: string): Waypoint[] {
  return raw
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line, index) => {
      const [labelPart, coordsPart] = line.includes(":")
        ? line.split(":", 2)
        : [`WP ${index + 1}`, line];
      const [xText, yText] = coordsPart.split(",", 2);
      const x = Number(xText?.trim() ?? "");
      const y = Number(yText?.trim() ?? "50");

      return {
        id: `wp-${index + 1}`,
        label: labelPart.trim(),
        x: Number.isFinite(x) ? x : 50,
        y: Number.isFinite(y) ? y : 50,
      };
    });
}

function titleCase(raw: string) {
  return raw.replaceAll("_", " ").replace(/\b\w/g, (char) => char.toUpperCase());
}

function relocationSourceLabel(source: "aa" | "item" | null) {
  if (source === "aa") {
    return "AA";
  }
  if (source === "item") {
    return "Item";
  }
  return "Unknown";
}

function relocationStatusLabel(
  option: DashboardSnapshot["relocation"]["destinations"][number]["options"][number]
) {
  if (option.ready) {
    return "Ready";
  }
  if (typeof option.cooldownRemainingSecs === "number") {
    return `${option.cooldownRemainingSecs}s cooldown`;
  }
  return "Unavailable";
}

export default function OperatorDashboard() {
  const { snapshot, loading, error, connected, refresh, submitAction } = useDashboard();
  const [boxChatOpen, setBoxChatOpen] = useState(false);
  const [chatLogOpen, setChatLogOpen] = useState(false);
  const [gmAlertOpen, setGmAlertOpen] = useState(false);
  const [sessionWizardOpen, setSessionWizardOpen] = useState(false);
  const [sessionProfile, setSessionProfile] = useState("");
  const [sessionCharacterName, setSessionCharacterName] = useState("");
  const [groupFormOpen, setGroupFormOpen] = useState(false);
  const [groupName, setGroupName] = useState("");
  const [groupZone, setGroupZone] = useState("");
  const [groupFormation, setGroupFormation] = useState("");
  const [routeFormOpen, setRouteFormOpen] = useState(false);
  const [routeName, setRouteName] = useState("");
  const [routeZone, setRouteZone] = useState("");
  const [routeDestination, setRouteDestination] = useState("");
  const [routeWaypoints, setRouteWaypoints] = useState("Camp:10,20\nRidge:64,48\nRing:88,20");
  const [wishlistDraft, setWishlistDraft] = useState("");
  const [pendingError, setPendingError] = useState<string | null>(null);

  const activeRoute = useMemo(
    () =>
      snapshot?.navigation.routes.find(
        (route) => route.id === snapshot.navigation.activeRouteId
      ),
    [snapshot]
  );

  if (loading && !snapshot) {
    return (
      <div className="flex h-screen items-center justify-center bg-[#090611] px-6 text-center text-white">
        <div className={panelClasses("cyan")}>
          <Pulse size={28} className="mx-auto text-cyan-300" />
          <h1 className="mt-4 font-archaic text-2xl">Booting Operator Console</h1>
          <p className="mt-2 max-w-md text-sm text-white/60">
            Fetching session topology, live metrics, and command surfaces from the
            Axum dashboard backend.
          </p>
        </div>
      </div>
    );
  }

  if (!snapshot) {
    return (
      <div className="flex h-screen items-center justify-center bg-[#090611] px-6 text-center text-white">
        <div className={panelClasses("amber")}>
          <WarningDiamond size={28} className="mx-auto text-amber-200" />
          <h1 className="mt-4 font-archaic text-2xl">Dashboard Unavailable</h1>
          <p className="mt-2 max-w-md text-sm text-white/60">
            {error ?? "The dashboard snapshot is not available yet."}
          </p>
          <button
            type="button"
            onClick={() => void refresh()}
            className="mt-4 rounded-full border border-white/10 bg-white/5 px-4 py-2 text-sm text-white transition hover:border-white/30 hover:bg-white/10"
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  const dashboard = snapshot;
  const wsHealthy = connected;

  async function runAction(action: DashboardActionRequest) {
    setPendingError(null);
    try {
      await submitAction(action);
    } catch (nextError) {
      setPendingError(
        nextError instanceof Error ? nextError.message : "Dashboard action failed"
      );
    }
  }

  async function handleCreateSession(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const characterName = sessionCharacterName.trim();
    if (!characterName) {
      setPendingError("Character name is required");
      return;
    }
    await runAction({
      type: "create_session",
      profile: sessionProfile,
      character_name: characterName,
    });
    setSessionCharacterName("");
    setSessionWizardOpen(false);
  }

  async function handleCreateGroup(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await runAction({
      type: "create_group",
      name: groupName.trim(),
      zone: groupZone.trim(),
      formation: groupFormation.trim(),
    });
    setGroupName("");
    setGroupZone("");
    setGroupFormation("");
    setGroupFormOpen(false);
  }

  async function handleCreateRoute(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await runAction({
      type: "create_route",
      name: routeName.trim(),
      zone: routeZone.trim(),
      destination: routeDestination.trim(),
      waypoints: parseWaypoints(routeWaypoints),
    });
    setRouteName("");
    setRouteZone("");
    setRouteDestination("");
    setRouteFormOpen(false);
  }

  async function handleWishlistAdd() {
    const nextItem = wishlistDraft.trim();
    if (!nextItem || dashboard.economy.wishlist.includes(nextItem)) {
      return;
    }

    await runAction({
      type: "update_wishlist",
      items: [...dashboard.economy.wishlist, nextItem],
    });
    setWishlistDraft("");
  }

  async function updateGroupMemberRole(group: GroupCard, memberName: string, role: string) {
    const nextMembers = group.members.map((member) =>
      member.characterName === memberName ? { ...member, role } : member
    );

    await runAction({
      type: "update_group",
      group_id: group.id,
      formation: group.formation,
      members: nextMembers,
    });
  }

  async function issueAutomationCommand(
    command: "pause" | "unpause" | "camp" | "chase"
  ) {
    await runAction({
      type: "issue_automation_command",
      command: { type: command },
    });
  }

  return (
    <div className="h-screen overflow-auto bg-[#090611] text-white">
      <main className="mx-auto flex min-h-screen max-w-[1680px] flex-col gap-6 px-4 py-5 lg:px-6">
        <header className="rounded-[1.8rem] border border-white/10 bg-[linear-gradient(135deg,rgba(19,11,35,0.96),rgba(9,7,19,0.92))] px-5 py-5 shadow-[0_16px_60px_rgba(0,0,0,0.32)]">
          <div className="flex flex-col gap-5 xl:flex-row xl:items-end xl:justify-between">
            <div className="max-w-3xl">
              <div className="mb-3 flex items-center gap-3">
                <span className="rounded-full border border-cyan-300/20 bg-cyan-300/10 px-3 py-1 font-tech text-[11px] uppercase tracking-[0.34em] text-cyan-100">
                  TextQuest Command Deck
                </span>
                <span
                  className={`rounded-full border px-3 py-1 font-tech text-[11px] uppercase tracking-[0.34em] ${
                    wsHealthy
                      ? "border-emerald-400/30 bg-emerald-400/10 text-emerald-100"
                      : "border-amber-300/30 bg-amber-300/10 text-amber-100"
                  }`}
                >
                  {wsHealthy ? "Live Telemetry" : "Reconnecting"}
                </span>
              </div>
              <h1 className="font-archaic text-3xl text-white md:text-4xl">
                Multi-Session Operations for {snapshot.environment.cluster}
              </h1>
              <p className="mt-3 max-w-2xl text-sm leading-6 text-white/65">
                Unified control for sessions, group roles, navigation, economy
                loops, combat output, and client health. The console is tuned for
                live intervention, not passive viewing.
              </p>
            </div>

            <div className="flex flex-col gap-3 xl:items-end">
              <div className="grid gap-3 sm:grid-cols-4">
                <StatChip label="Shard" value={snapshot.environment.shard} />
                <StatChip
                  label="Zone"
                  value={snapshot.environment.zone}
                  tone="good"
                />
                <StatChip
                  label="Alerts"
                  value={String(snapshot.environment.alerts)}
                  tone={snapshot.environment.alerts > 2 ? "critical" : "warning"}
                />
                <StatChip
                  label="Updated"
                  value={new Date(snapshot.generatedAt).toLocaleTimeString()}
                />
              </div>
              <button
                type="button"
                onClick={() => setChatLogOpen((current) => !current)}
                className={`inline-flex items-center gap-2 self-start rounded-full border px-4 py-2 text-sm transition xl:self-end ${
                  chatLogOpen
                    ? "border-fuchsia-300/30 bg-fuchsia-300/10 text-fuchsia-100"
                    : "border-white/10 bg-white/5 text-white/70 hover:border-white/25 hover:bg-white/10"
                }`}
              >
                <Notepad size={16} />
                {chatLogOpen ? "Hide Chat Logging" : "Chat Logging"}
              </button>
              <button
                type="button"
                onClick={() => setBoxChatOpen((current) => !current)}
                className={`inline-flex items-center gap-2 self-start rounded-full border px-4 py-2 text-sm transition xl:self-end ${
                  boxChatOpen
                    ? "border-cyan-300/30 bg-cyan-300/10 text-cyan-100"
                    : "border-white/10 bg-white/5 text-white/70 hover:border-white/25 hover:bg-white/10"
                }`}
              >
                <Broadcast size={16} />
                {boxChatOpen ? "Hide Network Box Chat" : "Network Box Chat"}
              </button>
              <button
                type="button"
                onClick={() => setGmAlertOpen((current) => !current)}
                className={`inline-flex items-center gap-2 self-start rounded-full border px-4 py-2 text-sm transition xl:self-end ${
                  gmAlertOpen
                    ? "border-rose-400/30 bg-rose-400/10 text-rose-100"
                    : "border-white/10 bg-white/5 text-white/70 hover:border-white/25 hover:bg-white/10"
                }`}
              >
                <ShieldWarning size={16} />
                {gmAlertOpen ? "Hide GM Alerts" : "GM Alerts"}
              </button>
            </div>
          </div>

          {(error || pendingError) && (
            <div className="mt-4 rounded-2xl border border-rose-400/30 bg-rose-400/10 px-4 py-3 text-sm text-rose-100">
              {pendingError ?? error}
            </div>
          )}
        </header>

        {chatLogOpen && <ChatLogPanel />}
        {boxChatOpen && <BoxChatPanel />}
        {gmAlertOpen && <GmAlertPanel />}

        <div className="grid gap-6 xl:grid-cols-[1.2fr_0.9fr]">
          <div className="grid gap-6">
            <Panel
              title="Session Command Center"
              subtitle="Live client status, creation, recovery, and termination"
              icon={<ShieldChevron size={20} />}
              actions={
                <div className="flex flex-wrap gap-2">
                  <button
                    type="button"
                    onClick={() => {
                      setSessionProfile(snapshot.sessions.profiles[0] ?? "");
                      setSessionWizardOpen((current) => !current);
                    }}
                    className="rounded-full border border-fuchsia-400/25 bg-fuchsia-400/10 px-4 py-2 text-sm text-fuchsia-100 transition hover:bg-fuchsia-400/20"
                  >
                    New Session
                  </button>
                  <button
                    type="button"
                    onClick={() => void refresh()}
                    className="rounded-full border border-white/10 bg-white/5 px-4 py-2 text-sm text-white/70 transition hover:border-white/25 hover:bg-white/10"
                  >
                    <span className="inline-flex items-center gap-2">
                      <ArrowsClockwise size={16} />
                      Refresh
                    </span>
                  </button>
                </div>
              }
            >
              <div className="grid gap-4 lg:grid-cols-[0.95fr_1.05fr]">
                <div className="grid gap-3 sm:grid-cols-3">
                  <StatChip
                    label="Online"
                    value={String(
                      snapshot.sessions.items.filter((session) => session.status === "online")
                        .length
                    )}
                    tone="good"
                  />
                  <StatChip
                    label="Stuck"
                    value={String(
                      snapshot.sessions.items.filter((session) => session.status === "stuck")
                        .length
                    )}
                    tone="warning"
                  />
                  <StatChip
                    label="Auto Recovery"
                    value={snapshot.sessions.recoveryEnabled ? "Enabled" : "Off"}
                    tone={snapshot.sessions.recoveryEnabled ? "good" : "critical"}
                  />
                </div>

                {sessionWizardOpen && (
                  <form
                    className="grid gap-3 rounded-3xl border border-white/10 bg-[#0d0715] p-4"
                    onSubmit={(event) => void handleCreateSession(event)}
                  >
                    <div className="grid gap-1">
                      <label htmlFor="session-profile" className="text-sm text-white/75">
                        Profile
                      </label>
                      <select
                        id="session-profile"
                        value={sessionProfile}
                        onChange={(event) => setSessionProfile(event.target.value)}
                        className="rounded-2xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-cyan-300/35"
                      >
                        {snapshot.sessions.profiles.map((profile) => (
                          <option key={profile} value={profile} className="bg-[#120a1d]">
                            {profile}
                          </option>
                        ))}
                      </select>
                    </div>
                    <div className="grid gap-1">
                      <label htmlFor="session-character-name" className="text-sm text-white/75">
                        Character Name
                      </label>
                      <input
                        id="session-character-name"
                        required
                        value={sessionCharacterName}
                        onChange={(event) => setSessionCharacterName(event.target.value)}
                        className="rounded-2xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-cyan-300/35"
                      />
                    </div>
                    <div className="flex justify-end gap-2">
                      <button
                        type="button"
                        onClick={() => setSessionWizardOpen(false)}
                        className="rounded-full border border-white/10 px-4 py-2 text-sm text-white/65 transition hover:border-white/30 hover:bg-white/5"
                      >
                        Cancel
                      </button>
                      <button
                        type="submit"
                        className="rounded-full border border-cyan-300/25 bg-cyan-300/10 px-4 py-2 text-sm text-cyan-100 transition hover:bg-cyan-300/20"
                      >
                        Create Session
                      </button>
                    </div>
                  </form>
                )}
              </div>

              <div className="mt-4 space-y-3">
                {snapshot.sessions.items.map((session) => (
                  <div
                    key={session.clientId}
                    className="rounded-3xl border border-white/10 bg-[#0d0715] p-4"
                  >
                    <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
                      <div>
                        <div className="flex items-center gap-2">
                          <h3 className="font-archaic text-xl text-white">
                            {session.characterName}
                          </h3>
                          <span
                            className={`rounded-full border px-3 py-1 font-tech text-[11px] uppercase tracking-[0.3em] ${statusTone(
                              session.status
                            )}`}
                          >
                            {titleCase(session.status)}
                          </span>
                          <span className="rounded-full border border-white/10 bg-white/5 px-3 py-1 font-rune text-xs text-white/60">
                            {titleCase(session.recoveryState)}
                          </span>
                        </div>
                        <p className="mt-1 font-tech text-xs uppercase tracking-[0.28em] text-white/40">
                          {session.profile} · {session.zone} · Group {session.groupId.replace("grp-", "")}
                        </p>
                      </div>

                      <div className="flex flex-wrap gap-2">
                        <button
                          type="button"
                          onClick={() =>
                            void runAction({
                              type: "recover_session",
                              client_id: session.clientId,
                            })
                          }
                          className="rounded-full border border-cyan-300/25 bg-cyan-300/10 px-3 py-2 text-sm text-cyan-100 transition hover:bg-cyan-300/20"
                        >
                          Recover
                        </button>
                        <button
                          type="button"
                          onClick={() =>
                            void runAction({
                              type: "terminate_session",
                              client_id: session.clientId,
                            })
                          }
                          className="rounded-full border border-rose-400/25 bg-rose-400/10 px-3 py-2 text-sm text-rose-100 transition hover:bg-rose-400/20"
                        >
                          Terminate
                        </button>
                      </div>
                    </div>

                    <div className="mt-4 grid gap-3 sm:grid-cols-4">
                      <StatChip label="Level" value={String(session.level)} />
                      <StatChip label="HP" value={`${session.hpPct}%`} tone={session.hpPct < 50 ? "warning" : "good"} />
                      <StatChip label="Mana" value={`${session.manaPct}%`} tone={session.manaPct < 35 ? "warning" : "neutral"} />
                      <StatChip label="Heartbeat" value={session.lastHeartbeat} />
                    </div>
                  </div>
                ))}
              </div>
            </Panel>

            <Panel
              title="Unified Box Control"
              subtitle="Global commands with per-client automation state"
              icon={<Broadcast size={20} />}
              accent="amber"
              actions={
                <div className="flex flex-wrap gap-2">
                  <button
                    type="button"
                    onClick={() => void issueAutomationCommand("pause")}
                    className="rounded-full border border-amber-300/25 bg-amber-300/10 px-4 py-2 text-sm text-amber-100 transition hover:bg-amber-300/20"
                  >
                    Pause All
                  </button>
                  <button
                    type="button"
                    onClick={() => void issueAutomationCommand("unpause")}
                    className="rounded-full border border-emerald-400/25 bg-emerald-400/10 px-4 py-2 text-sm text-emerald-100 transition hover:bg-emerald-400/20"
                  >
                    Unpause All
                  </button>
                  <button
                    type="button"
                    onClick={() => void issueAutomationCommand("camp")}
                    className="rounded-full border border-cyan-300/25 bg-cyan-300/10 px-4 py-2 text-sm text-cyan-100 transition hover:bg-cyan-300/20"
                  >
                    Camp All
                  </button>
                  <button
                    type="button"
                    onClick={() => void issueAutomationCommand("chase")}
                    className="rounded-full border border-fuchsia-400/25 bg-fuchsia-400/10 px-4 py-2 text-sm text-fuchsia-100 transition hover:bg-fuchsia-400/20"
                  >
                    Chase All
                  </button>
                </div>
              }
            >
              <div className="grid gap-3 sm:grid-cols-3">
                <StatChip
                  label="Connected"
                  value={String(snapshot.automation.connectedClients)}
                  tone={snapshot.automation.connectedClients > 0 ? "good" : "warning"}
                />
                <StatChip
                  label="Relay"
                  value={snapshot.automation.relayEnabled ? "Online" : "Offline"}
                  tone={snapshot.automation.relayEnabled ? "good" : "critical"}
                />
                <StatChip
                  label="Last Command"
                  value={
                    snapshot.automation.lastCommand
                      ? titleCase(snapshot.automation.lastCommand.type)
                      : "Idle"
                  }
                  tone="neutral"
                />
              </div>

              <div className="mt-4 space-y-3">
                {snapshot.automation.clients.length === 0 ? (
                  <div className="rounded-3xl border border-dashed border-white/10 bg-[#0d0715] p-4 text-sm text-white/45">
                    No automation clients have published controller state yet.
                  </div>
                ) : (
                  snapshot.automation.clients.map((client) => (
                    <div
                      key={`${client.nodeName}:${client.characterName}`}
                      className="rounded-3xl border border-white/10 bg-[#0d0715] p-4"
                    >
                      <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
                        <div>
                          <div className="flex items-center gap-2">
                            <h3 className="font-archaic text-xl text-white">
                              {client.characterName}
                            </h3>
                            <span
                              className={`rounded-full border px-3 py-1 font-tech text-[11px] uppercase tracking-[0.3em] ${automationTone(
                                client.mode
                              )}`}
                            >
                              {titleCase(client.mode)}
                            </span>
                          </div>
                          <p className="mt-1 font-tech text-xs uppercase tracking-[0.28em] text-white/40">
                            {client.nodeName}
                          </p>
                        </div>

                        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
                          <StatChip
                            label="Burn Requests"
                            value={String(client.burnRequests)}
                            tone={client.burnRequests > 0 ? "warning" : "neutral"}
                          />
                          <StatChip
                            label="Raid Assist"
                            value={
                              client.raidAssistNum === null
                                ? "Unset"
                                : String(client.raidAssistNum)
                            }
                          />
                          <StatChip label="Node" value={client.nodeName} />
                        </div>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </Panel>

            <Panel
              title="Group Coordination"
              subtitle="Role assignment, formation state, and command dispatch"
              icon={<UsersThree size={20} />}
              accent="cyan"
              actions={
                <button
                  type="button"
                  onClick={() => setGroupFormOpen((current) => !current)}
                  className="rounded-full border border-cyan-300/25 bg-cyan-300/10 px-4 py-2 text-sm text-cyan-100 transition hover:bg-cyan-300/20"
                >
                  New Group
                </button>
              }
            >
              {groupFormOpen && (
                <form
                  className="mb-4 grid gap-3 rounded-3xl border border-white/10 bg-[#0d0715] p-4 md:grid-cols-3"
                  onSubmit={(event) => void handleCreateGroup(event)}
                >
                  <input
                    aria-label="Group name"
                    value={groupName}
                    onChange={(event) => setGroupName(event.target.value)}
                    placeholder="Group name"
                    className="rounded-2xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-cyan-300/35"
                  />
                  <input
                    aria-label="Group zone"
                    value={groupZone}
                    onChange={(event) => setGroupZone(event.target.value)}
                    placeholder="Zone"
                    className="rounded-2xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-cyan-300/35"
                  />
                  <div className="flex gap-2">
                    <input
                      aria-label="Group formation"
                      value={groupFormation}
                      onChange={(event) => setGroupFormation(event.target.value)}
                      placeholder="Formation"
                      className="flex-1 rounded-2xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-cyan-300/35"
                    />
                    <button
                      type="submit"
                      className="rounded-full border border-cyan-300/25 bg-cyan-300/10 px-4 py-2 text-sm text-cyan-100 transition hover:bg-cyan-300/20"
                    >
                      Save
                    </button>
                  </div>
                </form>
              )}

              <div className="space-y-4">
                {snapshot.groups.items.map((group) => (
                  <div
                    key={group.id}
                    className="rounded-3xl border border-white/10 bg-[#0d0715] p-4"
                  >
                    <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                      <div>
                        <h3 className="font-archaic text-xl text-white">{group.name}</h3>
                        <p className="mt-1 font-tech text-xs uppercase tracking-[0.28em] text-white/45">
                          {group.zone} · {group.formation} · {titleCase(group.currentCommand)}
                        </p>
                      </div>

                      <div className="flex flex-wrap gap-2">
                        {["camp", "pull", "navigate"].map((command) => (
                          <button
                            key={command}
                            type="button"
                            onClick={() =>
                              void runAction({
                                type: "issue_group_command",
                                group_id: group.id,
                                command,
                              })
                            }
                            className={`rounded-full border px-3 py-2 text-sm transition ${
                              group.currentCommand === command
                                ? "border-fuchsia-400/25 bg-fuchsia-400/10 text-fuchsia-100"
                                : "border-white/10 bg-white/5 text-white/70 hover:border-white/25 hover:bg-white/10"
                            }`}
                          >
                            {titleCase(command)}
                          </button>
                        ))}
                      </div>
                    </div>

                    <div className="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                      {group.members.map((member) => (
                        <div
                          key={`${group.id}-${member.characterName}`}
                          className="rounded-2xl border border-white/10 bg-white/[0.03] p-3"
                        >
                          <div className="font-rune text-sm text-white">
                            {member.characterName}
                          </div>
                          <p className="mt-1 text-xs uppercase tracking-[0.28em] text-white/40">
                            {member.status}
                          </p>
                          <label className="mt-3 block text-xs uppercase tracking-[0.28em] text-white/35">
                            Role
                            <select
                              aria-label={`${member.characterName} role`}
                              value={member.role}
                              onChange={(event) =>
                                void updateGroupMemberRole(
                                  group,
                                  member.characterName,
                                  event.target.value
                                )
                              }
                              className="mt-2 w-full rounded-2xl border border-white/10 bg-[#0d0715] px-3 py-2 text-sm text-white outline-none focus:border-fuchsia-300/35"
                            >
                              {["tank", "healer", "dps", "puller", "support", "cc"].map(
                                (role) => (
                                  <option key={role} value={role} className="bg-[#120a1d]">
                                    {titleCase(role)}
                                  </option>
                                )
                              )}
                            </select>
                          </label>
                        </div>
                      ))}
                    </div>
                  </div>
                ))}
              </div>

              <div className="mt-4 rounded-3xl border border-white/10 bg-[#0d0715] p-4">
                <div className="mb-3 flex items-center gap-2 text-white">
                  <Sparkle size={18} className="text-fuchsia-200" />
                  <h3 className="font-archaic text-lg">Recent Group Commands</h3>
                </div>
                <div className="space-y-2">
                  {snapshot.groups.commandLog.slice(0, 4).map((entry) => (
                    <div
                      key={entry.id}
                      className="flex flex-wrap items-center justify-between gap-2 rounded-2xl border border-white/8 bg-white/[0.03] px-3 py-2 text-sm"
                    >
                      <span className="font-rune text-white/80">
                        {entry.issuedAt} · {entry.groupName}
                      </span>
                      <span className="text-white/65">{titleCase(entry.command)}</span>
                      <span className="rounded-full border border-white/10 bg-white/5 px-3 py-1 text-xs uppercase tracking-[0.24em] text-white/55">
                        {titleCase(entry.status)}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            </Panel>

            <Panel
              title="Navigation Control"
              subtitle="Routes, waypoint map preview, and stuck monitoring"
              icon={<CompassTool size={20} />}
              accent="amber"
              actions={
                <button
                  type="button"
                  onClick={() => setRouteFormOpen((current) => !current)}
                  className="rounded-full border border-amber-300/25 bg-amber-300/10 px-4 py-2 text-sm text-amber-100 transition hover:bg-amber-300/20"
                >
                  Create Route
                </button>
              }
            >
              <div className="grid gap-4 xl:grid-cols-[0.95fr_1.05fr]">
                <div className="space-y-4">
                  <div className="grid gap-3 sm:grid-cols-3">
                    <StatChip label="Zone" value={snapshot.navigation.currentZone} />
                    <StatChip
                      label="Active Route"
                      value={activeRoute?.name ?? "None"}
                      tone="good"
                    />
                    <StatChip
                      label="Stuck Clients"
                      value={String(snapshot.navigation.stuckClients)}
                      tone={snapshot.navigation.stuckClients > 0 ? "warning" : "good"}
                    />
                  </div>

                  {routeFormOpen && (
                    <form
                      className="grid gap-3 rounded-3xl border border-white/10 bg-[#0d0715] p-4"
                      onSubmit={(event) => void handleCreateRoute(event)}
                    >
                      <div className="grid gap-3 md:grid-cols-3">
                        <input
                          aria-label="Route name"
                          value={routeName}
                          onChange={(event) => setRouteName(event.target.value)}
                          placeholder="Route name"
                          className="rounded-2xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-amber-300/35"
                        />
                        <input
                          aria-label="Route zone"
                          value={routeZone}
                          onChange={(event) => setRouteZone(event.target.value)}
                          placeholder="Zone"
                          className="rounded-2xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-amber-300/35"
                        />
                        <input
                          aria-label="Route destination"
                          value={routeDestination}
                          onChange={(event) => setRouteDestination(event.target.value)}
                          placeholder="Destination"
                          className="rounded-2xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-amber-300/35"
                        />
                      </div>
                      <label className="text-sm text-white/75">
                        Waypoints
                        <textarea
                          aria-label="Route waypoints"
                          value={routeWaypoints}
                          onChange={(event) => setRouteWaypoints(event.target.value)}
                          rows={4}
                          className="mt-2 w-full rounded-2xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-amber-300/35"
                        />
                      </label>
                      <div className="flex justify-end">
                        <button
                          type="submit"
                          className="rounded-full border border-amber-300/25 bg-amber-300/10 px-4 py-2 text-sm text-amber-100 transition hover:bg-amber-300/20"
                        >
                          Save Route
                        </button>
                      </div>
                    </form>
                  )}

                  <div className="space-y-3">
                    {snapshot.navigation.routes.map((route) => (
                      <button
                        key={route.id}
                        type="button"
                        onClick={() =>
                          void runAction({
                            type: "set_active_route",
                            route_id: route.id,
                          })
                        }
                        className={`flex w-full items-center justify-between rounded-3xl border px-4 py-3 text-left transition ${
                          route.id === snapshot.navigation.activeRouteId
                            ? "border-amber-300/30 bg-amber-300/10"
                            : "border-white/10 bg-[#0d0715] hover:border-white/25 hover:bg-white/5"
                        }`}
                      >
                        <div>
                          <div className="font-archaic text-lg text-white">{route.name}</div>
                          <p className="font-tech text-xs uppercase tracking-[0.24em] text-white/45">
                            {route.zone} · {route.destination}
                          </p>
                        </div>
                        <div className="min-w-28">
                          <div className="mb-2 flex items-center justify-between text-xs text-white/45">
                            <span>Progress</span>
                            <span>{route.progressPct}%</span>
                          </div>
                          <div className="h-2 overflow-hidden rounded-full bg-white/5">
                            <div
                              className="h-full rounded-full bg-gradient-to-r from-amber-300 to-orange-400"
                              style={{ width: `${route.progressPct}%` }}
                            />
                          </div>
                        </div>
                      </button>
                    ))}
                  </div>
                </div>

                <WaypointMap route={activeRoute} />
              </div>
            </Panel>
          </div>

          <div className="grid gap-6">
            <AutoAcceptPanel embedded />
            <TradeskillTrophyPanel embedded />
            <SpawnFinderPanel
              spawnFinder={snapshot.spawnFinder}
              submitAction={submitAction}
            />

            <Panel
              title="Relocation Network"
              subtitle="AA and clicky travel coverage with cooldown visibility"
              icon={<Sparkle size={20} />}
              accent="cyan"
            >
              <div className="grid gap-3 sm:grid-cols-3">
                <StatChip
                  label="Ready Destinations"
                  value={String(snapshot.relocation.readyDestinations)}
                  tone="good"
                />
                <StatChip
                  label="Cooling Down"
                  value={String(snapshot.relocation.coolingDownCount)}
                  tone={snapshot.relocation.coolingDownCount > 0 ? "warning" : "good"}
                />
                <StatChip
                  label="Catalog"
                  value={String(snapshot.relocation.destinations.length)}
                />
              </div>

              <div className="mt-4 space-y-3">
                {snapshot.relocation.destinations.map((destination) => (
                  <div
                    key={destination.zone}
                    className="rounded-3xl border border-white/10 bg-[#0d0715] p-4"
                  >
                    <div className="flex flex-wrap items-start justify-between gap-3">
                      <div>
                        <div className="font-archaic text-lg text-white">
                          {destination.label}
                        </div>
                        <p className="font-tech text-xs uppercase tracking-[0.24em] text-white/45">
                          {destination.zone}
                        </p>
                      </div>
                      <div className="text-right text-sm text-white/70">
                        <div className="font-rune text-white">
                          {destination.preferredOption ?? "No preferred option"}
                        </div>
                        <div className="font-tech text-[10px] uppercase tracking-[0.24em] text-cyan-100/80">
                          Preferred {relocationSourceLabel(destination.preferredSource)}
                        </div>
                      </div>
                    </div>

                    <div className="mt-3 grid gap-2">
                      {destination.options.map((option) => (
                        <div
                          key={option.id}
                          className="flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-white/8 bg-white/[0.03] px-3 py-3"
                        >
                          <div>
                            <div className="font-rune text-white/90">{option.name}</div>
                            <div className="mt-1 flex flex-wrap gap-2">
                              <span className="rounded-full border border-cyan-300/20 bg-cyan-300/10 px-2 py-1 font-tech text-[10px] uppercase tracking-[0.2em] text-cyan-100">
                                {relocationSourceLabel(option.source)}
                              </span>
                              {!option.owned && (
                                <span className="rounded-full border border-rose-400/20 bg-rose-400/10 px-2 py-1 font-tech text-[10px] uppercase tracking-[0.2em] text-rose-100">
                                  Missing
                                </span>
                              )}
                            </div>
                          </div>
                          <span
                            className={`rounded-full border px-3 py-1 text-xs uppercase tracking-[0.24em] ${
                              option.ready
                                ? "border-emerald-400/25 bg-emerald-400/10 text-emerald-100"
                                : "border-amber-300/25 bg-amber-300/10 text-amber-100"
                            }`}
                          >
                            {relocationStatusLabel(option)}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </Panel>

            <Panel
              title="Economy Monitoring"
              subtitle="Loot intake, vendor cadence, profit trend, and wishlist"
              icon={<Coins size={20} />}
            >
              <div className="grid gap-3 sm:grid-cols-3">
                <StatChip
                  label="Items Received"
                  value={String(snapshot.economy.itemsReceived)}
                  tone="good"
                />
                <StatChip label="Vendor Cycle" value={snapshot.economy.lastVendorRun} />
                <StatChip
                  label="Profit"
                  value={`${snapshot.economy.totalProfit.toLocaleString()} pp`}
                  tone="good"
                />
              </div>

              <div className="mt-4">
                <TrendBars
                  points={snapshot.economy.profitTrend}
                  colorClass="bg-gradient-to-t from-fuchsia-500 to-cyan-300"
                />
              </div>

              <div className="mt-4 grid gap-4 lg:grid-cols-[1fr_0.95fr]">
                <div className="rounded-3xl border border-white/10 bg-[#0d0715] p-4">
                  <div className="mb-3 flex items-center gap-2 text-white">
                    <TrendUp size={18} className="text-cyan-200" />
                    <h3 className="font-archaic text-lg">Recent Loot</h3>
                  </div>
                  <div className="space-y-2">
                    {snapshot.economy.recentLoot.map((loot) => (
                      <div
                        key={loot.id}
                        className="rounded-2xl border border-white/8 bg-white/[0.03] px-3 py-3"
                      >
                        <div className="font-rune text-sm text-white">{loot.itemName}</div>
                        <div className="mt-1 text-xs uppercase tracking-[0.24em] text-white/45">
                          {loot.recipient} · {loot.source} · {loot.distribution}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>

                <div className="rounded-3xl border border-white/10 bg-[#0d0715] p-4">
                  <div className="mb-3 flex items-center gap-2 text-white">
                    <Plus size={18} className="text-fuchsia-200" />
                    <h3 className="font-archaic text-lg">Wishlist</h3>
                  </div>
                  <div className="mb-3 flex gap-2">
                    <input
                      value={wishlistDraft}
                      onChange={(event) => setWishlistDraft(event.target.value)}
                      placeholder="Add wishlist item"
                      className="flex-1 rounded-2xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-fuchsia-300/35"
                    />
                    <button
                      type="button"
                      onClick={() => void handleWishlistAdd()}
                      className="rounded-full border border-fuchsia-400/25 bg-fuchsia-400/10 px-4 py-2 text-sm text-fuchsia-100 transition hover:bg-fuchsia-400/20"
                    >
                      Add
                    </button>
                  </div>
                  <div className="flex flex-wrap gap-2">
                    {snapshot.economy.wishlist.map((item) => (
                      <button
                        key={item}
                        type="button"
                        onClick={() =>
                          void runAction({
                            type: "update_wishlist",
                            items: snapshot.economy.wishlist.filter((entry) => entry !== item),
                          })
                        }
                        className="rounded-full border border-white/10 bg-white/5 px-3 py-2 text-sm text-white/80 transition hover:border-white/30 hover:bg-white/10"
                      >
                        {item}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            </Panel>

            <Panel
              title="Combat Analytics"
              subtitle="DPS curves, spell usage, deaths, and rotation efficiency"
              icon={<Sword size={20} />}
              accent="cyan"
            >
              <PolylineChart series={snapshot.combat.dpsSeries} />

              <div className="mt-4 grid gap-4 lg:grid-cols-[1fr_0.95fr]">
                <div className="rounded-3xl border border-white/10 bg-[#0d0715] p-4">
                  <div className="mb-3 flex items-center gap-2 text-white">
                    <Pulse size={18} className="text-cyan-200" />
                    <h3 className="font-archaic text-lg">Spell Usage</h3>
                  </div>
                  <div className="space-y-3">
                    {snapshot.combat.spellUsage.map((spell) => (
                      <div key={spell.spellName}>
                        <div className="mb-2 flex items-center justify-between text-sm text-white/75">
                          <span>{spell.spellName}</span>
                          <span className="font-rune text-xs">
                            {spell.casts} casts · {spell.efficiency}%
                          </span>
                        </div>
                        <div className="h-2 overflow-hidden rounded-full bg-white/5">
                          <div
                            className="h-full rounded-full bg-gradient-to-r from-cyan-300 to-fuchsia-400"
                            style={{ width: `${spell.efficiency}%` }}
                          />
                        </div>
                      </div>
                    ))}
                  </div>
                </div>

                <div className="rounded-3xl border border-white/10 bg-[#0d0715] p-4">
                  <div className="mb-3 flex items-center gap-2 text-white">
                    <Skull size={18} className="text-rose-200" />
                    <h3 className="font-archaic text-lg">Death Log</h3>
                  </div>
                  <div className="space-y-3">
                    {snapshot.combat.deathLog.map((entry) => (
                      <div
                        key={entry.id}
                        className="rounded-2xl border border-white/8 bg-white/[0.03] px-3 py-3"
                      >
                        <div className="font-rune text-sm text-white">
                          {entry.characterName}
                        </div>
                        <div className="mt-1 text-xs uppercase tracking-[0.24em] text-white/45">
                          {entry.reason} · recovered {entry.recoveredAt}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              </div>

              <div className="mt-4 rounded-3xl border border-white/10 bg-[#0d0715] p-4">
                <div className="mb-3 flex items-center gap-2 text-white">
                  <Heartbeat size={18} className="text-emerald-200" />
                  <h3 className="font-archaic text-lg">Rotation Efficiency</h3>
                </div>
                <div className="space-y-3">
                  {snapshot.combat.rotations.map((rotation) => (
                    <div key={rotation.characterName}>
                      <div className="mb-2 flex items-center justify-between text-sm text-white/75">
                        <span>{rotation.characterName}</span>
                        <span className="font-rune text-xs">
                          {rotation.efficiency}% · drift {rotation.driftMs}ms
                        </span>
                      </div>
                      <div className="h-2 overflow-hidden rounded-full bg-white/5">
                        <div
                          className="h-full rounded-full bg-gradient-to-r from-emerald-300 to-cyan-300"
                          style={{ width: `${rotation.efficiency}%` }}
                        />
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </Panel>

            <Panel
              title="System Health"
              subtitle="Client memory, IPC latency percentiles, frame rate, and recovery log"
              icon={<Cpu size={20} />}
              accent="amber"
            >
              <div className="grid gap-3 sm:grid-cols-3">
                <StatChip label="P50" value={`${snapshot.health.ipcLatency.p50} ms`} />
                <StatChip label="P95" value={`${snapshot.health.ipcLatency.p95} ms`} tone="warning" />
                <StatChip label="P99" value={`${snapshot.health.ipcLatency.p99} ms`} tone="critical" />
              </div>

              <div className="mt-4 rounded-3xl border border-white/10 bg-[#0d0715] p-4">
                <div className="mb-3 flex items-center gap-2 text-white">
                  <Pulse size={18} className="text-amber-200" />
                  <h3 className="font-archaic text-lg">Client Footprint</h3>
                </div>
                <div className="space-y-3">
                  {snapshot.health.clients.map((client) => (
                    <div
                      key={client.clientId}
                      className="grid gap-3 rounded-2xl border border-white/8 bg-white/[0.03] px-3 py-3 md:grid-cols-[1.2fr_0.8fr_0.8fr_auto]"
                    >
                      <div>
                        <div className="font-rune text-sm text-white">{client.characterName}</div>
                        <div className="mt-1 text-xs uppercase tracking-[0.24em] text-white/45">
                          Client {client.clientId}
                        </div>
                      </div>
                      <div className="text-sm text-white/75">
                        <div className="font-tech text-[10px] uppercase tracking-[0.22em] text-white/40">
                          Memory
                        </div>
                        <div className="mt-1 font-rune">{client.memoryMb} MB</div>
                      </div>
                      <div className="text-sm text-white/75">
                        <div className="font-tech text-[10px] uppercase tracking-[0.22em] text-white/40">
                          Frame Rate
                        </div>
                        <div className="mt-1 font-rune">{client.frameRate} FPS</div>
                      </div>
                      <div
                        className={`inline-flex items-center rounded-full border px-3 py-1 text-xs uppercase tracking-[0.24em] ${statusTone(
                          client.status
                        )}`}
                      >
                        {titleCase(client.status)}
                      </div>
                    </div>
                  ))}
                </div>
              </div>

              <div className="mt-4 rounded-3xl border border-white/10 bg-[#0d0715] p-4">
                <div className="mb-3 flex items-center gap-2 text-white">
                  <WarningDiamond size={18} className="text-rose-200" />
                  <h3 className="font-archaic text-lg">Error Log</h3>
                </div>
                <div className="space-y-3">
                  {snapshot.health.errorLog.map((entry) => (
                    <div
                      key={entry.id}
                      className={`rounded-2xl border px-3 py-3 ${severityTone(
                        entry.severity
                      )}`}
                    >
                      <div className="flex items-center justify-between gap-3">
                        <div className="font-rune text-sm">{entry.message}</div>
                        <div className="rounded-full border border-white/10 bg-white/5 px-3 py-1 text-[11px] uppercase tracking-[0.24em] text-white/60">
                          {titleCase(entry.severity)}
                        </div>
                      </div>
                      <div className="mt-2 text-xs uppercase tracking-[0.24em] text-white/60">
                        Recovery: {entry.recoveryAction}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </Panel>

            <KillTrackerPanel />
          </div>
        </div>

        <DiscordConfigPanel />
      </main>
    </div>
  );
}
