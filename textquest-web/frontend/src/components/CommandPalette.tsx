import { useEffect, useMemo, useRef, useState } from "react";
import { Command, CornerDownLeft, Search } from "lucide-react";

export interface PaletteAction {
  id: string;
  label: string;
  section: string;
  shortcut?: string;
  run: () => void;
}

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  actions: PaletteAction[];
}

export function CommandPalette({ open, onClose, actions }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setActive(0);
      setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [open]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return actions;
    return actions.filter(
      (a) => a.label.toLowerCase().includes(q) || a.section.toLowerCase().includes(q),
    );
  }, [actions, query]);

  const grouped = useMemo(() => {
    const map = new Map<string, PaletteAction[]>();
    for (const a of filtered) {
      const list = map.get(a.section) ?? [];
      list.push(a);
      map.set(a.section, list);
    }
    return [...map.entries()];
  }, [filtered]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setActive((i) => Math.min(i + 1, filtered.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setActive((i) => Math.max(i - 1, 0));
      } else if (e.key === "Enter") {
        e.preventDefault();
        const sel = filtered[active];
        if (sel) {
          sel.run();
          onClose();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, filtered, active, onClose]);

  if (!open) return null;

  let counter = -1;

  return (
    <div
      className="fixed inset-0 z-50 bg-[#0d0618]/80 backdrop-blur-sm flex items-start justify-center pt-[12vh]"
      onClick={onClose}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="w-[560px] max-w-[90vw] bg-[#1a0a2e] border border-[#cc44ff]/40 rounded-md shadow-[0_0_40px_-8px_rgba(204,68,255,0.5)] overflow-hidden"
      >
        <div className="flex items-center gap-2 px-4 py-3 border-b border-[#503c6e]">
          <Search className="w-4 h-4 text-[#cc44ff]" strokeWidth={1.75} />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setActive(0);
            }}
            placeholder="type a command…"
            className="flex-1 bg-transparent outline-none font-mono text-sm text-[#e2d7f4] placeholder-[#503c6e]"
          />
          <kbd className="font-mono text-[10px] text-[#503c6e] border border-[#503c6e] rounded-sm px-1.5 py-0.5">
            esc
          </kbd>
        </div>

        <div className="max-h-[50vh] overflow-y-auto py-1">
          {grouped.length === 0 ? (
            <div className="px-4 py-6 text-center text-[#503c6e] font-mono text-sm">no matches</div>
          ) : (
            grouped.map(([section, items]) => (
              <div key={section}>
                <div className="px-4 py-1 font-mono text-[10px] uppercase tracking-[0.18em] text-[#503c6e]">
                  {section}
                </div>
                {items.map((a) => {
                  counter += 1;
                  const isActive = counter === active;
                  return (
                    <button
                      key={a.id}
                      onMouseEnter={(() => {
                        const idx = counter;
                        return () => setActive(idx);
                      })()}
                      onClick={() => {
                        a.run();
                        onClose();
                      }}
                      className={`w-full flex items-center gap-3 px-4 py-2 font-mono text-sm ${
                        isActive
                          ? "bg-[#2d1e41] text-[#cc44ff]"
                          : "text-[#e2d7f4] hover:bg-[#2d1e41]/50"
                      }`}
                    >
                      <span className="flex-1 text-left">{a.label}</span>
                      {a.shortcut && (
                        <kbd className="font-mono text-[10px] text-[#a096b4] border border-[#503c6e] rounded-sm px-1.5 py-0.5">
                          {a.shortcut}
                        </kbd>
                      )}
                      {isActive && (
                        <CornerDownLeft className="w-3 h-3 text-[#cc44ff]" strokeWidth={1.75} />
                      )}
                    </button>
                  );
                })}
              </div>
            ))
          )}
        </div>

        <div className="flex items-center gap-3 px-4 py-2 border-t border-[#503c6e] font-mono text-[10px] text-[#503c6e] uppercase tracking-[0.15em]">
          <Command className="w-3 h-3" strokeWidth={2} />
          <span>palette</span>
          <span className="ml-auto">↑↓ nav · ⏎ run · esc close</span>
        </div>
      </div>
    </div>
  );
}
