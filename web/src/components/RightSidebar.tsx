import { User, Funnel, Terminal } from "@phosphor-icons/react";
import { players, combatLog } from "../data/demo";
import type { Player } from "../types";

function PlayerRow({ player }: { player: Player }) {
  const isDead = player.status === "dead";
  return (
    <div
      className={`flex items-center gap-3 p-2 hover:bg-white/5 transition-colors cursor-pointer group ${isDead ? "bg-red-900/10 border border-red-900/30" : ""}`}
    >
      <div
        className={`w-8 h-8 bg-void border ${isDead ? "border-red-500" : "border-white/20"} rounded-sm flex items-center justify-center ${isDead ? "text-red-400" : "text-white/30 group-hover:text-spectral"} transition-colors`}
      >
        <User weight="fill" />
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex justify-between items-baseline mb-0.5">
          <span
            className={`text-sm font-bold truncate ${isDead ? "text-red-400 line-through" : "text-white"}`}
          >
            {player.name}
          </span>
          <span
            className={`text-[10px] font-rune ${isDead ? "text-red-500" : "text-spectral"}`}
          >
            {isDead ? "DEAD" : `HP: ${player.hp_pct}%`}
          </span>
        </div>
        <div
          className={`text-[10px] uppercase truncate flex items-center gap-1 ${isDead ? "text-red-500/50" : "text-white/50"}`}
        >
          {!isDead && (
            <div
              className={`w-1.5 h-1.5 rounded-full ${player.hp_pct > 50 ? "bg-green-500" : "bg-yellow-500"}`}
            />
          )}
          {player.zone}
        </div>
      </div>
    </div>
  );
}

export default function RightSidebar() {
  const [unsafeHacksEnabled, setUnsafeHacksEnabled] = useState(false);

  return (
    <aside className="stone-pillar w-[380px] h-full flex flex-col relative z-20 overflow-hidden bg-void/80 backdrop-blur-sm">
      {/* Soul Tethers */}
      <div className="p-5 border-b border-white/10 bg-violet/40">
        <h3 className="font-archaic text-lg text-white flex justify-between items-end border-b border-white/10 pb-2 mb-4">
          <span className="text-glow-cyan">Soul Tethers</span>
          <span className="text-xs font-tech text-white/50 uppercase tracking-widest font-normal">
            Active: 1,402
          </span>
        </h3>
        <div className="flex gap-2 mb-4">
          <input
            type="text"
            placeholder="Scry for name..."
            className="w-full bg-void border border-white/20 text-white text-sm px-3 py-1.5 focus:outline-none focus:border-magentaglow font-rune placeholder:text-white/30"
          />
          <button className="bg-white/5 border border-white/20 px-2 hover:bg-white/10 transition-colors">
            <Funnel className="text-white/70" />
          </button>
        </div>
        <div className="space-y-2 h-[200px] overflow-y-auto pr-1">
          {players.map((p) => (
            <PlayerRow key={p.name} player={p} />
          ))}
        </div>
      </div>

      {/* Unsafe Interventions (Active Hacks) */}
      <div className="p-4 border-b border-white/10 bg-red-950/20">
        <h4 className="font-archaic text-xs text-red-400 uppercase tracking-widest flex items-center gap-2 mb-2">
          <Warning weight="fill" className="text-red-500" /> Unsafe Interventions
        </h4>
        <div className="flex items-center justify-between">
          <p className="text-[10px] text-white/50 font-rune leading-tight w-2/3">
            Enable risky active hacks (Warp, Living Shield). Warning: High risk of detection.
          </p>
          <button
            onClick={() => setUnsafeHacksEnabled(!unsafeHacksEnabled)}
            className={`w-10 h-5 rounded-full relative transition-colors ${unsafeHacksEnabled ? "bg-red-500" : "bg-white/10"}`}
          >
            <div
              className={`w-4 h-4 bg-white rounded-full absolute top-0.5 transition-transform ${unsafeHacksEnabled ? "translate-x-5" : "translate-x-1"}`}
            />
          </button>
        </div>
      </div>

      {/* Combat Terminal */}
      <div className="flex-1 flex flex-col p-0 relative bg-black/50">
        <div className="absolute top-0 left-0 w-full h-1 bg-gradient-to-r from-transparent via-spectral to-transparent opacity-50" />
        <h4 className="font-rune text-[10px] text-spectral uppercase tracking-widest px-5 py-2 border-b border-white/5 bg-void flex items-center gap-2">
          <Terminal weight="fill" className="text-magentaglow" /> Terminal:
          Combat Matrix Stream
        </h4>
        <div className="flex-1 log-container overflow-hidden p-5 font-rune text-[11px] leading-relaxed flex flex-col justify-end text-white/70 space-y-1">
          {combatLog.map((entry, i) => (
            <p key={i}>
              <span className="text-white/40">[{entry.timestamp}]</span>{" "}
              {entry.highlights.map((h, j) => (
                <span
                  key={j}
                  className={`${h.color} ${h.bold ? "font-bold" : ""}`}
                >
                  {h.text}
                </span>
              ))}
            </p>
          ))}
        </div>
        <div className="p-3 border-t border-white/10 bg-void flex gap-2">
          <span className="text-magentaglow font-rune text-sm">&gt;</span>
          <input
            type="text"
            className="bg-transparent border-none w-full text-white font-rune text-xs focus:outline-none placeholder:text-white/20"
            placeholder="Enter override command..."
          />
        </div>
      </div>
    </aside>
  );
}
