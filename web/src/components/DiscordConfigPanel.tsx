import { useEffect, useState, type ElementType, type ReactNode } from "react";
import {
  ArrowClockwise,
  BellRinging,
  Broadcast,
  CheckCircle,
  FloppyDisk,
  Link,
  ShieldWarning,
  Skull,
  Warning,
} from "@phosphor-icons/react";

import { useDiscordConfig } from "../hooks/useDiscordConfig";
import type {
  DiscordMentionPolicy,
  DiscordMessageMode,
  DiscordRouteConfig,
  DiscordSettings,
  DiscordSeverity,
} from "../types";

const CATEGORY_META = [
  {
    key: "kills",
    label: "Kill Feed",
    detail: "Encounter kills and DPS summary posts.",
  },
  {
    key: "loot",
    label: "Loot Feed",
    detail: "Item drops and looter summaries.",
  },
  {
    key: "timers",
    label: "Timer Feed",
    detail: "Dynamic zone lockouts and timer warnings.",
  },
  {
    key: "feats",
    label: "Feat Feed",
    detail: "Level-ups and achievement style updates.",
  },
  {
    key: "status",
    label: "Status Feed",
    detail: "Fallback operational messages and generic alerts.",
  },
] as const;

const ROUTE_META: {
  key: string;
  label: string;
  detail: string;
  Icon: ElementType;
}[] = [
  {
    key: "death",
    label: "Death Alerts",
    detail: "High-priority death and corpse notifications.",
    Icon: Skull,
  },
  {
    key: "status",
    label: "Info Alerts",
    detail: "General status updates and informational events.",
    Icon: BellRinging,
  },
  {
    key: "hvt",
    label: "HVT Alerts",
    detail: "Named spawn and high-value target sightings.",
    Icon: Broadcast,
  },
  {
    key: "crash",
    label: "Crash Alerts",
    detail: "Client crash and disconnect recovery notifications.",
    Icon: Warning,
  },
  {
    key: "mass_failure",
    label: "Mass Failure Alerts",
    detail: "Burst failure detection and escalation notices.",
    Icon: ShieldWarning,
  },
];

const SEVERITY_OPTIONS: { value: DiscordSeverity; label: string }[] = [
  { value: "INFO", label: "Info" },
  { value: "WARNING", label: "Warning" },
  { value: "ERROR", label: "Error" },
  { value: "CRITICAL", label: "Critical" },
];

const MESSAGE_MODE_OPTIONS: { value: DiscordMessageMode; label: string }[] = [
  { value: "rich_embed", label: "Rich Embed" },
  { value: "plain_text", label: "Plain Text" },
];

const MENTION_OPTIONS: { value: DiscordMentionPolicy; label: string }[] = [
  { value: "none", label: "No Ping" },
  { value: "everyone", label: "@everyone" },
];

function cloneSettings(settings: DiscordSettings): DiscordSettings {
  return {
    webhook_url: settings.webhook_url,
    channels: { ...settings.channels },
    notification_routes: Object.fromEntries(
      Object.entries(settings.notification_routes).map(([key, route]) => [
        key,
        { ...route },
      ]),
    ),
  };
}

function Section({
  title,
  icon: Icon,
  children,
}: {
  title: string;
  icon: ElementType;
  children: ReactNode;
}) {
  return (
    <div className="bg-violet/20 border border-white/8 p-5">
      <h3 className="font-archaic text-sm text-white/70 uppercase tracking-widest flex items-center gap-2 mb-4 pb-2 border-b border-white/8">
        <Icon size={14} className="text-magentaglow" weight="fill" />
        {title}
      </h3>
      {children}
    </div>
  );
}

function FieldLabel({
  title,
  detail,
}: {
  title: string;
  detail: string;
}) {
  return (
    <div>
      <div className="text-sm text-white font-medium">{title}</div>
      <div className="text-[11px] text-white/35 font-rune mt-1">{detail}</div>
    </div>
  );
}

function TextInput({
  value,
  placeholder,
  onChange,
}: {
  value: string;
  placeholder: string;
  onChange: (value: string) => void;
}) {
  return (
    <input
      type="url"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune placeholder:text-white/25"
    />
  );
}

function SelectField<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value as T)}
      className="w-full bg-void border border-white/20 text-white text-xs px-3 py-2 focus:outline-none focus:border-magentaglow font-tech uppercase tracking-wide"
    >
      {options.map((option) => (
        <option key={option.value} value={option.value} className="bg-[#0d0b1a]">
          {option.label}
        </option>
      ))}
    </select>
  );
}

