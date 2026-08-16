import type { LumaTheme } from "../theme-types";

export const lumadeckCinematicTheme = {
  id: "lumadeck-cinematic",
  name: "LumaDeck Cinematic",
  description: "Una experiencia enfocada en el arte y la inmersión",
  colors: {
    background: "#000000",
    accent: "#f1e8da",
    positive: "#d5c2a7",
    warning: "#d9ad72",
    danger: "#e38d82",
  },
  surfaces: {
    primary: "#090909",
    secondary: "rgba(10, 10, 10, 0.92)",
    elevated: "#151515",
    overlay: "rgba(0, 0, 0, 0.86)",
  },
  text: {
    primary: "#f7f1e8",
    secondary: "rgba(247, 241, 232, 0.74)",
    muted: "#a79f95",
  },
  borders: {
    subtle: "rgba(247, 241, 232, 0.16)",
    normal: "rgba(247, 241, 232, 0.3)",
    focus: "#fffaf0",
  },
  focus: {
    color: "#fffaf0",
    glow: "rgba(255, 250, 240, 0.18)",
  },
  shape: {
    radiusSmall: "5px",
    radiusMedium: "8px",
    radiusLarge: "10px",
  },
  effects: {
    glass: false,
    glowStrength: 0.18,
  },
  layout: {
    home: "cinematic",
  },
} satisfies LumaTheme;
