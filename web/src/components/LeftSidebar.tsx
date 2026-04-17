import {
  Eye,
  Sword,
  UsersThree,
  Globe,
  ChatTeardropText,
  ShieldWarning,
  BellRinging,
  Bag,
  Brain,
  Broadcast,
  Skull,
  ChatCircle,
} from "@phosphor-icons/react";

export type ActiveView =
  | "engagements"
  | "formations"
  | "map"
  | "security"
  | "alerts"
  | "loot"
  | "soul"
  | "boxchat"
  | "spawns"
  | "say"
  | "chat_pattern_rules";

const navItems: {
  icon: typeof Sword;
  label: string;
  id: ActiveView;
  pulse?: boolean;
}[] = [
  { icon: Sword, label: "Active Engagements", id: "engagements", pulse: true },
  { icon: UsersThree, label: "Fleet Formations", id: "formations" },
  { icon: Globe, label: "Realm Map (Norrath)", id: "map" },
  { icon: ShieldWarning, label: "Security Wards", id: "security" },
  { icon: BellRinging, label: "Alert Routing", id: "alerts", pulse: true },
  { icon: Skull, label: "Rare Spawn Alerts", id: "spawns", pulse: true },
  { icon: ChatTeardropText, label: "Say Detection", id: "say", pulse: true },
  { icon: Bag, label: "Loot Configuration", id: "loot" },
  { icon: Brain, label: "Soul Engine", id: "soul" },
  { icon: Broadcast, label: "Network Box Chat", id: "boxchat" },
  { icon: ChatCircle, label: "Chat Pattern Rules", id: "chat_pattern_rules" },
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
    <aside className="stone-pillar w-[320px] h-full flex flex-col pt-6 pb-2 px-1 relative z-20">
      {/* Eye icon crown */}
      <div className="absolute -top-4 left-1/2 -translate-x-1/2 w-32 h-8 border-b border-magentadark flex justify-center items-end pb-1">
        <Eye weight="fill" className="text-spectral text-xl pulsing-flame" />
      </div>

      {/* Title */}
      <div className="px-5 mb-8 mt-2 text-center">
        <h1 className="font-archaic font-bold text-3xl tracking-widest text-[#E2D7F4] uppercase text-glow-magenta">
          Neriak
        </h1>
        <p className="font-tech text-spectral text-sm tracking-[0.3em] mt-1 border-b border-white/10 pb-4">
          Third Gate Command
        </p>
        <p className="mt-4 italic text-xs text-white/30 tracking-widest font-archaic uppercase">
          "What happens in Neriak, stays in Neriak."
        </p>
      </div>

      {/* Leyline Network Stats */}
      <div className="px-5 mb-8 flex flex-col gap-5">
        <h2 className="font-archaic text-xs text-white/50 uppercase tracking-widest flex items-center gap-2">
          Leyline Network
        </h2>
        {/* Soul Harvest bar */}
        <div>
          <div className="flex justify-between text-xs mb-1">
            <span className="text-white/80">Soul Harvest Density</span>
            <span className="text-spectral font-rune">87%</span>
          </div>
          <div className="h-1 bg-void border border-white/10 w-full relative overflow-hidden">
            <div className="absolute top-0 left-0 h-full w-[87%] bg-gradient-to-r from-violet to-spectral shadow-[0_0_8px_#00E5FF]" />
          </div>
        </div>
        {/* Arcane Matrix bar */}
        <div>
          <div className="flex justify-between text-xs mb-1">
            <span className="text-white/80">Arcane Matrix Reserves</span>
            <span className="text-magentadark font-rune">4.2 TB</span>
          </div>
          <div className="h-1 bg-void border border-white/10 w-full relative overflow-hidden">
            <div className="absolute top-0 left-0 h-full w-[45%] bg-gradient-to-r from-violet to-magentaglow shadow-[0_0_8px_#FF00FF]" />
          </div>
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex-1 px-3 overflow-y-auto pr-2 flex flex-col gap-2">
        {navItems.map((item) => {
          const isActive = item.id === activeView;
          return (
            <button
              key={item.label}
              onClick={() => onNavigate(item.id)}
              className={`group flex items-center gap-3 px-4 py-3 border border-transparent transition-all w-full text-left ${
                isActive
                  ? "bg-white/5 border-magentadark/30 hover:border-magentadark hover:bg-violet/50"
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
                className={`font-medium tracking-wide transition-colors ${isActive ? "text-white" : "text-white/70 group-hover:text-white"}`}
              >
                {item.label}
              </span>
              {item.pulse && (
                <div className="ml-auto w-1.5 h-1.5 bg-magentaglow rounded-full pulsing-flame" />
              )}
            </button>
          );
        })}
      </nav>

      {/* System status footer */}
      <div className="px-5 mt-auto pt-6 border-t border-white/5">
        <div className="flex items-center justify-between text-xs text-white/40 font-rune">
          <span>SYS.SYNC: ACTIVE</span>
          <span className="text-spectral">O.K.</span>
        </div>
      </div>
    </aside>
  );
}
