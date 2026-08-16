import { describe, expect, it } from "vitest";
import { getTheme } from "./theme-registry";
import { applyThemeToRoot, themeToCssVariables } from "./theme-runtime";

describe("theme runtime", () => {
  it("projects semantic theme tokens to CSS custom properties", () => {
    const theme = getTheme("lumadeck-default");
    const variables = themeToCssVariables(theme);

    expect(variables["--theme-background"]).toBe("#07101d");
    expect(variables["--theme-text-primary"]).toBe("#edf4ff");
    expect(variables["--theme-border-focus"]).toBe("#83b7ff");
    expect(variables["--theme-focus-glow"]).toBe("rgba(100, 165, 255, 0.28)");
    expect(variables["--theme-radius-large"]).toBe("18px");
  });

  it("applies variables and the active theme marker to the root", () => {
    const root = document.documentElement;
    const theme = getTheme("lumadeck-default");

    applyThemeToRoot(root, theme);

    expect(root.style.getPropertyValue("--theme-accent")).toBe("#8ebcff");
    expect(root.dataset.theme).toBe("lumadeck-default");
  });
});
