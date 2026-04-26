import { useEffect, useState } from "react";

export type ThemeMode = "dark" | "light";
export type ContrastMode = "normal" | "high-contrast";

interface ThemeState {
  mode: ThemeMode;
  contrast: ContrastMode;
}

const STORAGE_KEY_MODE = "textquest-theme-mode";
const STORAGE_KEY_CONTRAST = "textquest-theme-contrast";

async function fetchThemeFromBackend(): Promise<ThemeState> {
  try {
    const response = await fetch("/api/theme/settings");
    if (!response.ok) {
      console.warn("Failed to fetch theme config from backend");
      return { mode: "dark", contrast: "normal" };
    }
    const data = await response.json();
    return {
      mode: (data.mode || "dark") as ThemeMode,
      contrast: (data.contrast || "normal") as ContrastMode,
    };
  } catch (error) {
    console.warn("Error fetching theme config from backend:", error);
    return { mode: "dark", contrast: "normal" };
  }
}

async function saveThemeToBackend(theme: ThemeState): Promise<void> {
  try {
    const response = await fetch("/api/theme/settings", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ mode: theme.mode, contrast: theme.contrast }),
    });
    if (!response.ok) {
      console.warn("Failed to save theme config to backend");
    }
  } catch (error) {
    console.warn("Error saving theme config to backend:", error);
  }
}

export function useTheme() {
  const [theme, setTheme] = useState<ThemeState>(() => {
    // First check localStorage for immediate fallback
    const savedMode = localStorage.getItem(STORAGE_KEY_MODE) as ThemeMode | null;
    const savedContrast = localStorage.getItem(STORAGE_KEY_CONTRAST) as ContrastMode | null;

    return {
      mode: savedMode || "dark",
      contrast: savedContrast || "normal",
    };
  });

  // On mount, fetch theme from backend to override localStorage
  useEffect(() => {
    const initializeTheme = async () => {
      const backendTheme = await fetchThemeFromBackend();
      setTheme(backendTheme);
    };
    initializeTheme();
  }, []);

  // Apply theme to DOM and persist to backend
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

    // Persist to backend
    saveThemeToBackend(theme);
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
