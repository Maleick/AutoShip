import {
  Bag,
  BellRinging,
  Brain,
  Broadcast,
  ChatCircle,
  ChatTeardropText,
  Coins,
  Copy,
  Cpu,
  Crosshair,
  Eye,
  Globe,
  Heartbeat,
  Key,
  ShieldWarning,
  Skull,
  Sword,
  UsersThree,
  Wrench,
} from "@phosphor-icons/react";

export type ActiveView =
  | "default"
  | "engagements"
  | "economy"
  | "formations"
  | "groups"
  | "map"
  | "security"
  | "alerts"
  | "loot"
  | "soul"
  | "boxchat"
  | "extensions"
  | "spawns"
  | "player_watch"
  | "say"
  | "chat_pattern_rules"
  | "xassist"
  | "credentials"
  | "config_copy"
  | "sessions"
  | "admin";

const navItems: {
  icon: typeof Sword;
  label: string;
  id: ActiveView;
  pulse?: boolean;
}[] = [
  { icon: Sword, label: "Active Engagements", id: "engagements", pulse: true },
  { icon: Coins, label: "Economy Ledger", id: "economy" },
  { icon: UsersThree, label: "Fleet Formations", id: "formations" },
  { icon: UsersThree, label: "Groups & Camp Config", id: "groups" },
  { icon: Globe, label: "Realm Map (Norrath)", id: "map" },
  { icon: ShieldWarning, label: "Security Wards", id: "security" },
  { icon: BellRinging, label: "Alert Routing", id: "alerts", pulse: true },
  { icon: Skull, label: "Rare Spawn Alerts", id: "spawns", pulse: true },
  { icon: Eye, label: "Player Watch", id: "player_watch", pulse: true },
  { icon: ChatTeardropText, label: "Say Detection", id: "say", pulse: true },
  { icon: Crosshair, label: "X-Assist", id: "xassist", pulse: true },
  { icon: Bag, label: "Loot Configuration", id: "loot" },
  { icon: Brain, label: "Soul Engine", id: "soul" },
  { icon: Broadcast, label: "Network Box Chat", id: "boxchat" },
  { icon: Cpu, label: "Extension Catalog", id: "extensions", pulse: true },
  { icon: ChatCircle, label: "Chat Pattern Rules", id: "chat_pattern_rules" },
  { icon: Key, label: "Credentials Management", id: "credentials" },
  { icon: Copy, label: "Config Copy", id: "config_copy" },
  { icon: Heartbeat, label: "Session Monitor", id: "sessions", pulse: true },
  { icon: Wrench, label: "Admin Tools", id: "admin" },
];

interface LeftSidebarProps {
  activeView: ActiveView;
  onNavigate: (view: ActiveView) => void;
}

export default function LeftSidebar({
  activeView,
  onNavigate,
}: LeftSidebarProps) {
  return (
    <aside className="stone-pillar relative z-20 flex h-full w-[320px] flex-col px-1 pb-2 pt-6">
      {/* Eye icon crown */}
      <div className="absolute -top-4 left-1/2 flex h-8 w-32 -translate-x-1/2 items-end justify-center border-b border-magentadark pb-1">
        <Eye weight="fill" className="pulsing-flame text-xl text-spectral" />
      </div>

      {/* Title */}
      <div className="mb-8 mt-2 px-5 text-center">
        <h1 className="font-archaic text-glow-magenta text-3xl font-bold uppercase tracking-widest text-[#E2D7F4]">
          Neriak
        </h1>
        <p className="mt-1 border-b border-white/10 pb-4 font-tech text-sm tracking-[0.3em] text-spectral">
          Third Gate Command
        </p>
        <p className="mt-4 font-archaic text-xs uppercase tracking-widest text-white/30 italic">
          "What happens in Neriak, stays in Neriak."
        </p>
      </div>

      {/* Leyline Network Stats */}
      <div className="mb-8 flex flex-col gap-5 px-5">
        <h2 className="flex items-center gap-2 font-archaic text-xs uppercase tracking-widest text-white/50">
          Leyline Network
        </h2>
        {/* Soul Harvest bar */}
        <div>
          <div className="mb-1 flex justify-between text-xs">
            <span className="text-white/80">Soul Harvest Density</span>
            <span className="font-rune text-spectral">87%</span>
          </div>
          <div className="relative h-1 w-full overflow-hidden border border-white/10 bg-void">
            <div className="absolute left-0 top-0 h-full w-[87%] bg-gradient-to-r from-violet to-spectral shadow-[0_0_8px_#00E5FF]" />
          </div>
        </div>
        {/* Arcane Matrix bar */}
        <div>
          <div className="mb-1 flex justify-between text-xs">
            <span className="text-white/80">Arcane Matrix Reserves</span>
            <span className="font-rune text-magentadark">4.2 TB</span>
          </div>
          <div className="relative h-1 w-full overflow-hidden border border-white/10 bg-void">
            <div className="absolute left-0 top-0 h-full w-[45%] bg-gradient-to-r from-violet to-magentaglow shadow-[0_0_8px_#FF00FF]" />
          </div>
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex flex-1 flex-col gap-2 overflow-y-auto px-3 pr-2">
        {navItems.map((item) => {
          const isActive = item.id === activeView;
          return (
            <button
              key={item.label}
              onClick={() => onNavigate(item.id)}
              className={`group flex w-full items-center gap-3 border border-transparent px-4 py-3 text-left transition-all ${
                isActive
                  ? "border-magentadark/30 bg-white/5 hover:border-magentadark hover:bg-violet/50"
                  : "bg-transparent hover:border-spectral hover:bg-violet/50"
              }`}
            >
              <item.icon
                size={20}
                className={`text-white/40 transition-colors ${
                  isActive
                    ? "text-magentaglow group-hover:text-magentaglow"
                    : "group-hover:text-spectral"
                }`}
              />
              <span
                className={`font-medium tracking-wide transition-colors ${
                  isActive
                    ? "text-white"
                    : "text-white/70 group-hover:text-white"
                }`}
              >
                {item.label}
              </span>
              {item.pulse && (
                <div className="ml-auto h-1.5 w-1.5 rounded-full bg-magentaglow pulsing-flame" />
              )}
            </button>
          );
        })}
      </nav>

      {/* System status footer */}
      <div className="mt-auto border-t border-white/5 px-5 pt-6">
        <div className="flex items-center justify-between font-rune text-xs text-white/40">
          <span>SYS.SYNC: ACTIVE</span>
          <span className="text-spectral">O.K.</span>
        </div>
      </div>
    </aside>
  );
}
