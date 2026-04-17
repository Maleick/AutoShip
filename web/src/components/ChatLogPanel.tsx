import { useEffect, useState } from "react";
import { FloppyDisk, FileText, Clock, Warning } from "@phosphor-icons/react";
import type { ChatLogConfig, ChatChannel, LogLevel, RotationStrategy } from "../types";

const DEFAULT_CONFIG: ChatLogConfig = {
  enabled: false,
  channels: ["mq2"],
  rotation_strategy: { size: 10 * 1024 * 1024 },
  max_file_size_bytes: 10 * 1024 * 1024,
  min_level: "info",
  log_eq_chat: false,
};

const ALL_CHANNELS: { id: ChatChannel; label: string }[] = [
  { id: "mq2", label: "MQ2 Output" },
  { id: "say", label: "Say" },
  { id: "tell", label: "Tell" },
  { id: "group", label: "Group" },
  { id: "raid", label: "Raid" },
  { id: "guild", label: "Guild" },
  { id: "ooc", label: "OOC" },
  { id: "shout", label: "Shout" },
  { id: "auction", label: "Auction" },
  { id: "pet", label: "Pet" },
];

const LOG_LEVELS: { id: LogLevel; label: string }[] = [
  { id: "trace", label: "Trace" },
  { id: "debug", label: "Debug" },
  { id: "info", label: "Info" },
  { id: "warn", label: "Warn" },
  { id: "error", label: "Error" },
];

function StatusBanner({
  tone,
  text,
}: {
  tone: "neutral" | "success" | "error";
  text: string;
}) {
  const className =
    tone === "success"
      ? "border-emerald-400/30 bg-emerald-500/10 text-emerald-200"
      : tone === "error"
        ? "border-rose-400/30 bg-rose-500/10 text-rose-200"
        : "border-white/10 bg-white/5 text-white/70";

  return <div className={`border px-4 py-3 text-sm ${className}`}>{text}</div>;
}

function ToggleField({
  label,
  value,
  onChange,
  detail,
}: {
  label: string;
  value: boolean;
  onChange: (value: boolean) => void;
  detail: string;
}) {
  return (
    <label className="flex items-start justify-between gap-4 rounded-2xl border border-white/10 bg-[#0d0715] px-4 py-3">
      <div>
        <div className="text-sm font-semibold text-white">{label}</div>
        <div className="mt-1 text-xs uppercase tracking-[0.18em] text-white/45">
          {detail}
        </div>
      </div>
      <input
        type="checkbox"
        checked={value}
        onChange={(event) => onChange(event.target.checked)}
        className="mt-1 h-4 w-4 accent-cyan-400"
      />
    </label>
  );
}

