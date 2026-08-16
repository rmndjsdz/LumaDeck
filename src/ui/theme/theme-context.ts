import { createContext, useContext } from "react";
import type { LumaTheme } from "./theme-types";

export interface ThemeContextValue {
  theme: LumaTheme;
  confirmedTheme: LumaTheme;
  previewThemeId: string | null;
  setTheme: (themeId: string) => void;
  previewTheme: (themeId: string) => void;
  clearThemePreview: () => void;
}

export const ThemeContext = createContext<ThemeContextValue | null>(null);

export function useTheme(): ThemeContextValue {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useTheme must be used within ThemeProvider");
  }
  return context;
}
