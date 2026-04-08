import { useState, type ReactNode } from "react";
import {
  Coins,
  Storefront,
  Bank,
  Knife,
  TrendUp,
  ToggleRight,
  ToggleLeft,
  PencilSimple,
  Plus,
  Trash,
  FloppyDisk,
} from "@phosphor-icons/react";
import {
  kronoSettings,
  vendorRoutes,
  bankingRules,
  tradeskillSupplies,
  wealthHistory,
} from "../data/demo";
import type {
  KronoSettings,
  VendorRoute,
  BankingRule,
  TradeskillSupply,
} from "../types";

// ── Helpers ──────────────────────────────────────────────────────────────────

function SectionHeader({
  icon,
  title,
  subtitle,
}: {
  icon: ReactNode;
  title: string;
  subtitle?: string;
}) {
  return (
    <div className="flex items-center gap-3 mb-5">
      <div className="w-8 h-8 border border-magentaglow/50 flex items-center justify-center bg-magentadark/10 text-magentaglow">
        {icon}
      </div>
      <div>
        <h3 className="font-archaic text-lg text-white leading-tight">{title}</h3>
        {subtitle && (
          <p className="text-[10px] uppercase tracking-widest text-white/40 font-rune">
            {subtitle}
          </p>
        )}
      </div>
    </div>
  );
}

function FieldRow({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="flex items-center justify-between py-2 border-b border-white/5">
      <span className="text-xs text-white/60 font-rune uppercase tracking-wide">
        {label}
      </span>
      <div className="flex items-center gap-2">{children}</div>
    </div>
  );
}

function NumberInput({
  value,
  onChange,
  suffix,
}: {
  value: number;
  onChange: (v: number) => void;
  suffix?: string;
}) {
  return (
    <div className="flex items-center gap-1">
      <input
        type="number"
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-24 bg-void border border-white/20 text-white text-sm px-2 py-1 focus:outline-none focus:border-magentaglow font-rune text-right"
      />
      {suffix && (
        <span className="text-[10px] text-white/40 font-rune">{suffix}</span>
      )}
    </div>
  );
}

function ToggleSwitch({
  enabled,
  onChange,
}: {
  enabled: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <button
      onClick={() => onChange(!enabled)}
      className={`flex items-center gap-1 text-xs px-2 py-1 border transition-colors ${
        enabled
          ? "border-magentaglow/50 text-magentaglow bg-magentadark/20 hover:bg-magentadark/30"
          : "border-white/20 text-white/40 bg-void hover:border-white/40"
      }`}
    >
      {enabled ? (
        <ToggleRight weight="fill" size={14} />
      ) : (
        <ToggleLeft size={14} />
      )}
      {enabled ? "ON" : "OFF"}
    </button>
  );
}

// ── Wealth Dashboard ─────────────────────────────────────────────────────────

