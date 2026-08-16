import { create } from "zustand";
import { DEFAULT_THEME_ID, isThemeId } from "../ui/theme/theme-registry";

export const THEME_STORAGE_KEY = "lumadeck.active-theme";

export function readPersistedThemeId(): string {
  if (typeof window === "undefined") return DEFAULT_THEME_ID;
  try {
    const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
    return stored && isThemeId(stored) ? stored : DEFAULT_THEME_ID;
  } catch {
    return DEFAULT_THEME_ID;
  }
}

function persistThemeId(themeId: string): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, themeId);
  } catch {
    // Storage is optional; the active theme remains valid for this session.
  }
}

interface ThemeState {
  activeThemeId: string;
  setTheme: (themeId: string) => void;
}

export const useThemeStore = create<ThemeState>((set) => ({
  activeThemeId: readPersistedThemeId(),
  setTheme: (themeId) => {
    const resolvedThemeId = isThemeId(themeId) ? themeId : DEFAULT_THEME_ID;
    persistThemeId(resolvedThemeId);
    set({ activeThemeId: resolvedThemeId });
  },
}));
