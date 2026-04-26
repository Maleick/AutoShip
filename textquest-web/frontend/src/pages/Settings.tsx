import { Palette, Sun, Moon, Contrast, KeyRound } from "lucide-react";
import { useTheme } from "../hooks/useTheme";
import { useKeybindingMode } from "../hooks/useKeybindingMode";
import { PageHeader } from "../components/PageHeader";

export function Settings() {
  const { mode, contrast, toggleMode, toggleContrast } = useTheme();
  const { mode: keybindingMode, setMode: setKeybindingMode } = useKeybindingMode();

  return (
    <div className="flex flex-col">
      <PageHeader title="Settings" icon={Palette} />

      <div className="flex-1 overflow-y-auto">
        <div className="p-6 space-y-8">
          {/* Theme Mode Section */}
          <section className="space-y-4">
            <div className="flex items-center gap-2">
              <Palette className="w-4 h-4 text-neriak-magenta" />
              <h2 className="text-sm font-semibold uppercase tracking-wider text-neriak-text">
                Theme Mode
              </h2>
            </div>
            <div className="bg-panel border border-neriak-dim rounded-md p-4 space-y-3">
              <p className="text-xs text-neriak-muted">
                Choose between dark and light theme for your interface.
              </p>
              <button
                onClick={toggleMode}
                className={`w-full flex items-center justify-between px-4 py-3 rounded-md border-2 font-mono text-sm uppercase tracking-wider transition-all ${
                  mode === "dark"
                    ? "border-neriak-magenta text-neriak-magenta bg-neriak-magenta/10"
                    : "border-neriak-dim text-neriak-muted hover:border-neriak-magenta hover:text-neriak-magenta"
                }`}
              >
                <span className="flex items-center gap-2">
                  {mode === "dark" ? (
                    <Moon className="w-4 h-4" strokeWidth={2} />
                  ) : (
                    <Sun className="w-4 h-4" strokeWidth={2} />
                  )}
                  {mode === "dark" ? "Dark Mode" : "Light Mode"}
                </span>
                {mode === "dark" ? (
                  <span className="text-xs text-neriak-dim">Active</span>
                ) : (
                  <span className="text-xs text-neriak-dim">Inactive</span>
                )}
              </button>
            </div>
          </section>

          {/* Keybinding Mode Section */}
          <section className="space-y-4">
            <div className="flex items-center gap-2">
              <KeyRound className="w-4 h-4 text-neriak-cyan" />
              <h2 className="text-sm font-semibold uppercase tracking-wider text-neriak-text">
                Keybinding Mode
              </h2>
            </div>
            <div className="bg-panel border border-neriak-dim rounded-md p-4 space-y-3">
              <p className="text-xs text-neriak-muted">
                Choose your keyboard navigation style. Vi mode enables Vim-style keybindings
                throughout the TUI.
              </p>
              <button
                onClick={() =>
                  setKeybindingMode(keybindingMode === "vi" ? "default" : "vi")
                }
                className={`w-full flex items-center justify-between px-4 py-3 rounded-md border-2 font-mono text-sm uppercase tracking-wider transition-all ${
                  keybindingMode === "vi"
                    ? "border-neriak-cyan text-neriak-cyan bg-neriak-cyan/10"
                    : "border-neriak-dim text-neriak-muted hover:border-neriak-cyan hover:text-neriak-cyan"
                }`}
              >
                <span className="flex items-center gap-2">
                  <KeyRound className="w-4 h-4" strokeWidth={2} />
                  {keybindingMode === "vi" ? "Vi Mode" : "Default Mode"}
                </span>
                {keybindingMode === "vi" ? (
                  <span className="text-xs text-neriak-dim">Active</span>
                ) : (
                  <span className="text-xs text-neriak-dim">Inactive</span>
                )}
              </button>

              {/* Vi Keybindings Cheatsheet */}
              {keybindingMode === "vi" && (
                <div className="mt-4 pt-4 border-t border-neriak-dim space-y-2">
                  <p className="text-xs font-semibold text-neriak-text uppercase tracking-wider">
                    Vi Keybindings:
                  </p>
                  <ul className="space-y-1 text-xs text-neriak-muted font-mono">
                    <li className="flex items-center gap-2">
                      <span className="text-neriak-cyan min-w-12">hjkl</span>
                      <span>Movement</span>
                    </li>
                    <li className="flex items-center gap-2">
                      <span className="text-neriak-cyan min-w-12">w/b</span>
                      <span>Word navigation</span>
                    </li>
                    <li className="flex items-center gap-2">
                      <span className="text-neriak-cyan min-w-12">/</span>
                      <span>Search forward</span>
                    </li>
                    <li className="flex items-center gap-2">
                      <span className="text-neriak-cyan min-w-12">?</span>
                      <span>Search backward</span>
                    </li>
                    <li className="flex items-center gap-2">
                      <span className="text-neriak-cyan min-w-12">:</span>
                      <span>Command entry</span>
                    </li>
                    <li className="flex items-center gap-2">
                      <span className="text-neriak-cyan min-w-12">u</span>
                      <span>Undo</span>
                    </li>
                    <li className="flex items-center gap-2">
                      <span className="text-neriak-cyan min-w-12">ctrl+r</span>
                      <span>Redo</span>
                    </li>
                  </ul>
                </div>
              )}
            </div>
          </section>

          {/* High-Contrast Mode Section */}
          <section className="space-y-4">
            <div className="flex items-center gap-2">
              <Contrast className="w-4 h-4 text-neriak-cyan" />
              <h2 className="text-sm font-semibold uppercase tracking-wider text-neriak-text">
                Accessibility
              </h2>
            </div>
            <div className="bg-panel border border-neriak-dim rounded-md p-4 space-y-3">
              <p className="text-xs text-neriak-muted">
                High-contrast mode enhances visibility with WCAG AAA-compliant colors and bold
                fonts. Recommended for users with color blindness or low vision.
              </p>
              <button
                onClick={toggleContrast}
                className={`w-full flex items-center justify-between px-4 py-3 rounded-md border-2 font-mono text-sm uppercase tracking-wider transition-all ${
                  contrast === "high-contrast"
                    ? "border-neriak-cyan text-neriak-cyan bg-neriak-cyan/10"
                    : "border-neriak-dim text-neriak-muted hover:border-neriak-cyan hover:text-neriak-cyan"
                }`}
              >
                <span className="flex items-center gap-2">
                  <Contrast className="w-4 h-4" strokeWidth={2} />
                  {contrast === "high-contrast" ? "High-Contrast Mode" : "Standard Contrast"}
                </span>
                {contrast === "high-contrast" ? (
                  <span className="text-xs text-neriak-dim">Active</span>
                ) : (
                  <span className="text-xs text-neriak-dim">Inactive</span>
                )}
              </button>

              {/* Feature List */}
              <div className="mt-4 pt-4 border-t border-neriak-dim space-y-2">
                <p className="text-xs font-semibold text-neriak-text uppercase tracking-wider">
                  Features:
                </p>
                <ul className="space-y-1 text-xs text-neriak-muted">
                  <li className="flex items-start gap-2">
                    <span className="text-neriak-cyan mt-0.5">✓</span>
                    <span>WCAG AAA contrast ratios for all text</span>
                  </li>
                  <li className="flex items-start gap-2">
                    <span className="text-neriak-cyan mt-0.5">✓</span>
                    <span>Colorblind-friendly color palette</span>
                  </li>
                  <li className="flex items-start gap-2">
                    <span className="text-neriak-cyan mt-0.5">✓</span>
                    <span>Bold fonts and thicker borders</span>
                  </li>
                  <li className="flex items-start gap-2">
                    <span className="text-neriak-cyan mt-0.5">✓</span>
                    <span>Enhanced focus indicators</span>
                  </li>
                </ul>
              </div>
            </div>
          </section>

          {/* Color Palette Preview */}
          <section className="space-y-4">
            <h2 className="text-sm font-semibold uppercase tracking-wider text-neriak-text">
              Color Palette
            </h2>
            <div className="grid grid-cols-2 gap-3">
              {[
                { name: "Primary", color: "bg-neriak-magenta", label: "Magenta" },
                { name: "Accent", color: "bg-neriak-cyan", label: "Cyan" },
                { name: "Success", color: "bg-state-ok", label: "Green" },
                { name: "Warning", color: "bg-state-warn", label: "Amber" },
                { name: "Danger", color: "bg-state-danger", label: "Red" },
                { name: "Info", color: "bg-state-info", label: "Blue" },
              ].map(({ name, color, label }) => (
                <div key={name} className="flex items-center gap-3">
                  <div className={`w-8 h-8 rounded border border-neriak-dim ${color}`} />
                  <div>
                    <p className="text-xs font-semibold text-neriak-text uppercase">{label}</p>
                    <p className="text-[10px] text-neriak-muted">{name}</p>
                  </div>
                </div>
              ))}
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