function WealthDashboard() {
  const { current, snapshots } = wealthHistory;
  const oldest = snapshots[0];
  const platDelta = current.plat - oldest.plat;
  const kronoDelta = current.krono - oldest.krono;

  const maxPlat = Math.max(...snapshots.map((s) => s.plat));
  const barHeight = 40;

  return (
    <div className="bg-violet/20 border border-white/10 p-5 mb-6">
      <SectionHeader
        icon={<TrendUp size={16} />}
        title="Wealth Ledger"
        subtitle="7-day historical tracking"
      />

      {/* KPI row */}
      <div className="grid grid-cols-3 gap-4 mb-6">
        {[
          {
            label: "Platinum",
            value: current.plat.toLocaleString(),
            delta: `+${platDelta.toLocaleString()}`,
            color: "text-yellow-300",
          },
          {
            label: "Krono",
            value: current.krono.toLocaleString(),
            delta: `+${kronoDelta}`,
            color: "text-spectral",
          },
          {
            label: "Item Value Est.",
            value: `${(current.item_value_estimate / 1000).toFixed(0)}k pp`,
            delta: "+102k",
            color: "text-magentaglow",
          },
        ].map((kpi) => (
          <div
            key={kpi.label}
            className="bg-void/60 border border-white/10 p-3 flex flex-col gap-1"
          >
            <span className="text-[10px] text-white/40 uppercase tracking-widest font-rune">
              {kpi.label}
            </span>
            <span className={`font-rune text-2xl ${kpi.color}`}>
              {kpi.value}
            </span>
            <span className="text-[10px] text-green-400 font-rune">
              {kpi.delta} this week
            </span>
          </div>
        ))}
      </div>

      {/* Sparkline */}
      <div>
        <p className="text-[10px] text-white/40 uppercase tracking-widest font-rune mb-2">
          Plat trend (7d)
        </p>
        <div className="flex items-end gap-1 h-10">
          {snapshots.map((s, i) => {
            const h = Math.round((s.plat / maxPlat) * barHeight);
            const isLast = i === snapshots.length - 1;
            return (
              <div
                key={i}
                className="flex-1 relative group"
                style={{ height: `${barHeight}px` }}
              >
                <div
                  className={`absolute bottom-0 left-0 right-0 ${
                    isLast
                      ? "bg-magentaglow shadow-[0_0_8px_rgba(204,68,255,0.6)]"
                      : "bg-magentadark/50"
                  } transition-all`}
                  style={{ height: `${h}px` }}
                />
                {/* Tooltip */}
                <div className="absolute bottom-full mb-1 left-1/2 -translate-x-1/2 bg-void border border-white/20 px-2 py-1 text-[9px] font-rune text-white whitespace-nowrap opacity-0 group-hover:opacity-100 pointer-events-none z-10">
                  {s.plat.toLocaleString()} pp
                  <br />
                  {s.timestamp.slice(0, 10)}
                </div>
              </div>
            );
          })}
        </div>
        <div className="flex justify-between text-[9px] text-white/30 font-rune mt-1">
          <span>{snapshots[0].timestamp.slice(5, 10)}</span>
          <span>{snapshots[snapshots.length - 1].timestamp.slice(5, 10)}</span>
        </div>
      </div>
    </div>
  );
}

// ── Krono Farm Panel ─────────────────────────────────────────────────────────

function KronoFarmPanel() {
  const [settings, setSettings] = useState<KronoSettings>({ ...kronoSettings });
  const [saved, setSaved] = useState(false);

  function update<K extends keyof KronoSettings>(key: K, val: KronoSettings[K]) {
    setSettings((prev) => ({ ...prev, [key]: val }));
    setSaved(false);
  }

  function save() {
    // TODO: PUT /api/economy/krono-settings
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  }

  return (
    <div className="bg-violet/20 border border-white/10 p-5 mb-6">
      <SectionHeader
        icon={<Coins size={16} />}
        title="Krono Farm Parameters"
        subtitle="Automated Krono acquisition targets"
      />

      <FieldRow label="Enabled">
        <ToggleSwitch
          enabled={settings.enabled}
          onChange={(v) => update("enabled", v)}
        />
      </FieldRow>
      <FieldRow label="Target rate">
        <NumberInput
          value={settings.target_rate_per_day}
          onChange={(v) => update("target_rate_per_day", v)}
          suffix="/ day"
        />
      </FieldRow>
      <FieldRow label="Min sell price">
        <NumberInput
          value={settings.min_sell_price}
          onChange={(v) => update("min_sell_price", v)}
          suffix="pp"
        />
      </FieldRow>
      <FieldRow label="Max buy price">
        <NumberInput
          value={settings.max_buy_price}
          onChange={(v) => update("max_buy_price", v)}
          suffix="pp"
        />
      </FieldRow>
      <FieldRow label="Restock threshold">
        <NumberInput
          value={settings.restock_threshold}
          onChange={(v) => update("restock_threshold", v)}
          suffix="Krono"
        />
      </FieldRow>

      <div className="mt-4 flex justify-end">
        <button
          onClick={save}
          className={`flex items-center gap-2 px-4 py-1.5 border text-sm font-medium uppercase tracking-wider transition-colors ${
            saved
              ? "border-green-500 text-green-400 bg-green-900/20"
              : "border-magentaglow text-white bg-magentadark/20 hover:bg-magentadark/40"
          }`}
        >
          <FloppyDisk size={14} />
          {saved ? "Saved!" : "Save Settings"}
        </button>
      </div>
    </div>
  );
}