export default function DiscordConfigPanel() {
  const { config, loading, saving, error, saveConfig, refresh } = useDiscordConfig();
  const [draft, setDraft] = useState<DiscordSettings>(config);
  const [saveSuccess, setSaveSuccess] = useState(false);

  useEffect(() => {
    setDraft(cloneSettings(config));
  }, [config]);

  const configuredChannelCount = Object.values(draft.channels).filter(Boolean).length;
  const configuredRouteOverrides = Object.values(draft.notification_routes).filter(
    (route) => route.webhook_url.trim() !== "",
  ).length;
  const everyoneRoutes = Object.values(draft.notification_routes).filter(
    (route) => route.enabled && route.mention_policy === "everyone",
  ).length;
  const isDirty = JSON.stringify(draft) !== JSON.stringify(config);

  const updateChannel = (key: string, value: string) => {
    setDraft((prev) => ({
      ...prev,
      channels: {
        ...prev.channels,
        [key]: value,
      },
    }));
  };

  const updateRoute = (key: string, patch: Partial<DiscordRouteConfig>) => {
    setDraft((prev) => ({
      ...prev,
      notification_routes: {
        ...prev.notification_routes,
        [key]: {
          ...prev.notification_routes[key],
          ...patch,
        },
      },
    }));
  };

  const handleSave = async () => {
    const saved = await saveConfig(draft);
    if (!saved) return;
    setSaveSuccess(true);
    setTimeout(() => setSaveSuccess(false), 2000);
  };

  if (loading) {
    return (
      <div className="flex min-h-48 items-center justify-center rounded-[1.8rem] border border-white/10 bg-[linear-gradient(135deg,rgba(19,11,35,0.96),rgba(9,7,19,0.92))] px-6 py-10 text-white/40 shadow-[0_16px_60px_rgba(0,0,0,0.32)]">
        Loading Discord webhook wards…
      </div>
    );
  }

  return (
    <section className="relative z-20 overflow-hidden rounded-[1.8rem] border border-white/10 bg-[linear-gradient(135deg,rgba(19,11,35,0.96),rgba(9,7,19,0.92))] shadow-[0_16px_60px_rgba(0,0,0,0.32)]">
      <header className="flex flex-wrap items-center justify-between gap-4 border-b border-white/10 bg-violet/30 px-6 py-4 backdrop-blur-md">
        <div className="flex items-center gap-4">
          <div className="w-8 h-8 rounded border border-magentadark flex items-center justify-center bg-void">
            <ShieldWarning weight="fill" className="text-magentaglow" />
          </div>
          <div>
            <h2 className="font-archaic text-lg text-white leading-tight">
              Security Wards
            </h2>
            <p className="text-[10px] uppercase tracking-widest text-white/50 font-rune">
              Discord webhook routing and escalation policy
            </p>
          </div>
        </div>
        <div className="flex gap-3 items-center">
          {error && (
            <span className="flex items-center gap-1 text-xs text-yellow-400 font-tech">
              <Warning size={12} weight="fill" /> {error}
            </span>
          )}
          <button
            onClick={refresh}
            title="Refresh from server"
            className="px-3 py-1.5 border border-white/20 text-white/50 text-sm hover:border-spectral hover:text-spectral transition-colors"
          >
            <ArrowClockwise size={14} />
          </button>
          <button
            onClick={handleSave}
            disabled={saving || !isDirty}
            className={`px-4 py-1.5 border text-sm font-medium uppercase tracking-wider flex items-center gap-2 transition-all ${
              saveSuccess
                ? "border-green-500 text-green-400 bg-green-900/20"
                : saving || !isDirty
                  ? "border-white/20 text-white/30"
                  : "bg-magentadark/20 border-magentaglow text-white hover:bg-magentadark/40 shadow-[0_0_15px_rgba(204,68,255,0.3)]"
            }`}
          >
            <FloppyDisk size={14} weight="fill" />
            {saveSuccess ? "Saved!" : saving ? "Saving…" : "Save Wards"}
          </button>
        </div>
      </header>

      <div className="flex items-center gap-6 px-6 py-3 border-b border-white/5 bg-violet/10">
        <span className="text-xs font-rune text-spectral flex items-center gap-1.5">
          <Link size={12} weight="fill" /> {configuredChannelCount} category feeds mapped
        </span>
        <span className="text-xs font-rune text-magentaglow flex items-center gap-1.5">
          <Broadcast size={12} weight="fill" /> {configuredRouteOverrides} route overrides
        </span>
        <span className="text-xs font-rune text-yellow-300 flex items-center gap-1.5">
          <BellRinging size={12} weight="fill" /> {everyoneRoutes} routes ping everyone
        </span>
        <span className="ml-auto text-[11px] text-white/35 font-rune">
          Discord limit: 30 requests/minute per webhook
        </span>
      </div>

      <div className="space-y-6 p-6">
        <Section title="Default Webhook" icon={Link}>
          <div className="space-y-3">
            <FieldLabel
              title="Fallback webhook URL"
              detail="Used whenever a category feed or alert route does not set its own webhook override."
            />
            <TextInput
              value={draft.webhook_url}
              onChange={(value) => setDraft((prev) => ({ ...prev, webhook_url: value }))}
              placeholder="https://discord.com/api/webhooks/..."
            />
          </div>
        </Section>

        <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)]">
          <Section title="Category Channels" icon={Broadcast}>
            <div className="space-y-4">
              {CATEGORY_META.map((category) => (
                <div key={category.key} className="space-y-2">
                  <FieldLabel title={category.label} detail={category.detail} />
                  <TextInput
                    value={draft.channels[category.key] ?? ""}
                    onChange={(value) => updateChannel(category.key, value)}
                    placeholder="Blank uses the fallback webhook"
                  />
                </div>
              ))}
            </div>
          </Section>

          <Section title="Per-Event Routing" icon={BellRinging}>
            <div className="space-y-4">
              {ROUTE_META.map(({ key, label, detail, Icon }) => {
                const route = draft.notification_routes[key];
                const disabled = !route?.enabled;

                return (
                  <div
                    key={key}
                    className={`border p-4 transition-colors ${
                      disabled
                        ? "border-white/5 bg-void/30"
                        : "border-white/10 bg-void/50"
                    }`}
                  >
                    <div className="flex items-start gap-3 justify-between mb-4">
                      <div className="flex items-start gap-3">
                        <div className="w-9 h-9 border border-white/10 bg-void flex items-center justify-center">
                          <Icon size={16} className="text-magentaglow" weight="fill" />
                        </div>
                        <FieldLabel title={label} detail={detail} />
                      </div>
                      <button
                        onClick={() => updateRoute(key, { enabled: !route.enabled })}
                        className={`px-3 py-1.5 text-[11px] font-tech uppercase tracking-widest border transition-colors ${
                          route.enabled
                            ? "border-green-500/40 bg-green-900/20 text-green-300"
                            : "border-white/10 bg-void text-white/35 hover:text-white/60"
                        }`}
                      >
                        {route.enabled ? "Enabled" : "Disabled"}
                      </button>
                    </div>

                    <div className="grid grid-cols-3 gap-3 mb-3">
                      <div>
                        <div className="text-[10px] text-white/45 uppercase tracking-widest font-tech mb-2">
                          Severity
                        </div>
                        <SelectField
                          value={route.level}
                          options={SEVERITY_OPTIONS}
                          onChange={(value) => updateRoute(key, { level: value })}
                        />
                      </div>
                      <div>
                        <div className="text-[10px] text-white/45 uppercase tracking-widest font-tech mb-2">
                          Payload
                        </div>
                        <SelectField
                          value={route.message_mode}
                          options={MESSAGE_MODE_OPTIONS}
                          onChange={(value) => updateRoute(key, { message_mode: value })}
                        />
                      </div>
                      <div>
                        <div className="text-[10px] text-white/45 uppercase tracking-widest font-tech mb-2">
                          Mention
                        </div>
                        <SelectField
                          value={route.mention_policy}
                          options={MENTION_OPTIONS}
                          onChange={(value) => updateRoute(key, { mention_policy: value })}
                        />
                      </div>
                    </div>

                    <div className="space-y-2">
                      <div className="text-[10px] text-white/45 uppercase tracking-widest font-tech">
                        Route webhook override
                      </div>
                      <TextInput
                        value={route.webhook_url}
                        onChange={(value) => updateRoute(key, { webhook_url: value })}
                        placeholder="Blank uses the fallback or category webhook"
                      />
                    </div>
                  </div>
                );
              })}
            </div>
          </Section>
        </div>

        <Section title="Delivery Notes" icon={CheckCircle}>
          <div className="grid gap-4 text-sm md:grid-cols-3">
            <div className="border border-white/10 bg-void/40 p-4">
              <div className="text-xs uppercase tracking-widest text-white/45 font-tech mb-2">
                Death escalation
              </div>
              <p className="text-white/70 font-rune">
                Use the <span className="text-white">@everyone</span> mention on the{" "}
                <span className="text-magentaglow">Death Alerts</span> route to mirror MQ2Discord-style
                urgent pings.
              </p>
            </div>
            <div className="border border-white/10 bg-void/40 p-4">
              <div className="text-xs uppercase tracking-widest text-white/45 font-tech mb-2">
                Info routing
              </div>
              <p className="text-white/70 font-rune">
                Keep informational routes on <span className="text-spectral">Rich Embed</span> with
                <span className="text-white"> No Ping</span> to avoid noisy status spam.
              </p>
            </div>
            <div className="border border-white/10 bg-void/40 p-4">
              <div className="text-xs uppercase tracking-widest text-white/45 font-tech mb-2">
                Rate limit
              </div>
              <p className="text-white/70 font-rune">
                The sender enforces Discord’s 30 requests/minute limit independently for each webhook URL.
              </p>
            </div>
          </div>
        </Section>
      </div>
    </section>
  );
}
