import type { ReactNode } from "react";
import {
  ArrowsClockwise,
  CheckCircle,
  Plus,
  Trash,
} from "@phosphor-icons/react";

import type {
  CollectionRoute,
  CursorAction,
  CursorRule,
  InventoryUtilityConfig,
  PluginCoverageStatus,
  RelocationRule,
  RewardRoutingRule,
  VendorWatchRule,
} from "../types";

const CURSOR_ACTIONS: CursorAction[] = ["keep", "sell", "destroy", "consume"];
const COLLECTION_ROUTES: CollectionRoute[] = ["keep", "bank", "tribute", "sell"];

function sectionCard(title: string, description: string, content: ReactNode) {
  return (
    <div className="bg-void/60 border border-white/10 p-4 space-y-3">
      <div>
        <h4 className="text-sm font-semibold text-white">{title}</h4>
        <p className="text-[11px] text-white/45 mt-1">{description}</p>
      </div>
      {content}
    </div>
  );
}

function badgeColor(status: PluginCoverageStatus): string {
  switch (status) {
    case "native":
      return "border-cyan-300/40 bg-cyan-400/10 text-cyan-100";
    case "adapted":
      return "border-yellow-400/40 bg-yellow-400/10 text-yellow-100";
    case "deferred":
      return "border-red-400/40 bg-red-400/10 text-red-100";
  }
}

function ToggleField({
  label,
  description,
  checked,
  onChange,
}: {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex items-start gap-3 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white">
      <input
        aria-label={label}
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
        className="mt-1 h-4 w-4 rounded border-white/20 bg-black/30 text-cyan-300"
      />
      <span>
        <span className="block font-medium">{label}</span>
        {description ? (
          <span className="block text-[11px] text-white/45 mt-0.5">{description}</span>
        ) : null}
      </span>
    </label>
  );
}

function CsvField({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string;
  value: string[];
  onChange: (value: string[]) => void;
  placeholder?: string;
}) {
  return (
    <label className="block space-y-1">
      <span className="text-xs uppercase tracking-wider text-white/45">{label}</span>
      <input
        aria-label={label}
        value={value.join(", ")}
        onChange={(event) =>
          onChange(
            event.target.value
              .split(",")
              .map((entry) => entry.trim())
              .filter(Boolean),
          )
        }
        placeholder={placeholder}
        className="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-cyan-300/50 focus:outline-none placeholder:text-white/25"
      />
    </label>
  );
}

function ActionSelect({
  label,
  value,
  onChange,
  allowNone = false,
}: {
  label: string;
  value: CursorAction | null;
  onChange: (value: CursorAction | null) => void;
  allowNone?: boolean;
}) {
  return (
    <label className="block space-y-1">
      <span className="text-[10px] uppercase tracking-wider text-white/45">{label}</span>
      <select
        aria-label={label}
        value={value ?? "none"}
        onChange={(event) =>
          onChange(
            event.target.value === "none"
              ? null
              : (event.target.value as CursorAction),
          )
        }
        className="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-cyan-300/50 focus:outline-none"
      >
        {allowNone ? <option value="none">None</option> : null}
        {CURSOR_ACTIONS.map((action) => (
          <option key={action} value={action}>
            {action}
          </option>
        ))}
      </select>
    </label>
  );
}

function RouteSelect({
  label,
  value,
  onChange,
}: {
  label: string;
  value: CollectionRoute;
  onChange: (value: CollectionRoute) => void;
}) {
  return (
    <label className="block space-y-1">
      <span className="text-[10px] uppercase tracking-wider text-white/45">{label}</span>
      <select
        aria-label={label}
        value={value}
        onChange={(event) => onChange(event.target.value as CollectionRoute)}
        className="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-cyan-300/50 focus:outline-none"
      >
        {COLLECTION_ROUTES.map((route) => (
          <option key={route} value={route}>
            {route}
          </option>
        ))}
      </select>
    </label>
  );
}

