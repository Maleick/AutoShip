import {
  ArrowClockwise,
  BellRinging,
  Check,
  EnvelopeSimple,
  WarningDiamond,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState } from "react";

import { useAlerts } from "../hooks/useAlerts";
import type { AlertingConfig, OperationalAlert } from "../types";

const SEVERITY_STYLE: Record<
  OperationalAlert["severity"],
  { chip: string; accent: string; label: string }
> = {
  critical: {
    chip: "border-red-400/40 bg-red-500/10 text-red-300",
    accent: "border-l-red-400",
    label: "Critical",
  },
  warning: {
    chip: "border-amber-300/40 bg-amber-400/10 text-amber-200",
    accent: "border-l-amber-300",
    label: "Warning",
  },
  info: {
    chip: "border-cyan-300/40 bg-cyan-400/10 text-cyan-200",
    accent: "border-l-cyan-300",
    label: "Info",
  },
};

function SummaryCard({
  label,
  value,
  detail,
}: {
  label: string;
  value: string;
  detail: string;
}) {
  return (
    <div className="border border-white/10 bg-violet/10 px-4 py-3">
      <div className="text-[10px] uppercase tracking-[0.3em] text-white/35 font-rune">
        {label}
      </div>
      <div className="mt-2 font-archaic text-2xl text-white">{value}</div>
      <div className="mt-1 text-[11px] text-white/35">{detail}</div>
    </div>
  );
}

function formatAlertTimestamp(value: string) {
  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) {
    return value;
  }
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(timestamp);
}

function ConfigField({
  label,
  hint,
  children,
}: {
  label: string;
  hint: string;
  children: React.ReactNode;
}) {
  return (
    <label className="flex flex-col gap-2">
      <div className="flex items-baseline justify-between gap-4">
        <span className="text-[11px] uppercase tracking-[0.25em] text-white/60 font-rune">
          {label}
        </span>
        <span className="text-[10px] text-white/30">{hint}</span>
      </div>
      {children}
    </label>
  );
}

function textInputClass() {
  return "w-full border border-white/10 bg-black/20 px-3 py-2 text-sm text-white outline-none transition focus:border-magentaglow/60 focus:bg-black/30";
}

