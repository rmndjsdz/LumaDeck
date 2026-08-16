import { beforeEach, describe, expect, it } from "vitest";
import {
  THEME_STORAGE_KEY,
  readPersistedThemeId,
  useThemeStore,
} from "./theme-store";
import { DEFAULT_THEME_ID } from "../ui/theme/theme-registry";

describe("theme store", () => {
  beforeEach(() => {
    window.localStorage.clear();
    useThemeStore.setState({ activeThemeId: DEFAULT_THEME_ID });
  });

  it("starts with the default theme", () => {
    expect(useThemeStore.getState().activeThemeId).toBe(DEFAULT_THEME_ID);
  });

  it("persists a valid selection without expanding the store", () => {
    useThemeStore.getState().setTheme(DEFAULT_THEME_ID);

    expect(useThemeStore.getState().activeThemeId).toBe(DEFAULT_THEME_ID);
    expect(window.localStorage.getItem(THEME_STORAGE_KEY)).toBe(
      DEFAULT_THEME_ID,
    );
  });

  it("restores a valid persisted theme id on startup", () => {
    window.localStorage.setItem(THEME_STORAGE_KEY, DEFAULT_THEME_ID);

    expect(readPersistedThemeId()).toBe(DEFAULT_THEME_ID);
  });

  it("normalizes an invalid selection to the default theme", () => {
    useThemeStore.getState().setTheme("invalid-theme");

    expect(useThemeStore.getState().activeThemeId).toBe(DEFAULT_THEME_ID);
    expect(window.localStorage.getItem(THEME_STORAGE_KEY)).toBe(
      DEFAULT_THEME_ID,
    );
  });
});
