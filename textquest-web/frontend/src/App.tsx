import { useEffect, useMemo, useState } from "react";
import { CommandPalette, type PaletteAction } from "./components/CommandPalette.tsx";
import { useBackendStatus } from "./hooks/useBackendStatus.ts";
import {
  Activity,
  Coins,
  Command,
  Eye,
  EyeOff,
  KeyRound,
  Package,
  Server,
  Users,
  UsersRound,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { Dashboard } from "./pages/Dashboard.tsx";
import { Sessions } from "./pages/Sessions.tsx";
import { Groups } from "./pages/Groups.tsx";
import { Credentials } from "./pages/Credentials.tsx";
import { Characters } from "./pages/Characters.tsx";
import { Loot } from "./pages/Loot.tsx";
import { Economy } from "./pages/Economy.tsx";

type Tab = "dashboard" | "sessions" | "groups" | "loot" | "economy" | "characters" | "credentials";

interface NavItem {
  id: Tab;
  label: string;
  icon: LucideIcon;
}

const NAV: NavItem[] = [
  { id: "dashboard", label: "Dashboard", icon: Activity },
  { id: "sessions", label: "Sessions", icon: Server },
  { id: "groups", label: "Groups", icon: UsersRound },
  { id: "loot", label: "Loot", icon: Package },
  { id: "economy", label: "Economy", icon: Coins },
  { id: "characters", label: "Characters", icon: Users },
  { id: "credentials", label: "Credentials", icon: KeyRound },
];

const KBD_HINTS: { key: string; label: string }[] = [
  { key: "Q", label: "quit" },
  { key: "R", label: "refresh" },
  { key: "/", label: "search" },
  { key: "G", label: "groups" },
  { key: "C", label: "clients" },
  { key: ":", label: "command" },
];

const VERSION = "v0.7.0-alpha";

export default function App() {
  const [tab, setTab] = useState<Tab>("sessions");
  const [privacy, setPrivacy] = useState(false);
  const [huntMode, setHuntMode] = useState(true);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const backend = useBackendStatus();
  const connected = backend === "online";
  const serverName = "Bertoxxulous";

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((o) => !o);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const paletteActions: PaletteAction[] = useMemo(() => {
    const navActions: PaletteAction[] = NAV.map((n) => ({
      id: `nav-${n.id}`,
      label: `Go to ${n.label}`,
      section: "navigate",
      run: () => setTab(n.id),
    }));
    const toggleActions: PaletteAction[] = [
      {
        id: "toggle-hunt",
        label: huntMode ? "Exit Hunt mode" : "Enter Hunt mode",
        section: "mode",
        run: () => setHuntMode((h) => !h),
      },
      {
        id: "toggle-privacy",
        label: privacy ? "Show KPI values" : "Hide KPI values (privacy)",
        section: "mode",
        run: () => setPrivacy((p) => !p),
      },
    ];
    return [...navActions, ...toggleActions];
  }, [huntMode, privacy]);

  const activeLabel = NAV.find((n) => n.id === tab)?.label ?? "";

  return (
    <div className="min-h-screen bg-[#0d0618] text-[#e2d7f4] flex flex-col">
      {/* ─── Top bar ──────────────────────────────────────────── */}
      <header className="flex items-center gap-6 px-5 py-3 border-b border-[#503c6e] bg-[#0d0618]">
        <div className="flex items-baseline gap-3">
          <span className="font-[Cinzel,serif] text-xl tracking-[0.2em] text-[#cc44ff]">
            TEXTQUEST
          </span>
          <span className="font-mono text-[10px] text-[#503c6e]">{VERSION}</span>
        </div>
        <nav className="flex items-center gap-1 font-mono text-[11px] text-[#a096b4] uppercase tracking-[0.18em]">
          <span>operator</span>
          <span className="text-[#503c6e]">/</span>
          <span className="text-[#e2d7f4]">{activeLabel}</span>
        </nav>
        <div className="ml-auto flex items-center gap-2">
          <span
            className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-sm border font-mono text-[10px] uppercase tracking-[0.15em] ${
              connected
                ? "border-[#34d399]/40 text-[#34d399] bg-[#34d399]/5"
                : "border-[#ef4444]/40 text-[#ef4444] bg-[#ef4444]/5"
            }`}
          >
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                connected ? "bg-[#34d399] shadow-[0_0_6px_#34d399]" : "bg-[#ef4444]"
              }`}
            />
            {connected ? "connected" : "offline"}
            <span className="text-[#a096b4] normal-case tracking-normal ml-1">{serverName}</span>
          </span>
          <button
            onClick={() => setHuntMode((h) => !h)}
            className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-sm border font-mono text-[10px] uppercase tracking-[0.18em] ${
              huntMode
                ? "border-[#cc44ff] text-[#cc44ff] bg-[#cc44ff]/10 shadow-[0_0_12px_-4px_#cc44ff]"
                : "border-[#503c6e] text-[#a096b4]"
            }`}
          >
            <Zap className="w-3 h-3" strokeWidth={2} />
            hunt
          </button>
          <button
            onClick={() => setPaletteOpen(true)}
            className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-sm border border-[#503c6e] hover:border-[#cc44ff] hover:text-[#cc44ff] text-[#a096b4] font-mono text-[10px] uppercase tracking-[0.15em]"
          >
            <Command className="w-3 h-3" strokeWidth={2} />
            ⌘K palette
          </button>
          <button
            onClick={() => setPrivacy((p) => !p)}
            className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-sm border font-mono text-[10px] uppercase tracking-[0.15em] ${
              privacy
                ? "border-[#fbbf24] text-[#fbbf24] bg-[#fbbf24]/5"
                : "border-[#503c6e] text-[#a096b4]"
            }`}
          >
            {privacy ? (
              <EyeOff className="w-3 h-3" strokeWidth={2} />
            ) : (
              <Eye className="w-3 h-3" strokeWidth={2} />
            )}
            {privacy ? "hidden" : "visible"}
          </button>
        </div>
      </header>

      {!connected && (
        <div className="flex items-center gap-2 px-5 py-1.5 bg-[#fbbf24]/5 border-b border-[#fbbf24]/30 font-mono text-[10px] text-[#fbbf24] uppercase tracking-[0.2em]">
          <span className="w-1.5 h-1.5 rounded-full bg-[#fbbf24] animate-pulse" />
          backend offline · showing mock data
          <span className="ml-auto text-[#503c6e] normal-case tracking-normal">
            retrying every 15s
          </span>
        </div>
      )}

      {/* ─── Body (sidebar + main) ────────────────────────────── */}
      <div className="flex flex-1 min-h-0">
        <aside className="w-52 border-r border-[#503c6e] bg-[#0d0618] flex flex-col">
          <nav className="flex-1 p-2 space-y-0.5">
            {NAV.map(({ id, label, icon: Icon }) => {
              const active = tab === id;
              return (
                <button
                  key={id}
                  onClick={() => setTab(id)}
                  className={`w-full flex items-center gap-2 px-3 py-2 rounded-sm font-mono text-xs uppercase tracking-[0.18em] transition-colors ${
                    active
                      ? "bg-[#2d1e41] text-[#cc44ff] border-l-2 border-[#cc44ff]"
                      : "text-[#a096b4] hover:bg-[#1a0a2e] hover:text-[#e2d7f4] border-l-2 border-transparent"
                  }`}
                >
                  <Icon className="w-4 h-4" strokeWidth={1.75} />
                  {label}
                </button>
              );
            })}
          </nav>
        </aside>

        <main className={`flex-1 overflow-y-auto ${privacy ? "blur-sm pointer-events-none" : ""}`}>
          {tab === "dashboard" && <Dashboard />}
          {tab === "sessions" && <Sessions />}
          {tab === "groups" && <Groups />}
          {tab === "loot" && <Loot />}
          {tab === "economy" && <Economy />}
          {tab === "characters" && <Characters />}
          {tab === "credentials" && <Credentials />}
        </main>
      </div>

      {/* ─── Footer kbd hints ─────────────────────────────────── */}
      <footer className="flex items-center gap-4 px-5 py-2 border-t border-[#503c6e] bg-[#0d0618] font-mono text-[11px] text-[#a096b4]">
        <div className="flex items-center gap-3">
          {KBD_HINTS.map(({ key, label }) => (
            <span key={key} className="flex items-center gap-1.5">
              <kbd className="inline-block min-w-[1.25rem] text-center px-1 py-0.5 border border-[#503c6e] rounded-sm text-[#cc44ff] bg-[#1a0a2e] text-[10px]">
                {key}
              </kbd>
              <span className="text-[10px]">{label}</span>
            </span>
          ))}
        </div>
        <div className="ml-auto flex items-center gap-3 text-[10px] uppercase tracking-[0.15em]">
          <span>theme</span>
          <span className="text-[#ff00ff]">neriak</span>
        </div>
      </footer>

      <CommandPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        actions={paletteActions}
      />
    </div>
  );
}