export default function AlertsPanel() {
  const {
    alerts,
    unreadCount,
    config,
    loading,
    saving,
    error,
    refresh,
    acknowledgeAlert,
    acknowledgeAll,
    saveConfig,
  } = useAlerts();
  const [draft, setDraft] = useState<AlertingConfig>(config);
  const isDirtyRef = useRef(false);

  useEffect(() => {
    // Only sync the draft when the operator hasn't started editing. If they
    // have unsaved changes, background polls are ignored until they save
    // (which clears isDirtyRef) or discard edits by navigating away.
    if (!isDirtyRef.current) {
      setDraft(config);
    }
  }, [config]);

  const counts = useMemo(() => {
    const unread = alerts.filter((alert) => !alert.acknowledged_at);
    return {
      critical: unread.filter((alert) => alert.severity === "critical").length,
      warning: unread.filter((alert) => alert.severity === "warning").length,
      total: alerts.length,
    };
  }, [alerts]);

  const updateField = (patch: Partial<AlertingConfig>) => {
    isDirtyRef.current = true;
    setDraft((current) => ({ ...current, ...patch }));
  };

  const updateThreshold = (
    field: keyof AlertingConfig["thresholds"],
    value: number | boolean,
  ) => {
    isDirtyRef.current = true;
    setDraft((current) => ({
      ...current,
      thresholds: {
        ...current.thresholds,
        [field]: value,
      },
    }));
  };

  const handleSave = async () => {
    const payload = {
      ...draft,
      email_recipients: draft.email_recipients.filter(Boolean),
    };
    await saveConfig(payload);
    isDirtyRef.current = false;
    setDraft(payload);
  };

  return (
    <div className="flex-1 min-w-0 grid grid-cols-1 xl:grid-cols-[1.4fr_1fr] gap-6 overflow-hidden">
      <section className="min-w-0 flex flex-col gap-5 overflow-hidden">
        <div className="flex flex-wrap items-center gap-3">
          <div className="w-10 h-10 border border-red-300/30 bg-red-500/10 text-red-200 flex items-center justify-center shrink-0">
            <BellRinging size={18} weight="fill" />
          </div>
          <div>
            <h2 className="font-archaic text-base text-white">
              Operational Alerts
            </h2>
            <p className="text-[10px] uppercase tracking-[0.3em] text-white/35 font-rune">
              Critical deaths, stuck recoveries, system thresholds, and audit trail
            </p>
          </div>
          <button
            type="button"
            onClick={() => void refresh()}
            className="ml-auto inline-flex items-center gap-2 border border-white/10 px-3 py-2 text-xs uppercase tracking-[0.25em] text-white/60 hover:border-spectral/40 hover:text-spectral transition"
          >
            <ArrowClockwise size={14} />
            Refresh
          </button>
          <button
            type="button"
            onClick={() => void acknowledgeAll()}
            className="inline-flex items-center gap-2 border border-magentaglow/20 bg-magentadark/15 px-3 py-2 text-xs uppercase tracking-[0.25em] text-magentaglow hover:border-magentaglow/60 transition"
          >
            <Check size={14} />
            Ack All
          </button>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <SummaryCard
            label="Unread"
            value={String(unreadCount)}
            detail="All unacknowledged operational alerts"
          />
          <SummaryCard
            label="Critical"
            value={String(counts.critical)}
            detail="Immediate operator attention required"
          />
          <SummaryCard
            label="Warning"
            value={String(counts.warning)}
            detail="Batched degradations and threshold breaches"
          />
        </div>

        <div className="flex-1 overflow-y-auto pr-1">
          <div className="grid gap-3">
            {alerts.map((alert) => {
              const severity = SEVERITY_STYLE[alert.severity];
              return (
                <article
                  key={alert.id}
                  className={`border border-white/10 bg-violet/10 border-l-4 ${severity.accent} px-4 py-4 transition hover:bg-violet/15`}
                >
                  <div className="flex flex-wrap items-center gap-2">
                    <span
                      className={`border px-2 py-0.5 text-[10px] uppercase tracking-[0.25em] font-rune ${severity.chip}`}
                    >
                      {severity.label}
                    </span>
                    <span className="text-[10px] uppercase tracking-[0.25em] text-white/35 font-rune">
                      {alert.kind.replaceAll("_", " ")}
                    </span>
                    <span className="ml-auto text-[11px] text-white/35">
                      {formatAlertTimestamp(alert.created_at)}
                    </span>
                  </div>
                  <p className="mt-3 text-sm text-white leading-relaxed">
                    {alert.message}
                  </p>
                  <div className="mt-3 flex flex-wrap gap-3 text-[11px] text-white/35">
                    <span>Actor: {alert.actor ?? "n/a"}</span>
                    <span>Zone: {alert.zone ?? "n/a"}</span>
                    <span>Source: {alert.source ?? "n/a"}</span>
                    <span>
                      Status: {alert.acknowledged_at ? "Acknowledged" : "Unread"}
                    </span>
                  </div>
                  <div className="mt-4 flex items-center justify-between gap-3">
                    <div className="text-[11px] text-white/25">
                      {alert.acknowledged_at
                        ? `Acked by ${alert.acknowledged_by ?? "operator"}`
                        : "Awaiting operator acknowledgment"}
                    </div>
                    {!alert.acknowledged_at && (
                      <button
                        type="button"
                        onClick={() => void acknowledgeAlert(alert.id)}
                        className="inline-flex items-center gap-2 border border-cyan-300/20 bg-cyan-400/10 px-3 py-1.5 text-[11px] uppercase tracking-[0.25em] text-cyan-100 hover:border-cyan-300/60 transition"
                      >
                        <Check size={12} />
                        Acknowledge
                      </button>
                    )}
                  </div>
                </article>
              );
            })}

            {!alerts.length && (
              <div className="border border-dashed border-white/10 px-6 py-12 text-center text-white/25 font-archaic">
                No alerts recorded yet.
              </div>
            )}
          </div>
        </div>
      </section>

      <aside className="min-w-0 overflow-y-auto border border-white/10 bg-black/20 px-5 py-5">
        <div className="flex items-center gap-3">
          <EnvelopeSimple size={18} className="text-spectral" />
          <div>
            <h3 className="font-archaic text-base text-white">Routing Config</h3>
            <p className="text-[10px] uppercase tracking-[0.25em] text-white/35 font-rune">
              Discord, email, and threshold controls
            </p>
          </div>
        </div>

        {error && (
          <div className="mt-4 border border-red-400/20 bg-red-500/10 px-3 py-2 text-sm text-red-200">
            {error}
          </div>
        )}

        <div className="mt-5 space-y-4">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <label className="flex items-center gap-3 border border-white/10 px-3 py-3">
              <input
                type="checkbox"
                checked={draft.enable_discord}
                onChange={(event) =>
                   updateField({ enable_discord: event.target.checked })
                }
              />
              <span className="text-sm text-white">Enable Discord alerts</span>
            </label>
            <label className="flex items-center gap-3 border border-white/10 px-3 py-3">
              <input
                type="checkbox"
                checked={draft.enable_email}
                onChange={(event) =>
                   updateField({ enable_email: event.target.checked })
                }
              />
              <span className="text-sm text-white">Enable email summary</span>
            </label>
          </div>

          <ConfigField
            label="Discord Webhook"
            hint="Primary farmer notification channel"
          >
            <input
              className={textInputClass()}
              value={draft.discord_webhook_url}
              onChange={(event) =>
                updateField({ discord_webhook_url: event.target.value })
              }
            />
          </ConfigField>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <ConfigField label="SMTP Server" hint="Outgoing relay">
              <input
                className={textInputClass()}
                value={draft.smtp_server}
                onChange={(event) =>
                  updateField({ smtp_server: event.target.value })
                }
              />
            </ConfigField>
            <ConfigField label="SMTP Port" hint="TLS port">
              <input
                className={textInputClass()}
                type="number"
                value={draft.smtp_port}
                onChange={(event) =>
                  updateField({ smtp_port: Number(event.target.value) || 0 })
                }
              />
            </ConfigField>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <ConfigField label="SMTP Username" hint="Optional auth">
              <input
                className={textInputClass()}
                value={draft.smtp_username}
                onChange={(event) =>
                  updateField({ smtp_username: event.target.value })
                }
              />
            </ConfigField>
            <ConfigField label="SMTP Password" hint="Stored in runtime config">
              <input
                className={textInputClass()}
                type="password"
                value={draft.smtp_password}
                onChange={(event) =>
                  updateField({ smtp_password: event.target.value })
                }
              />
            </ConfigField>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <ConfigField label="Email From" hint="Sender address">
              <input
                className={textInputClass()}
                value={draft.email_from}
                onChange={(event) =>
                  updateField({ email_from: event.target.value })
                }
              />
            </ConfigField>
            <ConfigField label="Recipients" hint="Comma separated">
              <input
                className={textInputClass()}
                value={draft.email_recipients.join(", ")}
                onChange={(event) =>
                  updateField({
                    email_recipients: event.target.value
                      .split(",")
                      .map((item) => item.trim()),
                  })
                }
              />
            </ConfigField>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <ConfigField label="Subject Prefix" hint="Daily summary prefix">
              <input
                className={textInputClass()}
                value={draft.email_subject_prefix}
                onChange={(event) =>
                  updateField({ email_subject_prefix: event.target.value })
                }
              />
            </ConfigField>
            <ConfigField label="Batch Window" hint="Warning summary cadence">
              <input
                className={textInputClass()}
                type="number"
                value={draft.warning_batch_window_secs}
                onChange={(event) =>
                  updateField({
                    warning_batch_window_secs: Number(event.target.value) || 0,
                  })
                }
              />
            </ConfigField>
          </div>

          <div className="border border-white/10 px-4 py-4">
            <div className="flex items-center gap-2 text-white">
              <WarningDiamond size={16} className="text-amber-300" />
              <span className="font-archaic text-sm">Thresholds</span>
            </div>
            <div className="mt-4 grid grid-cols-1 md:grid-cols-2 gap-4">
              <label className="flex items-center gap-3 text-sm text-white/80">
                <input
                  type="checkbox"
                  checked={draft.thresholds.death_alert}
                  onChange={(event) =>
                    updateThreshold("death_alert", event.target.checked)
                  }
                />
                Death alert
              </label>
              <label className="flex items-center gap-3 text-sm text-white/80">
                <input
                  type="checkbox"
                  checked={draft.thresholds.stuck_alert}
                  onChange={(event) =>
                    updateThreshold("stuck_alert", event.target.checked)
                  }
                />
                Stuck alert
              </label>
              <ConfigField label="Memory MB" hint="Per-client warning">
                <input
                  className={textInputClass()}
                  type="number"
                  value={draft.thresholds.memory_warning_mb}
                  onChange={(event) =>
                    updateThreshold(
                      "memory_warning_mb",
                      Number(event.target.value) || 0,
                    )
                  }
                />
              </ConfigField>
              <ConfigField label="IPC Latency" hint="p95 milliseconds">
                <input
                  className={textInputClass()}
                  type="number"
                  value={draft.thresholds.ipc_latency_warning_ms}
                  onChange={(event) =>
                    updateThreshold(
                      "ipc_latency_warning_ms",
                      Number(event.target.value) || 0,
                    )
                  }
                />
              </ConfigField>
              <ConfigField label="Error Rate" hint="Errors per minute">
                <input
                  className={textInputClass()}
                  type="number"
                  value={draft.thresholds.error_rate_warning_per_min}
                  onChange={(event) =>
                    updateThreshold(
                      "error_rate_warning_per_min",
                      Number(event.target.value) || 0,
                    )
                  }
                />
              </ConfigField>
              <ConfigField label="DPS Drop %" hint="Below baseline">
                <input
                  className={textInputClass()}
                  type="number"
                  value={draft.thresholds.dps_drop_warning_pct}
                  onChange={(event) =>
                    updateThreshold(
                      "dps_drop_warning_pct",
                      Number(event.target.value) || 0,
                    )
                  }
                />
              </ConfigField>
              <ConfigField label="Zone Timeout" hint="Seconds before critical">
                <input
                  className={textInputClass()}
                  type="number"
                  value={draft.thresholds.zone_timeout_secs}
                  onChange={(event) =>
                    updateThreshold(
                      "zone_timeout_secs",
                      Number(event.target.value) || 0,
                    )
                  }
                />
              </ConfigField>
            </div>
          </div>
        </div>

        <div className="mt-6 flex items-center justify-between gap-3">
          <div className="text-[11px] text-white/30">
            {loading ? "Loading latest alert history..." : `${counts.total} stored alert(s)`}
          </div>
          <button
            type="button"
            disabled={saving}
            onClick={() => void handleSave()}
            className="inline-flex items-center gap-2 border border-spectral/30 bg-spectral/10 px-4 py-2 text-xs uppercase tracking-[0.25em] text-spectral hover:border-spectral/70 disabled:opacity-50 transition"
          >
            <Check size={14} />
            {saving ? "Saving" : "Save Config"}
          </button>
        </div>
      </aside>
    </div>
  );
}
