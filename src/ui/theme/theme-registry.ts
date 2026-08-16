import { lumadeckDefaultTheme } from "./themes/lumadeck-default";
import { lumadeckCinematicTheme } from "./themes/lumadeck-cinematic";
import type { LumaTheme, ThemeDescriptor } from "./theme-types";

export const DEFAULT_THEME_ID = lumadeckDefaultTheme.id;

const themeRegistry = new Map<string, ThemeDescriptor>([
  [
    lumadeckDefaultTheme.id,
    { theme: lumadeckDefaultTheme, preview: { home: "default" } },
  ],
  [
    lumadeckCinematicTheme.id,
    { theme: lumadeckCinematicTheme, preview: { home: "cinematic" } },
  ],
]);

export function getTheme(id: string | null | undefined): LumaTheme {
  return themeRegistry.get(id ?? "")?.theme ?? lumadeckDefaultTheme;
}

export function getThemeDescriptor(
  id: string | null | undefined,
): ThemeDescriptor {
  return (
    themeRegistry.get(id ?? "") ?? {
      theme: lumadeckDefaultTheme,
      preview: { home: "default" },
    }
  );
}

export function getAvailableThemes(): readonly LumaTheme[] {
  return [...themeRegistry.values()].map(({ theme }) => theme);
}

export function getAvailableThemeDescriptors(): readonly ThemeDescriptor[] {
  return [...themeRegistry.values()];
}

export function isThemeId(id: string): boolean {
  return themeRegistry.has(id);
}
