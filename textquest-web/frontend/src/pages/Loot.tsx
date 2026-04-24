import { useState } from "react";
import { History, Package, Plus, Settings2, Skull, Users, X } from "lucide-react";
import { PageHeader } from "../components/PageHeader.tsx";
import { SaveButton } from "../components/SaveButton.tsx";
import { useSave } from "../hooks/useSave.ts";
import { MOCK_LOOT_HISTORY, MOCK_LOOT_RULES } from "../lib/mocks.ts";

type Tab = "rules" | "master" | "distribution" | "history";

const TABS: { id: Tab; label: string; icon: typeof Package }[] = [
  { id: "rules", label: "Rules", icon: Package },
  { id: "master", label: "Master Looter", icon: Users },
  { id: "distribution", label: "Distribution", icon: Settings2 },
  { id: "history", label: "History", icon: History },
];

function ItemList({
  title,
  items,
  color,
  onAdd,
  onRemove,
  placeholder,
}: {
  title: string;
  items: string[];
  color: string;
  onAdd: (v: string) => void;
  onRemove: (v: string) => void;
  placeholder: string;
}) {
  const [input, setInput] = useState("");
  return (
    <section
      className="border rounded-md bg-[#1a0a2e] flex flex-col"
      style={{ borderColor: `${color}44` }}
    >
      <div
        className="flex items-center gap-2 px-3 py-2 border-b text-xs font-mono uppercase tracking-[0.15em]"
        style={{ borderColor: `${color}44`, color }}
      >
        {title}
        <span className="ml-auto text-[#503c6e]">{items.length}</span>
      </div>
      <div className="flex-1 p-2 space-y-1 min-h-[180px] max-h-[320px] overflow-y-auto">
        {items.map((it) => (
          <div
            key={it}
            className="group flex items-center gap-2 px-2 py-1 border border-[#503c6e]/30 bg-[#0d0618] rounded-sm font-mono text-sm text-[#e2d7f4]"
          >
            <span className="flex-1">{it}</span>
            <button
              onClick={() => onRemove(it)}
              className="opacity-0 group-hover:opacity-100 text-[#503c6e] hover:text-[#ef4444]"
            >
              <X className="w-3 h-3" strokeWidth={2} />
            </button>
          </div>
        ))}
        {items.length === 0 && (
          <div className="px-2 py-6 text-center text-[#503c6e] font-mono text-xs italic">empty</div>
        )}
      </div>
      <div className="flex items-center gap-2 p-2 border-t border-[#503c6e]/50">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && input.trim()) {
              onAdd(input.trim());
              setInput("");
            }
          }}
          placeholder={placeholder}
          className="flex-1 bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1 font-mono text-xs text-[#e2d7f4] placeholder-[#503c6e] focus:border-[color:var(--ring)] outline-none"
          style={{ ["--ring" as string]: color } as React.CSSProperties}
        />
        <button
          onClick={() => {
            if (input.trim()) {
              onAdd(input.trim());
              setInput("");
            }
          }}
          className="text-[#503c6e] hover:text-[color:var(--accent)]"
          style={{ ["--accent" as string]: color } as React.CSSProperties}
        >
          <Plus className="w-3.5 h-3.5" strokeWidth={2} />
        </button>
      </div>
    </section>
  );
}