function NumberField({
  label,
  value,
  min = 0,
  onChange,
}: {
  label: string;
  value: number | null;
  min?: number;
  onChange: (value: number | null) => void;
}) {
  return (
    <label className="block space-y-1">
      <span className="text-[10px] uppercase tracking-wider text-white/45">{label}</span>
      <input
        aria-label={label}
        type="number"
        min={min}
        value={value ?? ""}
        onChange={(event) =>
          onChange(event.target.value === "" ? null : Number(event.target.value))
        }
        className="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-cyan-300/50 focus:outline-none"
      />
    </label>
  );
}

export default function InventoryUtilitySection({
  config,
  loading,
  saving,
  error,
  savedAt,
  onChange,
  onRefresh,
}: {
  config: InventoryUtilityConfig;
  loading: boolean;
  saving: boolean;
  error: string | null;
  savedAt: number | null;
  onChange: (config: InventoryUtilityConfig) => void;
  onRefresh: () => void;
}) {
  const updateCursorRule = (
    index: number,
    patch: Partial<CursorRule>,
  ) => {
    const next = config.cursor_rules.map((rule, ruleIndex) =>
      ruleIndex === index ? { ...rule, ...patch } : rule,
    );
    onChange({ ...config, cursor_rules: next });
  };

  const updateRewardRule = (
    index: number,
    patch: Partial<RewardRoutingRule>,
  ) => {
    const next = config.reward_routing.map((rule, ruleIndex) =>
      ruleIndex === index ? { ...rule, ...patch } : rule,
    );
    onChange({ ...config, reward_routing: next });
  };

  const removeRewardRule = (index: number) => {
    onChange({
      ...config,
      reward_routing: config.reward_routing.filter((_, ruleIndex) => ruleIndex !== index),
    });
  };

  const updateVendorWatchRule = (
    index: number,
    patch: Partial<VendorWatchRule>,
  ) => {
    const next = config.vendor_watch.map((rule, ruleIndex) =>
      ruleIndex === index ? { ...rule, ...patch } : rule,
    );
    onChange({ ...config, vendor_watch: next });
  };

  const updateRelocationRule = (
    index: number,
    patch: Partial<RelocationRule>,
  ) => {
    const next = config.relocation_rules.map((rule, ruleIndex) =>
      ruleIndex === index ? { ...rule, ...patch } : rule,
    );
    onChange({ ...config, relocation_rules: next });
  };

  if (loading) {
    return (
      <div className="rounded-2xl border border-white/10 bg-black/20 p-6 text-sm text-white/60">
        Loading inventory utility config…
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between rounded-2xl border border-white/10 bg-black/20 px-4 py-3">
        <div>
          <h3 className="text-sm font-semibold text-white">Shared Inventory Utility Parity</h3>
          <p className="text-[11px] text-white/45 mt-1">
            Provenance-aware item knowledge, routing, claim, consume, and relocation rules.
          </p>
        </div>
        <button
          type="button"
          onClick={onRefresh}
          className="inline-flex items-center gap-2 rounded-xl border border-white/10 px-3 py-2 text-xs uppercase tracking-wider text-white/70 hover:bg-white/5"
        >
          <ArrowsClockwise size={14} />
          Refresh
        </button>
      </div>

      {error ? (
        <div className="rounded-2xl border border-red-400/30 bg-red-400/10 px-4 py-3 text-sm text-red-100">
          {error}
        </div>
      ) : null}

      {savedAt && !saving ? (
        <div className="rounded-2xl border border-cyan-300/25 bg-cyan-400/10 px-4 py-3 text-sm text-cyan-100">
          Inventory utility config saved.
        </div>
      ) : null}

      {sectionCard(
        "Plugin Coverage",
        "Every RedGuides plugin in scope has an explicit owner, status, and config surface.",
        <div className="overflow-x-auto">
          <table className="w-full min-w-[760px] text-left text-sm">
            <thead className="text-[10px] uppercase tracking-wider text-white/40">
              <tr>
                <th className="pb-2">Plugin</th>
                <th className="pb-2">Owner</th>
                <th className="pb-2">Status</th>
                <th className="pb-2">Surface</th>
                <th className="pb-2">Notes</th>
              </tr>
            </thead>
            <tbody>
              {config.plugin_mappings.map((mapping) => (
                <tr key={mapping.plugin} className="border-t border-white/10 align-top">
                  <td className="py-2 pr-3 font-medium text-white">{mapping.plugin}</td>
                  <td className="py-2 pr-3 text-white/70">{mapping.owner}</td>
                  <td className="py-2 pr-3">
                    <span
                      className={`inline-flex rounded-full border px-2 py-1 text-[10px] uppercase tracking-wider ${badgeColor(mapping.status)}`}
                    >
                      {mapping.status}
                    </span>
                  </td>
                  <td className="py-2 pr-3 text-white/60">{mapping.config_surface}</td>
                  <td className="py-2 text-white/60">{mapping.notes}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>,
      )}

      {sectionCard(
        "Legacy Adapter Warnings",
        "Provenance and unsupported-field warnings shown when legacy plugin configs are adapted into shared TextQuest rules.",
        <div className="space-y-3">
          {config.legacy_adapters.map((warning) => (
            <div
              key={`${warning.plugin}-${warning.source_reference}`}
              className="rounded-xl border border-yellow-400/20 bg-yellow-400/5 px-3 py-3"
            >
              <div className="flex items-center gap-2 text-sm text-yellow-50">
                <CheckCircle size={14} className="text-yellow-300" />
                <span className="font-medium">{warning.plugin}</span>
                <span className="text-white/40">from {warning.source_reference}</span>
              </div>
              <p className="mt-1 text-[11px] text-white/55">
                Adapted into <span className="text-white/80">{warning.adapted_into}</span>
              </p>
              {warning.unsupported_fields.length > 0 ? (
                <div className="mt-2 flex flex-wrap gap-2">
                  {warning.unsupported_fields.map((field) => (
                    <span
                      key={field}
                      className="rounded-full border border-yellow-300/25 bg-black/20 px-2 py-1 text-[10px] text-yellow-100"
                    >
                      {field}
                    </span>
                  ))}
                </div>
              ) : null}
            </div>
          ))}
        </div>,
      )}

      <div className="grid gap-6 lg:grid-cols-2">
        {sectionCard(
          "Item Knowledge",
          "Controls provenance badges and unsupported-field visibility for adapted configs.",
          <div className="space-y-3">
            <ToggleField
              label="Show Provenance"
              description="Display which native or adapted source produced each item knowledge rule."
              checked={config.item_knowledge.show_provenance}
              onChange={(show_provenance) =>
                onChange({
                  ...config,
                  item_knowledge: { ...config.item_knowledge, show_provenance },
                })
              }
            />
            <ToggleField
              label="Show Unsupported Fields"
              description="Flag legacy plugin fields that do not map cleanly into TextQuest."
              checked={config.item_knowledge.show_unsupported_fields}
              onChange={(show_unsupported_fields) =>
                onChange({
                  ...config,
                  item_knowledge: { ...config.item_knowledge, show_unsupported_fields },
                })
              }
            />
            <CsvField
              label="Knowledge Sources"
              value={config.item_knowledge.link_sources}
              onChange={(link_sources) =>
                onChange({
                  ...config,
                  item_knowledge: { ...config.item_knowledge, link_sources },
                })
              }
              placeholder="item-db, loot-history, reward-rules"
            />
          </div>,
        )}

        {sectionCard(
          "Consumption",
          "Shared food and drink policies used to replace `MQ2FeedMe`-style automation.",
          <div className="space-y-3">
            <ToggleField
              label="Enable Consumption Rules"
              checked={config.consumption.enabled}
              onChange={(enabled) =>
                onChange({
                  ...config,
                  consumption: { ...config.consumption, enabled },
                })
              }
            />
            <CsvField
              label="Preferred Food"
              value={config.consumption.preferred_food}
              onChange={(preferred_food) =>
                onChange({
                  ...config,
                  consumption: { ...config.consumption, preferred_food },
                })
              }
              placeholder="Fish Roll, Misty Thicket Picnic"
            />
            <CsvField
              label="Preferred Drink"
              value={config.consumption.preferred_drink}
              onChange={(preferred_drink) =>
                onChange({
                  ...config,
                  consumption: { ...config.consumption, preferred_drink },
                })
              }
              placeholder="Water Flask, Kaladim Constitutional"
            />
            <CsvField
              label="Ignored Items"
              value={config.consumption.ignored_items}
              onChange={(ignored_items) =>
                onChange({
                  ...config,
                  consumption: { ...config.consumption, ignored_items },
                })
              }
              placeholder="Summoned: Modulation Shard"
            />
          </div>,
        )}
      </div>

      {sectionCard(
        "Cursor Rules",
        "Editable keep, sell, destroy, or consume actions for cursor-handled items.",
        <div className="space-y-3">
          {config.cursor_rules.map((rule, index) => (
            <div
              key={`${rule.item_matcher}-${index}`}
              className="grid gap-3 rounded-xl border border-white/10 bg-black/20 p-3 lg:grid-cols-[2fr_1fr_1fr_1fr_auto]"
            >
              <label className="block space-y-1">
                <span className="text-[10px] uppercase tracking-wider text-white/45">Item</span>
                <input
                  aria-label={`Cursor item ${index + 1}`}
                  value={rule.item_matcher}
                  onChange={(event) =>
                    updateCursorRule(index, { item_matcher: event.target.value })
                  }
                  className="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-cyan-300/50 focus:outline-none"
                />
              </label>
              <ActionSelect
                label={`Cursor action ${index + 1}`}
                value={rule.action}
                onChange={(action) => updateCursorRule(index, { action: action ?? "keep" })}
              />
              <NumberField
                label={`Keep limit ${index + 1}`}
                value={rule.keep_at_or_below}
                onChange={(keep_at_or_below) => updateCursorRule(index, { keep_at_or_below })}
              />
              <ActionSelect
                label={`Overflow action ${index + 1}`}
                value={rule.overflow_action}
                allowNone
                onChange={(overflow_action) => updateCursorRule(index, { overflow_action })}
              />
              <button
                type="button"
                onClick={() =>
                  onChange({
                    ...config,
                    cursor_rules: config.cursor_rules.filter((_, ruleIndex) => ruleIndex !== index),
                  })
                }
                className="self-end rounded-xl border border-white/10 px-3 py-2 text-white/60 hover:bg-white/5"
              >
                <Trash size={14} />
              </button>
            </div>
          ))}
          <button
            type="button"
            onClick={() =>
              onChange({
                ...config,
                cursor_rules: [
                  ...config.cursor_rules,
                  {
                    item_matcher: "",
                    action: "keep",
                    keep_at_or_below: null,
                    overflow_action: null,
                  },
                ],
              })
            }
            className="inline-flex items-center gap-2 rounded-xl border border-white/10 px-3 py-2 text-xs uppercase tracking-wider text-white/70 hover:bg-white/5"
          >
            <Plus size={14} />
            Add Cursor Rule
          </button>
        </div>,
      )}

      <div className="grid gap-6 lg:grid-cols-2">
        {sectionCard(
          "Collection Routing",
          "Route incomplete, completed, and duplicate collection items consistently.",
          <div className="space-y-3">
            {config.collection_routing.map((rule, index) => (
              <div
                key={`${rule.set_matcher}-${index}`}
                className="grid gap-3 rounded-xl border border-white/10 bg-black/20 p-3"
              >
                <label className="block space-y-1">
                  <span className="text-[10px] uppercase tracking-wider text-white/45">Collection Set</span>
                  <input
                    aria-label={`Collection set ${index + 1}`}
                    value={rule.set_matcher}
                    onChange={(event) =>
                      onChange({
                        ...config,
                        collection_routing: config.collection_routing.map((entry, entryIndex) =>
                          entryIndex === index
                            ? { ...entry, set_matcher: event.target.value }
                            : entry,
                        ),
                      })
                    }
                    className="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-cyan-300/50 focus:outline-none"
                  />
                </label>
                <div className="grid gap-3 md:grid-cols-3">
                  <RouteSelect
                    label={`Incomplete route ${index + 1}`}
                    value={rule.incomplete_route}
                    onChange={(incomplete_route) =>
                      onChange({
                        ...config,
                        collection_routing: config.collection_routing.map((entry, entryIndex) =>
                          entryIndex === index ? { ...entry, incomplete_route } : entry,
                        ),
                      })
                    }
                  />
                  <RouteSelect
                    label={`Completed route ${index + 1}`}
                    value={rule.completed_route}
                    onChange={(completed_route) =>
                      onChange({
                        ...config,
                        collection_routing: config.collection_routing.map((entry, entryIndex) =>
                          entryIndex === index ? { ...entry, completed_route } : entry,
                        ),
                      })
                    }
                  />
                  <RouteSelect
                    label={`Duplicate route ${index + 1}`}
                    value={rule.duplicate_route}
                    onChange={(duplicate_route) =>
                      onChange({
                        ...config,
                        collection_routing: config.collection_routing.map((entry, entryIndex) =>
                          entryIndex === index ? { ...entry, duplicate_route } : entry,
                        ),
                      })
                    }
                  />
                </div>
              </div>
            ))}
            <button
              type="button"
              onClick={() =>
                onChange({
                  ...config,
                  collection_routing: [
                    ...config.collection_routing,
                    {
                      set_matcher: "",
                      incomplete_route: "keep",
                      completed_route: "bank",
                      duplicate_route: "bank",
                    },
                  ],
                })
              }
              className="inline-flex items-center gap-2 rounded-xl border border-white/10 px-3 py-2 text-xs uppercase tracking-wider text-white/70 hover:bg-white/5"
            >
              <Plus size={14} />
              Add Collection Rule
            </button>
          </div>,
        )}

        {sectionCard(
          "Reward Routing",
          "Choose reward name or one-based reward position and whether the choice auto-claims.",
          <div className="space-y-3">
            {config.reward_routing.map((rule, index) => (
              <div
                key={`${rule.task_matcher}-${index}`}
                className="space-y-3 rounded-xl border border-white/10 bg-black/20 p-3"
              >
                <label className="block space-y-1">
                  <span className="text-[10px] uppercase tracking-wider text-white/45">Task Matcher</span>
                  <input
                    aria-label={`Reward task ${index + 1}`}
                    value={rule.task_matcher}
                    onChange={(event) =>
                      updateRewardRule(index, { task_matcher: event.target.value })
                    }
                    className="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-cyan-300/50 focus:outline-none"
                  />
                </label>
                <div className="grid gap-3 md:grid-cols-[1fr_1fr_auto]">
                  <label className="block space-y-1">
                    <span className="text-[10px] uppercase tracking-wider text-white/45">Preference Type</span>
                    <select
                      aria-label={`Reward preference type ${index + 1}`}
                      value={rule.preference.kind}
                      onChange={(event) =>
                        updateRewardRule(index, {
                          preference:
                            event.target.value === "by_name"
                              ? { kind: "by_name", reward_name: "" }
                              : { kind: "by_position", reward_position: 1 },
                        })
                      }
                      className="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-cyan-300/50 focus:outline-none"
                    >
                      <option value="by_name">Reward name</option>
                      <option value="by_position">Reward position</option>
                    </select>
                  </label>
                  {rule.preference.kind === "by_name" ? (
                    <label className="block space-y-1">
                      <span className="text-[10px] uppercase tracking-wider text-white/45">Reward Name</span>
                      <input
                        aria-label={`Reward name ${index + 1}`}
                        value={rule.preference.reward_name}
                        onChange={(event) =>
                          updateRewardRule(index, {
                            preference: {
                              kind: "by_name",
                              reward_name: event.target.value,
                            },
                          })
                        }
                        className="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-cyan-300/50 focus:outline-none"
                      />
                    </label>
                  ) : (
                    <NumberField
                      label={`Reward position ${index + 1}`}
                      min={1}
                      value={rule.preference.reward_position}
                      onChange={(reward_position) =>
                        updateRewardRule(index, {
                          preference: {
                            kind: "by_position",
                            reward_position: reward_position ?? 1,
                          },
                        })
                      }
                    />
                  )}
                  <div className="self-end">
                    <ToggleField
                      label={`Auto-Claim Rule ${index + 1}`}
                      checked={rule.auto_claim}
                      onChange={(auto_claim) => updateRewardRule(index, { auto_claim })}
                    />
                  </div>
                  <button
                    type="button"
                    aria-label={`Remove reward rule ${index + 1}`}
                    onClick={() => removeRewardRule(index)}
                    className="self-end rounded-xl border border-white/10 px-3 py-2 text-white/60 hover:bg-white/5"
                  >
                    <Trash size={14} />
                  </button>
                </div>
              </div>
            ))}
            <button
              type="button"
              onClick={() =>
                onChange({
                  ...config,
                  reward_routing: [
                    ...config.reward_routing,
                    {
                      task_matcher: "*",
                      preference: { kind: "by_position", reward_position: 1 },
                      auto_claim: false,
                    },
                  ],
                })
              }
              className="inline-flex items-center gap-2 rounded-xl border border-white/10 px-3 py-2 text-xs uppercase tracking-wider text-white/70 hover:bg-white/5"
            >
              <Plus size={14} />
              Add Reward Rule
            </button>
          </div>,
        )}
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        {sectionCard(
          "Vendor Watch",
          "Watch specific vendor items and optionally cap the acceptable price in platinum.",
          <div className="space-y-3">
            {config.vendor_watch.map((rule, index) => (
              <div
                key={`${rule.item_name}-${index}`}
                className="grid gap-3 rounded-xl border border-white/10 bg-black/20 p-3 md:grid-cols-[2fr_1fr_auto_auto]"
              >
                <label className="block space-y-1">
                  <span className="text-[10px] uppercase tracking-wider text-white/45">Item</span>
                  <input
                    aria-label={`Vendor watch item ${index + 1}`}
                    value={rule.item_name}
                    onChange={(event) =>
                      updateVendorWatchRule(index, { item_name: event.target.value })
                    }
                    className="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-cyan-300/50 focus:outline-none"
                  />
                </label>
                <NumberField
                  label={`Max price ${index + 1}`}
                  value={rule.max_price_pp}
                  onChange={(max_price_pp) => updateVendorWatchRule(index, { max_price_pp })}
                />
                <div className="self-end">
                  <ToggleField
                    label={`Notify ${index + 1}`}
                    checked={rule.notify}
                    onChange={(notify) => updateVendorWatchRule(index, { notify })}
                  />
                </div>
                <button
                  type="button"
                  onClick={() =>
                    onChange({
                      ...config,
                      vendor_watch: config.vendor_watch.filter((_, ruleIndex) => ruleIndex !== index),
                    })
                  }
                  className="self-end rounded-xl border border-white/10 px-3 py-2 text-white/60 hover:bg-white/5"
                >
                  <Trash size={14} />
                </button>
              </div>
            ))}
            <button
              type="button"
              onClick={() =>
                onChange({
                  ...config,
                  vendor_watch: [
                    ...config.vendor_watch,
                    { item_name: "", max_price_pp: null, notify: true },
                  ],
                })
              }
              className="inline-flex items-center gap-2 rounded-xl border border-white/10 px-3 py-2 text-xs uppercase tracking-wider text-white/70 hover:bg-white/5"
            >
              <Plus size={14} />
              Add Vendor Watch
            </button>
          </div>,
        )}

        {sectionCard(
          "Relocation Rules",
          "Track destination-specific clicky or AA retention rules for relocation utilities.",
          <div className="space-y-3">
            {config.relocation_rules.map((rule, index) => (
              <div
                key={`${rule.destination}-${index}`}
                className="grid gap-3 rounded-xl border border-white/10 bg-black/20 p-3"
              >
                <div className="grid gap-3 md:grid-cols-2">
                  <label className="block space-y-1">
                    <span className="text-[10px] uppercase tracking-wider text-white/45">Destination</span>
                    <input
                      aria-label={`Relocation destination ${index + 1}`}
                      value={rule.destination}
                      onChange={(event) =>
                        updateRelocationRule(index, { destination: event.target.value })
                      }
                      className="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-cyan-300/50 focus:outline-none"
                    />
                  </label>
                  <label className="block space-y-1">
                    <span className="text-[10px] uppercase tracking-wider text-white/45">Required Option ID</span>
                    <input
                      aria-label={`Required relocation option ${index + 1}`}
                      value={rule.required_option_id ?? ""}
                      onChange={(event) =>
                        updateRelocationRule(index, {
                          required_option_id: event.target.value || null,
                        })
                      }
                      className="w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-cyan-300/50 focus:outline-none"
                    />
                  </label>
                </div>
                <div className="grid gap-3 md:grid-cols-[1fr_auto]">
                  <NumberField
                    label={`Keep on hand ${index + 1}`}
                    min={0}
                    value={rule.keep_on_hand}
                    onChange={(keep_on_hand) =>
                      updateRelocationRule(index, { keep_on_hand: keep_on_hand ?? 0 })
                    }
                  />
                  <div className="self-end">
                    <ToggleField
                      label={`Notify if unavailable ${index + 1}`}
                      checked={rule.notify_if_unavailable}
                      onChange={(notify_if_unavailable) =>
                        updateRelocationRule(index, { notify_if_unavailable })
                      }
                    />
                  </div>
                </div>
              </div>
            ))}
            <button
              type="button"
              onClick={() =>
                onChange({
                  ...config,
                  relocation_rules: [
                    ...config.relocation_rules,
                    {
                      destination: "",
                      required_option_id: null,
                      keep_on_hand: 1,
                      notify_if_unavailable: true,
                    },
                  ],
                })
              }
              className="inline-flex items-center gap-2 rounded-xl border border-white/10 px-3 py-2 text-xs uppercase tracking-wider text-white/70 hover:bg-white/5"
            >
              <Plus size={14} />
              Add Relocation Rule
            </button>
          </div>,
        )}
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        {sectionCard(
          "Tradeskill Trophy",
          "Trophy restore and equip preferences for `MQ2TSTrophy` parity.",
          <div className="space-y-3">
            <ToggleField
              label="Enable Trophy Handling"
              checked={config.trophy_preferences.enabled}
              onChange={(enabled) =>
                onChange({
                  ...config,
                  trophy_preferences: { ...config.trophy_preferences, enabled },
                })
              }
            />
            <ToggleField
              label="Auto-Equip Trophy"
              checked={config.trophy_preferences.auto_equip}
              onChange={(auto_equip) =>
                onChange({
                  ...config,
                  trophy_preferences: { ...config.trophy_preferences, auto_equip },
                })
              }
            />
            <ToggleField
              label="Restore After Craft"
              checked={config.trophy_preferences.restore_after_craft}
              onChange={(restore_after_craft) =>
                onChange({
                  ...config,
                  trophy_preferences: { ...config.trophy_preferences, restore_after_craft },
                })
              }
            />
            <CsvField
              label="Trophy Items"
              value={config.trophy_preferences.trophy_items}
              onChange={(trophy_items) =>
                onChange({
                  ...config,
                  trophy_preferences: { ...config.trophy_preferences, trophy_items },
                })
              }
              placeholder="Blacksmithing Trophy"
            />
          </div>,
        )}

        {sectionCard(
          "Auto-Claim",
          "Shared claim policy for reward windows and membership grants.",
          <div className="space-y-3">
            <ToggleField
              label="Enable Auto-Claim"
              checked={config.auto_claim.enabled}
              onChange={(enabled) =>
                onChange({
                  ...config,
                  auto_claim: { ...config.auto_claim, enabled },
                })
              }
            />
            <ToggleField
              label="Claim Membership Grants"
              checked={config.auto_claim.claim_membership_grants}
              onChange={(claim_membership_grants) =>
                onChange({
                  ...config,
                  auto_claim: { ...config.auto_claim, claim_membership_grants },
                })
              }
            />
            <ToggleField
              label="Claim Task Windows"
              checked={config.auto_claim.claim_task_windows}
              onChange={(claim_task_windows) =>
                onChange({
                  ...config,
                  auto_claim: { ...config.auto_claim, claim_task_windows },
                })
              }
            />
            <ToggleField
              label="Claim Once Per Session"
              checked={config.auto_claim.once_per_session}
              onChange={(once_per_session) =>
                onChange({
                  ...config,
                  auto_claim: { ...config.auto_claim, once_per_session },
                })
              }
            />
          </div>,
        )}
      </div>
    </div>
  );
}
