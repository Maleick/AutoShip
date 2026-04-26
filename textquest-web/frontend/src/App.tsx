import { useEffect, useMemo, useState } from "react";
import { CommandPalette, type PaletteAction } from "./components/CommandPalette.tsx";
import { ThemeToggle } from "./components/ThemeToggle.tsx";
import { useTheme } from "./hooks/useTheme.ts";
import { useBackendStatus } from "./hooks/useBackendStatus.ts";
import {
  Activity,
  Coins,
  Command,
  Eye,
  EyeOff,
  KeyRound,
  Package,
  Palette,
  Play,
  Server,
  Sparkles,
  Users,
  UsersRound,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { Dashboard } from "./pages/Dashboard.tsx";
import { Sessions } from "./pages/Sessions.tsx";
import { Replay } from "./pages/Replay.tsx";
import { Groups } from "./pages/Groups.tsx";
import { Credentials } from "./pages/Credentials.tsx";
import { Characters } from "./pages/Characters.tsx";
import { Loot } from "./pages/Loot.tsx";
import { Economy } from "./pages/Economy.tsx";
import { Improvement } from "./pages/Improvement.tsx";
import { Settings } from "./pages/Settings.tsx";

type Tab =
  | "dashboard"
  | "sessions"
  | "groups"
  | "loot"
  | "economy"
  | "characters"
  | "credentials"
  | "improvement"
  | "settings";

interface NavItem {
  id: Tab;
  label: string;
  icon: LucideIcon;
}

const NAV: NavItem[] = [
  { id: "dashboard", label: "Dashboard", icon: Activity },
  { id: "sessions", label: "Sessions", icon: Server },
  { id: "replay", label: "Replay", icon: Play },
  { id: "groups", label: "Groups", icon: UsersRound },
  { id: "improvement", label: "Improvement", icon: Zap },
  { id: "loot", label: "Loot", icon: Package },
  { id: "economy", label: "Economy", icon: Coins },
  { id: "characters", label: "Characters", icon: Users },
  { id: "credentials", label: "Credentials", icon: KeyRound },
  { id: "settings", label: "Settings", icon: Palette },
];

const KBD_HINTS: { key: string; label: string }[] = [
  { key: "Q", label: "quit" },
  { key: "R", label: "refresh" },
  { key: "/", label: "search" },
  { key: "G", label: "groups" },
  { key: "C", label: "clients" },
  { key: ":", label: "command" },
];

// Injected from textquest/Cargo.toml at build time via vite.config.ts (#2511).
const VERSION = __APP_VERSION__;

export default function App() {
  const [tab, setTab] = useState<Tab>("sessions");
  const [privacy, setPrivacy] = useState(false);
  const [huntMode, setHuntMode] = useState(true);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const { mode, contrast } = useTheme();
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
      {
        id: "toggle-theme-mode",
        label: mode === "dark" ? "Switch to Light mode" : "Switch to Dark mode",
        section: "appearance",
        run: () => toggleMode(),
      },
      {
        id: "toggle-high-contrast",
        label: contrast === "high-contrast" ? "Disable High-Contrast" : "Enable High-Contrast",
        section: "appearance",
        run: () => toggleContrast(),
      },
    ];
    return [...navActions, ...toggleActions];
  }, [huntMode, privacy, mode, contrast, toggleMode, toggleContrast]);

  const activeLabel = NAV.find((n) => n.id === tab)?.label ?? "";

  return (
    <>
      <style>{`
        .skip-link {
          position: absolute;
          left: 0.5rem;
          top: -3rem;
          z-index: 60;
          background: var(--color-neriak-magenta);
          color: var(--color-void);
          padding: 0.45rem 0.75rem;
          border-radius: 0 0 4px 4px;
          text-decoration: none;
          font: 600 0.75rem var(--font-mono);
          transition: transform 120ms ease;
          transform: translateY(-0.25rem);
        }

        .skip-link:focus-visible {
          top: 0.5rem;
          transform: translateY(0);
        }

        :where(
          a,
          button,
          [role=\"button\"],
          input,
          select,
          textarea,
          [tabindex]:not([tabindex=\"-1\"])
        ):focus-visible {
          outline: 2px solid var(--color-neriak-cyan);
          outline-offset: 2px;
        }
      `}</style>
      <a href="#main-content" className="skip-link">
        Skip to main content
      </a>
      <div className="min-h-screen bg-void text-neriak-text flex flex-col">
        {/* ─── Top bar ──────────────────────────────────────────── */}
        <header className="flex items-center gap-6 px-5 py-3 border-b border-neriak-dim bg-void">
          <div className="flex items-baseline gap-3">
            <span className="font-[Cinzel,serif] text-xl tracking-[0.2em] text-neriak-magenta">
              TEXTQUEST
            </span>
            <span className="font-mono text-[10px] text-neriak-dim">{VERSION}</span>
          </div>
          <nav className="flex items-center gap-1 font-mono text-[11px] text-neriak-muted uppercase tracking-[0.18em]">
            <span>operator</span>
            <span className="text-neriak-dim">/</span>
            <span className="text-neriak-text">{activeLabel}</span>
          </nav>
          <div className="ml-auto flex items-center gap-2">
            <span
              className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-sm border font-mono text-[10px] uppercase tracking-[0.15em] ${
                connected
                  ? "border-state-ok/40 text-state-ok bg-state-ok/5"
                  : "border-state-danger/40 text-state-danger bg-state-danger/5"
              }`}
            >
              <span
                className={`w-1.5 h-1.5 rounded-full ${
                  connected
                    ? "bg-state-ok shadow-[0_0_6px_var(--color-state-ok)]"
                    : "bg-state-danger"
                }`}
              />
              {connected ? "connected" : "offline"}
              <span className="text-neriak-muted normal-case tracking-normal ml-1">
                {serverName}
              </span>
            </span>
            <button
              onClick={() => setHuntMode((h) => !h)}
              className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-sm border font-mono text-[10px] uppercase tracking-[0.18em] ${
                huntMode
                  ? "border-neriak-magenta text-neriak-magenta bg-neriak-magenta/10 shadow-[0_0_12px_-4px_var(--color-neriak-magenta)]"
                  : "border-neriak-dim text-neriak-muted"
              }`}
            >
              <Zap className="w-3 h-3" strokeWidth={2} />
              hunt
            </button>
            <button
              onClick={() => setPaletteOpen(true)}
              className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-sm border border-neriak-dim hover:border-neriak-magenta hover:text-neriak-magenta text-neriak-muted font-mono text-[10px] uppercase tracking-[0.15em]"
            >
              <Command className="w-3 h-3" strokeWidth={2} />
              ⌘K palette
            </button>
            <button
              onClick={() => setPrivacy((p) => !p)}
              className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-sm border font-mono text-[10px] uppercase tracking-[0.15em] ${
                privacy
                  ? "border-state-warn text-state-warn bg-state-warn/5"
                  : "border-neriak-dim text-neriak-muted"
              }`}
            >
              {privacy ? (
                <EyeOff className="w-3 h-3" strokeWidth={2} />
              ) : (
                <Eye className="w-3 h-3" strokeWidth={2} />
              )}
              {privacy ? "hidden" : "visible"}
            </button>
            <ThemeToggle />
          </div>
        </header>

        {!connected && (
          <div
            role="status"
            aria-live="polite"
            className="flex items-center gap-2 px-5 py-1.5 bg-state-warn/5 border-b border-state-warn/30 font-mono text-[10px] text-state-warn uppercase tracking-[0.2em]"
          >
            <span className="w-1.5 h-1.5 rounded-full bg-state-warn animate-pulse" />
            backend offline · showing mock data
            <span className="ml-auto text-neriak-dim normal-case tracking-normal">
              retrying every 15s
            </span>
          </div>
        )}

        {/* ─── Body (sidebar + main) ────────────────────────────── */}
        <div className="flex flex-1 min-h-0">
          <aside className="w-52 border-r border-neriak-dim bg-void flex flex-col">
            <nav className="flex-1 p-2 space-y-0.5">
              {NAV.map(({ id, label, icon: Icon }) => {
                const active = tab === id;
                return (
                  <button
                    key={id}
                    onClick={() => setTab(id)}
                    className={`w-full flex items-center gap-2 px-3 py-2 rounded-sm font-mono text-xs uppercase tracking-[0.18em] transition-colors ${
                      active
                        ? "bg-elevated text-neriak-magenta border-l-2 border-neriak-magenta"
                        : "text-neriak-muted hover:bg-panel hover:text-neriak-text border-l-2 border-transparent"
                    }`}
                  >
                    <Icon className="w-4 h-4" strokeWidth={1.75} />
                    {label}
                  </button>
                );
              })}
            </nav>
          </aside>

          <main
            id="main-content"
            className={`flex-1 overflow-y-auto ${privacy ? "blur-sm pointer-events-none" : ""}`}
          >
            {tab === "dashboard" && <Dashboard />}
            {tab === "sessions" && <Sessions />}
            {tab === "replay" && <Replay />}
            {tab === "groups" && <Groups />}
            {tab === "improvement" && <Improvement />}
            {tab === "loot" && <Loot />}
            {tab === "economy" && <Economy />}
            {tab === "characters" && <Characters />}
            {tab === "credentials" && <Credentials />}
          </main>
        </div>
        <nav className="flex items-center gap-1 font-mono text-[11px] text-neriak-muted uppercase tracking-[0.18em]">
          <span>operator</span>
          <span className="text-neriak-dim">/</span>
          <span className="text-neriak-text">{activeLabel}</span>
        </nav>
        <div className="ml-auto flex items-center gap-2">
          <span
            className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-sm border font-mono text-[10px] uppercase tracking-[0.15em] ${
              connected
                ? "border-state-ok/40 text-state-ok bg-state-ok/5"
                : "border-state-danger/40 text-state-danger bg-state-danger/5"
            }`}
          >
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                connected ? "bg-state-ok shadow-[0_0_6px_var(--color-state-ok)]" : "bg-state-danger"
              }`}
            />
            {connected ? "connected" : "offline"}
            <span className="text-neriak-muted normal-case tracking-normal ml-1">{serverName}</span>
          </span>
          <button
            onClick={() => setHuntMode((h) => !h)}
            className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-sm border font-mono text-[10px] uppercase tracking-[0.18em] ${
              huntMode
                ? "border-neriak-magenta text-neriak-magenta bg-neriak-magenta/10 shadow-[0_0_12px_-4px_var(--color-neriak-magenta)]"
                : "border-neriak-dim text-neriak-muted"
            }`}
          >
            <Zap className="w-3 h-3" strokeWidth={2} />
            hunt
          </button>
          <button
            onClick={() => setPaletteOpen(true)}
            className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-sm border border-neriak-dim hover:border-neriak-magenta hover:text-neriak-magenta text-neriak-muted font-mono text-[10px] uppercase tracking-[0.15em]"
          >
            <Command className="w-3 h-3" strokeWidth={2} />
            ⌘K palette
          </button>
          <button
            onClick={() => setPrivacy((p) => !p)}
            className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-sm border font-mono text-[10px] uppercase tracking-[0.15em] ${
              privacy
                ? "border-state-warn text-state-warn bg-state-warn/5"
                : "border-neriak-dim text-neriak-muted"
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
        <div
          role="status"
          aria-live="polite"
          className="flex items-center gap-2 px-5 py-1.5 bg-state-warn/5 border-b border-state-warn/30 font-mono text-[10px] text-state-warn uppercase tracking-[0.2em]"
        >
          <span className="w-1.5 h-1.5 rounded-full bg-state-warn animate-pulse" />
          backend offline · showing mock data
          <span className="ml-auto text-neriak-dim normal-case tracking-normal">
            retrying every 15s
          </span>
        </div>
      )}

      {/* ─── Body (sidebar + main) ────────────────────────────── */}
      <div className="flex flex-1 min-h-0">
        <aside className="w-52 border-r border-neriak-dim bg-void flex flex-col">
          <nav className="flex-1 p-2 space-y-0.5">
            {NAV.map(({ id, label, icon: Icon }) => {
              const active = tab === id;
              return (
                <button
                  key={id}
                  onClick={() => setTab(id)}
                  className={`w-full flex items-center gap-2 px-3 py-2 rounded-sm font-mono text-xs uppercase tracking-[0.18em] transition-colors ${
                    active
                      ? "bg-elevated text-neriak-magenta border-l-2 border-neriak-magenta"
                      : "text-neriak-muted hover:bg-panel hover:text-neriak-text border-l-2 border-transparent"
                  }`}
                >
                  <Icon className="w-4 h-4" strokeWidth={1.75} />
                  {label}
                </button>
              );
            })}
          </nav>
        </aside>

        <main
          id="main-content"
          className={`flex-1 overflow-y-auto ${privacy ? "blur-sm pointer-events-none" : ""}`}
        >
          {tab === "dashboard" && <Dashboard />}
          {tab === "sessions" && <Sessions />}
          {tab === "replay" && <Replay />}
          {tab === "groups" && <Groups />}
          {tab === "improvement" && <Improvement />}
          {tab === "loot" && <Loot />}
          {tab === "economy" && <Economy />}
          {tab === "characters" && <Characters />}
          {tab === "credentials" && <Credentials />}
        </main>
      </div>

      {/* ─── Footer kbd hints ─────────────────────────────────── */}
      <footer className="flex items-center gap-4 px-5 py-2 border-t border-neriak-dim bg-void font-mono text-[11px] text-neriak-muted">
        <div className="flex items-center gap-3">
          {KBD_HINTS.map(({ key, label }) => (
            <span key={key} className="flex items-center gap-1.5">
              <kbd className="inline-block min-w-[1.25rem] text-center px-1 py-0.5 border border-neriak-dim rounded-sm text-neriak-magenta bg-panel text-[10px]">
                {key}
              </kbd>
              <span className="text-[10px]">{label}</span>
            </span>
          ))}
        </div>
        <div className="ml-auto flex items-center gap-3 text-[10px] uppercase tracking-[0.15em]">
          <span>theme</span>
          <span className="text-neriak-magenta-bright">neriak</span>
        </div>
      </footer>

      <CommandPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        actions={paletteActions}
      />
      </div>
    </>
  );
}
