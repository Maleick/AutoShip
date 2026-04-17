import { useEffect, useId, useMemo, useRef, useState } from "react";
import {
  ArrowsClockwise,
  FloppyDisk,
  Plug,
  ShieldWarning,
  SlidersHorizontal,
  Trash,
} from "@phosphor-icons/react";

import type {
  CompatibilityTier,
  ExtensionCatalogEntry,
  ExtensionFieldSchema,
  ExtensionRuntimeStatus,
  ScopeKind,
} from "../extensions";
import { useExtensionCatalog } from "../hooks/useExtensionCatalog";

function titleCase(raw: string) {
  return raw.replaceAll("_", " ").replace(/\b\w/g, (char) => char.toUpperCase());
}

function runtimeTone(runtime: ExtensionRuntimeStatus) {
  if (runtime.adapterHealth === "healthy") {
    return "border-emerald-400/25 bg-emerald-400/10 text-emerald-100";
  }
  if (runtime.adapterHealth === "degraded") {
    return "border-amber-300/25 bg-amber-300/10 text-amber-100";
  }
  return "border-white/10 bg-white/5 text-white/70";
}

function tierTone(tier: CompatibilityTier) {
  if (tier === "native") {
    return "border-cyan-300/25 bg-cyan-300/10 text-cyan-100";
  }
  if (tier === "legacy") {
    return "border-amber-300/25 bg-amber-300/10 text-amber-100";
  }
  return "border-fuchsia-300/25 bg-fuchsia-300/10 text-fuchsia-100";
}

function cloneSettings(settings: Record<string, unknown>) {
  return JSON.parse(JSON.stringify(settings)) as Record<string, unknown>;
}

function settingsSignature(settings: Record<string, unknown>) {
  return JSON.stringify(settings);
}

function stringArrayValue(value: unknown) {
  return Array.isArray(value) ? value.join("\n") : "";
}

function groupedEntries(entries: ExtensionCatalogEntry[]) {
  const groups = new Map<string, ExtensionCatalogEntry[]>();
  for (const entry of entries) {
    const key = entry.domain;
    const existing = groups.get(key) ?? [];
    existing.push(entry);
    groups.set(key, existing);
  }

  return Array.from(groups.entries()).sort(([left], [right]) =>
    left.localeCompare(right)
  );
}

