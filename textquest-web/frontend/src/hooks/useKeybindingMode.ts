import { useState, useEffect } from "react";

export type KeybindingMode = "default" | "vi";

interface KeybindingContextType {
  mode: KeybindingMode;
  setMode: (mode: KeybindingMode) => void;
}

const STORAGE_KEY = "textquest-keybinding-mode";

export function useKeybindingMode(): KeybindingContextType {
  const [mode, setModeState] = useState<KeybindingMode>("default");

  // Load from localStorage on mount
  useEffect(() => {
    const stored = localStorage.getItem(STORAGE_KEY) as KeybindingMode | null;
    if (stored && (stored === "default" || stored === "vi")) {
      setModeState(stored);
    }
  }, []);

  // Save to localStorage when mode changes
  const setMode = (newMode: KeybindingMode) => {
    setModeState(newMode);
    localStorage.setItem(STORAGE_KEY, newMode);
  };

  return { mode, setMode };
}