// ── Vendor Route Editor ───────────────────────────────────────────────────────

function VendorRouteEditor() {
  const [routes, setRoutes] = useState<VendorRoute[]>([...vendorRoutes]);
  const [editId, setEditId] = useState<string | null>(null);
  const [editBuf, setEditBuf] = useState<Partial<VendorRoute>>({});

  function startEdit(route: VendorRoute) {
    setEditId(route.id);
    setEditBuf({ ...route });
  }

  function commitEdit() {
    setRoutes((prev) =>
      prev.map((r) => (r.id === editId ? ({ ...r, ...editBuf } as VendorRoute) : r))
    );
    setEditId(null);
    setEditBuf({});
    // TODO: PUT /api/economy/vendor-routes/:id
  }

  function deleteRoute(id: string) {
    setRoutes((prev) => prev.filter((r) => r.id !== id));
    // TODO: DELETE /api/economy/vendor-routes/:id
  }

  function toggleRoute(id: string) {
    setRoutes((prev) =>
      prev.map((r) => (r.id === id ? { ...r, enabled: !r.enabled } : r))
    );
  }

  function addRoute() {
    const newRoute: VendorRoute = {
      id: `vr-${Date.now()}`,
      zone: "New Zone",
      npc_name: "New Vendor",
      path_notes: "",
      item_categories: [],
      enabled: false,
    };
    setRoutes((prev) => [...prev, newRoute]);
    startEdit(newRoute);
    // TODO: POST /api/economy/vendor-routes
  }

  return (
    <div className="bg-violet/20 border border-white/10 p-5 mb-6">
      <SectionHeader
        icon={<Storefront size={16} />}
        title="Vendor Route Editor"
        subtitle="Zone → NPC path mapping"
      />

      <div className="space-y-3">
        {routes.map((route) =>
          editId === route.id ? (
            <div
              key={route.id}
              className="bg-void/60 border border-magentaglow/40 p-3 space-y-2"
            >
              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label className="text-[10px] text-white/40 uppercase font-rune block mb-1">
                    Zone
                  </label>
                  <input
                    className="w-full bg-void border border-white/20 text-white text-xs px-2 py-1 focus:outline-none focus:border-magentaglow font-rune"
                    value={editBuf.zone ?? ""}
                    onChange={(e) =>
                      setEditBuf((b) => ({ ...b, zone: e.target.value }))
                    }
                  />
                </div>
                <div>
                  <label className="text-[10px] text-white/40 uppercase font-rune block mb-1">
                    NPC Name
                  </label>
                  <input
                    className="w-full bg-void border border-white/20 text-white text-xs px-2 py-1 focus:outline-none focus:border-magentaglow font-rune"
                    value={editBuf.npc_name ?? ""}
                    onChange={(e) =>
                      setEditBuf((b) => ({ ...b, npc_name: e.target.value }))
                    }
                  />
                </div>
              </div>
              <div>
                <label className="text-[10px] text-white/40 uppercase font-rune block mb-1">
                  Path Notes
                </label>
                <input
                  className="w-full bg-void border border-white/20 text-white text-xs px-2 py-1 focus:outline-none focus:border-magentaglow font-rune"
                  value={editBuf.path_notes ?? ""}
                  onChange={(e) =>
                    setEditBuf((b) => ({ ...b, path_notes: e.target.value }))
                  }
                />
              </div>
              <div>
                <label className="text-[10px] text-white/40 uppercase font-rune block mb-1">
                  Item Categories (comma-separated)
                </label>
                <input
                  className="w-full bg-void border border-white/20 text-white text-xs px-2 py-1 focus:outline-none focus:border-magentaglow font-rune"
                  value={(editBuf.item_categories ?? []).join(", ")}
                  onChange={(e) =>
                    setEditBuf((b) => ({
                      ...b,
                      item_categories: e.target.value
                        .split(",")
                        .map((s) => s.trim())
                        .filter(Boolean),
                    }))
                  }
                />
              </div>
              <div className="flex justify-end gap-2 pt-1">
                <button
                  onClick={() => {
                    setEditId(null);
                    setEditBuf({});
                  }}
                  className="text-xs px-3 py-1 border border-white/20 text-white/60 hover:text-white transition-colors"
                >
                  Cancel
                </button>
                <button
                  onClick={commitEdit}
                  className="flex items-center gap-1 text-xs px-3 py-1 border border-magentaglow text-magentaglow bg-magentadark/20 hover:bg-magentadark/40 transition-colors"
                >
                  <FloppyDisk size={12} />
                  Save Route
                </button>
              </div>
            </div>
          ) : (
            <div
              key={route.id}
              className={`flex items-start gap-3 p-3 border transition-colors ${
                route.enabled
                  ? "border-white/10 bg-void/40"
                  : "border-white/5 bg-void/20 opacity-60"
              }`}
            >
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-xs font-bold text-white">
                    {route.npc_name}
                  </span>
                  <span className="text-[10px] px-1.5 py-0.5 bg-spectral/10 text-spectral border border-spectral/30">
                    {route.zone}
                  </span>
                </div>
                {route.path_notes && (
                  <p className="text-[10px] text-white/40 font-rune mb-1">
                    {route.path_notes}
                  </p>
                )}
                <div className="flex flex-wrap gap-1">
                  {route.item_categories.map((cat) => (
                    <span
                      key={cat}
                      className="text-[9px] px-1.5 py-0.5 bg-magentadark/10 text-magentaglow/60 border border-magentaglow/20"
                    >
                      {cat}
                    </span>
                  ))}
                </div>
              </div>
              <div className="flex items-center gap-2 shrink-0">
                <ToggleSwitch
                  enabled={route.enabled}
                  onChange={() => toggleRoute(route.id)}
                />
                <button
                  onClick={() => startEdit(route)}
                  className="text-white/40 hover:text-spectral transition-colors"
                  title="Edit"
                >
                  <PencilSimple size={14} />
                </button>
                <button
                  onClick={() => deleteRoute(route.id)}
                  className="text-white/40 hover:text-red-400 transition-colors"
                  title="Delete"
                >
                  <Trash size={14} />
                </button>
              </div>
            </div>
          )
        )}
      </div>

      <button
        onClick={addRoute}
        className="mt-4 w-full flex items-center justify-center gap-2 py-2 border border-dashed border-white/20 text-white/40 hover:border-spectral hover:text-spectral transition-colors text-xs uppercase tracking-wider font-rune"
      >
        <Plus size={14} />
        Add Vendor Route
      </button>
    </div>
  );
}

