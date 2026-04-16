import { useEffect, useState } from "react";
import { Clock, FloppyDisk, Info } from "@phosphor-icons/react";
import type { TimestampConfig, TimestampFormat } from "../types";
import { useTimestampConfig } from "../hooks/useTimestampConfig";

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

const FORMAT_OPTIONS: { value: TimestampFormat; label: string; example: string }[] = [
  { value: "date_time_24", label: "Date + 24h Time", example: "2026-04-15 14:30:45" },
  { value: "time_24", label: "24h Time Only", example: "14:30:45" },
  { value: "date_time_12", label: "Date + 12h Time", example: "2026-04-15 02:30:45 PM" },
  { value: "time_12", label: "12h Time Only", example: "02:30:45 PM" },
];

interface TimestampPanelProps {
  character: string;
}

export default function TimestampPanel({ character }: TimestampPanelProps) {
  const { config, loading, saving, error, savedAt, save, setFormat } = useTimestampConfig(character);
  const [draft, setDraft] = useState<TimestampConfig>(config);

  useEffect(() => {
    setDraft(config);
  }, [config]);

  const canSave = !loading && !saving;

  async function handleSave() {
    if (!canSave) return;
    try {
      await save(draft);
    } catch {
      // error state is handled by the hook
    }
  }

  return (
    <section className="rounded-[1.5rem] border border-cyan-400/20 bg-[#120a1d]/88 p-5 shadow-[0_12px_40px_rgba(34,211,238,0.08)] backdrop-blur">
      <div className="flex items-center justify-between gap-6 border-b border-white/10 pb-6">
        <div>
          <div className="flex items-center gap-3 text-cyan-200">
            <Clock size={20} weight="fill" />
            <span className="text-xs uppercase tracking-[0.32em] text-white/45">
              MQ2Timestamp Parity
            </span>
          </div>
          <h2 className="mt-3 font-archaic text-3xl uppercase tracking-[0.12em] text-white">
            Chat Timestamps
          </h2>
          <p className="mt-3 max-w-3xl text-sm leading-6 text-white/65">
            Prepend configurable timestamps to all MQ2 chat messages for {character}.
            Changes apply immediately without restart.
          </p>
        </div>

        <button
          type="button"
          onClick={handleSave}
          disabled={!canSave}
          className="inline-flex items-center gap-2 rounded-full border border-cyan-300/30 bg-cyan-300/10 px-4 py-2 text-sm font-semibold text-cyan-100 transition-colors hover:bg-cyan-300/20 disabled:cursor-not-allowed disabled:opacity-60"
        >
          <FloppyDisk size={16} />
          {saving ? "Saving..." : "Save Settings"}
        </button>
      </div>

      <div className="mt-6 space-y-4">
        {loading ? (
          <StatusBanner tone="neutral" text="Loading timestamp settings..." />
        ) : error ? (
          <StatusBanner tone="error" text={error} />
        ) : savedAt ? (
          <StatusBanner tone="success" text={`Saved ${new Date(savedAt).toLocaleTimeString()}`} />
        ) : (
          <StatusBanner
            tone="neutral"
            text="Toggle timestamps and select format, then save to apply immediately."
          />
        )}
      </div>

      <div className="mt-8 grid gap-6 xl:grid-cols-[1.2fr_0.8fr]">
        <div className="space-y-4">
          <ToggleField
            label="Enable chat timestamps"
            value={draft.enabled}
            onChange={(enabled) => setDraft((prev) => ({ ...prev, enabled }))}
            detail="Prepend timestamps to all MQ2 chat messages"
          />

          <div className="space-y-3">
            <label className="text-xs uppercase tracking-[0.18em] text-white/45">
              Timestamp Format
            </label>
            <div className="grid gap-3 sm:grid-cols-2">
              {FORMAT_OPTIONS.map((opt) => (
                <label
                  key={opt.value}
                  className={`flex cursor-pointer items-start gap-3 rounded-2xl border p-4 transition-colors ${
                    draft.format === opt.value
                      ? "border-cyan-400/40 bg-cyan-400/10"
                      : "border-white/10 bg-[#0d0715] hover:border-white/25"
                  }`}
                >
                  <input
                    type="radio"
                    name="timestamp-format"
                    value={opt.value}
                    checked={draft.format === opt.value}
                    onChange={() => {
                      setDraft((prev) => ({ ...prev, format: opt.value }));
                    }}
                    className="mt-1 h-4 w-4 accent-cyan-400"
                  />
                  <div className="flex-1">
                    <div className="text-sm font-semibold text-white">{opt.label}</div>
                    <div className="mt-1 font-mono text-xs text-cyan-300/80">{opt.example}</div>
                  </div>
                </label>
              ))}
            </div>
          </div>
        </div>

        <div className="rounded-3xl border border-white/10 bg-[#0d0715] p-5">
          <div className="flex items-center gap-3 text-cyan-200">
            <Info size={18} weight="bold" />
            <h3 className="font-archaic text-xl uppercase tracking-[0.16em] text-white">
              Preview
            </h3>
          </div>
          <div className="mt-4 space-y-2 font-mono text-sm">
            {draft.enabled ? (
              <>
                <div className="text-white/40">
                  {getPreviewTimestamp(draft.format, true)} You say, 'Hello world'
                </div>
                <div className="text-white/40">
                  {getPreviewTimestamp(draft.format, false)} Soandso tells you, 'Incoming!'
                </div>
              </>
            ) : (
              <div className="text-white/50 italic">
                Enable timestamps to see preview
              </div>
            )}
          </div>
          <div className="mt-6 space-y-3 text-sm leading-6 text-white/65">
            <p>
              Timestamps make it easy to correlate game events with log entries
              and understand timing during session reviews.
            </p>
            <p>
              Format changes take effect immediately on the next chat message.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}

function getPreviewTimestamp(format: TimestampFormat, isMorning: boolean): string {
  const hour = isMorning ? 14 : 2;
  const ampm = isMorning ? "PM" : "AM";
  const date = "2026-04-15";

  switch (format) {
    case "date_time_24":
      return `${date} ${hour.toString().padStart(2, "0")}:30:45`;
    case "time_24":
      return `${hour.toString().padStart(2, "0")}:30:45`;
    case "date_time_12":
      return `${date} ${hour.toString().padStart(2, "0")}:30:45 ${ampm}`;
    case "time_12":
      return `${hour.toString().padStart(2, "0")}:30:45 ${ampm}`;
  }
}
