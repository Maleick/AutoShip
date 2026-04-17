import { useState, useEffect, useRef } from "react";
import type { ElementType } from "react";
import {
  Bag,
  ArrowsClockwise,
  User,
  ListBullets,
  Scroll,
  Plus,
  Trash,
  Crown,
  CheckCircle,
  ArrowClockwise,
  Star,
} from "@phosphor-icons/react";
import { useItemScoreConfig } from "../hooks/useItemScoreConfig";
import {
  demoLootRules,
  demoCharacterFilters,
  demoDistributionConfig,
  demoMasterLooter,
  demoLootHistory,
} from "../data/demo";
import type {
  LootRules,
  CharacterLootFilter,
  FilterAction,
  DistributionConfig,
  DistributionMethod,
  DistributionRule,
  ItemScoreConfig,
  MasterLooter,
  LootHistoryEntry,
  StatWeights,
} from "../types";

// ── Helpers ──────────────────────────────────────────────────────────────────

function actionColor(action: FilterAction) {
  switch (action) {
    case "keep":
      return "text-spectral border-spectral/40 bg-spectral/10";
    case "sell":
      return "text-yellow-400 border-yellow-400/40 bg-yellow-400/10";
    case "destroy":
      return "text-red-400 border-red-400/40 bg-red-400/10";
    case "bank":
      return "text-violet border-violet/60 bg-violet/20";
  }
}

function methodLabel(method: DistributionMethod): string {
  switch (method) {
    case "need_before_greed":
      return "Need before Greed";
    case "greed":
      return "Greed";
    case "round_robin":
      return "Round Robin";
    case "master_looter":
      return "Master Looter";
    case "free_for_all":
      return "Free For All";
  }
}

function methodColor(method: DistributionMethod): string {
  switch (method) {
    case "need_before_greed":
      return "text-spectral";
    case "greed":
      return "text-yellow-400";
    case "round_robin":
      return "text-magentaglow";
    case "master_looter":
      return "text-orange-400";
    case "free_for_all":
      return "text-white/60";
  }
}

// ── Section: Loot Rules ───────────────────────────────────────────────────────

function ItemListEditor({
  label,
  color,
  items,
  onChange,
}: {
  label: string;
  color: string;
  items: string[];
  onChange: (items: string[]) => void;
}) {
  const [draft, setDraft] = useState("");

  function add() {
    const trimmed = draft.trim();
    if (!trimmed || items.includes(trimmed)) return;
    onChange([...items, trimmed]);
    setDraft("");
  }

  return (
    <div className="flex-1 min-w-0">
      <h5 className={`text-[10px] uppercase tracking-widest font-tech mb-2 ${color}`}>
        {label}
      </h5>
      <div className="space-y-1 mb-2 min-h-[60px]">
        {items.map((item) => (
          <div
            key={item}
            className="flex items-center justify-between bg-void/60 border border-white/10 px-2 py-1 text-xs text-white/80"
          >
            <span className="truncate">{item}</span>
            <button
              onClick={() => onChange(items.filter((i) => i !== item))}
              className="ml-2 text-white/30 hover:text-red-400 transition-colors"
            >
              <Trash size={12} />
            </button>
          </div>
        ))}
        {items.length === 0 && (
          <p className="text-[10px] text-white/20 italic py-2">No items</p>
        )}
      </div>
      <div className="flex gap-1">
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
          placeholder="Item name…"
          className="flex-1 bg-void border border-white/20 text-white text-xs px-2 py-1 focus:outline-none focus:border-magentaglow placeholder:text-white/25"
        />
        <button
          onClick={add}
          className="px-2 border border-white/20 bg-white/5 hover:bg-white/10 transition-colors text-white/60 hover:text-white"
        >
          <Plus size={12} />
        </button>
      </div>
    </div>
  );
}

