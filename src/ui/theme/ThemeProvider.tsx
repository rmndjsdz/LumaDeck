import {
  useCallback,
  useLayoutEffect,
  useMemo,
  useState,
  type PropsWithChildren,
} from "react";
import { useThemeStore } from "../../stores/theme-store";
import { getTheme, isThemeId } from "./theme-registry";
import { applyThemeToRoot } from "./theme-runtime";
import { ThemeContext } from "./theme-context";

export function ThemeProvider({ children }: PropsWithChildren) {
  const activeThemeId = useThemeStore((state) => state.activeThemeId);
  const persistTheme = useThemeStore((state) => state.setTheme);
  const [previewThemeId, setPreviewThemeId] = useState<string | null>(null);
  const confirmedTheme = useMemo(
    () => getTheme(activeThemeId),
    [activeThemeId],
  );
  const theme = useMemo(
    () => getTheme(previewThemeId ?? activeThemeId),
    [activeThemeId, previewThemeId],
  );

  const setTheme = useCallback(
    (themeId: string) => {
      persistTheme(themeId);
      setPreviewThemeId(null);
    },
    [persistTheme],
  );
  const previewTheme = useCallback(
    (themeId: string) => {
      if (!isThemeId(themeId)) return;
      setPreviewThemeId(themeId === activeThemeId ? null : themeId);
    },
    [activeThemeId],
  );
  const clearThemePreview = useCallback(() => {
    setPreviewThemeId(null);
  }, []);

  useLayoutEffect(() => {
    applyThemeToRoot(document.documentElement, theme);
  }, [theme]);

  const value = useMemo(
    () => ({
      theme,
      confirmedTheme,
      previewThemeId,
      setTheme,
      previewTheme,
      clearThemePreview,
    }),
    [
      clearThemePreview,
      confirmedTheme,
      previewTheme,
      previewThemeId,
      setTheme,
      theme,
    ],
  );
  return (
    <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
  );
}
