import { ArrowsClockwise, Broadcast, Pulse, Stack, WarningDiamond } from "@phosphor-icons/react";

import type { AdminSessionRecord } from "../types";

function toneClasses(value: string | null) {
  const normalized = value?.toLowerCase() ?? "";

  if (normalized.includes("live") || normalized.includes("active")) {
    return "border-emerald-400/25 bg-emerald-400/10 text-emerald-100";
  }
  if (
    normalized.includes("recover") ||
    normalized.includes("launch") ||
    normalized.includes("wait") ||
    normalized.includes("paused")
  ) {
    return "border-amber-300/25 bg-amber-300/10 text-amber-100";
  }
  if (
    normalized.includes("block") ||
    normalized.includes("error") ||
    normalized.includes("exit") ||
    normalized.includes("offline")
  ) {
    return "border-rose-400/25 bg-rose-400/10 text-rose-100";
  }

  return "border-white/10 bg-white/5 text-white/80";
}

function DetailRow({
  label,
  value,
}: {
  label: string;
  value: string;
}) {
  return (
    <div className="flex items-center justify-between gap-3 text-sm">
      <span className="font-tech uppercase tracking-[0.22em] text-white/40">{label}</span>
      <span className="font-rune text-right text-white/80">{value}</span>
    </div>
  );
}

export function AdminSessionOverview({
  sessions,
  loading,
  error,
}: {
  sessions: AdminSessionRecord[];
  loading: boolean;
  error: string | null;
}) {
  if (loading) {
    return (
      <section className="rounded-[1.75rem] border border-cyan-400/20 bg-[#120a1d]/88 p-6 shadow-[0_18px_50px_rgba(34,211,238,0.08)]">
        <div className="flex items-center gap-3 text-cyan-100">
          <ArrowsClockwise className="animate-spin text-xl" />
          <div>
            <h2 className="font-archaic text-2xl">Session Overview</h2>
            <p className="font-tech text-xs uppercase tracking-[0.28em] text-white/45">
              Loading admin session inventory...
            </p>
          </div>
        </div>
      </section>
    );
  }

  if (error) {
    return (
      <section className="rounded-[1.75rem] border border-rose-400/25 bg-[#120a1d]/88 p-6 shadow-[0_18px_50px_rgba(244,63,94,0.08)]">
        <div className="flex items-start gap-3 text-rose-100">
          <WarningDiamond className="mt-1 text-xl" />
          <div>
            <h2 className="font-archaic text-2xl">Session Overview</h2>
            <p className="mt-2 text-sm text-rose-100/85">
              Admin session inventory unavailable: {error}
            </p>
            <p className="mt-2 text-sm text-white/55">
              This route is wired to the dedicated admin API and will populate once
              the backend inventory endpoint is available in the running build.
            </p>
          </div>
        </div>
      </section>
    );
  }

  if (sessions.length === 0) {
    return (
      <section className="rounded-[1.75rem] border border-white/10 bg-[#120a1d]/88 p-6 shadow-[0_18px_50px_rgba(15,23,42,0.25)]">
        <div className="flex items-center gap-3 text-white">
          <Stack className="text-xl text-cyan-200" />
          <div>
            <h2 className="font-archaic text-2xl">Session Overview</h2>
            <p className="font-tech text-xs uppercase tracking-[0.28em] text-white/45">
              No managed sessions reported by the admin API.
            </p>
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="rounded-[1.75rem] border border-cyan-400/20 bg-[#120a1d]/88 p-6 shadow-[0_18px_50px_rgba(34,211,238,0.08)]">
      <div className="mb-6 flex items-start justify-between gap-4">
        <div>
          <h2 className="font-archaic text-2xl text-white">Session Overview</h2>
          <p className="font-tech text-xs uppercase tracking-[0.28em] text-white/45">
            Managed session inventory from the dedicated admin API
          </p>
        </div>
        <div className="rounded-full border border-white/10 bg-white/5 px-4 py-2 font-rune text-sm text-white/85">
          {sessions.length} tracked
        </div>
      </div>
      <div className="grid gap-4 xl:grid-cols-2">
        {sessions.map((session) => (
          <article
            key={session.sessionId}
            className="rounded-[1.35rem] border border-white/10 bg-[#0d0715] p-5"
          >
            <div className="mb-4 flex items-start justify-between gap-3">
              <div>
                <h3 className="font-archaic text-xl text-white">{session.characterName}</h3>
                <p className="font-rune text-xs uppercase tracking-[0.22em] text-white/45">
                  Session {session.sessionId}
                </p>
              </div>
              <div className="flex flex-wrap justify-end gap-2 text-[11px]">
                <span className={`rounded-full border px-3 py-1 font-tech uppercase tracking-[0.22em] ${toneClasses(session.lifecycle)}`}>
                  <Pulse className="mr-1 inline-block align-text-bottom" />
                  {session.lifecycle ?? "unknown lifecycle"}
                </span>
                <span className={`rounded-full border px-3 py-1 font-tech uppercase tracking-[0.22em] ${toneClasses(session.status)}`}>
                  <Broadcast className="mr-1 inline-block align-text-bottom" />
                  {session.status ?? "unknown status"}
                </span>
              </div>
            </div>
            <div className="space-y-3">
              <DetailRow label="Profile" value={session.profile ?? "Unassigned"} />
              <DetailRow label="Group" value={session.groupId ?? "Ungrouped"} />
              <DetailRow label="Routing" value={session.routingScope ?? "Default"} />
              <DetailRow label="Zone" value={session.zone ?? "Unavailable"} />
              <DetailRow
                label="Identity"
                value={[
                  session.className,
                  session.level != null ? `Level ${session.level}` : null,
                ]
                  .filter(Boolean)
                  .join(" • ") || "Unavailable"}
              />
              <DetailRow label="Heartbeat" value={session.lastHeartbeat ?? "Unavailable"} />
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
