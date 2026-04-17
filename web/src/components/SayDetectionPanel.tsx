import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  BellRinging,
  ChatTeardropText,
  FloppyDisk,
  Plus,
  Trash,
} from "@phosphor-icons/react";
import { useSayDetection } from "../hooks/useSayDetection";
import type {
  SayDetectionConfig,
  SayDetectionRule,
  SayPatternType,
  SayRuleAction,
} from "../types";

function StatusBanner({
  tone,
  content,
}: {
  tone: "neutral" | "success" | "warning" | "error";
  content: ReactNode;
}) {
  const className =
    tone === "success"
      ? "border-emerald-400/30 bg-emerald-500/10 text-emerald-200"
      : tone === "warning"
        ? "border-amber-400/30 bg-amber-500/10 text-amber-200"
        : tone === "error"
          ? "border-rose-400/30 bg-rose-500/10 text-rose-200"
          : "border-white/10 bg-white/5 text-white/70";

  return <div className={`border px-4 py-3 text-sm ${className}`}>{content}</div>;
}

function ToggleField({
  label,
  detail,
  value,
  onChange,
}: {
  label: string;
  detail: string;
  value: boolean;
  onChange: (value: boolean) => void;
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
        className="mt-1 h-4 w-4 accent-cyan-300"
      />
    </label>
  );
}

function TextField({
  label,
  value,
  placeholder,
  onChange,
}: {
  label: string;
  value: string | null;
  placeholder?: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="flex flex-col gap-2">
      <span className="text-xs uppercase tracking-[0.18em] text-white/45">
        {label}
      </span>
      <input
        type="text"
        value={value ?? ""}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
        className="rounded-2xl border border-white/15 bg-[#0d0715] px-3 py-2 text-white outline-none transition-colors focus:border-cyan-300/40"
      />
    </label>
  );
}

