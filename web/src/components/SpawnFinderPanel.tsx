import { useDeferredValue, useState } from "react";

import type {
  DashboardActionRequest,
  DashboardSnapshot,
  SpawnFinderItem,
} from "../dashboard";

type SpawnSortKey = "name" | "level" | "class" | "race" | "distance" | "hpPct";

const SORT_OPTIONS: { label: string; value: SpawnSortKey }[] = [
  { label: "Distance", value: "distance" },
  { label: "Name", value: "name" },
  { label: "Level", value: "level" },
  { label: "Class", value: "class" },
  { label: "Race", value: "race" },
  { label: "HP%", value: "hpPct" },
];

function panelClasses() {
  return "rounded-[1.5rem] border border-cyan-400/20 bg-[#120a1d]/88 p-5 backdrop-blur shadow-[0_12px_40px_rgba(34,211,238,0.08)]";
}

function countChip(label: string, value: string) {
  return (
    <div className="rounded-2xl border border-white/10 bg-white/5 px-3 py-2">
      <div className="font-tech text-[10px] uppercase tracking-[0.25em] text-white/45">
        {label}
      </div>
      <div className="mt-1 font-rune text-lg text-white">{value}</div>
    </div>
  );
}

function matchesSearch(item: SpawnFinderItem, search: string) {
  if (!search) {
    return true;
  }

  const haystacks = [
    item.name,
    item.spawnType,
    item.className,
    item.raceName,
    item.observerName,
    item.observerZone,
    String(item.level),
    String(item.hpPct),
    `${item.hpPct}%`,
    String(item.spawnId),
  ];

  return haystacks.some((value) => value.toLowerCase().includes(search));
}

function compareItems(left: SpawnFinderItem, right: SpawnFinderItem, sortKey: SpawnSortKey) {
  switch (sortKey) {
    case "level":
      return left.level - right.level || left.name.localeCompare(right.name);
    case "class":
      return left.className.localeCompare(right.className) || left.name.localeCompare(right.name);
    case "race":
      return left.raceName.localeCompare(right.raceName) || left.name.localeCompare(right.name);
    case "distance":
      return left.distance - right.distance || left.name.localeCompare(right.name);
    case "hpPct":
      return left.hpPct - right.hpPct || left.name.localeCompare(right.name);
    case "name":
    default:
      return left.name.localeCompare(right.name);
  }
}