function FieldEditor({
  field,
  value,
  onChange,
}: {
  field: ExtensionFieldSchema;
  value: unknown;
  onChange: (nextValue: unknown) => void;
}) {
  const inputId = `${useId()}-${field.key}`;

  if (field.kind === "boolean") {
    return (
      <div className="rounded-2xl border border-white/10 bg-white/5 p-4">
        <label htmlFor={inputId} className="flex items-start justify-between gap-3">
          <div>
            <div className="font-medium text-white">{field.label}</div>
            <div className="mt-1 text-sm text-white/55">{field.description}</div>
          </div>
          <input
            id={inputId}
            type="checkbox"
            checked={Boolean(value)}
            onChange={(event) => onChange(event.target.checked)}
            className="mt-1 h-4 w-4 rounded border-white/20 bg-[#120b1d]"
          />
        </label>
      </div>
    );
  }

  return (
    <label htmlFor={inputId} className="block rounded-2xl border border-white/10 bg-white/5 p-4">
      <div className="font-medium text-white">{field.label}</div>
      <div className="mt-1 text-sm text-white/55">{field.description}</div>
      {field.kind === "enum" ? (
        <select
          id={inputId}
          value={typeof value === "string" ? value : String(field.defaultValue ?? "")}
          onChange={(event) => onChange(event.target.value)}
          className="mt-3 w-full rounded-xl border border-white/10 bg-[#120b1d] px-3 py-2 text-sm text-white outline-none"
        >
          {(field.options ?? []).map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      ) : field.kind === "string_array" ? (
        <textarea
          id={inputId}
          rows={4}
          value={stringArrayValue(value)}
          onChange={(event) =>
            onChange(
              event.target.value
                .split("\n")
                .map((item) => item.trim())
                .filter(Boolean)
            )
          }
          className="mt-3 w-full rounded-xl border border-white/10 bg-[#120b1d] px-3 py-2 text-sm text-white outline-none"
        />
      ) : (
        <input
          id={inputId}
          type={field.kind === "integer" ? "number" : "text"}
          min={field.min}
          max={field.max}
          value={
            field.kind === "integer"
              ? typeof value === "number"
                ? String(value)
                : typeof field.defaultValue === "number"
                  ? String(field.defaultValue)
                  : "0"
              : typeof value === "string"
                ? value
                : String(field.defaultValue ?? "")
          }
          onChange={(event) =>
            onChange(
              field.kind === "integer"
                ? Number.parseInt(event.target.value || "0", 10)
                : event.target.value
            )
          }
          className="mt-3 w-full rounded-xl border border-white/10 bg-[#120b1d] px-3 py-2 text-sm text-white outline-none"
        />
      )}
    </label>
  );
}

export default function ExtensionCatalogPanel() {
  const {
    entries,
    loading,
    error,
    refresh,
    saveSettings,
    saveScopeOverride,
    deleteScopeOverride,
    saveRuntime,
  } = useExtensionCatalog();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [settingsDraft, setSettingsDraft] = useState<Record<string, unknown>>({});
  const [scopeKind, setScopeKind] = useState<ScopeKind>("character");
  const [scopeId, setScopeId] = useState("");
  const [overrideDraft, setOverrideDraft] = useState<Record<string, unknown>>({});
  const [actionError, setActionError] = useState<string | null>(null);
  const settingsDraftSourceRef = useRef<{ entryId: string | null; signature: string | null }>({
    entryId: null,
    signature: null,
  });
  const overrideDraftSourceRef = useRef<{ key: string | null; signature: string | null }>({
    key: null,
    signature: null,
  });

  useEffect(() => {
    if (entries.length === 0) {
      if (selectedId !== null) {
        setSelectedId(null);
      }
      return;
    }

    if (!selectedId || !entries.some((entry) => entry.id === selectedId)) {
      setSelectedId(entries[0].id);
    }
  }, [entries, selectedId]);

  const selectedEntry = useMemo(
    () => entries.find((entry) => entry.id === selectedId) ?? null,
    [entries, selectedId]
  );

  useEffect(() => {
    if (!selectedEntry) {
      settingsDraftSourceRef.current = { entryId: null, signature: null };
      return;
    }

    const nextDraft = cloneSettings(selectedEntry.settings);
    const nextSignature = settingsSignature(nextDraft);
    const draftSignature = settingsSignature(settingsDraft);
    const currentSource = settingsDraftSourceRef.current;

    if (currentSource.entryId !== selectedEntry.id) {
      settingsDraftSourceRef.current = {
        entryId: selectedEntry.id,
        signature: nextSignature,
      };
      setSettingsDraft(nextDraft);
      return;
    }

    if (currentSource.signature === nextSignature) {
      return;
    }

    if (draftSignature === nextSignature) {
      settingsDraftSourceRef.current = {
        entryId: selectedEntry.id,
        signature: nextSignature,
      };
      return;
    }

    if (draftSignature !== currentSource.signature) {
      return;
    }

    settingsDraftSourceRef.current = {
      entryId: selectedEntry.id,
      signature: nextSignature,
    };
    setSettingsDraft(nextDraft);
  }, [selectedEntry, settingsDraft]);

  const selectedOverride = useMemo(() => {
    if (!selectedEntry || !scopeId.trim()) {
      return null;
    }

    return (
      selectedEntry.overrides.find(
        (override) => override.scope.kind === scopeKind && override.scope.id === scopeId.trim()
      ) ?? null
    );
  }, [scopeId, scopeKind, selectedEntry]);

  const overrideSourceKey = selectedEntry
    ? `${selectedEntry.id}:${scopeKind}:${scopeId.trim()}`
    : null;

  useEffect(() => {
    if (!selectedEntry) {
      overrideDraftSourceRef.current = { key: null, signature: null };
      return;
    }

    const nextDraft = cloneSettings(selectedOverride?.settings ?? selectedEntry.settings);
    const nextSignature = settingsSignature(nextDraft);
    const draftSignature = settingsSignature(overrideDraft);
    const currentSource = overrideDraftSourceRef.current;

    if (currentSource.key !== overrideSourceKey) {
      overrideDraftSourceRef.current = {
        key: overrideSourceKey,
        signature: nextSignature,
      };
      setOverrideDraft(nextDraft);
      return;
    }

    if (currentSource.signature === nextSignature) {
      return;
    }

    if (draftSignature === nextSignature) {
      overrideDraftSourceRef.current = {
        key: overrideSourceKey,
        signature: nextSignature,
      };
      return;
    }

    if (draftSignature !== currentSource.signature) {
      return;
    }

    overrideDraftSourceRef.current = {
      key: overrideSourceKey,
      signature: nextSignature,
    };
    setOverrideDraft(nextDraft);
  }, [overrideDraft, overrideSourceKey, selectedEntry, selectedOverride]);

  async function handleSaveSettings() {
    if (!selectedEntry) {
      return;
    }

    try {
      setActionError(null);
      await saveSettings(selectedEntry.id, settingsDraft);
    } catch (nextError) {
      setActionError(
        nextError instanceof Error ? nextError.message : "Failed to save settings"
      );
    }
  }

  async function handleSaveOverride() {
    if (!selectedEntry || !scopeId.trim()) {
      setActionError("Scope ID is required");
      return;
    }

    try {
      setActionError(null);
      await saveScopeOverride(selectedEntry.id, scopeKind, scopeId.trim(), overrideDraft);
    } catch (nextError) {
      setActionError(
        nextError instanceof Error ? nextError.message : "Failed to save override"
      );
    }
  }

  async function handleDeleteOverride() {
    if (!selectedEntry || !scopeId.trim()) {
      setActionError("Scope ID is required");
      return;
    }

    try {
      setActionError(null);
      await deleteScopeOverride(selectedEntry.id, scopeKind, scopeId.trim());
    } catch (nextError) {
      setActionError(
        nextError instanceof Error ? nextError.message : "Failed to delete override"
      );
    }
  }

  async function handleToggleRuntime() {
    if (!selectedEntry) {
      return;
    }

    try {
      setActionError(null);
      await saveRuntime(selectedEntry.id, !selectedEntry.runtime.enabled);
    } catch (nextError) {
      setActionError(
        nextError instanceof Error ? nextError.message : "Failed to toggle runtime"
      );
    }
  }

  if (loading && entries.length === 0) {
    return (
      <section className="flex-1 rounded-[1.8rem] border border-white/10 bg-[#090611] p-8 text-white">
        <div className="text-sm uppercase tracking-[0.3em] text-cyan-100">Loading extensions</div>
      </section>
    );
  }

  if (!selectedEntry) {
    return (
      <section className="flex-1 rounded-[1.8rem] border border-white/10 bg-[#090611] p-8 text-white">
        <h1 className="font-archaic text-3xl">Extension Catalog</h1>
        <p className="mt-4 text-sm text-white/60">
          {error ?? "No extension definitions are available yet."}
        </p>
      </section>
    );
  }

  const grouped = groupedEntries(entries);
  const message = actionError ?? error;

  return (
    <section className="flex-1 overflow-auto rounded-[1.8rem] border border-white/10 bg-[#090611] p-4 text-white lg:p-5">
      <div className="mb-5 flex flex-col gap-3 border-b border-white/10 pb-5 xl:flex-row xl:items-end xl:justify-between">
        <div>
          <div className="font-tech text-[11px] uppercase tracking-[0.34em] text-cyan-100">
            RedGuides / OpenVanilla parity
          </div>
          <h1 className="mt-2 font-archaic text-3xl">Extension Catalog</h1>
          <p className="mt-2 max-w-3xl text-sm leading-6 text-white/60">
            Review imported profiles, edit schema-driven settings, apply scoped
            overrides, and toggle runtime adapters without touching TOML or INI files.
          </p>
        </div>
        <button
          type="button"
          onClick={() => void refresh()}
          className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-4 py-2 text-sm text-white transition hover:border-white/30 hover:bg-white/10"
        >
          <ArrowsClockwise size={16} />
          Refresh
        </button>
      </div>

      {message ? (
        <div className="mb-4 rounded-2xl border border-amber-300/25 bg-amber-300/10 px-4 py-3 text-sm text-amber-100">
          {message}
        </div>
      ) : null}

      <div className="grid gap-5 xl:grid-cols-[320px_minmax(0,1fr)]">
        <aside className="space-y-4 rounded-[1.5rem] border border-white/10 bg-[#120b1d]/88 p-4">
          {grouped.map(([domain, domainEntries]) => (
            <div key={domain}>
              <div className="mb-2 font-tech text-[11px] uppercase tracking-[0.3em] text-white/45">
                {titleCase(domain)}
              </div>
              <div className="space-y-2">
                {domainEntries.map((entry) => {
                  const active = entry.id === selectedEntry.id;
                  return (
                    <button
                      key={entry.id}
                      type="button"
                      onClick={() => setSelectedId(entry.id)}
                      className={`w-full rounded-2xl border px-4 py-3 text-left transition ${
                        active
                          ? "border-cyan-300/30 bg-cyan-300/10"
                          : "border-white/10 bg-white/[0.03] hover:border-white/20 hover:bg-white/[0.06]"
                      }`}
                    >
                      <div className="flex items-start justify-between gap-3">
                        <div>
                          <div className="font-medium text-white">{entry.displayName}</div>
                          <div className="mt-1 text-xs text-white/55">{entry.description}</div>
                        </div>
                        <span
                          className={`rounded-full border px-2 py-1 text-[10px] uppercase tracking-[0.24em] ${tierTone(entry.compatibilityTier)}`}
                        >
                          {titleCase(entry.compatibilityTier)}
                        </span>
                      </div>
                    </button>
                  );
                })}
              </div>
            </div>
          ))}
        </aside>

        <div className="space-y-5">
          <section className="rounded-[1.5rem] border border-white/10 bg-[#120b1d]/88 p-5">
            <div className="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
              <div>
                <div className="mb-2 flex flex-wrap gap-2">
                  <span
                    className={`rounded-full border px-3 py-1 text-[10px] uppercase tracking-[0.3em] ${tierTone(selectedEntry.compatibilityTier)}`}
                  >
                    {titleCase(selectedEntry.compatibilityTier)}
                  </span>
                  <span className="rounded-full border border-white/10 bg-white/5 px-3 py-1 text-[10px] uppercase tracking-[0.3em] text-white/70">
                    {titleCase(selectedEntry.sourceKind)}
                  </span>
                </div>
                <h2 className="font-archaic text-2xl">{selectedEntry.displayName}</h2>
                <p className="mt-2 text-sm leading-6 text-white/60">{selectedEntry.description}</p>
                <div className="mt-4 grid gap-3 text-sm text-white/70 md:grid-cols-2">
                  <div>
                    <div className="font-tech text-[10px] uppercase tracking-[0.28em] text-white/40">
                      Provenance
                    </div>
                    <div className="mt-1">{selectedEntry.configProvenance.label}</div>
                    {selectedEntry.configProvenance.path ? (
                      <div className="mt-1 text-white/45">{selectedEntry.configProvenance.path}</div>
                    ) : null}
                  </div>
                  <div>
                    <div className="font-tech text-[10px] uppercase tracking-[0.28em] text-white/40">
                      Legacy Source
                    </div>
                    <div className="mt-1">{selectedEntry.legacySourceName ?? "Native extension"}</div>
                  </div>
                </div>
              </div>

              <div className={`rounded-2xl border px-4 py-3 text-sm ${runtimeTone(selectedEntry.runtime)}`}>
                <div className="font-tech text-[10px] uppercase tracking-[0.28em] text-white/50">
                  Runtime
                </div>
                <div className="mt-2 text-lg font-medium text-white">
                  {titleCase(selectedEntry.runtime.adapterHealth)}
                </div>
                <div className="mt-1 text-sm">{selectedEntry.runtime.lastSyncMessage}</div>
                {selectedEntry.runtime.degradedReason ? (
                  <div className="mt-2 flex items-start gap-2 text-sm">
                    <ShieldWarning size={16} className="mt-0.5 shrink-0" />
                    <span>{selectedEntry.runtime.degradedReason}</span>
                  </div>
                ) : null}
                {selectedEntry.runtime.lastSyncAt ? (
                  <div className="mt-2 text-xs text-white/55">
                    Last sync: {new Date(selectedEntry.runtime.lastSyncAt).toLocaleString()}
                  </div>
                ) : null}
                <button
                  type="button"
                  onClick={() => void handleToggleRuntime()}
                  className="mt-3 inline-flex items-center gap-2 rounded-full border border-white/10 bg-[#090611] px-4 py-2 text-sm text-white transition hover:border-white/30 hover:bg-white/10"
                >
                  <Plug size={16} />
                  {selectedEntry.runtime.enabled ? "Disable Runtime" : "Enable Runtime"}
                </button>
              </div>
            </div>

            <div className="mt-5 flex flex-wrap gap-2">
              {selectedEntry.supportedScopes.map((scope) => (
                <span
                  key={scope}
                  className="rounded-full border border-white/10 bg-white/5 px-3 py-1 text-[11px] uppercase tracking-[0.28em] text-white/60"
                >
                  {titleCase(scope)}
                </span>
              ))}
            </div>

            {selectedEntry.unsupportedFields.length > 0 ? (
              <div className="mt-5 rounded-2xl border border-amber-300/25 bg-amber-300/10 p-4">
                <div className="font-tech text-[10px] uppercase tracking-[0.28em] text-amber-100">
                  Unsupported Imported Fields
                </div>
                <div className="mt-3 flex flex-wrap gap-2">
                  {selectedEntry.unsupportedFields.map((field) => (
                    <span
                      key={field}
                      className="rounded-full border border-amber-200/20 bg-[#090611] px-3 py-1 text-sm text-amber-100"
                    >
                      {field}
                    </span>
                  ))}
                </div>
              </div>
            ) : null}
          </section>

          <section className="rounded-[1.5rem] border border-white/10 bg-[#120b1d]/88 p-5">
            <div className="mb-4 flex items-center gap-3">
              <SlidersHorizontal size={20} className="text-cyan-200" />
              <div>
                <h3 className="font-archaic text-xl">Base Settings</h3>
                <p className="text-sm text-white/55">
                  Persisted defaults for all sessions unless a scoped override takes precedence.
                </p>
              </div>
            </div>

            <div className="grid gap-4 lg:grid-cols-2">
              {selectedEntry.schema.map((field) => (
                <FieldEditor
                  key={field.key}
                  field={field}
                  value={settingsDraft[field.key]}
                  onChange={(nextValue) =>
                    setSettingsDraft((current) => ({
                      ...current,
                      [field.key]: nextValue,
                    }))
                  }
                />
              ))}
            </div>

            <div className="mt-4 flex flex-wrap gap-3">
              <button
                type="button"
                onClick={() => void handleSaveSettings()}
                className="inline-flex items-center gap-2 rounded-full border border-cyan-300/20 bg-cyan-300/10 px-4 py-2 text-sm text-cyan-100 transition hover:border-cyan-200/40 hover:bg-cyan-300/15"
              >
                <FloppyDisk size={16} />
                Save Settings
              </button>
              <button
                type="button"
                onClick={() => setSettingsDraft(cloneSettings(selectedEntry.settings))}
                className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-4 py-2 text-sm text-white transition hover:border-white/30 hover:bg-white/10"
              >
                <ArrowsClockwise size={16} />
                Revert Settings
              </button>
            </div>
          </section>

          <section className="rounded-[1.5rem] border border-white/10 bg-[#120b1d]/88 p-5">
            <div className="mb-4 flex items-center gap-3">
              <ShieldWarning size={20} className="text-fuchsia-200" />
              <div>
                <h3 className="font-archaic text-xl">Scoped Overrides</h3>
                <p className="text-sm text-white/55">
                  Apply per-character, per-group, or per-session settings without touching the base profile.
                </p>
              </div>
            </div>

            {selectedEntry.overrides.length > 0 ? (
              <div className="mb-4 flex flex-wrap gap-2">
                {selectedEntry.overrides.map((override) => (
                  <button
                    key={`${override.scope.kind}:${override.scope.id}`}
                    type="button"
                    onClick={() => {
                      setScopeKind(override.scope.kind);
                      setScopeId(override.scope.id);
                    }}
                    className="rounded-full border border-white/10 bg-white/5 px-3 py-1 text-sm text-white/75 transition hover:border-white/30 hover:bg-white/10"
                  >
                    {titleCase(override.scope.kind)}: {override.scope.id}
                  </button>
                ))}
              </div>
            ) : null}

            <div className="grid gap-4 md:grid-cols-[180px_minmax(0,1fr)]">
              <label className="block rounded-2xl border border-white/10 bg-white/5 p-4">
                <div className="font-medium text-white">Scope Kind</div>
                <select
                  value={scopeKind}
                  onChange={(event) => setScopeKind(event.target.value as ScopeKind)}
                  className="mt-3 w-full rounded-xl border border-white/10 bg-[#120b1d] px-3 py-2 text-sm text-white outline-none"
                >
                  {selectedEntry.supportedScopes.map((scope) => (
                    <option key={scope} value={scope}>
                      {titleCase(scope)}
                    </option>
                  ))}
                </select>
              </label>

              <label htmlFor="scope-id" className="block rounded-2xl border border-white/10 bg-white/5 p-4">
                <div className="font-medium text-white">Scope ID</div>
                <div className="mt-1 text-sm text-white/55">
                  Character name, group identifier, or session token.
                </div>
                <input
                  id="scope-id"
                  value={scopeId}
                  onChange={(event) => setScopeId(event.target.value)}
                  className="mt-3 w-full rounded-xl border border-white/10 bg-[#120b1d] px-3 py-2 text-sm text-white outline-none"
                />
              </label>
            </div>

            <div className="mt-4 grid gap-4 lg:grid-cols-2">
              {selectedEntry.schema.map((field) => (
                <FieldEditor
                  key={`override-${field.key}`}
                  field={field}
                  value={overrideDraft[field.key]}
                  onChange={(nextValue) =>
                    setOverrideDraft((current) => ({
                      ...current,
                      [field.key]: nextValue,
                    }))
                  }
                />
              ))}
            </div>

            <div className="mt-4 flex flex-wrap gap-3">
              <button
                type="button"
                onClick={() => void handleSaveOverride()}
                className="inline-flex items-center gap-2 rounded-full border border-fuchsia-300/20 bg-fuchsia-300/10 px-4 py-2 text-sm text-fuchsia-100 transition hover:border-fuchsia-200/40 hover:bg-fuchsia-300/15"
              >
                <FloppyDisk size={16} />
                Save Override
              </button>
              <button
                type="button"
                onClick={() =>
                  setOverrideDraft(
                    cloneSettings(selectedOverride?.settings ?? selectedEntry.settings)
                  )
                }
                className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-4 py-2 text-sm text-white transition hover:border-white/30 hover:bg-white/10"
              >
                <ArrowsClockwise size={16} />
                Revert Override
              </button>
              <button
                type="button"
                onClick={() => void handleDeleteOverride()}
                className="inline-flex items-center gap-2 rounded-full border border-rose-300/20 bg-rose-300/10 px-4 py-2 text-sm text-rose-100 transition hover:border-rose-200/40 hover:bg-rose-300/15"
              >
                <Trash size={16} />
                Delete Override
              </button>
            </div>
          </section>
        </div>
      </div>
    </section>
  );
}
