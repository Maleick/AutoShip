import { useEffect, useMemo, useState } from "react";
import {
  ArrowClockwise,
  FloppyDisk,
  ShieldCheck,
  Sparkle,
  WarningDiamond,
} from "@phosphor-icons/react";

import { useTradeskillTrophySettings } from "../hooks/useTradeskillTrophySettings";
import type {
  LiveTradeskillTrophyStatus,
  TradeskillTrophySettings,
} from "../types";

type TradeskillTrophyPanelProps = {
  embedded?: boolean;
};

function titleCase(value: string | null): string {
  if (!value) {
    return "idle";
  }
  return value
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function StatusCard({ entry }: { entry: LiveTradeskillTrophyStatus }) {
  const tone =
    entry.status.charges_remaining !== null && entry.status.charges_remaining <= 5;
  return (
    <div
      className={`border p-4 ${
        tone
          ? "border-amber-300/30 bg-amber-300/10"
          : "border-white/10 bg-white/[0.03]"
      }`}
    >
      <div className="flex items-center justify-between gap-3">
        <div>
          <div className="font-archaic text-lg text-white">Client {entry.pid}</div>
          <p className="mt-1 text-xs uppercase tracking-[0.22em] text-white/45">
            {entry.status.active ? "crafting active" : "standing by"}
          </p>
        </div>
        <span
          className={`border px-3 py-1 text-[10px] uppercase tracking-[0.24em] ${
            entry.status.equipped_by_manager
              ? "border-spectral/40 bg-spectral/10 text-spectral"
              : "border-white/10 bg-void text-white/45"
          }`}
        >
          {entry.status.equipped_by_manager ? "managed" : "observed"}
        </span>
      </div>
      <div className="mt-4 grid gap-3 sm:grid-cols-2">
        <div>
          <div className="text-[10px] uppercase tracking-[0.22em] text-white/35">
            Container
          </div>
          <div className="mt-1 text-sm text-white/70">
            {entry.status.open_container_name ?? "No open station"}
          </div>
        </div>
        <div>
          <div className="text-[10px] uppercase tracking-[0.22em] text-white/35">
            Equip Slot
          </div>
          <div className="mt-1 text-sm text-white/70">
            {titleCase(entry.status.target_slot)}
          </div>
        </div>
        <div>
          <div className="text-[10px] uppercase tracking-[0.22em] text-white/35">
            Skill
          </div>
          <div className="mt-1 text-sm text-white/70">
            {titleCase(entry.status.container_type)}
          </div>
        </div>
        <div>
          <div className="text-[10px] uppercase tracking-[0.22em] text-white/35">
            Charges
          </div>
          <div className="mt-1 text-sm text-white/70">
            {entry.status.charges_remaining ?? "Unknown"}
          </div>
        </div>
      </div>
      <div className="mt-4 border-t border-white/10 pt-3">
        <div className="text-[10px] uppercase tracking-[0.22em] text-white/35">
          Restores After Crafting
        </div>
        <div className="mt-1 text-sm text-white/70">
          {entry.status.previous_item_name ?? "Nothing displaced"}
        </div>
      </div>
    </div>
  );
}

export default function TradeskillTrophyPanel({
  embedded = false,
}: TradeskillTrophyPanelProps) {
  const {
    settings,
    statuses,
    loading,
    saving,
    error,
    saveSettings,
    refresh,
  } = useTradeskillTrophySettings();
  const [draft, setDraft] = useState<TradeskillTrophySettings>(settings);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  useEffect(() => {
    if (!notice) {
      return;
    }
    const timeout = window.setTimeout(() => setNotice(null), 2500);
    return () => window.clearTimeout(timeout);
  }, [notice]);

  const activeSessions = useMemo(
    () => statuses.filter((entry) => entry.status.active).length,
    [statuses],
  );
  const lowChargeCount = useMemo(
    () =>
      statuses.filter(
        (entry) =>
          entry.status.charges_remaining !== null &&
          entry.status.charges_remaining <= 5,
      ).length,
    [statuses],
  );

  async function handleSave() {
    if (await saveSettings(draft)) {
      setNotice("Tradeskill trophy profile saved");
    }
  }

  return (
    <section
      className={`relative z-20 overflow-hidden ${embedded ? "" : "min-w-[860px] flex-1"}`}
    >
      <div
        className={`flex flex-col border border-white/10 bg-void/70 backdrop-blur-md ${
          embedded ? "rounded-[1.5rem]" : "h-full"
        }`}
      >
        <header
          className={`border-b border-white/10 bg-violet/30 ${embedded ? "px-6 py-5" : "px-8 py-6"}`}
        >
          <div className="flex items-start justify-between gap-6">
            <div>
              <div className="flex items-center gap-3">
                <div className="flex h-11 w-11 items-center justify-center border border-spectral/40 bg-spectral/10 text-spectral">
                  <Sparkle size={20} weight="fill" />
                </div>
                <div>
                  <h2 className="font-archaic text-2xl tracking-wide text-white">
                    Tradeskill Trophy
                  </h2>
                  <p className="mt-1 text-xs uppercase tracking-[0.3em] text-white/45">
                    MQ2TSTrophy-style equip and restore control
                  </p>
                </div>
              </div>
              <p className="mt-5 max-w-3xl text-sm leading-6 text-white/60">
                Equip the configured trophy before a supported crafting station opens,
                restore the displaced item after the session, and keep a live eye on
                remaining charges.
              </p>
            </div>

            <div className="flex items-center gap-3">
              <button
                onClick={() => {
                  void refresh();
                }}
                className="flex items-center gap-2 border border-white/15 bg-white/5 px-4 py-2 text-xs uppercase tracking-[0.2em] text-white/65 transition-colors hover:border-white/30 hover:text-white"
              >
                <ArrowClockwise size={14} />
                Refresh
              </button>
              <button
                onClick={() => {
                  void handleSave();
                }}
                disabled={saving}
                className="flex items-center gap-2 border border-magentaglow/40 bg-magentadark/20 px-4 py-2 text-xs uppercase tracking-[0.2em] text-white transition-colors hover:bg-magentadark/35 disabled:cursor-wait disabled:opacity-70"
              >
                <FloppyDisk size={14} />
                {saving ? "Saving" : "Save Trophy"}
              </button>
            </div>
          </div>
        </header>

        <div className={`flex-1 overflow-y-auto ${embedded ? "px-6 py-5" : "px-8 py-6"}`}>
          <div className={`grid gap-6 ${embedded ? "" : "grid-cols-[1.2fr_1fr]"}`}>
            <div className="space-y-6">
              <div className="border border-white/10 bg-violet/20 p-5">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <h3 className="font-archaic text-lg text-white">
                      Trophy Automation
                    </h3>
                    <p className="mt-2 text-sm leading-6 text-white/55">
                      Enable this only for characters that craft during downtime.
                      The DLL equips the named trophy when a supported world
                      container is open and returns the original item afterwards.
                    </p>
                  </div>
                  <button
                    onClick={() =>
                      setDraft((current) => ({
                        ...current,
                        enabled: !current.enabled,
                      }))
                    }
                    className={`min-w-[140px] border px-4 py-3 text-xs uppercase tracking-[0.25em] transition-all ${
                      draft.enabled
                        ? "border-spectral/40 bg-spectral/10 text-spectral"
                        : "border-white/15 bg-void text-white/40"
                    }`}
                  >
                    {draft.enabled ? "armed" : "offline"}
                  </button>
                </div>
              </div>

              <div className="border border-white/10 bg-void/50 p-5">
                <label
                  htmlFor="tradeskill-trophy-item"
                  className="text-xs uppercase tracking-[0.24em] text-white/45"
                >
                  Trophy Item Name
                </label>
                <input
                  id="tradeskill-trophy-item"
                  value={draft.trophy_item_name}
                  onChange={(event) =>
                    setDraft((current) => ({
                      ...current,
                      trophy_item_name: event.target.value,
                    }))
                  }
                  placeholder="Geerlok Automated Hammer"
                  className="mt-3 w-full border border-white/10 bg-[#10081a] px-4 py-3 text-sm text-white outline-none transition-colors placeholder:text-white/25 focus:border-spectral/40"
                />
                <p className="mt-3 text-xs leading-5 text-white/45">
                  Enter the in-game item name exactly as it appears on the trophy.
                  Fishing trophies use the main hand slot. Most other trophies use
                  the ammo slot.
                </p>
              </div>
            </div>

            <div className="space-y-6">
              <div className="grid gap-3 sm:grid-cols-3 lg:grid-cols-1">
                <div className="border border-white/10 bg-white/[0.03] p-4">
                  <div className="text-[10px] uppercase tracking-[0.24em] text-white/35">
                    Connected Clients
                  </div>
                  <div className="mt-2 font-archaic text-2xl text-white">
                    {statuses.length}
                  </div>
                </div>
                <div className="border border-white/10 bg-white/[0.03] p-4">
                  <div className="text-[10px] uppercase tracking-[0.24em] text-white/35">
                    Active Sessions
                  </div>
                  <div className="mt-2 font-archaic text-2xl text-white">
                    {activeSessions}
                  </div>
                </div>
                <div className="border border-white/10 bg-white/[0.03] p-4">
                  <div className="text-[10px] uppercase tracking-[0.24em] text-white/35">
                    Low Charges
                  </div>
                  <div
                    className={`mt-2 font-archaic text-2xl ${
                      lowChargeCount > 0 ? "text-amber-200" : "text-white"
                    }`}
                  >
                    {lowChargeCount}
                  </div>
                </div>
              </div>

              <div className="border border-white/10 bg-violet/10 p-5">
                <div className="flex items-start gap-3">
                  <div className="mt-0.5 flex h-10 w-10 items-center justify-center border border-spectral/40 bg-spectral/10 text-spectral">
                    <ShieldCheck size={18} weight="fill" />
                  </div>
                  <div>
                    <h3 className="font-archaic text-lg text-white">
                      Live Trophy Telemetry
                    </h3>
                    <p className="mt-2 text-sm leading-6 text-white/55">
                      Status updates are queried from the injected clients. Charges
                      appear when the trophy is equipped or on the cursor during the
                      equip and restore sequence.
                    </p>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div className="mt-6 space-y-4">
            {error ? (
              <div className="flex items-start gap-3 border border-rose-400/25 bg-rose-400/10 px-4 py-3 text-sm text-rose-100">
                <WarningDiamond size={18} className="mt-0.5 shrink-0" />
                <span>{error}</span>
              </div>
            ) : null}

            {notice ? (
              <div className="border border-spectral/25 bg-spectral/10 px-4 py-3 text-sm text-spectral">
                {notice}
              </div>
            ) : null}

            {loading ? (
              <div className="border border-white/10 bg-white/[0.03] px-4 py-5 text-sm text-white/55">
                Loading tradeskill trophy profile...
              </div>
            ) : statuses.length > 0 ? (
              <div className="grid gap-4 xl:grid-cols-2">
                {statuses.map((entry) => (
                  <StatusCard key={entry.pid} entry={entry} />
                ))}
              </div>
            ) : (
              <div className="border border-white/10 bg-white/[0.03] px-4 py-5 text-sm text-white/55">
                No live clients are currently reporting tradeskill trophy activity.
              </div>
            )}
          </div>
        </div>
      </div>
    </section>
  );
}