function SelectField<T extends string>({
  label,
  value,
  onChange,
  options,
}: {
  label: string;
  value: T;
  onChange: (value: T) => void;
  options: { label: string; value: T }[];
}) {
  return (
    <label className="flex flex-col gap-2">
      <span className="text-xs uppercase tracking-[0.18em] text-white/45">
        {label}
      </span>
      <select
        value={value}
        onChange={(event) => onChange(event.target.value as T)}
        className="rounded-2xl border border-white/15 bg-[#0d0715] px-3 py-2 text-white outline-none transition-colors focus:border-cyan-300/40"
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}

const blankRule: SayDetectionRule = {
  name: "",
  pattern: "",
  patternType: "substring",
  actionType: "alert",
  actionValue: null,
  enabled: true,
};

export default function SayDetectionPanel() {
  const { config, status, loading, saving, error, savedAt, save } =
    useSayDetection();
  const [draft, setDraft] = useState<SayDetectionConfig>(config);
  const [newRule, setNewRule] = useState<SayDetectionRule>(blankRule);

  useEffect(() => {
    setDraft(config);
  }, [config]);

  const enabledRuleCount = useMemo(
    () => draft.rules.filter((rule) => rule.enabled).length,
    [draft.rules],
  );

  function updateRule(index: number, patch: Partial<SayDetectionRule>) {
    setDraft((prev) => ({
      ...prev,
      rules: prev.rules.map((rule, ruleIndex) =>
        ruleIndex === index ? { ...rule, ...patch } : rule,
      ),
    }));
  }

  function removeRule(index: number) {
    setDraft((prev) => ({
      ...prev,
      rules: prev.rules.filter((_, ruleIndex) => ruleIndex !== index),
    }));
  }

  function addRule() {
    const name = newRule.name.trim();
    const pattern = newRule.pattern.trim();
    const actionValue = newRule.actionValue?.trim() ?? "";
    const needsPayload =
      newRule.actionType === "broadcast" || newRule.actionType === "command";

    if (!name || !pattern || (needsPayload && !actionValue)) {
      return;
    }

    setDraft((prev) => ({
      ...prev,
      rules: [
        ...prev.rules,
        {
          ...newRule,
          name,
          pattern,
          actionValue: needsPayload ? actionValue : null,
        },
      ],
    }));
    setNewRule(blankRule);
  }

  async function handleSave() {
    try {
      await save({
        ...draft,
        soundFile: draft.soundFile?.trim() ? draft.soundFile.trim() : null,
        discordWebhookUrl: draft.discordWebhookUrl?.trim()
          ? draft.discordWebhookUrl.trim()
          : null,
        rules: draft.rules.map((rule) => ({
          ...rule,
          name: rule.name.trim(),
          pattern: rule.pattern.trim(),
          actionValue:
            rule.actionType === "alert" || !rule.actionValue?.trim()
              ? null
              : rule.actionValue.trim(),
        })),
      });
    } catch {
      // Hook already surfaces the error state.
    }
  }

  const canSave = !loading && !saving;
  const lastMatch = status.lastMatch;

  return (
    <section className="flex-1 h-full flex flex-col relative z-20 min-w-[760px]">
      <header className="h-16 border-b border-white/10 flex items-center justify-between px-6 bg-violet/30 backdrop-blur-md shrink-0">
        <div className="flex items-center gap-4">
          <div className="w-8 h-8 rounded border border-cyan-400/40 flex items-center justify-center bg-void">
            <ChatTeardropText weight="fill" className="text-cyan-300" />
          </div>
          <div>
            <h2 className="font-archaic text-lg text-white leading-tight">
              Say Channel Detection
            </h2>
            <p className="text-[10px] uppercase tracking-widest text-white/50 font-rune">
              MQ2Say parity · fast rule matching · operator alerts
            </p>
          </div>
        </div>

        <button
          type="button"
          onClick={handleSave}
          disabled={!canSave}
          className="inline-flex items-center gap-2 rounded-full border border-cyan-300/30 bg-cyan-300/10 px-4 py-2 text-sm font-semibold text-cyan-100 transition-colors hover:bg-cyan-300/20 disabled:cursor-not-allowed disabled:opacity-60"
        >
          <FloppyDisk size={16} />
          {saving ? "Saving..." : "Save Rules"}
        </button>
      </header>

      <div className="flex-1 overflow-y-auto p-6 space-y-6">
        {loading ? (
          <StatusBanner tone="neutral" content="Loading say-detection status..." />
        ) : error ? (
          <StatusBanner tone="error" content={error} />
        ) : savedAt ? (
          <StatusBanner
            tone="success"
            content={`Saved ${new Date(savedAt).toLocaleTimeString()}`}
          />
        ) : lastMatch ? (
          <StatusBanner
            tone="warning"
            content={
              <span>
                Last match: <strong>{lastMatch.ruleName}</strong> by{" "}
                <strong>{lastMatch.sender}</strong>
              </span>
            }
          />
        ) : (
          <StatusBanner
            tone="neutral"
            content="Configure say-channel rules and alert routing."
          />
        )}

        <div className="grid gap-4 md:grid-cols-4">
          <div className="rounded-3xl border border-white/10 bg-[#120a1d]/88 p-4">
            <div className="text-xs uppercase tracking-[0.22em] text-white/45">
              Engine
            </div>
            <div className="mt-2 font-rune text-2xl text-white">
              {draft.enabled ? "Armed" : "Offline"}
            </div>
          </div>
          <div className="rounded-3xl border border-white/10 bg-[#120a1d]/88 p-4">
            <div className="text-xs uppercase tracking-[0.22em] text-white/45">
              Enabled Rules
            </div>
            <div className="mt-2 font-rune text-2xl text-cyan-300">
              {enabledRuleCount}
            </div>
          </div>
          <div className="rounded-3xl border border-white/10 bg-[#120a1d]/88 p-4">
            <div className="text-xs uppercase tracking-[0.22em] text-white/45">
              Total Matches
            </div>
            <div className="mt-2 font-rune text-2xl text-white">
              {status.totalMatches}
            </div>
          </div>
          <div className="rounded-3xl border border-white/10 bg-[#120a1d]/88 p-4">
            <div className="text-xs uppercase tracking-[0.22em] text-white/45">
              Last Rule
            </div>
            <div className="mt-2 font-rune text-lg text-white">
              {lastMatch?.ruleName ?? "No matches yet"}
            </div>
          </div>
        </div>

        <div className="grid gap-6 xl:grid-cols-[0.9fr_1.1fr]">
          <div className="space-y-4 rounded-[1.5rem] border border-cyan-400/20 bg-[#120a1d]/88 p-5 shadow-[0_12px_40px_rgba(34,211,238,0.08)] backdrop-blur">
            <div className="flex items-center gap-3 text-cyan-200">
              <BellRinging size={18} weight="fill" />
              <h3 className="font-archaic text-xl uppercase tracking-[0.16em] text-white">
                Alert Routing
              </h3>
            </div>
            <ToggleField
              label="Enable Say Detection"
              detail="Poll the /say channel each orchestrator pulse"
              value={draft.enabled}
              onChange={(enabled) => setDraft((prev) => ({ ...prev, enabled }))}
            />
            <ToggleField
              label="Sound Routing"
              detail="Persist sound preference for alert-style rules"
              value={draft.soundEnabled}
              onChange={(soundEnabled) =>
                setDraft((prev) => ({ ...prev, soundEnabled }))
              }
            />
            <ToggleField
              label="Toast Routing"
              detail="Persist toast preference for alert-style rules"
              value={draft.toastEnabled}
              onChange={(toastEnabled) =>
                setDraft((prev) => ({ ...prev, toastEnabled }))
              }
            />
            <ToggleField
              label="Broadcast Alert Echo"
              detail="Mirror alert-style matches to all connected clients"
              value={draft.broadcastAllClients}
              onChange={(broadcastAllClients) =>
                setDraft((prev) => ({ ...prev, broadcastAllClients }))
              }
            />
            <TextField
              label="Sound File"
              value={draft.soundFile}
              placeholder="say_alert.wav"
              onChange={(soundFile) =>
                setDraft((prev) => ({ ...prev, soundFile: soundFile || null }))
              }
            />
            <TextField
              label="Discord Webhook URL"
              value={draft.discordWebhookUrl}
              placeholder="https://discord.com/api/webhooks/..."
              onChange={(discordWebhookUrl) =>
                setDraft((prev) => ({
                  ...prev,
                  discordWebhookUrl: discordWebhookUrl || null,
                }))
              }
            />
          </div>

          <div className="rounded-[1.5rem] border border-cyan-400/20 bg-[#120a1d]/88 p-5 shadow-[0_12px_40px_rgba(34,211,238,0.08)] backdrop-blur">
            <div className="flex items-center justify-between gap-4 border-b border-white/10 pb-4">
              <div>
                <h3 className="font-archaic text-xl uppercase tracking-[0.16em] text-white">
                  Rule Editor
                </h3>
                <p className="mt-2 text-sm leading-6 text-white/65">
                  Match specific `/say` lines and choose whether the rule should
                  alert, run a local command, or broadcast a command fleet-wide.
                </p>
              </div>
            </div>

            <div className="mt-5 grid gap-3 md:grid-cols-2">
              <TextField
                label="Rule Name"
                value={newRule.name}
                onChange={(name) => setNewRule((prev) => ({ ...prev, name }))}
              />
              <TextField
                label="Pattern"
                value={newRule.pattern}
                onChange={(pattern) =>
                  setNewRule((prev) => ({ ...prev, pattern }))
                }
              />
              <SelectField<SayPatternType>
                label="Pattern Type"
                value={newRule.patternType}
                onChange={(patternType) =>
                  setNewRule((prev) => ({ ...prev, patternType }))
                }
                options={[
                  { label: "Substring", value: "substring" },
                  { label: "Exact", value: "exact" },
                  { label: "Regex", value: "regex" },
                ]}
              />
              <SelectField<SayRuleAction>
                label="Action"
                value={newRule.actionType}
                onChange={(actionType) =>
                  setNewRule((prev) => ({
                    ...prev,
                    actionType,
                    actionValue: actionType === "alert" ? null : prev.actionValue,
                  }))
                }
                options={[
                  { label: "Alert", value: "alert" },
                  { label: "Broadcast Command", value: "broadcast" },
                  { label: "Run Local Command", value: "command" },
                ]}
              />
              {(newRule.actionType === "broadcast" ||
                newRule.actionType === "command") && (
                <div className="md:col-span-2">
                  <TextField
                    label="Action Payload"
                    value={newRule.actionValue}
                    placeholder={
                      newRule.actionType === "broadcast"
                        ? "/bc Incoming hail"
                        : "/say Ready"
                    }
                    onChange={(actionValue) =>
                      setNewRule((prev) => ({ ...prev, actionValue }))
                    }
                  />
                </div>
              )}
            </div>

            <div className="mt-4 flex justify-end">
              <button
                type="button"
                onClick={addRule}
                className="inline-flex items-center gap-2 rounded-full border border-cyan-300/30 bg-cyan-300/10 px-4 py-2 text-sm font-semibold text-cyan-100 transition-colors hover:bg-cyan-300/20"
              >
                <Plus size={16} />
                Add Rule
              </button>
            </div>

            <div className="mt-6 space-y-4">
              {draft.rules.length === 0 ? (
                <div className="rounded-2xl border border-dashed border-white/10 bg-[#0d0715] px-4 py-5 text-sm text-white/50">
                  No say-detection rules configured yet.
                </div>
              ) : (
                draft.rules.map((rule, index) => {
                  const needsPayload =
                    rule.actionType === "broadcast" || rule.actionType === "command";

                  return (
                    <div
                      key={`${rule.name}-${index}`}
                      className="rounded-3xl border border-white/10 bg-[#0d0715] p-4"
                    >
                      <div className="grid gap-3 md:grid-cols-[1.3fr_1.3fr_0.8fr_0.9fr_auto]">
                        <TextField
                          label="Rule Name"
                          value={rule.name}
                          onChange={(name) => updateRule(index, { name })}
                        />
                        <TextField
                          label="Pattern"
                          value={rule.pattern}
                          onChange={(pattern) => updateRule(index, { pattern })}
                        />
                        <SelectField<SayPatternType>
                          label="Pattern"
                          value={rule.patternType}
                          onChange={(patternType) =>
                            updateRule(index, { patternType })
                          }
                          options={[
                            { label: "Substring", value: "substring" },
                            { label: "Exact", value: "exact" },
                            { label: "Regex", value: "regex" },
                          ]}
                        />
                        <SelectField<SayRuleAction>
                          label="Action"
                          value={rule.actionType}
                          onChange={(actionType) =>
                            updateRule(index, {
                              actionType,
                              actionValue:
                                actionType === "alert" ? null : rule.actionValue,
                            })
                          }
                          options={[
                            { label: "Alert", value: "alert" },
                            { label: "Broadcast", value: "broadcast" },
                            { label: "Command", value: "command" },
                          ]}
                        />
                        <div className="flex items-end gap-3">
                          <label className="flex items-center gap-2 text-sm text-white/70">
                            <input
                              type="checkbox"
                              checked={rule.enabled}
                              onChange={(event) =>
                                updateRule(index, { enabled: event.target.checked })
                              }
                              className="h-4 w-4 accent-cyan-300"
                            />
                            Enabled
                          </label>
                          <button
                            type="button"
                            onClick={() => removeRule(index)}
                            className="inline-flex h-10 w-10 items-center justify-center rounded-full border border-rose-300/20 bg-rose-300/10 text-rose-100 transition-colors hover:bg-rose-300/20"
                            aria-label={`Remove ${rule.name}`}
                          >
                            <Trash size={16} />
                          </button>
                        </div>
                      </div>

                      {needsPayload && (
                        <div className="mt-3">
                          <TextField
                            label="Action Payload"
                            value={rule.actionValue}
                            placeholder={
                              rule.actionType === "broadcast"
                                ? "/bc Incoming say match"
                                : "/say Acknowledged"
                            }
                            onChange={(actionValue) =>
                              updateRule(index, { actionValue })
                            }
                          />
                        </div>
                      )}
                    </div>
                  );
                })
              )}
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
