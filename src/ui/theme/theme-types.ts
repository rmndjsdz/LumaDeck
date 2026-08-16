export type HomeLayoutVariant = "default" | "cinematic";

export interface LumaTheme {
  id: string;
  name: string;
  description: string;
  colors: {
    background: string;
    accent: string;
    positive: string;
    warning: string;
    danger: string;
  };
  surfaces: {
    primary: string;
    secondary: string;
    elevated: string;
    overlay: string;
  };
  text: {
    primary: string;
    secondary: string;
    muted: string;
  };
  borders: {
    subtle: string;
    normal: string;
    focus: string;
  };
  focus: {
    color: string;
    glow: string;
  };
  shape: {
    radiusSmall: string;
    radiusMedium: string;
    radiusLarge: string;
  };
  effects: {
    glass: boolean;
    glowStrength: number;
  };
  layout: {
    home: HomeLayoutVariant;
  };
}

export interface ThemeDescriptor {
  theme: LumaTheme;
  preview: {
    home: HomeLayoutVariant;
  };
}

export type ThemeCssVariables = Readonly<Record<`--theme-${string}`, string>>;
