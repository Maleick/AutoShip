import { useEffect, useState } from "react";
import { Broadcast, FloppyDisk, LinkSimple } from "@phosphor-icons/react";
import type { BoxChatSettings } from "../types";
import { useBoxChatSettings } from "../hooks/useBoxChatSettings";

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

function TextField({
  label,
  value,
  onChange,
  type = "text",
}: {
  label: string;
  value: string | number;
  onChange: (value: string) => void;
  type?: "text" | "number";
}) {
  return (
    <label className="flex flex-col gap-2">
      <span className="text-xs uppercase tracking-[0.18em] text-white/45">
        {label}
      </span>
      <input
        type={type}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className="rounded-2xl border border-white/15 bg-[#0d0715] px-3 py-2 text-white outline-none transition-colors focus:border-cyan-300/40"
      />
    </label>
  );
}

export default function BoxChatPanel() {
  const { settings, loading, saving, error, savedAt, save } = useBoxChatSettings();
  const [draft, setDraft] = useState<BoxChatSettings>(settings);

  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  async function handleSave() {
    await save({
      ...draft,
      host: draft.host.trim(),
      port: Number(draft.port),
    });
  }

  return (
    <section className="rounded-[1.5rem] border border-cyan-400/20 bg-[#120a1d]/88 p-5 shadow-[0_12px_40px_rgba(34,211,238,0.08)] backdrop-blur">
      <div className="flex items-center justify-between gap-6 border-b border-white/10 pb-6">
        <div>
          <div className="flex items-center gap-3 text-cyan-200">
            <Broadcast size={20} weight="fill" />
            <span className="text-xs uppercase tracking-[0.32em] text-white/45">
              Network Box Chat
            </span>
          </div>
          <h2 className="mt-3 font-archaic text-3xl uppercase tracking-[0.12em] text-white">
            EQBC-style Relay Control
          </h2>
          <p className="mt-3 max-w-3xl text-sm leading-6 text-white/65">
            Persist the cross-machine relay endpoint used for <code>/bc</code>{" "}
            and <code>/bct</code>. Running TextQuest instances reload this
            section from <code>config/textquest.toml</code> without a restart.
          </p>
        </div>

        <button
          type="button"
          onClick={handleSave}
          disabled={loading || saving}
          className="inline-flex items-center gap-2 rounded-full border border-cyan-300/30 bg-cyan-300/10 px-4 py-2 text-sm font-semibold text-cyan-100 transition-colors hover:bg-cyan-300/20 disabled:cursor-not-allowed disabled:opacity-60"
        >
          <FloppyDisk size={16} />
          {saving ? "Saving..." : "Save Settings"}
        </button>
      </div>

      <div className="mt-6 space-y-4">
        {loading ? (
          <StatusBanner tone="neutral" text="Loading persisted box-chat settings..." />
        ) : error ? (
          <StatusBanner tone="error" text={error} />
        ) : savedAt ? (
          <StatusBanner
            tone="success"
            text={`Saved ${new Date(savedAt).toLocaleTimeString()}`}
          />
        ) : (
          <StatusBanner
            tone="neutral"
            text="Edit the relay endpoint, save it, and the runtime will pick up changes on the next config poll."
          />
        )}
      </div>

      <div className="mt-8 grid gap-6 xl:grid-cols-[1.2fr_0.8fr]">
        <div className="space-y-4">
          <ToggleField
            label="Enable box-chat runtime"
            value={draft.enabled}
            onChange={(enabled) => setDraft((prev) => ({ ...prev, enabled }))}
            detail="Starts the local listener and allows network relay hot-reload"
          />
          <ToggleField
            label="Auto-connect to upstream host"
            value={draft.auto_connect}
            onChange={(auto_connect) =>
              setDraft((prev) => ({ ...prev, auto_connect }))
            }
            detail="Maintains a client connection to the configured relay host"
          />
          <div className="grid gap-4 md:grid-cols-2">
            <TextField
              label="Relay Host"
              value={draft.host}
              onChange={(host) => setDraft((prev) => ({ ...prev, host }))}
            />
            <TextField
              label="Relay Port"
              type="number"
              value={draft.port}
              onChange={(port) =>
                setDraft((prev) => ({
                  ...prev,
                  port: Number(port) || 0,
                }))
              }
            />
          </div>
        </div>

        <div className="rounded-3xl border border-white/10 bg-[#0d0715] p-5">
          <div className="flex items-center gap-3 text-cyan-200">
            <LinkSimple size={18} weight="bold" />
            <h3 className="font-archaic text-xl uppercase tracking-[0.16em] text-white">
              Usage Notes
            </h3>
          </div>
          <ul className="mt-4 space-y-3 text-sm leading-6 text-white/65">
            <li>
              Set one machine to a reachable host and port, then point other
              TextQuest instances at that endpoint with auto-connect enabled.
            </li>
            <li>
              The runtime polls the shared config file and applies endpoint
              changes without restarting the TUI or orchestrator.
            </li>
            <li>
              Use TUI commands like <code>:bc /assist MainTank</code> or{" "}
              <code>:bct Cleric01 //cast 1</code> after saving.
            </li>
          </ul>
        </div>
      </div>
    </section>
  );
}
