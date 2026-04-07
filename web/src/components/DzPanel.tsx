import { useState, useEffect } from "react";
import {
  Timer,
  ArrowCounterClockwise,
  UsersThree,
  ClockCountdown,
  Scroll,
  Package,
  CheckCircle,
} from "@phosphor-icons/react";
import { dzLockouts, raidInstances, dzHistory } from "../data/demo";
import type { DzLockout, RaidInstance, DzHistoryEntry } from "../types";

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Format a seconds-remaining value into "Xd Xh Xm" or "Xh Xm" etc. */
function formatCountdown(secs: number): string {
  if (secs <= 0) return "Expired";
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const parts: string[] = [];
  if (d > 0) parts.push(`${d}d`);
  if (h > 0) parts.push(`${h}h`);
  if (m > 0 || parts.length === 0) parts.push(`${m}m`);
  return parts.join(" ");
}

/** Format ISO date string to "MM/DD HH:MM UTC". */
function formatDate(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString("en-US", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    timeZone: "UTC",
    timeZoneName: "short",
  });
}

/** Format duration in seconds to "Xh Ym Zs" style. */
function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

/** Colour class for the urgency of a lockout countdown. */
function urgencyColor(secs: number): string {
  if (secs <= 0) return "text-white/40";
  if (secs < 6 * 3600) return "text-red-400"; // < 6h
  if (secs < 24 * 3600) return "text-yellow-400"; // < 24h
  return "text-spectral";
}

/** Milliseconds to display the reset-queued toast notification. */
const TOAST_DURATION_MS = 3500;

