import { useEffect, useState, type ReactNode } from "react";
import { FloppyDisk, ShieldWarning, Warning } from "@phosphor-icons/react";
import type { ReactNode } from "react";
import type { GmAlertConfig } from "../types";
import { useGmAlerts } from "../hooks/useGmAlerts";

function StatusBanner({
  tone,
  text,
}: {
  tone: "neutral" | "success" | "warning" | "error";
  text: ReactNode;
}) {
  const className =
    tone === "success"
      ? "border-emerald-400/30 bg-emerald-500/10 text-emerald-200"
      : tone === "warning"
        ? "border-amber-400/30 bg-amber-500/10 text-amber-200"
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
  value: string | number | null;
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
        value={value ?? ""}
        onChange={(event) => onChange(event.target.value)}
        className="rounded-2xl border border-white/15 bg-[#0d0715] px-3 py-2 text-white outline-none transition-colors focus:border-cyan-300/40"
      />
    </label>
  );
}

export default function GmAlertPanel() {
  const { config, presence, automationPaused, loading, saving, error, savedAt, save } =
    useGmAlerts();
  const [draft, setDraft] = useState<GmAlertConfig>(config);

  useEffect(() => {
    setDraft(config);
  }, [config]);

  const canSave = !loading && !saving;

  async function handleSave() {
    if (!canSave) {
      return;
    }

    try {
      await save({
        ...draft,
        discordWebhookUrl: draft.discordWebhookUrl || null,
      });
    } catch {
      // useGmAlerts already exposes the error state for UI feedback.
    }
  }

  const isActive = presence.isGmInZone;

  return (
    <section className="rounded-[1.5rem] border border-rose-400/20 bg-[#120a1d]/88 p-5 shadow-[0_12px_40px_rgba(239,68,68,0.08)] backdrop-blur">
      <div className="flex items-center justify-between gap-6 border-b border-white/10 pb-6">
        <div>
          <div className="flex items-center gap-3 text-rose-200">
            <ShieldWarning size={20} weight="fill" />
            <span className="text-xs uppercase tracking-[0.32em] text-white/45">
              Safety System
            </span>
          </div>
          <h2 className="mt-3 font-archaic text-3xl uppercase tracking-[0.12em] text-white">
            GM Detection &amp; Alerts
          </h2>
          <p className="mt-3 max-w-3xl text-sm leading-6 text-white/65">
            Monitors zone for Game Master presence and provides immediate alerts
            with optional automation pause. Parity with MQ2GMCheck.
          </p>
        </div>

        <button
          type="button"
          onClick={handleSave}
          disabled={!canSave}
          className="inline-flex items-center gap-2 rounded-full border border-rose-300/30 bg-rose-300/10 px-4 py-2 text-sm font-semibold text-rose-100 transition-colors hover:bg-rose-300/20 disabled:cursor-not-allowed disabled:opacity-60"
        >
          <FloppyDisk size={16} />
          {saving ? "Saving..." : "Save Settings"}
        </button>
      </div>

      <div className="mt-6 space-y-4">
        {loading ? (
          <StatusBanner tone="neutral" text="Loading GM alert configuration..." />
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
            text="Configure GM detection alerts and automation pause behavior."
          />
        )}
      </div>

      {isActive && (
        <div className="mt-6">
          <StatusBanner
            tone="warning"
            text={
              <span className="flex items-center gap-2">
                <Warning size={16} weight="fill" />
                <span>
                  <strong>GM DETECTED:</strong> {presence.gmNames.join(", ")} in
                  zone. Automation {automationPaused ? "PAUSED" : "running"}.
                </span>
              </span>
            }
          />
        </div>
      )}

      <div className="mt-8 grid gap-6 xl:grid-cols-[1.2fr_0.8fr]">
        <div className="space-y-4">
          <ToggleField
            label="Enable GM Detection"
            value={draft.enabled}
            onChange={(enabled) => setDraft((prev) => ({ ...prev, enabled }))}
            detail="Monitor zone for Game Master spawns"
          />
          <ToggleField
            label="Sound Alert"
            value={draft.soundEnabled}
            onChange={(soundEnabled) =>
              setDraft((prev) => ({ ...prev, soundEnabled }))
            }
            detail="Play audio alert when GM is detected"
          />
          <ToggleField
            label="Toast Notification"
            value={draft.toastEnabled}
            onChange={(toastEnabled) =>
              setDraft((prev) => ({ ...prev, toastEnabled }))
            }
            detail="Show on-screen notification in TUI"
          />
          <ToggleField
            label="Auto-Pause Automation"
            value={draft.autoPauseEnabled}
            onChange={(autoPauseEnabled) =>
              setDraft((prev) => ({ ...prev, autoPauseEnabled }))
            }
            detail="Pause all automation when GM is in zone"
          />
          <ToggleField
            label="Broadcast to All Clients"
            value={draft.broadcastAllClients}
            onChange={(broadcastAllClients) =>
              setDraft((prev) => ({ ...prev, broadcastAllClients }))
            }
            detail="Send GM alert to all connected clients"
          />
          <TextField
            label="Sound File"
            value={draft.soundFile}
            onChange={(soundFile) =>
              setDraft((prev) => ({ ...prev, soundFile: soundFile || null }))
            }
          />
          <TextField
            label="Discord Webhook URL"
            value={draft.discordWebhookUrl}
            onChange={(discordWebhookUrl) =>
              setDraft((prev) => ({
                ...prev,
                discordWebhookUrl: discordWebhookUrl || null,
              }))
            }
          />
        </div>

        <div className="rounded-3xl border border-white/10 bg-[#0d0715] p-5">
          <div className="flex items-center gap-3 text-rose-200">
            <Warning size={18} weight="bold" />
            <h3 className="font-archaic text-xl uppercase tracking-[0.16em] text-white">
              Safety Notes
            </h3>
          </div>
          <ul className="mt-4 space-y-3 text-sm leading-6 text-white/65">
            <li>
              GM detection runs each game pulse (~250ms) for sub-second alert
              latency.
            </li>
            <li>
              Auto-pause will immediately halt all automation when a GM enters
              your zone.
            </li>
            <li>
              Automation automatically resumes when the GM leaves the zone.
            </li>
            <li>
              Discord webhook requires a valid URL ending with{" "}
              <code>/api/webhooks</code>.
            </li>
            <li>
              Sound files should be placed in{" "}
              <code>config/sounds/</code> directory.
            </li>
          </ul>
        </div>
      </div>
    </section>
  );
}
