import { useEffect, useMemo, useRef, useState } from "react";
import {
  Crown,
  Funnel,
  FloppyDisk,
  MagnifyingGlass,
  Package,
  Scroll,
  UsersThree,
} from "@phosphor-icons/react";
import {
  autoLootFilters as initialAutoLootFilters,
  lootHistory as initialLootHistory,
  lootRules as initialLootRules,
} from "../data/demo";
import type {
  AutoLootFilter,
  LootFilterAction,
  LootHistoryEntry,
  LootPolicy,
  LootRule,
} from "../types";

const policyOptions: { value: LootPolicy; label: string }[] = [
  { value: "need-before-greed", label: "Need before greed" },
  { value: "round-robin", label: "Round robin" },
  { value: "master-looter", label: "Master looter" },
  { value: "greed-only", label: "Greed only" },
];

const filterActions: LootFilterAction[] = ["keep", "bank", "sell", "destroy"];

function Section({
  icon,
  title,
  subtitle,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  subtitle: string;
  children: React.ReactNode;
}) {
  return (
    <section className="bg-violet/20 border border-white/10 p-5">
      <div className="flex items-center gap-3 mb-4">
        <div className="w-8 h-8 border border-magentaglow/40 bg-magentadark/15 text-magentaglow flex items-center justify-center">
          {icon}
        </div>
        <div>
          <h3 className="font-archaic text-base text-white">{title}</h3>
          <p className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
            {subtitle}
          </p>
        </div>
      </div>
      {children}
    </section>
  );
}

