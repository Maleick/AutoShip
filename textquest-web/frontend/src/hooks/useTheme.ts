import { useEffect, useState } from "react";

export type ThemeMode = "dark" | "light";
export type ContrastMode = "normal" | "high-contrast";

interface ThemeState {
  mode: ThemeMode;
  contrast: ContrastMode;
}

const STORAGE_KEY_MODE = "textquest-theme-mode";
const STORAGE_KEY_CONTRAST = "textquest-theme-contrast";

export function useTheme() {
  const [theme, setTheme] = useState<ThemeState>(() => {
    const savedMode = localStorage.getItem(STORAGE_KEY_MODE) as ThemeMode | null;
    const savedContrast = localStorage.getItem(STORAGE_KEY_CONTRAST) as ContrastMode | null;

    return {
      mode: savedMode || "dark",
      contrast: savedContrast || "normal",
    };
  });

  // Apply theme to DOM
  useEffect(() => {
    const root = document.documentElement;

    // Remove all theme classes
    root.classList.remove("light-mode", "high-contrast");

    // Apply mode
    if (theme.mode === "light") {
      root.classList.add("light-mode");
    }

    // Apply contrast
    if (theme.contrast === "high-contrast") {
      root.classList.add("high-contrast");
    }

    // Persist to localStorage
    localStorage.setItem(STORAGE_KEY_MODE, theme.mode);
    localStorage.setItem(STORAGE_KEY_CONTRAST, theme.contrast);
  }, [theme]);

  const toggleMode = () => {
    setTheme((prev) => ({
      ...prev,
      mode: prev.mode === "dark" ? "light" : "dark",
    }));
  };

  const toggleContrast = () => {
    setTheme((prev) => ({
      ...prev,
      contrast: prev.contrast === "normal" ? "high-contrast" : "normal",
    }));
  };

  return {
    ...theme,
    toggleMode,
    toggleContrast,
  };
}
