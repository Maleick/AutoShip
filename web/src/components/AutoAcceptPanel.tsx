import { useEffect, useState } from "react";
import {
  ArrowClockwise,
  CompassTool,
  FloppyDisk,
  Handshake,
  MapTrifold,
  ShieldCheck,
  Sparkle,
  Stack,
  UsersThree,
} from "@phosphor-icons/react";

import { useAutoAcceptSettings } from "../hooks/useAutoAcceptSettings";
import type { AutoAcceptSettings } from "../types";

type AutoAcceptPanelProps = {
  embedded?: boolean;
};

type AutoAcceptToggleKey =
  | "accept_group_invites"
  | "accept_trades"
  | "accept_task_adds"
  | "accept_dz_adds"
  | "accept_translocates"
  | "accept_anchors";

const REQUEST_TOGGLES: {
  key: AutoAcceptToggleKey;
  label: string;
  detail: string;
  icon: typeof UsersThree;
}[] = [
  {
    key: "accept_group_invites",
    label: "Group Invites",
    detail: "Accept incoming invites before idle boxes time out.",
    icon: UsersThree,
  },
  {
    key: "accept_trades",
    label: "Trades",
    detail: "Confirm trade windows from trusted runners or mule characters.",
    icon: Handshake,
  },
  {
    key: "accept_task_adds",
    label: "Task Adds",
    detail: "Join shared tasks without manual prompts.",
    icon: Stack,
  },
  {
    key: "accept_dz_adds",
    label: "DZ Adds",
    detail: "Accept expedition and dynamic-zone invitations.",
    icon: MapTrifold,
  },
  {
    key: "accept_translocates",
    label: "Translocates",
    detail: "Accept wizard and druid transport prompts.",
    icon: CompassTool,
  },
  {
    key: "accept_anchors",
    label: "Anchors",
    detail: "Take primary and secondary guild-hall anchor ports.",
    icon: Sparkle,
  },
];

function normalizeTrustedPlayers(text: string): string[] {
  return text
    .split(/[\n,]/)
    .map((value) => value.trim())
    .filter(Boolean);
}

function ToggleCard({
  enabled,
  label,
  detail,
  icon: Icon,
  onToggle,
}: {
  enabled: boolean;
  label: string;
  detail: string;
  icon: typeof UsersThree;
  onToggle: () => void;
}) {
  return (
    <button
      onClick={onToggle}
      className={`group border p-4 text-left transition-all ${
        enabled
          ? "border-spectral/40 bg-spectral/10 shadow-[0_0_30px_rgba(0,229,255,0.08)]"
          : "border-white/10 bg-void/50 hover:border-white/30 hover:bg-white/5"
      }`}
    >
      <div className="flex items-start gap-3">
        <div
          className={`mt-0.5 flex h-10 w-10 items-center justify-center border ${
            enabled
              ? "border-spectral/40 bg-spectral/10 text-spectral"
              : "border-white/15 bg-void text-white/35 group-hover:text-white/70"
          }`}
        >
          <Icon size={18} weight="fill" />
        </div>
        <div className="flex-1">
          <div className="flex items-center justify-between gap-3">
            <span className="font-archaic text-lg text-white">{label}</span>
            <span
              className={`text-[10px] uppercase tracking-[0.25em] ${
                enabled ? "text-spectral" : "text-white/35"
              }`}
            >
              {enabled ? "enabled" : "disabled"}
            </span>
          </div>
          <p className="mt-2 text-xs leading-5 text-white/50">{detail}</p>
        </div>
      </div>
    </button>
  );
}