function LootRulesSection({
  rules,
  onChange,
}: {
  rules: LootRules;
  onChange: (r: LootRules) => void;
}) {
  function toggle(key: "loot_all" | "auto_split") {
    onChange({ ...rules, [key]: !rules[key] });
  }

  return (
    <div className="space-y-6">
      {/* Toggles */}
      <div className="grid grid-cols-2 gap-4">
        {(
          [
            { key: "loot_all", label: "Loot All", desc: "Pick up everything not in destroy list" },
            { key: "auto_split", label: "Auto-Split Coin", desc: "Automatically split coin with group" },
          ] as const
        ).map(({ key, label, desc }) => (
          <button
            key={key}
            onClick={() => toggle(key)}
            className={`flex items-start gap-3 p-3 border text-left transition-all ${
              rules[key]
                ? "border-spectral/40 bg-spectral/10"
                : "border-white/10 bg-void/40 hover:bg-white/5"
            }`}
          >
            <div
              className={`mt-0.5 w-4 h-4 rounded-sm border flex items-center justify-center transition-colors ${
                rules[key] ? "border-spectral bg-spectral/30" : "border-white/30"
              }`}
            >
              {rules[key] && <CheckCircle size={12} weight="fill" className="text-spectral" />}
            </div>
            <div>
              <div className="text-sm font-medium text-white">{label}</div>
              <div className="text-[10px] text-white/40 mt-0.5">{desc}</div>
            </div>
          </button>
        ))}
      </div>

      {/* Item lists */}
      <div className="flex gap-4">
        <ItemListEditor
          label="Keep"
          color="text-spectral"
          items={rules.keep_items}
          onChange={(v) => onChange({ ...rules, keep_items: v })}
        />
        <ItemListEditor
          label="Sell"
          color="text-yellow-400"
          items={rules.sell_items}
          onChange={(v) => onChange({ ...rules, sell_items: v })}
        />
        <ItemListEditor
          label="Destroy"
          color="text-red-400"
          items={rules.destroy_items}
          onChange={(v) => onChange({ ...rules, destroy_items: v })}
        />
      </div>
    </div>
  );
}

// ── Section: Auto-Loot Filters ────────────────────────────────────────────────

function CharacterFilterCard({
  filter,
  onChange,
}: {
  filter: CharacterLootFilter;
  onChange: (f: CharacterLootFilter) => void;
}) {
  const [draft, setDraft] = useState("");
  const [draftAction, setDraftAction] = useState<FilterAction>("keep");

  function addEntry() {
    const trimmed = draft.trim();
    if (!trimmed) return;
    if (filter.filters.some((e) => e.item_name === trimmed)) return;
    onChange({
      ...filter,
      filters: [...filter.filters, { item_name: trimmed, action: draftAction }],
    });
    setDraft("");
  }

  function removeEntry(item_name: string) {
    onChange({ ...filter, filters: filter.filters.filter((e) => e.item_name !== item_name) });
  }

  return (
    <div className="bg-void/60 border border-white/10 p-4">
      <div className="flex items-center gap-2 mb-3">
        <User size={14} className="text-spectral" />
        <span className="font-medium text-white text-sm">{filter.character}</span>
        <span className="text-[10px] text-white/30 ml-auto font-rune">
          {filter.filters.length} rule{filter.filters.length !== 1 ? "s" : ""}
        </span>
      </div>
      <div className="space-y-1 mb-3 min-h-[40px]">
        {filter.filters.map((entry) => (
          <div
            key={entry.item_name}
            className="flex items-center gap-2 text-xs"
          >
            <span
              className={`px-1.5 py-0.5 border text-[10px] uppercase tracking-wide font-tech ${actionColor(entry.action)}`}
            >
              {entry.action}
            </span>
            <span className="flex-1 text-white/70 truncate">{entry.item_name}</span>
            <button
              onClick={() => removeEntry(entry.item_name)}
              className="text-white/20 hover:text-red-400 transition-colors"
            >
              <Trash size={11} />
            </button>
          </div>
        ))}
        {filter.filters.length === 0 && (
          <p className="text-[10px] text-white/20 italic">No filters set</p>
        )}
      </div>
      <div className="flex gap-1">
        <select
          value={draftAction}
          onChange={(e) => setDraftAction(e.target.value as FilterAction)}
          className="bg-void border border-white/20 text-white text-[10px] px-1 py-1 focus:outline-none focus:border-magentaglow uppercase"
        >
          <option value="keep">Keep</option>
          <option value="sell">Sell</option>
          <option value="destroy">Destroy</option>
          <option value="bank">Bank</option>
        </select>
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && addEntry()}
          placeholder="Item name…"
          className="flex-1 bg-void border border-white/20 text-white text-xs px-2 py-1 focus:outline-none focus:border-magentaglow placeholder:text-white/25"
        />
        <button
          onClick={addEntry}
          className="px-2 border border-white/20 bg-white/5 hover:bg-white/10 transition-colors text-white/60 hover:text-white"
        >
          <Plus size={12} />
        </button>
      </div>
    </div>
  );
}

