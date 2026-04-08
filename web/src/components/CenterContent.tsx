import {
  Target,
  Faders,
  ChartBar,
  BellRinging,
  WarningCircle,
  Info,
  Plus,
  Skull,
} from "@phosphor-icons/react";
import { assaults, dpsRankings, alerts } from "../data/demo";
import type { Assault, DpsEntry, Alert } from "../types";

function TriquetraSvg() {
  return (
    <svg className="triquetra" viewBox="0 0 100 100">
      <path d="M50 20 C60 0, 80 0, 90 20 C100 40, 90 60, 70 70 C50 80, 50 80, 50 100 C50 80, 50 80, 30 70 C10 60, 0 40, 10 20 C20 0, 40 0, 50 20 Z" />
    </svg>
  );
}

function AssaultCard({ assault }: { assault: Assault }) {
  const isCyan = assault.variant === "cyan";
  return (
    <article
      className={`arcane-tablet ${isCyan ? "arcane-tablet-cyan" : ""} p-5 flex flex-col gap-4 ${isCyan ? "floating-delayed" : "floating"} relative overflow-hidden group`}
    >
      {!isCyan && (
        <Skull
          weight="fill"
          className="absolute -right-4 -bottom-4 text-[120px] text-white/[0.02] group-hover:text-magentaglow/[0.05] transition-colors pointer-events-none rotate-12"
        />
      )}
      <div className="flex justify-between items-start">
        <div>
          <span
            className={`${isCyan ? "bg-spectral/10 text-spectral border-spectral/30" : "bg-magentadark/20 text-magentaglow border-magentaglow/30"} text-[10px] px-2 py-0.5 rounded-sm uppercase tracking-widest border`}
          >
            {assault.zone}
          </span>
          <h4
            className={`font-archaic text-2xl mt-1 text-white ${isCyan ? "group-hover:text-glow-cyan" : "group-hover:text-glow-magenta"} transition-all`}
          >
            {assault.target_name}
          </h4>
        </div>
        <div className="text-right">
          <div className="text-xs text-white/50 uppercase tracking-wider mb-1">
            Engagement Time
          </div>
          <div className="font-rune text-spectral text-lg">
            {assault.engagement_time}
          </div>
        </div>
      </div>
      <div className="mt-2">
        <div className="flex justify-between text-xs mb-1 font-tech">
          <span className="text-red-400">Target Vitality</span>
          <span className="text-red-400 font-bold">
            {assault.target_hp_pct}%
          </span>
        </div>
        <div className="h-2 bg-void border border-white/10 w-full relative">
          <div
            className="absolute top-0 left-0 h-full bg-gradient-to-r from-red-900 to-red-500 shadow-[0_0_10px_rgba(239,68,68,0.5)]"
            style={{ width: `${assault.target_hp_pct}%` }}
          />
        </div>
      </div>
      <div className="grid grid-cols-3 gap-2 mt-2 pt-4 border-t border-white/10">
        <div className="text-center">
          <div className="text-[10px] text-white/40 uppercase">Forces</div>
          <div className="text-lg font-rune mt-0.5 text-white">
            {assault.forces_active}/{assault.forces_total}
          </div>
        </div>
        <div className="text-center border-l border-r border-white/10 relative">
          <div className="text-[10px] text-white/40 uppercase">Avg Mana</div>
          <div className="text-lg font-rune mt-0.5 text-spectral">
            {assault.avg_mana_pct}%
          </div>
          {assault.avg_mana_pct < 50 && (
            <div className="absolute -top-1 -right-1 w-2 h-2 bg-yellow-500 rounded-full animate-ping" />
          )}
        </div>
        <div className="text-center">
          <div className="text-[10px] text-white/40 uppercase">Casualties</div>
          <div
            className={`text-lg font-rune mt-0.5 ${assault.casualties > 0 ? "text-magentaglow" : "text-white/60"}`}
          >
            {assault.casualties}
          </div>
        </div>
      </div>
    </article>
  );
}