export default function AutoAcceptPanel({ embedded = false }: AutoAcceptPanelProps) {
  const { settings, loading, saving, error, saveSettings, refresh } = useAutoAcceptSettings();
  const [draft, setDraft] = useState<AutoAcceptSettings>(settings);
  const [trustedPlayersText, setTrustedPlayersText] = useState("");
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    setDraft(settings);
    setTrustedPlayersText(settings.trusted_players.join("\n"));
  }, [settings]);

  useEffect(() => {
    if (!notice) {
      return;
    }
    const timeout = window.setTimeout(() => setNotice(null), 2500);
    return () => window.clearTimeout(timeout);
  }, [notice]);

  const trustMode = draft.trust_mode;
  const trustedPlayers = normalizeTrustedPlayers(trustedPlayersText);

  async function handleSave() {
    const next = {
      ...draft,
      trusted_players: trustedPlayers,
    };
    if (await saveSettings(next)) {
      setNotice("Auto-accept ward profile saved");
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
                  <ShieldCheck size={20} weight="fill" />
                </div>
                <div>
                  <h2 className="font-archaic text-2xl tracking-wide text-white">
                    Auto-Accept Wards
                  </h2>
                  <p className="mt-1 text-xs uppercase tracking-[0.3em] text-white/45">
                    MQ2AutoAccept parity controls for unattended boxes
                  </p>
                </div>
              </div>
              <p className="mt-5 max-w-3xl text-sm leading-6 text-white/60">
                Define which prompts the client accepts automatically and whether
                those requests are open to everyone or limited to a trusted roster.
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
                {saving ? "Saving" : "Save Wards"}
              </button>
            </div>
          </div>
        </header>

        <div className={`flex-1 overflow-y-auto ${embedded ? "px-6 py-5" : "px-8 py-6"}`}>
          <div className={`grid gap-6 ${embedded ? "" : "grid-cols-[1.5fr_1fr]"}`}>
            <div className="space-y-6">
              <div className="border border-white/10 bg-violet/20 p-5">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <h3 className="font-archaic text-lg text-white">
                      Master Ward
                    </h3>
                    <p className="mt-2 text-sm leading-6 text-white/55">
                      Disable this to ignore every supported prompt without
                      changing the per-request policy underneath it.
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

              <div>
                <div className="mb-4 flex items-center justify-between">
                  <div>
                    <h3 className="font-archaic text-lg text-white">
                      Accepted Prompt Types
                    </h3>
                    <p className="mt-1 text-xs uppercase tracking-[0.18em] text-white/35">
                      Toggle each request family independently
                    </p>
                  </div>
                  <span className="text-xs font-rune text-white/35">
                    {REQUEST_TOGGLES.filter(({ key }) => draft[key] === true).length}/6 active
                  </span>
                </div>
                <div className={`grid gap-4 ${embedded ? "sm:grid-cols-2" : "grid-cols-2"}`}>
                  {REQUEST_TOGGLES.map(({ key, label, detail, icon }) => (
                    <ToggleCard
                      key={key}
                      enabled={Boolean(draft[key])}
                      label={label}
                      detail={detail}
                      icon={icon}
                      onToggle={() =>
                        setDraft((current) => ({
                          ...current,
                          [key]: !current[key],
                        }))
                      }
                    />
                  ))}
                </div>
              </div>
            </div>

            <div className="space-y-6">
              <div className="border border-white/10 bg-violet/20 p-5">
                <h3 className="font-archaic text-lg text-white">
                  Trust Policy
                </h3>
                <p className="mt-2 text-sm leading-6 text-white/55">
                  Trust-list mode blocks requests that do not name a sender or
                  come from someone outside the configured roster.
                </p>

                <div className="mt-5 grid grid-cols-2 gap-3">
                  <button
                    onClick={() =>
                      setDraft((current) => ({ ...current, trust_mode: "anyone" }))
                    }
                    className={`border px-4 py-3 text-left transition-all ${
                      trustMode === "anyone"
                        ? "border-magentaglow/40 bg-magentadark/20"
                        : "border-white/10 bg-void/60 hover:border-white/25"
                    }`}
                  >
                    <div className="text-xs uppercase tracking-[0.25em] text-white/40">
                      Anyone
                    </div>
                    <div className="mt-2 text-sm text-white/80">
                      Accept allowed prompt types from any sender.
                    </div>
                  </button>

                  <button
                    onClick={() =>
                      setDraft((current) => ({
                        ...current,
                        trust_mode: "trust_list",
                      }))
                    }
                    className={`border px-4 py-3 text-left transition-all ${
                      trustMode === "trust_list"
                        ? "border-spectral/40 bg-spectral/10"
                        : "border-white/10 bg-void/60 hover:border-white/25"
                    }`}
                  >
                    <div className="text-xs uppercase tracking-[0.25em] text-white/40">
                      Trust List
                    </div>
                    <div className="mt-2 text-sm text-white/80">
                      Require the sender to match a trusted character name.
                    </div>
                  </button>
                </div>
              </div>

              <div className="border border-white/10 bg-void/60 p-5">
                <h3 className="font-archaic text-lg text-white">
                  Trusted Players
                </h3>
                <p className="mt-2 text-sm leading-6 text-white/55">
                  Enter one character per line or separate them with commas. Names
                  are matched case-insensitively.
                </p>
                <textarea
                  value={trustedPlayersText}
                  onChange={(event) => setTrustedPlayersText(event.target.value)}
                  placeholder={"Leaderone\nClericone\nWizardport"}
                  className="mt-4 h-52 w-full resize-none border border-white/15 bg-black/30 px-4 py-3 font-rune text-sm text-white outline-none transition-colors placeholder:text-white/20 focus:border-spectral/35"
                />
                <div className="mt-4 flex flex-wrap gap-2">
                  {trustedPlayers.length > 0 ? (
                    trustedPlayers.map((player) => (
                      <span
                        key={player}
                        className="border border-spectral/30 bg-spectral/10 px-2 py-1 text-[10px] uppercase tracking-[0.22em] text-spectral"
                      >
                        {player}
                      </span>
                    ))
                  ) : (
                    <span className="text-xs text-white/30">
                      No trusted players configured.
                    </span>
                  )}
                </div>
              </div>

              <div className="border border-white/10 bg-violet/20 p-5">
                <div className="flex items-center gap-3">
                  <ShieldCheck size={16} className="text-spectral" weight="fill" />
                  <h3 className="font-archaic text-lg text-white">
                    Current Ward Summary
                  </h3>
                </div>
                <dl className="mt-4 space-y-3 text-sm">
                  <div className="flex items-center justify-between border-b border-white/5 pb-3">
                    <dt className="text-white/45">Master state</dt>
                    <dd className="font-rune uppercase tracking-[0.2em] text-white">
                      {draft.enabled ? "armed" : "offline"}
                    </dd>
                  </div>
                  <div className="flex items-center justify-between border-b border-white/5 pb-3">
                    <dt className="text-white/45">Trust mode</dt>
                    <dd className="font-rune uppercase tracking-[0.2em] text-white">
                      {trustMode === "trust_list" ? "trust list" : "anyone"}
                    </dd>
                  </div>
                  <div className="flex items-center justify-between">
                    <dt className="text-white/45">Trusted entries</dt>
                    <dd className="font-rune uppercase tracking-[0.2em] text-white">
                      {trustedPlayers.length}
                    </dd>
                  </div>
                </dl>
              </div>
            </div>
          </div>

          {(error || notice || loading) && (
            <div className="mt-6 flex flex-wrap gap-3">
              {loading && (
                <div className="border border-white/10 bg-white/5 px-3 py-2 text-xs uppercase tracking-[0.18em] text-white/45">
                  Loading ward profile
                </div>
              )}
              {notice && (
                <div className="border border-spectral/30 bg-spectral/10 px-3 py-2 text-xs uppercase tracking-[0.18em] text-spectral">
                  {notice}
                </div>
              )}
              {error && (
                <div className="border border-yellow-500/30 bg-yellow-500/10 px-3 py-2 text-xs tracking-[0.08em] text-yellow-200">
                  {error}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