function SelectField({
  label,
  value,
  onChange,
  options,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: { id: string; label: string }[];
}) {
  return (
    <label className="flex flex-col gap-2">
      <span className="text-xs uppercase tracking-[0.18em] text-white/45">{label}</span>
      <select
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className="rounded-2xl border border-white/15 bg-[#0d0715] px-3 py-2 text-white outline-none transition-colors focus:border-cyan-300/40"
      >
        {options.map((opt) => (
          <option key={opt.id} value={opt.id}>
            {opt.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function NumberField({
  label,
  value,
  onChange,
  suffix,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
  suffix?: string;
}) {
  return (
    <label className="flex flex-col gap-2">
      <span className="text-xs uppercase tracking-[0.18em] text-white/45">
        {label}
      </span>
      <div className="flex items-center gap-2">
        <input
          type="number"
          value={value}
          onChange={(event) => onChange(Number(event.target.value))}
          className="flex-1 rounded-2xl border border-white/15 bg-[#0d0715] px-3 py-2 text-white outline-none transition-colors focus:border-cyan-300/40"
        />
        {suffix && <span className="text-sm text-white/45">{suffix}</span>}
      </div>
    </label>
  );
}

export default function ChatLogPanel() {
  const [config, setConfig] = useState<ChatLogConfig>(DEFAULT_CONFIG);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  useEffect(() => {
    async function loadConfig() {
      setLoading(true);
      try {
        const res = await fetch("/api/chat-log/settings");
        if (res.ok) {
          const data = await res.json();
          setConfig(data);
        }
      } catch {
        // Use defaults
      } finally {
        setLoading(false);
      }
    }
    loadConfig();
  }, []);

  async function handleSave() {
    setSaving(true);
    setError(null);
    try {
      const res = await fetch("/api/chat-log/settings", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(config),
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error ?? `HTTP ${res.status}`);
      }
      setSavedAt(Date.now());
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to save chat log settings");
    } finally {
      setSaving(false);
    }
  }

  function toggleChannel(channel: ChatChannel) {
    setConfig((prev) => ({
      ...prev,
      channels: prev.channels.includes(channel)
        ? prev.channels.filter((c) => c !== channel)
        : [...prev.channels, channel],
    }));
  }

  function formatRotationStrategy(strategy: RotationStrategy): string {
    if (strategy === "none") return "none";
    if ("daily" in strategy) return "daily";
    if ("size" in strategy) {
      const mb = Math.round(strategy.size / (1024 * 1024));
      return `size:${mb}`;
    }
    return "daily";
  }

  function parseRotationStrategy(value: string): RotationStrategy {
    if (value === "none") return "none";
    if (value === "daily") return { daily: null };
    if (value.startsWith("size:")) {
      const mb = parseInt(value.replace("size:", ""), 10);
      return { size: mb * 1024 * 1024 };
    }
    return { size: 10 * 1024 * 1024 };
  }

  const rotationDisplay = formatRotationStrategy(config.rotation_strategy);

  return (
    <section className="rounded-[1.5rem] border border-cyan-400/20 bg-[#120a1d]/88 p-5 shadow-[0_12px_40px_rgba(34,211,238,0.08)] backdrop-blur">
      <div className="flex items-center justify-between gap-6 border-b border-white/10 pb-6">
        <div>
          <div className="flex items-center gap-3 text-cyan-200">
            <FileText size={20} weight="fill" />
            <span className="text-xs uppercase tracking-[0.32em] text-white/45">
              Chat Logging
            </span>
          </div>
          <h2 className="mt-3 font-archaic text-3xl uppercase tracking-[0.12em] text-white">
            Per-Character Chat Log
          </h2>
          <p className="mt-3 max-w-3xl text-sm leading-6 text-white/65">
            Write chat output to <code>logs/server_charname.log</code> files.
            Log rotation and level filtering supported. Config persisted to{" "}
            <code>config/textquest.toml</code>.
          </p>
        </div>

        <button
          type="button"
          onClick={handleSave}
          disabled={saving}
          className="inline-flex items-center gap-2 rounded-full border border-cyan-300/30 bg-cyan-300/10 px-4 py-2 text-sm font-semibold text-cyan-100 transition-colors hover:bg-cyan-300/20 disabled:cursor-not-allowed disabled:opacity-60"
        >
          <FloppyDisk size={16} />
          {saving ? "Saving..." : "Save Settings"}
        </button>
      </div>

      <div className="mt-6 space-y-4">
        {loading ? (
          <StatusBanner tone="neutral" text="Loading chat log settings..." />
        ) : error ? (
          <StatusBanner tone="error" text={error} />
        ) : savedAt ? (
          <StatusBanner tone="success" text={`Saved ${new Date(savedAt).toLocaleTimeString()}`} />
        ) : null}
      </div>

      <div className="mt-8 space-y-6">
        <ToggleField
          label="Enable chat logging"
          value={config.enabled}
          onChange={(enabled) => setConfig((prev) => ({ ...prev, enabled }))}
          detail="Write chat output to log files per character"
        />

        <ToggleField
          label="Log EQ chat channels"
          value={config.log_eq_chat}
          onChange={(log_eq_chat) => setConfig((prev) => ({ ...prev, log_eq_chat }))}
          detail="Capture regular game chat (say, tell, group, etc.) in addition to MQ2 output"
        />

        <div className="rounded-3xl border border-white/10 bg-[#0d0715] p-5">
          <h3 className="mb-4 font-archaic text-lg uppercase tracking-[0.16em] text-white">
            Channels to Log
          </h3>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-5">
            {ALL_CHANNELS.map((ch) => (
              <label
                key={ch.id}
                className={`flex items-center gap-2 rounded-xl border px-3 py-2 cursor-pointer transition-colors ${
                  config.channels.includes(ch.id)
                    ? "border-cyan-400/40 bg-cyan-400/10 text-cyan-200"
                    : "border-white/10 bg-white/5 text-white/60 hover:border-white/20"
                }`}
              >
                <input
                  type="checkbox"
                  checked={config.channels.includes(ch.id)}
                  onChange={() => toggleChannel(ch.id)}
                  className="h-3 w-3 accent-cyan-400"
                />
                <span className="text-sm">{ch.label}</span>
              </label>
            ))}
          </div>
        </div>

        <div className="grid gap-4 md:grid-cols-3">
          <SelectField
            label="Rotation Strategy"
            value={rotationDisplay}
            onChange={(value) =>
              setConfig((prev) => ({
                ...prev,
                rotation_strategy: parseRotationStrategy(value),
              }))
            }
            options={[
              { id: "daily", label: "Daily" },
              { id: "size:10", label: "10 MB" },
              { id: "size:25", label: "25 MB" },
              { id: "size:50", label: "50 MB" },
              { id: "none", label: "None (append)" },
            ]}
          />

          <SelectField
            label="Minimum Log Level"
            value={config.min_level}
            onChange={(value) =>
              setConfig((prev) => ({ ...prev, min_level: value as LogLevel }))
            }
            options={LOG_LEVELS}
          />

          <NumberField
            label="Max File Size (MB)"
            value={Math.round(config.max_file_size_bytes / (1024 * 1024))}
            onChange={(value) =>
              setConfig((prev) => ({
                ...prev,
                max_file_size_bytes: value * 1024 * 1024,
              }))
            }
            suffix="MB"
          />
        </div>

        <div className="rounded-3xl border border-white/10 bg-[#0d0715] p-5">
          <div className="flex items-center gap-3 text-cyan-200">
            <Clock size={18} weight="bold" />
            <h3 className="font-archaic text-xl uppercase tracking-[0.16em] text-white">
              Log File Format
            </h3>
          </div>
          <ul className="mt-4 space-y-3 text-sm leading-6 text-white/65">
            <li>
              Files are written to the <code>logs/</code> directory relative to the
              TextQuest executable.
            </li>
            <li>
              File naming: <code>server_character.log</code> (e.g.,{" "}
              <code>Firiona Vie_Kira.log</code>).
            </li>
            <li>
              Each log entry is timestamped with millisecond precision:{" "}
              <code>2024-01-15 14:32:05.123 [INFO] message</code>.
            </li>
            <li>
              <Warning size={14} className="inline text-amber-400" /> Log files can grow
              large during long sessions. Use rotation to manage disk usage.
            </li>
          </ul>
        </div>
      </div>
    </section>
  );
}