function AutoLootFiltersSection({
  filters,
  onChange,
}: {
  filters: CharacterLootFilter[];
  onChange: (f: CharacterLootFilter[]) => void;
}) {
  function updateFilter(updated: CharacterLootFilter) {
    onChange(filters.map((f) => (f.character === updated.character ? updated : f)));
  }

  return (
    <div className="grid grid-cols-2 gap-4">
      {filters.map((f) => (
        <CharacterFilterCard key={f.character} filter={f} onChange={updateFilter} />
      ))}
    </div>
  );
}

// ── Section: Distribution Rules ───────────────────────────────────────────────

const ALL_METHODS: DistributionMethod[] = [
  "need_before_greed",
  "greed",
  "round_robin",
  "master_looter",
  "free_for_all",
];

const ALL_TYPES = ["all", "armor", "weapon", "jewelry", "misc"];
const ALL_QUALITIES = ["nodrop", "rare", "magical", "common"];

function DistributionRuleRow({
  rule,
  onRemove,
  onChange,
}: {
  rule: DistributionRule;
  onRemove: () => void;
  onChange: (r: DistributionRule) => void;
}) {
  return (
    <div className="flex items-center gap-3 px-3 py-2 bg-void/60 border border-white/10 text-sm">
      <select
        value={rule.item_type}
        onChange={(e) => onChange({ ...rule, item_type: e.target.value })}
        className="bg-void border border-white/20 text-white text-xs px-1 py-0.5 focus:outline-none focus:border-magentaglow capitalize"
      >
        {ALL_TYPES.map((t) => (
          <option key={t} value={t}>
            {t}
          </option>
        ))}
      </select>
      <select
        value={rule.quality ?? ""}
        onChange={(e) =>
          onChange({ ...rule, quality: e.target.value === "" ? null : e.target.value })
        }
        className="bg-void border border-white/20 text-white text-xs px-1 py-0.5 focus:outline-none focus:border-magentaglow capitalize"
      >
        <option value="">Any quality</option>
        {ALL_QUALITIES.map((q) => (
          <option key={q} value={q}>
            {q}
          </option>
        ))}
      </select>
      <span className="text-white/30">→</span>
      <select
        value={rule.method}
        onChange={(e) => onChange({ ...rule, method: e.target.value as DistributionMethod })}
        className={`bg-void border border-white/20 text-xs px-1 py-0.5 focus:outline-none focus:border-magentaglow flex-1 ${methodColor(rule.method)}`}
      >
        {ALL_METHODS.map((m) => (
          <option key={m} value={m}>
            {methodLabel(m)}
          </option>
        ))}
      </select>
      <button
        onClick={onRemove}
        className="text-white/20 hover:text-red-400 transition-colors"
      >
        <Trash size={13} />
      </button>
    </div>
  );
}

function DistributionSection({
  config,
  onChange,
}: {
  config: DistributionConfig;
  onChange: (c: DistributionConfig) => void;
}) {
  function addRule() {
    const newRule: DistributionRule = {
      id: `rule-${Date.now()}`,
      item_type: "all",
      quality: null,
      method: "round_robin",
    };
    onChange({ rules: [...config.rules, newRule] });
  }

  function updateRule(id: string, updated: DistributionRule) {
    const rules = config.rules.map((r) => (r.id === id ? updated : r));
    onChange({ rules });
  }

  function removeRule(id: string) {
    onChange({ rules: config.rules.filter((r) => r.id !== id) });
  }

  return (
    <div className="space-y-2">
      <p className="text-[10px] text-white/40 mb-4">
        Rules are evaluated top-to-bottom. The first matching rule is applied.
      </p>
      {config.rules.map((rule) => (
        <DistributionRuleRow
          key={rule.id}
          rule={rule}
          onChange={(updated) => updateRule(rule.id, updated)}
          onRemove={() => removeRule(rule.id)}
        />
      ))}
      <button
        onClick={addRule}
        className="flex items-center gap-2 text-xs text-white/40 hover:text-white transition-colors mt-2 px-3 py-2 border border-dashed border-white/10 hover:border-white/30 w-full justify-center"
      >
        <Plus size={12} /> Add Rule
      </button>
    </div>
  );
}

