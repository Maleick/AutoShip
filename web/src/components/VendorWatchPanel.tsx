import { useEffect, useMemo, useState } from "react";
import {
  BellRinging,
  Coins,
  FloppyDisk,
  Plus,
  Storefront,
  Trash,
  WarningCircle,
  WifiHigh,
  WifiSlash,
} from "@phosphor-icons/react";

import type { VendorWatchConfig, VendorWatchItem } from "../types";
import { useVendorWatch } from "../hooks/useVendorWatch";

type EditableWatchItem = {
  id: string;
  item_name: string;
  max_price_pp: string;
  enabled: boolean;
};

function makeDraftId() {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function copperToPp(copper: number | null): string {
  if (copper === null) {
    return "";
  }
  return (copper / 1000).toFixed(3).replace(/\.?0+$/, "");
}

function ppToCopper(pp: string): number | null {
  const trimmed = pp.trim();
  if (!trimmed) {
    return null;
  }
  const parsed = Number(trimmed);
  if (!Number.isFinite(parsed) || parsed < 0) {
    return null;
  }
  return Math.round(parsed * 1000);
}

function formatMoney(copper: number | null): string {
  if (copper === null) {
    return "Any price";
  }
  return `${(copper / 1000).toLocaleString(undefined, {
    minimumFractionDigits: 0,
    maximumFractionDigits: 3,
  })} pp`;
}

function formatDelta(deltaCopper: number | null): string {
  if (deltaCopper === null) {
    return "No benchmark";
  }
  const sign = deltaCopper > 0 ? "+" : "";
  return `${sign}${(deltaCopper / 1000).toLocaleString(undefined, {
    minimumFractionDigits: 0,
    maximumFractionDigits: 3,
  })} pp`;
}

function formatTimestamp(timestamp: string): string {
  return new Date(timestamp).toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatQuantity(quantity: number): string {
  return quantity === 0 ? "Infinite" : quantity.toString();
}

function toDraftItems(items: VendorWatchItem[]): EditableWatchItem[] {
  return items.map((item, index) => ({
    id: `${item.item_name}-${index}`,
    item_name: item.item_name,
    max_price_pp: copperToPp(item.max_price_copper),
    enabled: item.enabled,
  }));
}

export default function VendorWatchPanel() {
  const {
    alerts,
    config,
    stats,
    loading,
    saving,
    error,
    savedAt,
    connected,
    save,
    clearAlerts,
  } = useVendorWatch();

  const [draftEnabled, setDraftEnabled] = useState(true);
  const [draftItems, setDraftItems] = useState<EditableWatchItem[]>([]);
  const [newItemName, setNewItemName] = useState("");
  const [newMaxPricePp, setNewMaxPricePp] = useState("");

  useEffect(() => {
    setDraftEnabled(config.enabled);
    setDraftItems(toDraftItems(config.items));
  }, [config]);

  const budgetMisses = useMemo(
    () => alerts.filter((alert) => alert.within_budget === false).length,
    [alerts],
  );

  function updateDraftItem(
    id: string,
    patch: Partial<EditableWatchItem>,
  ) {
    setDraftItems((current) =>
      current.map((item) => (item.id === id ? { ...item, ...patch } : item)),
    );
  }

  function removeDraftItem(id: string) {
    setDraftItems((current) => current.filter((item) => item.id !== id));
  }

  function addDraftItem() {
    if (!newItemName.trim()) {
      return;
    }
    setDraftItems((current) => [
      ...current,
      {
        id: makeDraftId(),
        item_name: newItemName.trim(),
        max_price_pp: newMaxPricePp,
        enabled: true,
      },
    ]);
    setNewItemName("");
    setNewMaxPricePp("");
  }

  async function persistDraft() {
    const nextConfig: VendorWatchConfig = {
      enabled: draftEnabled,
      items: draftItems
        .map((item) => ({
          item_name: item.item_name.trim(),
          max_price_copper: ppToCopper(item.max_price_pp),
          enabled: item.enabled,
        }))
        .filter((item) => item.item_name.length > 0),
    };

    await save(nextConfig);
  }

  return (
    <div className="bg-violet/20 border border-white/10 p-5 mb-6">
      <div className="flex items-center justify-between gap-4 mb-5">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 border border-magentaglow/50 flex items-center justify-center bg-magentadark/10 text-magentaglow">
            <Storefront size={16} />
          </div>
          <div>
            <h3 className="font-archaic text-lg text-white leading-tight">
              Vendor Item Watch
            </h3>
            <p className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
              MQ2Vendors parity for merchant browse alerts
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2 text-xs font-rune">
          {connected ? (
            <span className="flex items-center gap-1 text-green-400">
              <WifiHigh size={14} />
              Live feed
            </span>
          ) : (
            <span className="flex items-center gap-1 text-white/40">
              <WifiSlash size={14} />
              Poll only
            </span>
          )}
        </div>
      </div>

      <div className="grid grid-cols-4 gap-4 mb-6">
        <div className="bg-void/60 border border-white/10 p-3">
          <div className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
            Watched Items
          </div>
          <div className="font-rune text-2xl text-white mt-1">
            {stats.watched_items}
          </div>
        </div>
        <div className="bg-void/60 border border-white/10 p-3">
          <div className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
            Alerts Logged
          </div>
          <div className="font-rune text-2xl text-white mt-1">
            {stats.total_alerts}
          </div>
        </div>
        <div className="bg-void/60 border border-white/10 p-3">
          <div className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
            Under Budget
          </div>
          <div className="font-rune text-2xl text-green-400 mt-1">
            {stats.budget_hits}
          </div>
        </div>
        <div className="bg-void/60 border border-white/10 p-3">
          <div className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
            Over Budget
          </div>
          <div className="font-rune text-2xl text-red-400 mt-1">
            {budgetMisses}
          </div>
        </div>
      </div>

      <div className="flex items-center justify-between gap-4 mb-4">
        <label className="flex items-center gap-2 text-xs text-white/70 font-rune uppercase tracking-widest">
          <input
            type="checkbox"
            checked={draftEnabled}
            onChange={(event) => setDraftEnabled(event.target.checked)}
            className="accent-magentaglow"
          />
          Merchant watch enabled
        </label>

        <div className="flex items-center gap-3">
          {savedAt && (
            <span className="text-[10px] text-green-400 font-rune uppercase tracking-widest">
              Saved {new Date(savedAt).toLocaleTimeString()}
            </span>
          )}
          <button
            onClick={() => void persistDraft()}
            disabled={saving}
            className="flex items-center gap-2 border border-magentaglow/40 bg-magentadark/20 px-3 py-2 text-xs text-magentaglow hover:bg-magentadark/30 disabled:opacity-50"
          >
            <FloppyDisk size={14} />
            {saving ? "Saving..." : "Save Watch List"}
          </button>
        </div>
      </div>

      {error && (
        <div className="mb-4 flex items-center gap-2 border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-200">
          <WarningCircle size={14} />
          {error}
        </div>
      )}

      <div className="border border-white/10 bg-void/40 mb-4">
        <div className="grid grid-cols-[minmax(0,1.6fr)_180px_90px_56px] gap-3 px-4 py-2 border-b border-white/10 text-[10px] uppercase tracking-widest text-white/40 font-rune">
          <span>Item</span>
          <span>Max Price (pp)</span>
          <span>Enabled</span>
          <span />
        </div>
        <div className="divide-y divide-white/5">
          {draftItems.map((item) => (
            <div
              key={item.id}
              className="grid grid-cols-[minmax(0,1.6fr)_180px_90px_56px] gap-3 px-4 py-3 items-center"
            >
              <input
                value={item.item_name}
                onChange={(event) =>
                  updateDraftItem(item.id, { item_name: event.target.value })
                }
                placeholder="Item name"
                className="bg-void border border-white/15 px-3 py-2 text-sm text-white focus:outline-none focus:border-magentaglow"
              />
              <input
                type="number"
                min="0"
                step="0.001"
                value={item.max_price_pp}
                onChange={(event) =>
                  updateDraftItem(item.id, { max_price_pp: event.target.value })
                }
                placeholder="Any"
                className="bg-void border border-white/15 px-3 py-2 text-sm text-white focus:outline-none focus:border-magentaglow"
              />
              <label className="flex items-center gap-2 text-xs text-white/60 font-rune">
                <input
                  type="checkbox"
                  checked={item.enabled}
                  onChange={(event) =>
                    updateDraftItem(item.id, { enabled: event.target.checked })
                  }
                  className="accent-magentaglow"
                />
                Live
              </label>
              <button
                onClick={() => removeDraftItem(item.id)}
                className="w-9 h-9 border border-white/10 text-white/50 hover:text-red-300 hover:border-red-400/40 flex items-center justify-center"
                aria-label={`Delete ${item.item_name}`}
              >
                <Trash size={14} />
              </button>
            </div>
          ))}
          {!draftItems.length && !loading && (
            <div className="px-4 py-6 text-sm text-white/45">
              No watched vendor items configured yet.
            </div>
          )}
        </div>
      </div>

      <div className="grid grid-cols-[minmax(0,1.6fr)_180px_120px] gap-3 mb-6">
        <input
          value={newItemName}
          onChange={(event) => setNewItemName(event.target.value)}
          placeholder="Add item name"
          className="bg-void border border-white/15 px-3 py-2 text-sm text-white focus:outline-none focus:border-magentaglow"
        />
        <input
          type="number"
          min="0"
          step="0.001"
          value={newMaxPricePp}
          onChange={(event) => setNewMaxPricePp(event.target.value)}
          placeholder="Max price in pp"
          className="bg-void border border-white/15 px-3 py-2 text-sm text-white focus:outline-none focus:border-magentaglow"
        />
        <button
          onClick={addDraftItem}
          className="flex items-center justify-center gap-2 border border-spectral/30 bg-spectral/10 px-3 py-2 text-xs text-spectral hover:bg-spectral/15"
        >
          <Plus size={14} />
          Add Watch
        </button>
      </div>

      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <BellRinging size={15} className="text-yellow-300" />
          <span className="text-xs text-white/70 font-rune uppercase tracking-widest">
            Recent Vendor Alerts
          </span>
        </div>
        <button
          onClick={() => void clearAlerts()}
          className="text-[10px] uppercase tracking-widest text-white/45 hover:text-white"
        >
          Clear history
        </button>
      </div>

      <div className="space-y-3">
        {alerts.map((alert) => (
          <div
            key={alert.id}
            className="border border-white/10 bg-void/50 px-4 py-3"
          >
            <div className="flex items-start justify-between gap-4 mb-2">
              <div>
                <div className="flex items-center gap-2">
                  <span className="font-archaic text-white text-base">
                    {alert.item_name}
                  </span>
                  {alert.within_budget === true ? (
                    <span className="text-[10px] uppercase tracking-widest text-green-300 border border-green-400/30 bg-green-400/10 px-2 py-0.5">
                      Under cap
                    </span>
                  ) : alert.within_budget === false ? (
                    <span className="text-[10px] uppercase tracking-widest text-red-300 border border-red-400/30 bg-red-400/10 px-2 py-0.5">
                      Over cap
                    </span>
                  ) : (
                    <span className="text-[10px] uppercase tracking-widest text-white/45 border border-white/10 px-2 py-0.5">
                      No cap
                    </span>
                  )}
                </div>
                <div className="text-xs text-white/55 font-rune uppercase tracking-widest mt-1">
                  {alert.vendor_name} • qty {formatQuantity(alert.quantity)}
                </div>
              </div>
              <div className="text-[10px] text-white/40 font-rune uppercase tracking-widest">
                {formatTimestamp(alert.timestamp)}
              </div>
            </div>

            <div className="grid grid-cols-3 gap-3 text-sm">
              <div className="border border-white/10 bg-white/5 px-3 py-2">
                <div className="text-[10px] uppercase tracking-widest text-white/40 font-rune mb-1">
                  Observed
                </div>
                <div className="flex items-center gap-2 text-white">
                  <Coins size={14} className="text-yellow-300" />
                  {formatMoney(alert.actual_price_copper)}
                </div>
              </div>
              <div className="border border-white/10 bg-white/5 px-3 py-2">
                <div className="text-[10px] uppercase tracking-widest text-white/40 font-rune mb-1">
                  Expected Max
                </div>
                <div className="text-white">
                  {formatMoney(alert.expected_max_price_copper)}
                </div>
              </div>
              <div className="border border-white/10 bg-white/5 px-3 py-2">
                <div className="text-[10px] uppercase tracking-widest text-white/40 font-rune mb-1">
                  Delta
                </div>
                <div
                  className={
                    alert.price_delta_copper !== null && alert.price_delta_copper > 0
                      ? "text-red-300"
                      : "text-green-300"
                  }
                >
                  {formatDelta(alert.price_delta_copper)}
                </div>
              </div>
            </div>
          </div>
        ))}

        {!alerts.length && !loading && (
          <div className="border border-dashed border-white/10 px-4 py-6 text-sm text-white/45">
            No vendor alerts recorded yet.
          </div>
        )}
      </div>
    </div>
  );
}
