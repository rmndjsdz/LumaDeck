import { beforeEach } from "vitest";
import { useThemeStore } from "./stores/theme-store";
import { DEFAULT_THEME_ID } from "./ui/theme/theme-registry";

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });

beforeEach(() => {
  window.localStorage.removeItem("lumadeck.active-theme");
  useThemeStore.setState({ activeThemeId: DEFAULT_THEME_ID });
});
