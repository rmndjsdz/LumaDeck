import type { LumaTheme, ThemeCssVariables } from "./theme-types";

export function themeToCssVariables(theme: LumaTheme): ThemeCssVariables {
  return {
    "--theme-background": theme.colors.background,
    "--theme-accent": theme.colors.accent,
    "--theme-positive": theme.colors.positive,
    "--theme-warning": theme.colors.warning,
    "--theme-danger": theme.colors.danger,
    "--theme-surface-primary": theme.surfaces.primary,
    "--theme-surface-secondary": theme.surfaces.secondary,
    "--theme-surface-elevated": theme.surfaces.elevated,
    "--theme-surface-overlay": theme.surfaces.overlay,
    "--theme-text-primary": theme.text.primary,
    "--theme-text-secondary": theme.text.secondary,
    "--theme-text-muted": theme.text.muted,
    "--theme-border-subtle": theme.borders.subtle,
    "--theme-border-normal": theme.borders.normal,
    "--theme-border-focus": theme.borders.focus,
    "--theme-focus": theme.focus.color,
    "--theme-focus-glow": theme.focus.glow,
    "--theme-radius-small": theme.shape.radiusSmall,
    "--theme-radius-medium": theme.shape.radiusMedium,
    "--theme-radius-large": theme.shape.radiusLarge,
    "--theme-effect-glass": String(theme.effects.glass),
    "--theme-effect-glow-strength": String(theme.effects.glowStrength),
  };
}

export function applyThemeToRoot(root: HTMLElement, theme: LumaTheme): void {
  const variables = themeToCssVariables(theme);
  for (const [name, value] of Object.entries(variables)) {
    root.style.setProperty(name, value);
  }
  root.dataset.theme = theme.id;
}