// ── Section: Master Looter ────────────────────────────────────────────────────

function MasterLooterSection({
  masterLooter,
  characters,
  onChange,
}: {
  masterLooter: MasterLooter;
  characters: string[];
  onChange: (m: MasterLooter) => void;
}) {
  return (
    <div className="space-y-4">
      <p className="text-[10px] text-white/40">
        The master looter receives all loot windows and distributes items manually. Leave unset to
        use distribution rules.
      </p>
      <div className="grid grid-cols-2 gap-3">
        <button
          onClick={() => onChange({ character: null })}
          className={`flex items-center gap-3 p-3 border text-left transition-all ${
            masterLooter.character === null
              ? "border-spectral/40 bg-spectral/10"
              : "border-white/10 bg-void/40 hover:bg-white/5"
          }`}
        >
          <ArrowsClockwise
            size={18}
            className={masterLooter.character === null ? "text-spectral" : "text-white/30"}
          />
          <div>
            <div className="text-sm font-medium text-white">Use Distribution Rules</div>
            <div className="text-[10px] text-white/40">No dedicated master looter</div>
          </div>
        </button>
        {characters.map((char) => (
          <button
            key={char}
            onClick={() => onChange({ character: char })}
            className={`flex items-center gap-3 p-3 border text-left transition-all ${
              masterLooter.character === char
                ? "border-orange-400/40 bg-orange-400/10"
                : "border-white/10 bg-void/40 hover:bg-white/5"
            }`}
          >
            <Crown
              size={18}
              className={masterLooter.character === char ? "text-orange-400" : "text-white/30"}
            />
            <div>
              <div className="text-sm font-medium text-white">{char}</div>
              <div className="text-[10px] text-white/40">Master Looter</div>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

// ── Section: Loot History ─────────────────────────────────────────────────────

function LootHistorySection({
  history,
  characters,
}: {
  history: LootHistoryEntry[];
  characters: string[];
}) {
  const [search, setSearch] = useState("");
  const [charFilter, setCharFilter] = useState("");

  const filtered = history.filter((e) => {
    if (search && !e.item_name.toLowerCase().includes(search.toLowerCase())) return false;
    if (charFilter && e.recipient !== charFilter) return false;
    return true;
  });

  return (
    <div className="space-y-4">
      {/* Filters */}
      <div className="flex gap-3">
        <input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search items…"
          className="flex-1 bg-void border border-white/20 text-white text-sm px-3 py-1.5 focus:outline-none focus:border-magentaglow font-rune placeholder:text-white/30"
        />
        <select
          value={charFilter}
          onChange={(e) => setCharFilter(e.target.value)}
          className="bg-void border border-white/20 text-white text-sm px-2 py-1.5 focus:outline-none focus:border-magentaglow"
        >
          <option value="">All characters</option>
          {characters.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </select>
      </div>

      {/* Table */}
      <div className="border border-white/10 overflow-hidden">
        <table className="w-full text-xs">
          <thead>
            <tr className="border-b border-white/10 bg-void/80">
              <th className="px-3 py-2 text-left text-white/50 uppercase tracking-wider font-tech">
                Timestamp
              </th>
              <th className="px-3 py-2 text-left text-white/50 uppercase tracking-wider font-tech">
                Item
              </th>
              <th className="px-3 py-2 text-left text-white/50 uppercase tracking-wider font-tech">
                Recipient
              </th>
              <th className="px-3 py-2 text-left text-white/50 uppercase tracking-wider font-tech">
                Source
              </th>
              <th className="px-3 py-2 text-left text-white/50 uppercase tracking-wider font-tech">
                Zone
              </th>
              <th className="px-3 py-2 text-center text-white/50 uppercase tracking-wider font-tech">
                Qty
              </th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((e, i) => (
              <tr
                key={e.id}
                className={`border-b border-white/5 ${i % 2 === 0 ? "bg-transparent" : "bg-white/[0.02]"} hover:bg-white/5 transition-colors`}
              >
                <td className="px-3 py-2 text-white/40 font-rune">{e.timestamp}</td>
                <td className="px-3 py-2 text-white font-medium">{e.item_name}</td>
                <td className="px-3 py-2 text-spectral">{e.recipient}</td>
                <td className="px-3 py-2 text-white/50">{e.source_mob ?? "—"}</td>
                <td className="px-3 py-2 text-white/50">{e.zone ?? "—"}</td>
                <td className="px-3 py-2 text-center text-white/60">{e.quantity}</td>
              </tr>
            ))}
            {filtered.length === 0 && (
              <tr>
                <td colSpan={6} className="px-3 py-8 text-center text-white/20 italic">
                  No matching entries
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <p className="text-[10px] text-white/30 text-right font-rune">
        Showing {filtered.length} of {history.length} entries
      </p>
    </div>
  );
}

// ── Section: Item Score ──────────────────────────────────────────────────────

const ITEM_SCORE_PRIMARY_STATS = [
  "STR",
  "STA",
  "AGI",
  "DEX",
  "WIS",
  "INT",
  "AC",
  "HP",
  "MANA",
  "DAMAGE",
  "DELAY",
  "WEIGHT",
];

function normalizeItemScoreStatKey(value: string): string {
  return value.trim().toUpperCase().replace(/\s+/g, "_");
}

function orderedItemScoreStats(weights: StatWeights): string[] {
  const seen = new Set<string>();
  const ordered: string[] = [];

  for (const stat of ITEM_SCORE_PRIMARY_STATS) {
    seen.add(stat);
    ordered.push(stat);
  }

  for (const stat of Object.keys(weights).sort((left, right) => left.localeCompare(right))) {
    if (!seen.has(stat)) {
      seen.add(stat);
      ordered.push(stat);
    }
  }

  return ordered;
}

function ItemScoreStatus({
  loading,
  error,
  savedAt,
}: {
  loading: boolean;
  error: string | null;
  savedAt: number | null;
}) {
  const className = error
    ? "border-rose-400/30 bg-rose-500/10 text-rose-200"
    : loading
      ? "border-white/10 bg-white/5 text-white/70"
      : savedAt
        ? "border-emerald-400/30 bg-emerald-500/10 text-emerald-200"
        : "border-cyan-400/20 bg-cyan-400/10 text-cyan-100";

  const text = error
    ? error
    : loading
      ? "Loading item score weights..."
      : savedAt
        ? `Saved ${new Date(savedAt).toLocaleTimeString()}`
        : "Set per-class weights for upgrade scoring and save to persist them.";

  return <div className={`border px-4 py-3 text-sm ${className}`}>{text}</div>;
}

function ItemScoreSection({
  config,
  loading,
  saving,
  error,
  savedAt,
  onChange,
  onRefresh,
}: {
  config: ItemScoreConfig;
  loading: boolean;
  saving: boolean;
  error: string | null;
  savedAt: number | null;
  onChange: (config: ItemScoreConfig) => void;
  onRefresh: () => void;
}) {
  const classes = Object.keys(config.class_weights).sort((left, right) => left.localeCompare(right));
  const [selectedClass, setSelectedClass] = useState<string>(classes[0] ?? "Warrior");
  const [newStat, setNewStat] = useState("");

  useEffect(() => {
    if (classes.length === 0) {
      return;
    }
    if (!classes.includes(selectedClass)) {
      setSelectedClass(classes[0]);
    }
  }, [classes, selectedClass]);

  const activeClass = classes.includes(selectedClass) ? selectedClass : (classes[0] ?? "Warrior");
  const activeWeights = config.class_weights[activeClass] ?? {};
  const stats = orderedItemScoreStats(activeWeights);

  function setMinUpgradeDelta(value: number) {
    onChange({
      ...config,
      min_upgrade_delta: Number.isFinite(value) ? value : 0,
    });
  }

  function setWeight(stat: string, value: number) {
    onChange({
      ...config,
      class_weights: {
        ...config.class_weights,
        [activeClass]: {
          ...activeWeights,
          [stat]: Number.isFinite(value) ? value : 0,
        },
      },
    });
  }

  function removeStat(stat: string) {
    if (ITEM_SCORE_PRIMARY_STATS.includes(stat)) {
      setWeight(stat, 0);
      return;
    }

    const nextWeights = { ...activeWeights };
    delete nextWeights[stat];
    onChange({
      ...config,
      class_weights: {
        ...config.class_weights,
        [activeClass]: nextWeights,
      },
    });
  }

  function addStat() {
    const stat = normalizeItemScoreStatKey(newStat);
    if (!stat || activeWeights[stat] !== undefined) {
      return;
    }
    setWeight(stat, 0);
    setNewStat("");
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h4 className="font-archaic text-xl uppercase tracking-[0.14em] text-white">
            Item Upgrade Scoring
          </h4>
          <p className="mt-2 max-w-3xl text-sm leading-6 text-white/60">
            MQ2ItemScore-style weights control how TextQuest compares a looted item against the
            currently equipped slot for each class.
          </p>
        </div>
        <button
          type="button"
          onClick={onRefresh}
          disabled={loading || saving}
          className="inline-flex items-center gap-2 rounded-full border border-white/15 bg-white/5 px-4 py-2 text-xs font-semibold uppercase tracking-[0.22em] text-white/65 transition-colors hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-50"
        >
          <ArrowsClockwise size={14} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      <ItemScoreStatus loading={loading} error={error} savedAt={savedAt} />

      <div className="grid gap-6 xl:grid-cols-[260px_1fr]">
        <div className="space-y-5 rounded-[1.5rem] border border-white/10 bg-[#120a1d]/78 p-5">
          <div>
            <div className="text-[10px] uppercase tracking-[0.32em] text-white/40">
              Upgrade Threshold
            </div>
            <input
              type="number"
              step="0.05"
              value={config.min_upgrade_delta}
              onChange={(event) => setMinUpgradeDelta(Number(event.target.value))}
              className="mt-3 w-full rounded-2xl border border-cyan-400/20 bg-[#0d0715] px-4 py-3 text-sm text-white focus:border-cyan-300/50 focus:outline-none"
            />
            <p className="mt-2 text-xs leading-5 text-white/45">
              Candidate score delta required before the loot engine marks an item as a keep.
            </p>
          </div>

          <div>
            <div className="text-[10px] uppercase tracking-[0.32em] text-white/40">
              Class Profiles
            </div>
            <div className="mt-3 grid gap-2">
              {classes.map((className) => (
                <button
                  key={className}
                  type="button"
                  onClick={() => setSelectedClass(className)}
                  className={`rounded-2xl border px-4 py-3 text-left transition-colors ${
                    activeClass === className
                      ? "border-cyan-300/40 bg-cyan-400/10 text-cyan-100"
                      : "border-white/10 bg-[#0d0715] text-white/70 hover:border-white/25 hover:text-white"
                  }`}
                >
                  <div className="text-sm font-semibold">{className}</div>
                  <div className="mt-1 text-[10px] uppercase tracking-[0.18em] text-white/40">
                    {Object.keys(config.class_weights[className] ?? {}).length} tracked stats
                  </div>
                </button>
              ))}
            </div>
          </div>
        </div>

        <div className="rounded-[1.5rem] border border-white/10 bg-[#120a1d]/78 p-5">
          <div className="flex flex-wrap items-start justify-between gap-4 border-b border-white/10 pb-5">
            <div>
              <div className="text-[10px] uppercase tracking-[0.32em] text-white/40">
                Active Class
              </div>
              <h5 className="mt-2 font-archaic text-2xl uppercase tracking-[0.16em] text-white">
                {activeClass}
              </h5>
              <p className="mt-2 text-sm leading-6 text-white/55">
                Positive weights make the stat more desirable. Negative weights penalize attributes
                like delay or weight when scoring an item.
              </p>
            </div>

            <div className="flex gap-2">
              <input
                type="text"
                value={newStat}
                onChange={(event) => setNewStat(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    addStat();
                  }
                }}
                placeholder="Add custom stat"
                className="rounded-full border border-white/15 bg-[#0d0715] px-4 py-2 text-xs uppercase tracking-[0.18em] text-white placeholder:text-white/30 focus:border-cyan-300/40 focus:outline-none"
              />
              <button
                type="button"
                onClick={addStat}
                className="rounded-full border border-cyan-300/30 bg-cyan-400/10 px-4 py-2 text-xs font-semibold uppercase tracking-[0.18em] text-cyan-100 transition-colors hover:bg-cyan-400/20"
              >
                Add Stat
              </button>
            </div>
          </div>

          <div className="mt-5 grid gap-3 md:grid-cols-2">
            {stats.map((stat) => {
              const isPrimary = ITEM_SCORE_PRIMARY_STATS.includes(stat);
              return (
                <div
                  key={stat}
                  className="rounded-2xl border border-white/10 bg-[#0d0715] px-4 py-3"
                >
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <div className="text-sm font-semibold text-white">{stat}</div>
                      <div className="mt-1 text-[10px] uppercase tracking-[0.18em] text-white/35">
                        {isPrimary ? "Core scoring stat" : "Custom scoring stat"}
                      </div>
                    </div>
                    <button
                      type="button"
                      onClick={() => removeStat(stat)}
                      className="text-white/25 transition-colors hover:text-rose-300"
                      aria-label={`Remove ${stat}`}
                    >
                      <Trash size={13} />
                    </button>
                  </div>
                  <input
                    type="number"
                    step="0.05"
                    value={activeWeights[stat] ?? 0}
                    onChange={(event) => setWeight(stat, Number(event.target.value))}
                    className="mt-3 w-full rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white focus:border-cyan-300/50 focus:outline-none"
                  />
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Tab navigation ────────────────────────────────────────────────────────────

type Tab = "rules" | "filters" | "distribution" | "master-looter" | "item-score" | "history";

const TABS: { id: Tab; label: string; icon: ElementType }[] = [
  { id: "rules", label: "Loot Rules", icon: ListBullets },
  { id: "filters", label: "Auto-Loot Filters", icon: Scroll },
  { id: "distribution", label: "Distribution Policy", icon: ArrowClockwise },
  { id: "master-looter", label: "Master Looter", icon: Crown },
  { id: "item-score", label: "Item Score", icon: CheckCircle },
  { id: "history", label: "Loot History", icon: Star },
];

// ── Main component ────────────────────────────────────────────────────────────

export default function LootConfig() {
  const [activeTab, setActiveTab] = useState<Tab>("rules");

  const [rules, setRules] = useState<LootRules>(demoLootRules);
  const [filters, setFilters] = useState<CharacterLootFilter[]>(demoCharacterFilters);
  const [distribution, setDistribution] = useState<DistributionConfig>(demoDistributionConfig);
  const [masterLooter, setMasterLooter] = useState<MasterLooter>(demoMasterLooter);
  const [saved, setSaved] = useState(false);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const {
    config: itemScoreConfig,
    loading: itemScoreLoading,
    saving: itemScoreSaving,
    error: itemScoreError,
    savedAt: itemScoreSavedAt,
    loaded: itemScoreLoaded,
    refresh: refreshItemScore,
    save: saveItemScore,
  } = useItemScoreConfig();
  const [draftItemScore, setDraftItemScore] = useState<ItemScoreConfig>(itemScoreConfig);

  // Derive character list from current filters — stays in sync as filters change.
  const characters = Array.from(new Set(filters.map((f) => f.character)));
  const itemScoreDirty =
    JSON.stringify(draftItemScore) !== JSON.stringify(itemScoreConfig);

  useEffect(() => {
    setDraftItemScore(itemScoreConfig);
  }, [itemScoreConfig]);

  useEffect(() => {
    if (!saved) {
      return;
    }

    if (saveTimerRef.current !== null) {
      clearTimeout(saveTimerRef.current);
    }

    saveTimerRef.current = setTimeout(() => {
      saveTimerRef.current = null;
      setSaved(false);
    }, 2000);

    return () => {
      if (saveTimerRef.current !== null) {
        clearTimeout(saveTimerRef.current);
        saveTimerRef.current = null;
      }
    };
  }, [saved]);

  async function handleSave() {
    if (activeTab === "item-score") {
      await saveItemScore(draftItemScore);
      return;
    }

    // The legacy loot tabs are still demo-backed in the dashboard.
    setSaved(true);
  }

  const saveDisabled = activeTab === "item-score"
    ? itemScoreLoading || itemScoreSaving || !itemScoreLoaded
    : false;
  const saveLabel =
    activeTab === "item-score"
      ? itemScoreSaving
        ? "Saving..."
        : itemScoreDirty
          ? "Save Weights"
          : itemScoreSavedAt
            ? "Saved!"
            : "Save Weights"
      : saved
        ? "Saved!"
        : "Save Changes";

  return (
    <section className="flex-1 h-full flex flex-col relative z-20 min-w-[600px]">
      {/* Header */}
      <header className="h-16 border-b border-white/10 flex items-center justify-between px-6 bg-violet/30 backdrop-blur-md">
        <div className="flex items-center gap-4">
          <div className="w-8 h-8 rounded border border-magentadark flex items-center justify-center bg-void">
            <Bag weight="fill" className="text-magentaglow" />
          </div>
          <div>
            <h2 className="font-archaic text-lg text-white leading-tight">
              Loot Configuration
            </h2>
            <p className="text-[10px] uppercase tracking-widest text-white/50 font-rune">
              Rules · Filters · Distribution · Item Score · History
            </p>
          </div>
        </div>
        <button
          onClick={() => {
            void handleSave();
          }}
          disabled={saveDisabled}
          className={`px-4 py-1.5 border text-sm font-medium uppercase tracking-wider transition-all ${
            activeTab === "item-score" && itemScoreSavedAt && !itemScoreDirty
              ? "border-cyan-300/40 bg-cyan-400/15 text-cyan-100 shadow-[0_0_18px_rgba(34,211,238,0.18)]"
              : saved
              ? "border-spectral/60 bg-spectral/20 text-spectral shadow-[0_0_15px_rgba(0,229,255,0.3)]"
              : "bg-magentadark/20 border-magentaglow text-white hover:bg-magentadark/40 shadow-[0_0_15px_rgba(204,68,255,0.3)]"
          } ${saveDisabled ? "cursor-not-allowed opacity-60" : ""}`}
        >
          {saveLabel}
        </button>
      </header>

      {/* Tab bar */}
      <div className="flex border-b border-white/10 bg-void/60 px-6 gap-1 pt-2">
        {TABS.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            onClick={() => setActiveTab(id)}
            className={`flex items-center gap-2 px-4 py-2 text-xs uppercase tracking-wider font-tech border-b-2 transition-all ${
              activeTab === id
                ? "border-magentaglow text-magentaglow bg-magentaglow/5"
                : "border-transparent text-white/40 hover:text-white/70 hover:border-white/20"
            }`}
          >
            <Icon size={14} />
            {label}
          </button>
        ))}
      </div>

      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto p-6 scroll-smooth">
        {activeTab === "rules" && (
          <LootRulesSection rules={rules} onChange={setRules} />
        )}
        {activeTab === "filters" && (
          <AutoLootFiltersSection filters={filters} onChange={setFilters} />
        )}
        {activeTab === "distribution" && (
          <DistributionSection config={distribution} onChange={setDistribution} />
        )}
        {activeTab === "master-looter" && (
          <MasterLooterSection
            masterLooter={masterLooter}
            characters={characters}
            onChange={setMasterLooter}
          />
        )}
        {activeTab === "item-score" && (
          <ItemScoreSection
            config={draftItemScore}
            loading={itemScoreLoading}
            saving={itemScoreSaving}
            error={itemScoreError}
            savedAt={itemScoreSavedAt}
            onChange={setDraftItemScore}
            onRefresh={() => {
              void refreshItemScore();
            }}
          />
        )}
        {activeTab === "history" && (
          <LootHistorySection history={demoLootHistory} characters={characters} />
        )}
      </div>
    </section>
  );
}