function DpsMeter({ rankings }: { rankings: DpsEntry[] }) {
  const maxDps = rankings[0]?.dps ?? 1;
  return (
    <div className="bg-violet/30 border border-white/5 p-5 relative overflow-hidden">
      <div className="absolute top-0 left-0 w-1 h-full bg-magentadark" />
      <h3 className="font-archaic text-lg text-white mb-4 flex items-center gap-2">
        <ChartBar weight="fill" className="text-magentaglow" /> Damage Metre
        (Global)
      </h3>
      <div className="space-y-3">
        {rankings.map((entry) => (
          <div
            key={entry.name}
            className="relative w-full h-7 bg-void border border-white/5 flex items-center px-3 z-10 overflow-hidden"
          >
            <div
              className="absolute top-0 left-0 h-full bg-magentadark/30 z-0"
              style={{
                width: `${(entry.dps / maxDps) * 100}%`,
                opacity: entry.dps / maxDps,
              }}
            />
            <div className="relative z-10 flex w-full justify-between items-center text-sm">
              <span className="font-medium text-white flex items-center gap-2">
                <div
                  className="w-2 h-2 rotate-45"
                  style={{ backgroundColor: entry.color }}
                />
                {entry.name}
              </span>
              <span className="font-rune text-magentaglow">
                {entry.dps.toLocaleString()} DPS
              </span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function Divinations({ alerts: alertList }: { alerts: Alert[] }) {
  return (
    <div className="bg-violet/30 border border-white/5 p-5 border-t-2 border-t-spectral">
      <h3 className="font-archaic text-lg text-white mb-4 flex items-center gap-2">
        <BellRinging weight="fill" className="text-spectral" /> Divinations
      </h3>
      <ul className="space-y-4 text-sm">
        {alertList.map((alert, i) => (
          <li
            key={i}
            className={`flex gap-3 items-start ${i < alertList.length - 1 ? "border-b border-white/5 pb-3" : ""}`}
          >
            {alert.type === "warning" ? (
              <WarningCircle weight="fill" className="text-yellow-500 mt-0.5" />
            ) : (
              <Info weight="fill" className="text-spectral mt-0.5" />
            )}
            <div>
              <p className="text-white/80">
                {alert.message}{" "}
                {alert.highlight && (
                  <span className="text-white font-medium">
                    {alert.highlight}
                  </span>
                )}
              </p>
              <span className="text-[10px] text-white/40 font-rune mt-1 block">
                {alert.detail}
              </span>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}

export default function CenterContent() {
  return (
    <section className="flex-1 h-full flex flex-col relative z-20 min-w-[600px]">
      {/* Header */}
      <header className="h-16 border-b border-white/10 flex items-center justify-between px-6 bg-violet/30 backdrop-blur-md">
        <div className="flex items-center gap-4">
          <div className="w-8 h-8 rounded border border-magentadark flex items-center justify-center bg-void">
            <Target weight="fill" className="text-magentaglow" />
          </div>
          <div>
            <h2 className="font-archaic text-lg text-white leading-tight">
              Tactical Scrying Pool
            </h2>
            <p className="text-[10px] uppercase tracking-widest text-white/50 font-rune">
              Live Combat Telemetry
            </p>
          </div>
        </div>
        <div className="flex gap-4">
          <button className="px-4 py-1.5 border border-spectral/30 text-spectral text-sm font-medium hover:bg-spectral/10 transition-colors uppercase tracking-wider flex items-center gap-2">
            <Faders /> Calibrate
          </button>
          <button className="px-4 py-1.5 bg-magentadark/20 border border-magentaglow text-white text-sm font-medium hover:bg-magentadark/40 transition-colors uppercase tracking-wider shadow-[0_0_15px_rgba(204,68,255,0.3)]">
            Issue Decree
          </button>
        </div>
      </header>

      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto p-6 scroll-smooth">
        {/* Assaults header */}
        <div className="flex items-center gap-3 mb-6">
          <TriquetraSvg />
          <h3 className="font-archaic text-xl tracking-wider text-glow-magenta">
            Assaults in Progress
          </h3>
          <div className="h-[1px] flex-1 bg-gradient-to-r from-magentadark/50 to-transparent" />
        </div>

        {/* Assault cards grid */}
        <div className="grid grid-cols-2 gap-6">
          {assaults.map((a) => (
            <AssaultCard key={a.id} assault={a} />
          ))}
          {/* New engagement card */}
          <article className="arcane-tablet bg-void/50 border-white/5 p-5 flex flex-col justify-center items-center gap-2 group cursor-pointer border-dashed">
            <div className="w-12 h-12 rounded-full border border-white/20 flex items-center justify-center mb-2 group-hover:border-magentaglow transition-colors duration-500">
              <Plus className="text-2xl text-white/40 group-hover:text-magentaglow transition-colors duration-500" />
            </div>
            <h4 className="font-archaic text-lg text-white/60 group-hover:text-white transition-colors">
              Manifest New Legion
            </h4>
            <p className="text-xs text-white/30 uppercase tracking-widest font-rune">
              Open command portal
            </p>
          </article>
        </div>

        {/* Bottom row: DPS + Alerts */}
        <div className="mt-8 grid grid-cols-[2fr_1fr] gap-6">
          <DpsMeter rankings={dpsRankings} />
          <Divinations alerts={alerts} />
        </div>
      </div>
    </section>
  );
}
