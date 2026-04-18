import { useEffect, useState } from "react";
import { FloppyDisk, Notepad } from "@phosphor-icons/react";
import type { ChatChannel, ChatLogSettings, LogLevel, LogRotation } from "../types";
import { useChatLogSettings } from "../hooks/useChatLogSettings";

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
        className="mt-1 h-4 w-4 accent-magentaglow"
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
  options: { value: string; label: string }[];
}) {
  return (
    <label className="flex flex-col gap-2">
      <span className="text-xs uppercase tracking-[0.18em] text-white/45">
        {label}
      </span>
      <select
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className="rounded-2xl border border-white/15 bg-[#0d0715] px-3 py-2 text-white outline-none transition-colors focus:border-cyan-300/40"
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
    </label>
  );
}

const ALL_CHANNELS: { value: ChatChannel; label: string }[] = [
  { value: "say", label: "Say" },
  { value: "tell", label: "Tell (incoming)" },
  { value: "tell_out", label: "Tell (outgoing)" },
  { value: "group", label: "Group" },
  { value: "guild", label: "Guild" },
  { value: "raid", label: "Raid" },
  { value: "shout", label: "Shout" },
  { value: "ooc", label: "OOC" },
  { value: "auction", label: "Auction" },
];

export default function ChatLogPanel() {
  const { settings, loading, saving, error, savedAt, save, toggleChannel } =
    useChatLogSettings();
  const [draft, setDraft] = useState<ChatLogSettings>(settings);

  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  const canSave = !loading && !saving;

  async function handleSave() {
    if (!canSave) {
      return;
    }

    try {
      await save(draft);
    } catch {
      // Error is handled by the hook
    }
  }

  function updateRotation(rotation: LogRotation) {
    setDraft((prev) => ({ ...prev, rotation }));
  }

  function updateLevel(level: LogLevel) {
    setDraft((prev) => ({ ...prev, level }));
  }

  const rotationType =
    draft.rotation.type === "by_size" ? "by_size" : draft.rotation.type;

  return (
    <section className="rounded-[1.5rem] border border-fuchsia-400/20 bg-[#120a1d]/88 p-5 shadow-[0_12px_40px_rgba(217,70,239,0.08)] backdrop-blur">
      <div className="flex items-center justify-between gap-6 border-b border-white/10 pb-6">
        <div>
          <div className="flex items-center gap-3 text-fuchsia-200">
            <Notepad size={20} weight="fill" />
            <span className="text-xs uppercase tracking-[0.32em] text-white/45">
              MQ2Log Parity
            </span>
          </div>
          <h2 className="mt-3 font-archaic text-3xl uppercase tracking-[0.12em] text-white">
            Chat Output Logging
          </h2>
          <p className="mt-3 max-w-3xl text-sm leading-6 text-white/65">
            Persist per-character chat logs to <code>logs/server_charname.log</code>.{" "}
            Configure rotation strategy and channel filtering to control what gets
            logged.
          </p>
        </div>

        <button
          type="button"
          onClick={handleSave}
          disabled={!canSave}
          className="inline-flex items-center gap-2 rounded-full border border-fuchsia-300/30 bg-fuchsia-300/10 px-4 py-2 text-sm font-semibold text-fuchsia-100 transition-colors hover:bg-fuchsia-300/20 disabled:cursor-not-allowed disabled:opacity-60"
        >
          <FloppyDisk size={16} />
          {saving ? "Saving..." : "Save Settings"}
        </button>
      </div>

      <div className="mt-6 space-y-4">
        {loading ? (
          <StatusBanner tone="neutral" text="Loading persisted chat log settings..." />
        ) : error ? (
          <StatusBanner tone="error" text={error} />
        ) : savedAt ? (
          <StatusBanner tone="success" text={`Saved ${new Date(savedAt).toLocaleTimeString()}`} />
        ) : (
          <StatusBanner
            tone="neutral"
            text="Enable chat logging, choose your rotation strategy, and select which channels to capture."
          />
        )}
      </div>

      <div className="mt-8 grid gap-6 xl:grid-cols-[1fr_1.2fr]">
        <div className="space-y-4">
          <ToggleField
            label="Enable chat output logging"
            value={draft.enabled}
            onChange={(enabled) => setDraft((prev) => ({ ...prev, enabled }))}
            detail="Writes all captured chat to logs/server_charname.log"
          />

          <SelectField
            label="Log Rotation"
            value={rotationType}
            onChange={(value) => {
              if (value === "none") {
                updateRotation({ type: "none" });
              } else if (value === "daily") {
                updateRotation({ type: "daily" });
              } else {
                updateRotation({ type: "by_size", size: 5 * 1024 * 1024 });
              }
            }}
            options={[
              { value: "none", label: "No rotation (append indefinitely)" },
              { value: "daily", label: "Daily rotation" },
              { value: "by_size", label: "By file size (5 MB)" },
            ]}
          />

          <SelectField
            label="Log Level"
            value={draft.level}
            onChange={(value) => updateLevel(value as LogLevel)}
            options={[
              { value: "info", label: "Info (standard output)" },
              { value: "debug", label: "Debug (with server/character metadata)" },
            ]}
          />
        </div>

        <div className="space-y-4">
          <div className="text-xs uppercase tracking-[0.18em] text-white/45">
            Chat Channels
          </div>
          <div className="grid grid-cols-2 gap-2">
            {ALL_CHANNELS.map((channel) => (
              <label
                key={channel.value}
                className="flex items-center gap-2 rounded-xl border border-white/10 bg-[#0d0715] px-3 py-2 cursor-pointer hover:border-white/20 transition-colors"
              >
                <input
                  type="checkbox"
                  checked={draft.channels.length === 0 || draft.channels.includes(channel.value)}
                  onChange={() => toggleChannel(channel.value)}
                  className="h-4 w-4 accent-magentaglow"
                />
                <span className="text-sm text-white/80">{channel.label}</span>
              </label>
            ))}
          </div>
          <p className="text-xs text-white/40">
            Leave all unchecked to log all channels. Selected channels are
            included when list is non-empty.
          </p>
        </div>
      </div>
    </section>
  );
}
