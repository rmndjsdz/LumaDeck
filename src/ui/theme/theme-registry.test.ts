import { describe, expect, it } from "vitest";
import {
  DEFAULT_THEME_ID,
  getAvailableThemes,
  getTheme,
} from "./theme-registry";

describe("theme registry", () => {
  it("registers LumaDeck Default as the initial official theme", () => {
    const themes = getAvailableThemes();

    expect(DEFAULT_THEME_ID).toBe("lumadeck-default");
    expect(themes).toHaveLength(2);
    expect(themes[0]).toMatchObject({
      id: "lumadeck-default",
      name: "LumaDeck",
      description: "Tema original de LumaDeck",
    });
  });

  it("registers the cinematic theme with the cinematic Home variant", () => {
    expect(getTheme("lumadeck-default").layout.home).toBe("default");
    expect(getTheme("lumadeck-cinematic")).toMatchObject({
      id: "lumadeck-cinematic",
      name: "LumaDeck Cinematic",
      layout: { home: "cinematic" },
    });
  });

  it("falls back to the default theme for an unknown id", () => {
    expect(getTheme("theme-that-does-not-exist").id).toBe(DEFAULT_THEME_ID);
    expect(getTheme(undefined).id).toBe(DEFAULT_THEME_ID);
  });
});
