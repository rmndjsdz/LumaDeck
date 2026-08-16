import type { LumaTheme } from "../theme-types";

export const lumadeckDefaultTheme = {
  id: "lumadeck-default",
  name: "LumaDeck",
  description: "Tema original de LumaDeck",
  colors: {
    background: "#07101d",
    accent: "#8ebcff",
    positive: "#56e39f",
    warning: "#f4c66b",
    danger: "#ff8e9c",
  },
  surfaces: {
    primary: "#0d1b2f",
    secondary: "rgba(10, 22, 39, 0.86)",
    elevated: "#10223a",
    overlay: "rgba(3, 8, 17, 0.76)",
  },
  text: {
    primary: "#edf4ff",
    secondary: "rgba(218, 231, 249, 0.7)",
    muted: "#8397b3",
  },
  borders: {
    subtle: "rgba(133, 171, 220, 0.18)",
    normal: "rgba(181, 211, 247, 0.2)",
    focus: "#83b7ff",
  },
  focus: {
    color: "#83b7ff",
    glow: "rgba(100, 165, 255, 0.28)",
  },
  shape: {
    radiusSmall: "8px",
    radiusMedium: "12px",
    radiusLarge: "18px",
  },
  effects: {
    glass: true,
    glowStrength: 0.28,
  },
  layout: {
    home: "default",
  },
} satisfies LumaTheme;