// ── Banking Rules ─────────────────────────────────────────────────────────────

function BankingRulesPanel() {
  const [rules, setRules] = useState<BankingRule[]>([...bankingRules]);

  function updateRule<K extends keyof BankingRule>(
    id: string,
    key: K,
    val: BankingRule[K]
  ) {
    setRules((prev) =>
      prev.map((r) => (r.id === id ? { ...r, [key]: val } : r))
    );
    // TODO: PUT /api/economy/banking-rules/:id
  }

  return (
    <div className="bg-violet/20 border border-white/10 p-5 mb-6">
      <SectionHeader
        icon={<Bank size={16} />}
        title="Banking Rules"
        subtitle="Deposit thresholds per item category"
      />

      <div className="space-y-4">
        {rules.map((rule) => (
          <div
            key={rule.id}
            className="bg-void/40 border border-white/5 p-4"
          >
            <div className="flex items-center justify-between mb-3">
              <span className="font-archaic text-sm text-white">
                {rule.item_category}
              </span>
              <ToggleSwitch
                enabled={rule.auto_deposit}
                onChange={(v) => updateRule(rule.id, "auto_deposit", v)}
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="text-[10px] text-white/40 uppercase font-rune block mb-1">
                  Deposit when &gt;
                </label>
                <NumberInput
                  value={rule.deposit_threshold}
                  onChange={(v) =>
                    updateRule(rule.id, "deposit_threshold", v)
                  }
                />
              </div>
              <div>
                <label className="text-[10px] text-white/40 uppercase font-rune block mb-1">
                  Keep on hand
                </label>
                <NumberInput
                  value={rule.keep_on_hand}
                  onChange={(v) => updateRule(rule.id, "keep_on_hand", v)}
                />
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── Tradeskill Supply Chain ───────────────────────────────────────────────────

function TradeskillSupplyPanel() {
  const [supplies, setSupplies] = useState<TradeskillSupply[]>([
    ...tradeskillSupplies,
  ]);

  function toggleSupply(id: string) {
    setSupplies((prev) =>
      prev.map((s) => (s.id === id ? { ...s, enabled: !s.enabled } : s))
    );
    // TODO: PUT /api/economy/tradeskill/:id
  }

  function updateQuantity(id: string, qty: number) {
    setSupplies((prev) =>
      prev.map((s) => (s.id === id ? { ...s, restock_quantity: qty } : s))
    );
  }

  return (
    <div className="bg-violet/20 border border-white/10 p-5 mb-6">
      <SectionHeader
        icon={<Knife size={16} />}
        title="Tradeskill Supply Chain"
        subtitle="Material restock automation"
      />

      <div className="space-y-3">
        {supplies.map((supply) => (
          <div
            key={supply.id}
            className={`border p-4 transition-opacity ${
              supply.enabled
                ? "border-white/10 bg-void/40"
                : "border-white/5 bg-void/20 opacity-60"
            }`}
          >
            <div className="flex items-center justify-between mb-3">
              <div>
                <span className="font-archaic text-sm text-white">
                  {supply.skill}
                </span>
                <span className="ml-2 text-[10px] text-white/40 font-rune">
                  via {supply.source_zone}
                </span>
              </div>
              <ToggleSwitch
                enabled={supply.enabled}
                onChange={() => toggleSupply(supply.id)}
              />
            </div>
            <div className="flex flex-wrap gap-1 mb-3">
              {supply.materials.map((mat) => (
                <span
                  key={mat}
                  className="text-[9px] px-1.5 py-0.5 bg-spectral/10 text-spectral border border-spectral/20"
                >
                  {mat}
                </span>
              ))}
            </div>
            <FieldRow label="Restock qty">
              <NumberInput
                value={supply.restock_quantity}
                onChange={(v) => updateQuantity(supply.id, v)}
                suffix="units"
              />
            </FieldRow>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── Root Economy Panel ────────────────────────────────────────────────────────

export default function EconomyPanel() {
  return (
    <section className="flex-1 h-full flex flex-col relative z-20 min-w-[600px]">
      {/* Header */}
      <header className="h-16 border-b border-white/10 flex items-center justify-between px-6 bg-violet/30 backdrop-blur-md shrink-0">
        <div className="flex items-center gap-4">
          <div className="w-8 h-8 rounded border border-yellow-600/50 flex items-center justify-center bg-void">
            <Coins weight="fill" className="text-yellow-300" />
          </div>
          <div>
            <h2 className="font-archaic text-lg text-white leading-tight">
              Economy Ledger
            </h2>
            <p className="text-[10px] uppercase tracking-widest text-white/50 font-rune">
              M10 — Krono · Vendor · Banking · Tradeskill
            </p>
          </div>
        </div>
      </header>

      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto p-6 scroll-smooth">
        <WealthDashboard />
        <KronoFarmPanel />
        <VendorRouteEditor />
        <BankingRulesPanel />
        <TradeskillSupplyPanel />
      </div>
    </section>
  );
}