export default function SpawnFinderPanel({
  spawnFinder,
  submitAction,
}: {
  spawnFinder: DashboardSnapshot["spawnFinder"];
  submitAction: (action: DashboardActionRequest) => Promise<unknown>;
}) {
  const [search, setSearch] = useState("");
  const [observerFilter, setObserverFilter] = useState<string>("all");
  const [sortKey, setSortKey] = useState<SpawnSortKey>("distance");
  const [sortAscending, setSortAscending] = useState(true);
  const [pendingRow, setPendingRow] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const deferredSearch = useDeferredValue(search);
  const normalizedSearch = deferredSearch.trim().toLowerCase();

  const visibleItems = spawnFinder.items
    .filter((item) => observerFilter === "all" || String(item.observerClientId) === observerFilter)
    .filter((item) => matchesSearch(item, normalizedSearch))
    .sort((left, right) => {
      const order = compareItems(left, right, sortKey);
      return sortAscending ? order : -order;
    });

  async function handleTarget(item: SpawnFinderItem) {
    const rowKey = `${item.observerClientId}:${item.spawnId}`;
    setPendingRow(rowKey);
    setActionError(null);
    try {
      await submitAction({
        type: "target_spawn",
        client_id: item.observerClientId,
        spawn_id: item.spawnId,
      });
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Target request failed");
    } finally {
      setPendingRow(null);
    }
  }

  return (
    <section className={panelClasses()}>
      <div className="mb-4 flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <h2 className="font-archaic text-xl text-white">Spawn Finder</h2>
          <p className="font-tech text-xs uppercase tracking-[0.28em] text-white/45">
            Filterable, sortable zone spawn index with one-click targeting
          </p>
        </div>
        <div className="grid gap-3 sm:grid-cols-3">
          {countChip("Observers", String(spawnFinder.observers.length))}
          {countChip("Spawns", String(spawnFinder.items.length))}
          {countChip("Shown", String(visibleItems.length))}
        </div>
      </div>

      <div className="grid gap-3 lg:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)_minmax(0,0.8fr)_auto]">
        <label className="text-sm text-white/75">
          Search
          <input
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Filter by name, class, race, level, or HP"
            className="mt-2 w-full rounded-2xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-cyan-300/35"
          />
        </label>

        <label className="text-sm text-white/75">
          Observer
          <select
            aria-label="Spawn observer"
            value={observerFilter}
            onChange={(event) => setObserverFilter(event.target.value)}
            className="mt-2 w-full rounded-2xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-cyan-300/35"
          >
            <option value="all" className="bg-[#120a1d]">
              All observers
            </option>
            {spawnFinder.observers.map((observer) => (
              <option
                key={observer.clientId}
                value={String(observer.clientId)}
                className="bg-[#120a1d]"
              >
                {observer.characterName} · {observer.zone}
              </option>
            ))}
          </select>
        </label>

        <label className="text-sm text-white/75">
          Sort
          <select
            aria-label="Spawn sort"
            value={sortKey}
            onChange={(event) => setSortKey(event.target.value as SpawnSortKey)}
            className="mt-2 w-full rounded-2xl border border-white/10 bg-white/5 px-3 py-2 text-sm text-white outline-none focus:border-cyan-300/35"
          >
            {SORT_OPTIONS.map((option) => (
              <option key={option.value} value={option.value} className="bg-[#120a1d]">
                {option.label}
              </option>
            ))}
          </select>
        </label>

        <button
          type="button"
          onClick={() => setSortAscending((current) => !current)}
          className="self-end rounded-full border border-white/10 bg-white/5 px-4 py-2 text-sm text-white/75 transition hover:border-white/25 hover:bg-white/10"
        >
          {sortAscending ? "Ascending" : "Descending"}
        </button>
      </div>

      {actionError && (
        <div className="mt-4 rounded-2xl border border-rose-400/30 bg-rose-400/10 px-4 py-3 text-sm text-rose-100">
          {actionError}
        </div>
      )}

      <div className="mt-4 overflow-hidden rounded-3xl border border-white/10 bg-[#0d0715]">
        <div className="hidden grid-cols-[1.55fr_0.75fr_0.75fr_0.7fr_0.7fr_0.8fr_auto] gap-3 border-b border-white/10 px-4 py-3 font-tech text-[10px] uppercase tracking-[0.26em] text-white/40 md:grid">
          <span>Spawn</span>
          <span>Type</span>
          <span>Observer</span>
          <span>Class</span>
          <span>Race</span>
          <span>Lv / HP / Dist</span>
          <span />
        </div>

        <div className="max-h-[30rem] overflow-auto">
          {visibleItems.length === 0 ? (
            <div className="px-4 py-10 text-center text-sm text-white/50">
              No spawns match the current filters.
            </div>
          ) : (
            visibleItems.map((item) => {
              const rowKey = `${item.observerClientId}:${item.spawnId}`;
              const targeting = pendingRow === rowKey;
              return (
                <div
                  key={rowKey}
                  className="border-b border-white/8 px-4 py-4 last:border-b-0"
                >
                  <div className="grid gap-3 md:grid-cols-[1.55fr_0.75fr_0.75fr_0.7fr_0.7fr_0.8fr_auto] md:items-center">
                    <div>
                      <div className="font-rune text-sm text-white">{item.name}</div>
                      <div className="mt-1 text-[11px] uppercase tracking-[0.24em] text-white/40">
                        {item.observerZone} · Spawn {item.spawnId}
                      </div>
                    </div>

                    <div className="text-sm text-white/75">{item.spawnType}</div>
                    <div className="text-sm text-white/75">{item.observerName}</div>
                    <div className="text-sm text-white/75">{item.className}</div>
                    <div className="text-sm text-white/75">{item.raceName}</div>

                    <div className="flex flex-wrap gap-2 text-xs uppercase tracking-[0.2em] text-white/55">
                      <span className="rounded-full border border-white/10 bg-white/5 px-2 py-1">
                        Lv {item.level}
                      </span>
                      <span className="rounded-full border border-white/10 bg-white/5 px-2 py-1">
                        {item.hpPct}%
                      </span>
                      <span className="rounded-full border border-white/10 bg-white/5 px-2 py-1">
                        {item.distance}u
                      </span>
                    </div>

                    <div className="flex items-center justify-start gap-2 md:justify-end">
                      {item.isCurrentTarget && (
                        <span className="rounded-full border border-fuchsia-400/25 bg-fuchsia-400/10 px-3 py-1 text-[11px] uppercase tracking-[0.24em] text-fuchsia-100">
                          Current Target
                        </span>
                      )}
                      <button
                        type="button"
                        disabled={targeting}
                        onClick={() => void handleTarget(item)}
                        className="rounded-full border border-cyan-300/25 bg-cyan-300/10 px-3 py-2 text-sm text-cyan-100 transition hover:bg-cyan-300/20 disabled:cursor-wait disabled:opacity-60"
                      >
                        {targeting ? "Targeting..." : "Target"}
                      </button>
                    </div>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>
    </section>
  );
}