function LockoutRow({
  lockout,
  onReset,
}: {
  lockout: DzLockout;
  onReset: (lo: DzLockout) => void;
}) {
  // Derive remaining seconds from the ISO expiry timestamp so the countdown
  // stays accurate regardless of when the component mounts or re-renders.
  const getSecsLeft = () =>
    Math.max(0, Math.floor((new Date(lockout.expires_at).getTime() - Date.now()) / 1000));

  const [secsLeft, setSecsLeft] = useState(getSecsLeft);

  // Tick every 60 seconds, re-computing from the expiry timestamp each time.
  useEffect(() => {
    if (secsLeft <= 0) return;
    const id = setInterval(() => setSecsLeft(getSecsLeft), 60_000);
    return () => clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lockout.expires_at]);

  const isFull = lockout.lockout_type === "6.5d full";

  return (
    <div className="flex items-center gap-3 py-2 border-b border-white/5 last:border-0 group">
      {/* Expedition + character */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-white truncate">
            {lockout.expedition}
          </span>
          <span
            className={`text-[10px] px-1.5 py-0.5 border ${isFull ? "border-red-500/40 text-red-400 bg-red-900/10" : "border-spectral/30 text-spectral bg-spectral/5"} uppercase tracking-wider`}
          >
            {lockout.lockout_type}
          </span>
        </div>
        <div className="text-[11px] text-white/50 font-rune mt-0.5">
          {lockout.character}
        </div>
      </div>

      {/* Countdown */}
      <div className="text-right mr-2">
        <div className={`font-rune text-base ${urgencyColor(secsLeft)}`}>
          {formatCountdown(secsLeft)}
        </div>
        <div className="text-[10px] text-white/30 font-tech">
          {formatDate(lockout.expires_at)}
        </div>
      </div>

      {/* Reset button */}
      <button
        onClick={() => onReset(lockout)}
        title="Queue DZ reset"
        className="opacity-0 group-hover:opacity-100 transition-opacity w-7 h-7 border border-white/20 flex items-center justify-center hover:border-magentaglow hover:text-magentaglow text-white/40 shrink-0"
      >
        <ArrowCounterClockwise size={14} />
      </button>
    </div>
  );
}

function InstanceCard({ instance }: { instance: RaidInstance }) {
  const elapsed = Math.floor(
    (Date.now() - new Date(instance.entered_at).getTime()) / 1000
  );

  return (
    <div className="arcane-tablet p-4 flex flex-col gap-3">
      <div className="flex justify-between items-start">
        <div>
          <div className="text-[10px] text-spectral/70 uppercase tracking-widest font-tech mb-0.5">
            {instance.zone}
          </div>
          <h4 className="font-archaic text-lg text-white leading-tight">
            {instance.expedition}
          </h4>
        </div>
        <div className="text-right">
          <div className="text-[10px] text-white/40 uppercase tracking-wider mb-1">
            Elapsed
          </div>
          <div className="font-rune text-spectral">{formatDuration(elapsed)}</div>
        </div>
      </div>

      <div>
        <div className="text-[10px] text-white/40 uppercase tracking-wider mb-1.5 flex items-center gap-1.5">
          <UsersThree size={11} />
          {instance.group} · {instance.members.length} inside
        </div>
        <div className="flex flex-wrap gap-1.5">
          {instance.members.map((m) => (
            <span
              key={m}
              className="text-[10px] bg-void border border-white/10 px-2 py-0.5 text-white/70 font-tech"
            >
              {m}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}

function HistoryRow({ entry }: { entry: DzHistoryEntry }) {
  return (
    <div className="py-3 border-b border-white/5 last:border-0">
      <div className="flex justify-between items-start">
        <div className="flex items-center gap-2">
          <CheckCircle weight="fill" className="text-green-500 shrink-0" size={14} />
          <span className="text-sm font-medium text-white">
            {entry.expedition}
          </span>
          <span className="text-[10px] text-white/40 font-tech uppercase">
            {entry.zone}
          </span>
        </div>
        <span className="text-[10px] text-white/40 font-rune">
          {formatDuration(entry.duration_secs)}
        </span>
      </div>

      <div className="ml-5 mt-1 space-y-1">
        <div className="text-[10px] text-white/40 font-tech">
          {formatDate(entry.completed_at)} ·{" "}
          {entry.participants.join(", ")}
        </div>
        {entry.loot.length > 0 && (
          <div className="flex flex-wrap gap-1.5 mt-1">
            {entry.loot.map((item) => (
              <span
                key={item}
                className="text-[10px] flex items-center gap-1 bg-violet/20 border border-violet/30 px-2 py-0.5 text-magentaglow/80 font-tech"
              >
                <Package size={9} />
                {item}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

// ── Toast notification ────────────────────────────────────────────────────────

function ResetToast({
  message,
  onDismiss,
}: {
  message: string;
  onDismiss: () => void;
}) {
  useEffect(() => {
    const id = setTimeout(onDismiss, TOAST_DURATION_MS);
    return () => clearTimeout(id);
  }, [onDismiss]);

  return (
    <div className="fixed bottom-6 right-6 z-50 bg-void border border-magentaglow/50 px-4 py-3 shadow-[0_0_20px_rgba(204,68,255,0.3)] flex items-center gap-3 text-sm text-white font-tech animate-fade-in">
      <ArrowCounterClockwise className="text-magentaglow" size={16} />
      {message}
    </div>
  );
}

// ── Main panel ────────────────────────────────────────────────────────────────

type Tab = "lockouts" | "instances" | "history";

export default function DzPanel() {
  const [tab, setTab] = useState<Tab>("lockouts");
  const [toast, setToast] = useState<string | null>(null);

  const handleReset = (lo: DzLockout) => {
    // In production this would POST to /api/dz/reset.
    setToast(`Reset queued for ${lo.expedition} (${lo.character})`);
  };

  const tabs: { id: Tab; label: string; icon: React.ReactNode }[] = [
    {
      id: "lockouts",
      label: "Lockout Timers",
      icon: <ClockCountdown size={14} />,
    },
    {
      id: "instances",
      label: "Active Instances",
      icon: <UsersThree size={14} />,
    },
    { id: "history", label: "History", icon: <Scroll size={14} /> },
  ];

  return (
    <section className="flex-1 h-full flex flex-col relative z-20 min-w-[600px]">
      {/* Header */}
      <header className="h-16 border-b border-white/10 flex items-center justify-between px-6 bg-violet/30 backdrop-blur-md">
        <div className="flex items-center gap-4">
          <div className="w-8 h-8 rounded border border-magentadark flex items-center justify-center bg-void">
            <Timer weight="fill" className="text-magentaglow" />
          </div>
          <div>
            <h2 className="font-archaic text-lg text-white leading-tight">
              Dynamic Zone Control
            </h2>
            <p className="text-[10px] uppercase tracking-widest text-white/50 font-rune">
              Lockout Timers · Reset Queue · Instance Tracker
            </p>
          </div>
        </div>

        <button
          onClick={() =>
            setToast("Refreshing DZ data from live clients…")
          }
          className="px-4 py-1.5 bg-magentadark/20 border border-magentaglow text-white text-sm font-medium hover:bg-magentadark/40 transition-colors uppercase tracking-wider shadow-[0_0_15px_rgba(204,68,255,0.3)] flex items-center gap-2"
        >
          <ArrowCounterClockwise size={14} />
          Refresh
        </button>
      </header>

      {/* Tab bar */}
      <div className="flex border-b border-white/10 bg-void/40 backdrop-blur-sm px-6">
        {tabs.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className={`flex items-center gap-2 px-5 py-3 text-sm font-medium border-b-2 transition-colors ${
              tab === t.id
                ? "border-magentaglow text-white"
                : "border-transparent text-white/40 hover:text-white/70"
            }`}
          >
            {t.icon}
            {t.label}
          </button>
        ))}
      </div>

      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto p-6 scroll-smooth">
        {tab === "lockouts" && (
          <div>
            <div className="flex items-center justify-between mb-4">
              <h3 className="font-archaic text-base text-white/80 uppercase tracking-wider">
                Active Lockouts
              </h3>
              <span className="text-xs text-white/40 font-tech">
                {dzLockouts.length} entries · hover a row to queue reset
              </span>
            </div>

            <div className="bg-violet/20 border border-white/5 p-4">
              {dzLockouts.map((lo) => (
                <LockoutRow key={lo.id} lockout={lo} onReset={handleReset} />
              ))}
            </div>
          </div>
        )}

        {tab === "instances" && (
          <div>
            <div className="flex items-center justify-between mb-4">
              <h3 className="font-archaic text-base text-white/80 uppercase tracking-wider">
                Active Raid Instances
              </h3>
              <span className="text-xs text-white/40 font-tech">
                {raidInstances.length} instance
                {raidInstances.length !== 1 ? "s" : ""} running
              </span>
            </div>

            <div className="grid grid-cols-2 gap-4">
              {raidInstances.map((inst) => (
                <InstanceCard key={inst.id} instance={inst} />
              ))}
            </div>
          </div>
        )}

        {tab === "history" && (
          <div>
            <div className="flex items-center justify-between mb-4">
              <h3 className="font-archaic text-base text-white/80 uppercase tracking-wider">
                Completion Log
              </h3>
              <span className="text-xs text-white/40 font-tech">
                {dzHistory.length} runs recorded
              </span>
            </div>

            <div className="bg-violet/20 border border-white/5 p-4">
              {dzHistory.map((entry) => (
                <HistoryRow key={entry.id} entry={entry} />
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Toast */}
      {toast && (
        <ResetToast message={toast} onDismiss={() => setToast(null)} />
      )}
    </section>
  );
}