export function Loot() {
  const [tab, setTab] = useState<Tab>("rules");
  const [rules, setRules] = useState(MOCK_LOOT_RULES);
  const save = useSave<typeof MOCK_LOOT_RULES>("PUT", "/loot/rules");

  const addTo = (list: "keep_items" | "sell_items" | "destroy_items", v: string) =>
    setRules({ ...rules, [list]: [...rules[list], v] });
  const removeFrom = (list: "keep_items" | "sell_items" | "destroy_items", v: string) =>
    setRules({ ...rules, [list]: rules[list].filter((x) => x !== v) });

  return (
    <div>
      <PageHeader
        title="Loot"
        subtitle={
          <>
            <Package className="w-3.5 h-3.5 text-[#cc44ff]" strokeWidth={1.75} />
            <span className="text-[#34d399]">{rules.keep_items.length} keep</span>
            <span className="text-[#503c6e]">·</span>
            <span className="text-[#fbbf24]">{rules.sell_items.length} sell</span>
            <span className="text-[#503c6e]">·</span>
            <span className="text-[#ef4444]">{rules.destroy_items.length} destroy</span>
          </>
        }
        meta={
          <span className="text-[10px] font-mono text-[#fbbf24] border border-[#fbbf24]/40 bg-[#fbbf24]/5 rounded-sm px-2 py-1 uppercase tracking-[0.2em]">
            mock data
          </span>
        }
      />

      <div className="px-6 pt-4 flex items-center gap-1 border-b border-[#503c6e]/40">
        {TABS.map(({ id, label, icon: Icon }) => {
          const active = tab === id;
          return (
            <button
              key={id}
              onClick={() => setTab(id)}
              className={`flex items-center gap-2 px-4 py-2 font-mono text-xs uppercase tracking-[0.18em] border-b-2 transition-colors ${
                active
                  ? "border-[#cc44ff] text-[#cc44ff]"
                  : "border-transparent text-[#a096b4] hover:text-[#e2d7f4]"
              }`}
            >
              <Icon className="w-3.5 h-3.5" strokeWidth={1.75} />
              {label}
            </button>
          );
        })}
        <div className="ml-auto flex items-center gap-4 pb-2 font-mono text-xs text-[#a096b4]">
          <label className="flex items-center gap-1.5">
            <input
              type="checkbox"
              checked={rules.loot_all}
              onChange={() => setRules({ ...rules, loot_all: !rules.loot_all })}
              className="accent-[#cc44ff]"
            />
            loot all
          </label>
          <label className="flex items-center gap-1.5">
            <input
              type="checkbox"
              checked={rules.auto_split}
              onChange={() => setRules({ ...rules, auto_split: !rules.auto_split })}
              className="accent-[#cc44ff]"
            />
            auto split
          </label>
          <SaveButton state={save.state} error={save.error} onClick={() => save.save(rules)} />
        </div>
      </div>

      <div className="p-6">
        {tab === "rules" && (
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <ItemList
              title="keep"
              color="#34d399"
              items={rules.keep_items}
              onAdd={(v) => addTo("keep_items", v)}
              onRemove={(v) => removeFrom("keep_items", v)}
              placeholder="add item to keep…"
            />
            <ItemList
              title="sell"
              color="#fbbf24"
              items={rules.sell_items}
              onAdd={(v) => addTo("sell_items", v)}
              onRemove={(v) => removeFrom("sell_items", v)}
              placeholder="add item to sell…"
            />
            <ItemList
              title="destroy"
              color="#ef4444"
              items={rules.destroy_items}
              onAdd={(v) => addTo("destroy_items", v)}
              onRemove={(v) => removeFrom("destroy_items", v)}
              placeholder="add item to destroy…"
            />
          </div>
        )}

        {tab === "master" && (
          <section className="border border-[#503c6e] rounded-md bg-[#1a0a2e] p-6 max-w-xl">
            <div className="flex items-center gap-2 mb-4 text-xs font-mono text-[#a096b4] uppercase tracking-[0.15em]">
              <Users className="w-3.5 h-3.5 text-[#cc44ff]" strokeWidth={1.75} />
              master looter
            </div>
            <label className="flex flex-col gap-1">
              <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-[#a096b4]">
                designated character
              </span>
              <select className="bg-[#0d0618] border border-[#503c6e] rounded-sm px-2 py-1.5 font-mono text-sm text-[#e2d7f4] focus:border-[#cc44ff] outline-none">
                <option value="">— none —</option>
                <option>Thurgrek</option>
                <option>Sylunariel</option>
                <option>Venkhadrei</option>
              </select>
            </label>
          </section>
        )}

        {tab === "distribution" && (
          <section className="border border-[#503c6e] rounded-md bg-[#1a0a2e]">
            <div className="flex items-center gap-2 px-3 py-2 border-b border-[#503c6e] text-xs font-mono text-[#a096b4] uppercase tracking-[0.15em]">
              <Settings2 className="w-3.5 h-3.5 text-[#cc44ff]" strokeWidth={1.75} />
              distribution rules
              <button className="ml-auto flex items-center gap-1 text-[#cc44ff] hover:text-[#ff00ff]">
                <Plus className="w-3 h-3" strokeWidth={2} /> add rule
              </button>
            </div>
            <table className="w-full font-mono text-sm">
              <thead className="text-[#a096b4] uppercase tracking-[0.15em] text-[10px] bg-[#0d0618]">
                <tr>
                  <th className="px-3 py-2 text-left">item type</th>
                  <th className="px-3 py-2 text-left">quality</th>
                  <th className="px-3 py-2 text-left">method</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {[
                  { t: "weapon", q: "legendary", m: "master_looter" },
                  { t: "weapon", q: "rare", m: "need_before_greed" },
                  { t: "armor", q: "any", m: "round_robin" },
                  { t: "spell", q: "any", m: "random" },
                ].map((r, i) => (
                  <tr key={i} className="border-t border-[#503c6e]/40">
                    <td className="px-3 py-2 text-[#e2d7f4]">{r.t}</td>
                    <td className="px-3 py-2 text-[#a096b4]">{r.q}</td>
                    <td className="px-3 py-2 text-[#cc44ff]">{r.m}</td>
                    <td className="px-3 py-2 text-right">
                      <button className="text-[#503c6e] hover:text-[#ef4444]">
                        <X className="w-3 h-3" strokeWidth={2} />
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        )}

        {tab === "history" && (
          <section className="border border-[#503c6e] rounded-md bg-[#1a0a2e] overflow-hidden">
            <div className="flex items-center gap-2 px-3 py-2 border-b border-[#503c6e] text-xs font-mono text-[#a096b4] uppercase tracking-[0.15em]">
              <History className="w-3.5 h-3.5 text-[#cc44ff]" strokeWidth={1.75} />
              recent loot
              <span className="ml-auto text-[#503c6e]">{MOCK_LOOT_HISTORY.length} entries</span>
            </div>
            <table className="w-full font-mono text-sm">
              <thead className="text-[#a096b4] uppercase tracking-[0.15em] text-[10px] bg-[#0d0618]">
                <tr>
                  <th className="px-3 py-2 text-left">time</th>
                  <th className="px-3 py-2 text-left">item</th>
                  <th className="px-3 py-2 text-left">recipient</th>
                  <th className="px-3 py-2 text-left">source</th>
                  <th className="px-3 py-2 text-left">zone</th>
                  <th className="px-3 py-2 text-right">qty</th>
                  <th className="px-3 py-2 text-left">method</th>
                </tr>
              </thead>
              <tbody>
                {MOCK_LOOT_HISTORY.map((h) => (
                  <tr key={h.id} className="border-t border-[#503c6e]/40 hover:bg-[#2d1e41]/40">
                    <td className="px-3 py-2 text-[#503c6e] tabular-nums">{h.timestamp}</td>
                    <td className="px-3 py-2 text-[#cc44ff]">{h.item_name}</td>
                    <td className="px-3 py-2 text-[#e2d7f4]">{h.recipient}</td>
                    <td className="px-3 py-2 text-[#a096b4] flex items-center gap-1.5">
                      <Skull className="w-3 h-3 text-[#503c6e]" strokeWidth={1.75} />
                      {h.source_mob}
                    </td>
                    <td className="px-3 py-2 text-[#a096b4]">{h.zone}</td>
                    <td className="px-3 py-2 text-right tabular-nums text-[#e2d7f4]">
                      {h.quantity}
                    </td>
                    <td className="px-3 py-2 text-[10px] text-[#a096b4] uppercase tracking-[0.15em]">
                      {h.assigned_by ?? "—"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        )}
      </div>
    </div>
  );
}
