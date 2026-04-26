import { createContext, useContext, useEffect, useState } from "react";

type Theme = "light" | "dark";

interface ThemeContextType {
  theme: Theme;
  toggleTheme: () => void;
  setTheme: (theme: Theme) => void;
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(() => {
    // Check localStorage first
    const stored = localStorage.getItem("theme");
    if (stored === "light" || stored === "dark") {
      return stored;
    }

    // Check OS preference
    if (window.matchMedia && window.matchMedia("(prefers-color-scheme: light)").matches) {
      return "light";
    }

    // Default to dark
    return "dark";
  });

  // Load theme from server on mount
  useEffect(() => {
    const loadThemeFromServer = async () => {
      try {
        const response = await fetch("/api/theme/settings");
        if (response.ok) {
          const data = await response.json();
          const serverTheme = data.mode === "light" ? "light" : "dark";
          setThemeState(serverTheme);
          localStorage.setItem("theme", serverTheme);
        }
      } catch (error) {
        console.warn("Failed to load theme from server, using localStorage", error);
      }
    };

    loadThemeFromServer();
  }, []);

  // Persist theme changes to server and localStorage
  useEffect(() => {
    // Update localStorage
    localStorage.setItem("theme", theme);

    // Update DOM
    const root = document.documentElement;
    if (theme === "light") {
      root.classList.add("light-mode");
      root.classList.remove("dark-mode");
    } else {
      root.classList.add("dark-mode");
      root.classList.remove("light-mode");
    }

    // Persist to server
    const saveThemeToServer = async () => {
      try {
        await fetch("/api/theme/settings", {
          method: "PUT",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            mode: theme,
            contrast: "normal",
          }),
        });
      } catch (error) {
        console.warn("Failed to save theme to server", error);
      }
    };

    saveThemeToServer();
  }, [theme]);

  const setTheme = (newTheme: Theme) => {
    setThemeState(newTheme);
  };

  const toggleTheme = () => {
    setTheme(theme === "light" ? "dark" : "light");
  };

  return (
    <ThemeContext.Provider value={{ theme, toggleTheme, setTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme() {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useTheme must be used within ThemeProvider");
  }
  return context;
}