export default function LootPanel() {
  const [rules, setRules] = useState<LootRule[]>(initialLootRules);
  const [filters, setFilters] = useState<AutoLootFilter[]>(initialAutoLootFilters);
  const [history] = useState<LootHistoryEntry[]>(initialLootHistory);
  const [search, setSearch] = useState("");
  const [selectedCharacter, setSelectedCharacter] = useState(
    initialAutoLootFilters[0]?.character_name ?? "",
  );
  const [saved, setSaved] = useState<string | null>(null);
  const savedTimeoutRef = useRef<number | null>(null);

  const characterNames = useMemo(
    () => Array.from(new Set(filters.map((filter) => filter.character_name))).sort(),
    [filters],
  );

  useEffect(() => {
    return () => {
      if (savedTimeoutRef.current !== null) {
        window.clearTimeout(savedTimeoutRef.current);
      }
    };
  }, []);

  const effectiveSelectedCharacter =
    selectedCharacter && characterNames.includes(selectedCharacter)
      ? selectedCharacter
      : (characterNames[0] ?? "");

  const visibleFilters = filters.filter(
    (filter) => filter.character_name === effectiveSelectedCharacter,
  );

  const filteredHistory = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return history.filter((entry) => {
      if (!needle) return true;
      return [
        entry.item_name,
        entry.item_type,
        entry.looted_by,
        entry.assigned_to,
        entry.zone,
        entry.source,
        entry.policy,
      ].some((value) => value.toLowerCase().includes(needle));
    });
  }, [history, search]);

  const distributionRows = useMemo(() => {
    const summary = new Map<
      string,
      { count: number; totalValue: number; lastItem: string }
    >();

    for (const entry of filteredHistory) {
      const current = summary.get(entry.assigned_to) ?? {
        count: 0,
        totalValue: 0,
        lastItem: entry.item_name,
      };
      current.count += entry.quantity;
      current.totalValue += entry.estimated_value;
      current.lastItem = entry.item_name;
      summary.set(entry.assigned_to, current);
    }

    return Array.from(summary.entries())
      .map(([name, value]) => ({ name, ...value }))
      .sort((left, right) => right.totalValue - left.totalValue);
  }, [filteredHistory]);

  function flashSaved(message: string) {
    if (savedTimeoutRef.current !== null) {
      window.clearTimeout(savedTimeoutRef.current);
    }
    setSaved(message);
    savedTimeoutRef.current = window.setTimeout(() => {
      setSaved(null);
      savedTimeoutRef.current = null;
    }, 1800);
  }

  function updateRule(id: string, patch: Partial<LootRule>) {
    setRules((current) =>
      current.map((rule) => (rule.id === id ? { ...rule, ...patch } : rule)),
    );
  }

  function updateFilter(id: string, patch: Partial<AutoLootFilter>) {
    setFilters((current) =>
      current.map((filter) =>
        filter.id === id ? { ...filter, ...patch } : filter,
      ),
    );
  }

  function addFilter() {
    const fallbackCharacter =
      effectiveSelectedCharacter || characterNames[0] || "Frostreaver";
    if (!selectedCharacter) {
      setSelectedCharacter(fallbackCharacter);
    }
    setFilters((current) => [
      ...current,
      {
        id: `lf-${Date.now()}`,
        character_name: fallbackCharacter,
        matcher: "New item rule",
        action: "keep",
        notes: "",
      },
    ]);
  }

  function removeFilter(id: string) {
    setFilters((current) => {
      const nextFilters = current.filter((filter) => filter.id !== id);
      const nextCharacterNames = Array.from(
        new Set(nextFilters.map((filter) => filter.character_name)),
      ).sort();
      if (
        effectiveSelectedCharacter &&
        !nextCharacterNames.includes(effectiveSelectedCharacter)
      ) {
        setSelectedCharacter(nextCharacterNames[0] ?? "");
      }
      return nextFilters;
    });
  }

  return (
    <section className="flex-1 h-full flex flex-col relative z-20 min-w-[600px]">
      <header className="h-16 border-b border-white/10 flex items-center justify-between px-6 bg-violet/30 backdrop-blur-md shrink-0">
        <div className="flex items-center gap-4">
          <div className="w-8 h-8 rounded border border-magentadark flex items-center justify-center bg-void">
            <Package weight="fill" className="text-magentaglow" />
          </div>
          <div>
            <h2 className="font-archaic text-lg text-white leading-tight">
              Loot Governance
            </h2>
            <p className="text-[10px] uppercase tracking-widest text-white/50 font-rune">
              Rules · Auto-loot filters · Searchable history
            </p>
          </div>
        </div>
        <button
          onClick={() => flashSaved("Loot policy set saved locally")}
          className={`px-4 py-1.5 border text-sm font-medium uppercase tracking-wider flex items-center gap-2 transition-colors ${
            saved
              ? "border-green-500 text-green-400 bg-green-900/20"
              : "border-magentaglow text-white bg-magentadark/20 hover:bg-magentadark/40"
          }`}
        >
          <FloppyDisk size={14} />
          {saved ?? "Save Loot Rules"}
        </button>
      </header>

      <div className="flex-1 overflow-y-auto p-6 scroll-smooth space-y-6">
        <div className="grid grid-cols-4 gap-4">
          {[
            { label: "Active rules", value: rules.filter((rule) => rule.enabled).length },
            { label: "Characters filtered", value: characterNames.length },
            { label: "History rows", value: filteredHistory.length },
            {
              label: "Tracked value",
              value: `${filteredHistory
                .reduce((sum, entry) => sum + entry.estimated_value, 0)
                .toLocaleString()} pp`,
            },
          ].map((card) => (
            <div
              key={card.label}
              className="bg-void/60 border border-white/10 p-4 flex flex-col gap-1"
            >
              <span className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
                {card.label}
              </span>
              <span className="font-rune text-2xl text-white">{card.value}</span>
            </div>
          ))}
        </div>

        <div className="grid grid-cols-[1.4fr_1fr] gap-6">
          <Section
            icon={<Crown size={15} />}
            title="Distribution Rules"
            subtitle="Need / greed / round-robin policies"
          >
            <div className="space-y-3">
              {rules.map((rule) => (
                <div
                  key={rule.id}
                  className="border border-white/10 bg-void/40 p-4 grid grid-cols-[1.1fr_0.7fr_0.8fr_0.9fr_auto] gap-3 items-center"
                >
                  <div>
                    <label className="text-[10px] uppercase tracking-widest text-white/40 font-rune block mb-1">
                      Item scope
                    </label>
                    <input
                      value={rule.item_scope}
                      onChange={(event) =>
                        updateRule(rule.id, { item_scope: event.target.value })
                      }
                      className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
                    />
                  </div>
                  <div>
                    <label className="text-[10px] uppercase tracking-widest text-white/40 font-rune block mb-1">
                      Quality
                    </label>
                    <input
                      value={rule.quality}
                      onChange={(event) =>
                        updateRule(rule.id, { quality: event.target.value })
                      }
                      className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
                    />
                  </div>
                  <div>
                    <label className="text-[10px] uppercase tracking-widest text-white/40 font-rune block mb-1">
                      Policy
                    </label>
                    <select
                      value={rule.policy}
                      onChange={(event) =>
                        updateRule(rule.id, {
                          policy: event.target.value as LootPolicy,
                        })
                      }
                      className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
                    >
                      {policyOptions.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </select>
                  </div>
                  <div>
                    <label className="text-[10px] uppercase tracking-widest text-white/40 font-rune block mb-1">
                      Master looter
                    </label>
                    <input
                      value={rule.master_looter}
                      onChange={(event) =>
                        updateRule(rule.id, {
                          master_looter: event.target.value,
                        })
                      }
                      className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
                    />
                  </div>
                  <button
                    onClick={() =>
                      updateRule(rule.id, { enabled: !rule.enabled })
                    }
                    className={`mt-5 px-3 py-2 border text-xs uppercase tracking-wider font-rune ${
                      rule.enabled
                        ? "border-magentaglow/50 text-magentaglow bg-magentadark/20"
                        : "border-white/20 text-white/50 bg-void"
                    }`}
                  >
                    {rule.enabled ? "Enabled" : "Disabled"}
                  </button>
                </div>
              ))}
            </div>
          </Section>

          <Section
            icon={<UsersThree size={15} />}
            title="Distribution Report"
            subtitle="Who is receiving tracked loot"
          >
            <div className="space-y-3">
              {distributionRows.map((row) => (
                <div
                  key={row.name}
                  className="border border-white/10 bg-void/40 p-3 flex items-center justify-between gap-3"
                >
                  <div>
                    <div className="text-sm text-white">{row.name}</div>
                    <div className="text-[10px] uppercase tracking-widest text-white/35 font-rune">
                      Last item: {row.lastItem}
                    </div>
                  </div>
                  <div className="text-right">
                    <div className="text-spectral font-rune">
                      {row.totalValue.toLocaleString()} pp
                    </div>
                    <div className="text-[10px] text-white/35 font-rune">
                      {row.count} item units
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </Section>
        </div>

        <Section
          icon={<Funnel size={15} />}
          title="Auto-loot Filters"
          subtitle="Per-character keep / sell / destroy / bank rules"
        >
          <div className="flex items-center justify-between gap-4 mb-4">
            <select
              value={effectiveSelectedCharacter}
              onChange={(event) => setSelectedCharacter(event.target.value)}
              className="bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune min-w-56"
            >
              {characterNames.map((character) => (
                <option key={character} value={character}>
                  {character}
                </option>
              ))}
            </select>
            <button
              onClick={addFilter}
              className="px-3 py-2 border border-dashed border-white/20 text-white/60 hover:border-spectral hover:text-spectral transition-colors text-xs uppercase tracking-wider font-rune"
            >
              Add filter
            </button>
          </div>
          <div className="space-y-3">
            {visibleFilters.map((filter) => (
              <div
                key={filter.id}
                className="grid grid-cols-[1.2fr_0.7fr_1fr_auto] gap-3 border border-white/10 bg-void/40 p-3 items-center"
              >
                <input
                  value={filter.matcher}
                  onChange={(event) =>
                    updateFilter(filter.id, { matcher: event.target.value })
                  }
                  className="bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
                />
                <select
                  value={filter.action}
                  onChange={(event) =>
                    updateFilter(filter.id, {
                      action: event.target.value as LootFilterAction,
                    })
                  }
                  className="bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
                >
                  {filterActions.map((action) => (
                    <option key={action} value={action}>
                      {action}
                    </option>
                  ))}
                </select>
                <input
                  value={filter.notes ?? ""}
                  onChange={(event) =>
                    updateFilter(filter.id, { notes: event.target.value })
                  }
                  placeholder="Notes"
                  className="bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune placeholder:text-white/20"
                />
                <button
                  onClick={() => removeFilter(filter.id)}
                  className="text-xs uppercase tracking-wider font-rune px-3 py-2 border border-white/15 text-white/45 hover:text-red-400 hover:border-red-400/40 transition-colors"
                >
                  Remove
                </button>
              </div>
            ))}
          </div>
        </Section>

        <Section
          icon={<Scroll size={15} />}
          title="Loot History"
          subtitle="Searchable item tracking log"
        >
          <div className="relative mb-4">
            <MagnifyingGlass
              size={15}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-white/30"
            />
            <input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Search by item, looter, recipient, zone, source…"
              className="w-full bg-void border border-white/20 text-white text-sm pl-10 pr-3 py-2 focus:outline-none focus:border-magentaglow font-rune placeholder:text-white/20"
            />
          </div>
          <div className="space-y-2">
            {filteredHistory.map((entry) => (
              <div
                key={entry.id}
                className="grid grid-cols-[1.2fr_0.8fr_0.8fr_0.7fr_0.8fr] gap-3 border border-white/10 bg-void/40 p-3 text-sm"
              >
                <div>
                  <div className="text-white">{entry.item_name}</div>
                  <div className="text-[10px] uppercase tracking-widest text-white/35 font-rune">
                    {entry.item_type} · {entry.quality}
                  </div>
                </div>
                <div className="text-white/70">
                  {entry.assigned_to}
                  <div className="text-[10px] text-white/35 font-rune">
                    looted by {entry.looted_by}
                  </div>
                </div>
                <div className="text-white/70">
                  {entry.zone}
                  <div className="text-[10px] text-white/35 font-rune">
                    {entry.source}
                  </div>
                </div>
                <div className="text-spectral font-rune">
                  {entry.estimated_value.toLocaleString()} pp
                  <div className="text-[10px] text-white/35 font-rune">
                    {entry.policy}
                  </div>
                </div>
                <div className="text-right text-white/55 font-rune">
                  {new Date(entry.timestamp).toLocaleString("en-US", {
                    month: "short",
                    day: "numeric",
                    hour: "2-digit",
                    minute: "2-digit",
                  })}
                </div>
              </div>
            ))}
          </div>
        </Section>
      </div>
    </section>
  );
}
